//! Осмотр машины: что на ней есть.
//!
//! Заменяет `app_search.rs`. Тот брал три ветки реестра без единого отсева и
//! объявлял программой **каждую** подпапку в `Program Files`, `AppData` и
//! `LocalAppData` — вместе с `Temp`, `Packages` и кэшем пакетных менеджеров.
//! Плюс дедуп по имени схлопывал разные программы с одинаковым названием
//! папки, а `DisplayIcon` попадал в поле пути как есть, вместе с кавычками и
//! номером значка: `"C:\App\app.exe",0`.
//!
//! Здесь четыре источника, и каждый знает, что именно он находит:
//!
//! * реестр удаления — то, что ставилось установщиком;
//! * пакеты из Store — их в реестре удаления нет вовсе;
//! * средства разработки — они живут в `PATH` и не значатся нигде;
//! * портативные программы — папка с исполняемым файлом, за которой нет записи
//!   об установке. Отличить их можно только сопоставлением с первыми двумя
//!   источниками, поэтому порядок здесь важен.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

use crate::paths;

/// Откуда взялась находка. От этого зависит, что с ней можно делать.
#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Поставлено установщиком, значится в реестре удаления.
    Installed,
    /// Пакет из Store.
    Store,
    /// Средство разработки из PATH.
    Tool,
    /// Папка с программой без записи об установке.
    Portable,
}

/// Найденное на машине.
#[derive(Serialize)]
pub struct Finding {
    /// Устойчивый ключ: имя ветки реестра, имя пакета или путь. По нему
    /// находка связывается с записью каталога — случайные номера для этого не
    /// годятся, они меняются от осмотра к осмотру.
    pub key: String,
    pub name: String,
    pub kind: Kind,
    pub version: String,
    pub publisher: String,
    /// Папка программы. Пусто, если источник её не сообщает.
    pub location: String,
    /// Откуда брать значок. Может указывать на `.ico` или на библиотеку —
    /// реестр хранит здесь именно значок, а не программу.
    pub exe: String,
    /// Что запускать. Пусто — запускать нечего, и кнопки быть не должно.
    ///
    /// Отдельно от `exe` намеренно. Прежде на кнопку запуска шёл `exe`, а он
    /// приходит из `DisplayIcon` — то есть из записи о **значке**. Оттуда
    /// открывались то `.ico`, то деинсталлятор, лежащий рядом.
    pub launch: String,
    /// Размер по данным установщика, байты. Ноль — неизвестен.
    pub size: u64,
    /// Строка удаления из реестра. Пусто у всего остального.
    pub uninstall: String,
}

/// Ветки реестра, где Windows держит список установленного.
const UNINSTALL_KEYS: [(winreg::HKEY, &str); 3] = [
    (
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
        HKEY_CURRENT_USER,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
];

/// Средства разработки, которые стоит искать отдельно.
///
/// Их нет ни в реестре удаления, ни в Store: ставятся своими установщиками,
/// распаковкой или менеджерами версий, а обнаруживаются только по тому, что
/// команда доступна. Именно их обычно и забывают при переустановке системы.
const TOOLS: [(&str, &str); 14] = [
    ("node.exe", "Node.js"),
    ("npm.cmd", "npm"),
    ("bun.exe", "Bun"),
    ("deno.exe", "Deno"),
    ("cargo.exe", "Cargo"),
    ("rustc.exe", "Rust"),
    ("python.exe", "Python"),
    ("pip.exe", "pip"),
    ("git.exe", "Git"),
    ("go.exe", "Go"),
    ("java.exe", "Java"),
    ("docker.exe", "Docker"),
    ("wsl.exe", "WSL"),
    ("dotnet.exe", ".NET"),
];

/// Пакеты Store, которые программами не являются.
///
/// Среды выполнения и наборы ресурсов ставятся сами как зависимости; в списке
/// того, что нужно вернуть после переустановки, им не место.
const STORE_NOISE: [&str; 7] = [
    "Framework",
    "Runtime",
    "VCLibs",
    "WindowsAppRuntime",
    "DesktopAppInstaller",
    "NET.Native",
    "UI.Xaml",
];

/// Осматривает машину.
#[tauri::command]
pub async fn inventory_scan(app: AppHandle) -> Result<Vec<Finding>, String> {
    // Места нужны, чтобы искать портативные программы и внутри них тоже
    let roots: Vec<PathBuf> = paths::load_or_create_config(&app)
        .map(|config| {
            config
                .libraries
                .iter()
                .map(|library| PathBuf::from(&library.root).join("apps"))
                .collect()
        })
        .unwrap_or_default();

    tauri::async_runtime::spawn_blocking(move || {
        let mut findings = Vec::new();

        // Порядок важен: портативные определяются как то, что не встретилось
        // в двух первых источниках
        read_registry(&mut findings);
        read_store(&mut findings);
        read_tools(&mut findings);
        read_portable(&mut findings, &roots);

        findings.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        findings
    })
    .await
    .map_err(|e| format!("осмотр не завершён: {e}"))
}

// ── Реестр удаления ──────────────────────────────────────────────────────

fn read_registry(out: &mut Vec<Finding>) {
    for (hive, path) in UNINSTALL_KEYS {
        let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };

        for name in root.enum_keys().filter_map(Result::ok) {
            let Ok(entry) = root.open_subkey(&name) else {
                continue;
            };

            let Ok(display) = entry.get_value::<String, _>("DisplayName") else {
                continue;
            };

            if display.trim().is_empty() || is_registry_noise(&entry) {
                continue;
            }

            let mut location: String = entry.get_value("InstallLocation").unwrap_or_default();

            let icon =
                clean_icon_path(&entry.get_value::<String, _>("DisplayIcon").unwrap_or_default());

            /*
             * Что запускать — отдельный вопрос от того, откуда брать значок.
             *
             * `DisplayIcon` указывает на значок: это бывает `.ico`, бывает
             * библиотека, бывает деинсталлятор — и всё это открывалось по
             * кнопке запуска. Берём его, только если он и правда программа и не
             * похож на служебную; иначе ищем в папке установки.
             */
            let launch = runnable(&icon)
                .or_else(|| {
                    let dir = std::path::Path::new(location.trim());

                    if location.trim().is_empty() || !dir.is_dir() {
                        return None;
                    }

                    find_executable(dir, &display).map(|path| path.display().to_string())
                })
                .unwrap_or_default();

            // Папку реестр сообщает не всегда — тогда берём ту, где лежит
            // найденная программа: кнопка «открыть папку» появляется у куда
            // большего числа записей
            if location.trim().is_empty() {
                if let Some(parent) = std::path::Path::new(&launch).parent() {
                    if !launch.is_empty() {
                        location = parent.display().to_string();
                    }
                }
            }

            out.push(Finding {
                key: format!("reg:{name}"),
                name: display,
                kind: Kind::Installed,
                version: entry.get_value("DisplayVersion").unwrap_or_default(),
                publisher: entry.get_value("Publisher").unwrap_or_default(),
                exe: icon,
                launch,
                // В реестре размер в килобайтах
                size: entry.get_value::<u32, _>("EstimatedSize").unwrap_or(0) as u64 * 1024,
                uninstall: entry
                    .get_value("QuietUninstallString")
                    .or_else(|_| entry.get_value("UninstallString"))
                    .unwrap_or_default(),
                location,
            });
        }
    }
}

/// Отсев системного шума.
///
/// Без него список наполовину состоит из обновлений и компонентов, которые
/// ставились в составе чего-то другого и сами по себе не нужны никому.
fn is_registry_noise(entry: &RegKey) -> bool {
    if entry.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
        return true;
    }

    // Запись входит в состав другой программы
    if entry.get_value::<String, _>("ParentKeyName").is_ok()
        || entry.get_value::<String, _>("ParentDisplayName").is_ok()
    {
        return true;
    }

    let release: String = entry.get_value("ReleaseType").unwrap_or_default();

    matches!(
        release.as_str(),
        "Update" | "Hotfix" | "Security Update" | "ServicePack"
    )
}

/// Приводит `DisplayIcon` к пути.
///
/// Хранится он как `"C:\App\app.exe",0` — с кавычками и номером значка внутри
/// файла. Прежний код клал это в поле пути как есть.
fn clean_icon_path(raw: &str) -> String {
    let path = raw.split(',').next().unwrap_or("").trim();
    path.trim_matches('"').to_string()
}

// ── Пакеты Store ─────────────────────────────────────────────────────────

fn read_store(out: &mut Vec<Finding>) {
    const REPOSITORY: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

    let Ok(root) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(REPOSITORY, KEY_READ)
    else {
        return;
    };

    let mut seen = HashSet::new();

    for name in root.enum_keys().filter_map(Result::ok) {
        if STORE_NOISE.iter().any(|noise| name.contains(noise)) {
            continue;
        }

        let Ok(entry) = root.open_subkey(&name) else {
            continue;
        };

        // Полное имя пакета — `Издатель.Имя_версия_архитектура__хеш`. Нам нужна
        // часть до первого подчёркивания: она одна и та же у всех версий
        let family = name.split('_').next().unwrap_or(&name).to_string();

        if !seen.insert(family.clone()) {
            continue;
        }

        let display: String = entry.get_value("DisplayName").unwrap_or_default();

        // Ссылка на строку ресурса вместо самого названия — берём имя пакета
        let title = if display.is_empty() || display.starts_with("@{") || display.starts_with("ms-resource") {
            family.split('.').next_back().unwrap_or(&family).to_string()
        } else {
            display
        };

        out.push(Finding {
            key: format!("store:{family}"),
            name: title,
            kind: Kind::Store,
            version: name.split('_').nth(1).unwrap_or_default().to_string(),
            publisher: family.split('.').next().unwrap_or_default().to_string(),
            location: entry.get_value("PackageRootFolder").unwrap_or_default(),
            exe: String::new(),
            // Пакет Store запускается не файлом, а через оболочку по своему
            // имени — обычного пути к программе у него нет
            launch: String::new(),
            size: 0,
            uninstall: String::new(),
        });
    }
}

// ── Средства разработки ──────────────────────────────────────────────────

fn read_tools(out: &mut Vec<Finding>) {
    let Ok(path) = std::env::var("PATH") else {
        return;
    };

    let dirs: Vec<&str> = path.split(';').filter(|dir| !dir.is_empty()).collect();

    for (file, name) in TOOLS {
        let Some(found) = dirs
            .iter()
            .map(|dir| Path::new(dir).join(file))
            .find(|candidate| candidate.is_file())
        else {
            continue;
        };

        out.push(Finding {
            key: format!("tool:{name}"),
            name: name.to_string(),
            kind: Kind::Tool,
            version: String::new(),
            publisher: String::new(),
            location: found
                .parent()
                .map(|dir| dir.display().to_string())
                .unwrap_or_default(),
            exe: found.display().to_string(),
            launch: found.display().to_string(),
            size: 0,
            uninstall: String::new(),
        });
    }
}

// ── Портативные программы ────────────────────────────────────────────────

/// Папки, которые заведомо не программы.
const SKIP_DIRS: [&str; 12] = [
    "temp",
    "cache",
    "packages",
    "microsoft",
    "crashdumps",
    "connecteddevicesplatform",
    "publishers",
    "comms",
    "elevatediagnostics",
    "npm-cache",
    "pip",
    "d3dscache",
];

fn read_portable(out: &mut Vec<Finding>, libraries: &[PathBuf]) {
    // Всё, что уже нашлось установленным, — не портативное. Сравниваем по
    // папке установки: имена у одной программы в реестре и на диске расходятся
    let known: HashSet<String> = out
        .iter()
        .filter(|finding| !finding.location.is_empty())
        .map(|finding| normalize(&finding.location))
        .collect();

    let mut roots: Vec<PathBuf> = libraries.to_vec();

    // Портативные обычно живут здесь: программы, поставленные для пользователя,
    // и то, что просто распаковали
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Programs"));
    }

    let mut seen = HashSet::new();

    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };

        for entry in entries.flatten() {
            let dir = entry.path();

            if !dir.is_dir() {
                continue;
            }

            let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };

            if SKIP_DIRS.contains(&name.to_lowercase().as_str()) {
                continue;
            }

            if known.contains(&normalize(&dir.display().to_string())) {
                continue;
            }

            // Программа — это папка, в которой есть исполняемый файл. Без этой
            // проверки в список попадала любая папка подряд
            let Some(exe) = find_executable(&dir, &name) else {
                continue;
            };

            if !seen.insert(normalize(&dir.display().to_string())) {
                continue;
            }

            out.push(Finding {
                key: format!("portable:{}", dir.display()),
                name,
                kind: Kind::Portable,
                version: String::new(),
                publisher: String::new(),
                location: dir.display().to_string(),
                exe: exe.display().to_string(),
                launch: exe.display().to_string(),
                size: 0,
                uninstall: String::new(),
            });
        }
    }
}

/// Годится ли запись на роль запускаемого файла.
///
/// Отсекает две беды сразу. Первая — `.ico`, `.dll` и прочее не-программное:
/// в `DisplayIcon` лежит путь к **значку**, и он часто именно такой. Вторая —
/// служебные программы, которые лежат рядом с настоящей: деинсталлятор,
/// обновлятор, отправщик отчётов. Нажать «запустить» и попасть в удаление
/// программы — худшее, что может сделать эта кнопка.
fn runnable(path: &str) -> Option<String> {
    let candidate = Path::new(path.trim());

    if path.trim().is_empty() || !candidate.is_file() {
        return None;
    }

    let is_exe = candidate
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));

    if !is_exe || helper(candidate) {
        return None;
    }

    Some(candidate.display().to_string())
}

/// Похоже ли имя на служебную программу, а не на саму.
fn helper(path: &Path) -> bool {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    const MARKS: [&str; 8] = [
        "unins", "setup", "install", "update", "crash", "report", "helper", "service",
    ];

    MARKS.iter().any(|mark| name.contains(mark))
}

/// Насколько имя файла похоже на название программы.
///
/// Нужна, чтобы из десятка программ в папке выбрать ту самую. Без этого
/// брался первый попавшийся файл — а рядом с настоящей лежат и запускатор
/// обновлений, и вспомогательные утилиты.
fn resembles(exe: &Path, title: &str) -> bool {
    let stem = exe
        .file_stem()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let plain: String = title
        .to_lowercase()
        .chars()
        .filter(|symbol| symbol.is_alphanumeric())
        .collect();

    let bare: String = stem.chars().filter(|s| s.is_alphanumeric()).collect();

    if plain.is_empty() || bare.is_empty() {
        return false;
    }

    plain.contains(&bare) || bare.contains(&plain)
}

/// Исполняемый файл программы: в самой папке или на уровень глубже.
///
/// Глубже не ищем: полный обход дерева ради этого перебирал бы десятки тысяч
/// файлов на каждую папку.
///
/// `title` — название программы. По нему из нескольких кандидатов выбирается
/// похожий по имени: у `Sublime Text` это `sublime_text.exe`, а не соседний
/// `crash_reporter.exe`.
fn find_executable(dir: &Path, title: &str) -> Option<PathBuf> {
    if let Some(exe) = best_exe(dir, title) {
        return Some(exe);
    }

    let entries = fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(exe) = best_exe(&path, title) {
                return Some(exe);
            }
        }
    }

    None
}

/// Лучший исполняемый файл в папке.
///
/// Прежде брался первый попавшийся, и это подводило: рядом с настоящей
/// программой лежат обновлятор, отправщик отчётов и деинсталлятор, а порядок
/// обхода папки — какой придётся.
///
/// Теперь порядок предпочтений явный:
///
/// 1. похожий на название программы — он почти всегда и есть нужный;
/// 2. самый крупный из обычных: у настоящей программы кода больше, чем у
///    вспомогательных утилит рядом;
/// 3. служебный — только если ничего другого в папке нет вовсе.
fn best_exe(dir: &Path, title: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;

    let mut named = None;
    let mut biggest: Option<(u64, PathBuf)> = None;
    let mut fallback = None;

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let is_exe = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));

        if !is_exe {
            continue;
        }

        if helper(&path) {
            fallback.get_or_insert(path);
            continue;
        }

        if named.is_none() && resembles(&path, title) {
            named = Some(path.clone());
        }

        let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);

        if biggest.as_ref().is_none_or(|(known, _)| size > *known) {
            biggest = Some((size, path));
        }
    }

    named.or(biggest.map(|(_, path)| path)).or(fallback)
}

/// Путь в сравнимом виде: без регистра и хвостовой косой черты.
fn normalize(path: &str) -> String {
    path.trim_end_matches(['\\', '/']).to_lowercase()
}
