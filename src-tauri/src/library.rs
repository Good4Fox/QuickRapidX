//! Место — хранилище программ, которым владеет приложение.
//!
//! Смысл его в одном: забрать программу, которую установила Windows, нельзя —
//! её файлы лишь малая часть, остальное в реестре, службах и профиле. А унести
//! ту, что мы сами положили, можно целиком. Поэтому Место идёт первым: всё
//! остальное в разделе на него опирается.
//!
//! Мест может быть несколько, на разных дисках — как библиотеки Steam. Одно из
//! них считается основным: туда попадает то, для чего не выбрали другое.
//!
//! Пока Место — обычная папка. Отдельный том (VHDX) даёт «один файл — весь
//! набор» и позволяет подсунуть его установщику, знающему только диск C:, но
//! требует прав администратора при каждом подключении, поэтому появится
//! отдельно и по желанию.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::paths;

/// Подкаталоги внутри Места.
///
/// `apps` — сами программы, по папке на каждую. `data` — их данные, которые
/// переносятся отдельно от двоичных файлов. `shortcuts` — ярлыки, чтобы
/// запускать не углубляясь в дерево.
const SUBDIRS: [&str; 3] = ["apps", "data", "shortcuts"];

/// Одно Место.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: String,
    /// Как его называет человек. По умолчанию — метка тома.
    pub name: String,
    pub root: String,
}

/// Место вместе с тем, что о нём известно прямо сейчас.
///
/// Размеры и доступность не хранятся: носитель могли отключить, а папку —
/// удалить мимо приложения. Спрашиваем у файловой системы каждый раз.
#[derive(Serialize)]
pub struct LibraryState {
    #[serde(flatten)]
    pub library: Library,
    /// Папка на месте и доступна.
    pub available: bool,
    /// Сколько программ лежит внутри.
    pub apps: usize,
    /// Сколько занимает, байты.
    pub size: u64,
    /// Это Место основное.
    pub primary: bool,
}

/// Все Места с их состоянием.
#[tauri::command]
pub async fn library_list(app: AppHandle) -> Result<Vec<LibraryState>, String> {
    let config = paths::load_or_create_config(&app)?;
    let primary = config.primary_library.clone();

    tauri::async_runtime::spawn_blocking(move || {
        config
            .libraries
            .into_iter()
            .map(|library| {
                let root = PathBuf::from(&library.root);
                let available = root.is_dir();

                let (apps, size) = if available {
                    measure(&root.join("apps"))
                } else {
                    (0, 0)
                };

                LibraryState {
                    primary: primary.as_deref() == Some(library.id.as_str()),
                    library,
                    available,
                    apps,
                    size,
                }
            })
            .collect()
    })
    .await
    .map_err(|e| format!("не удалось осмотреть места: {e}"))
}

/// Заводит Место по указанному пути.
///
/// Путь проверяется на пригодность до записи в настройки: добавить Место,
/// которое не создаётся, значит показать человеку строку, ничего за собой не
/// имеющую.
#[tauri::command]
pub fn library_add(
    app: AppHandle,
    path: String,
    name: String,
    fallback: String,
) -> Result<Vec<Library>, String> {
    let root = PathBuf::from(path.trim());

    if root.as_os_str().is_empty() {
        return Err("путь не указан".into());
    }

    guard_path(&root)?;

    for sub in SUBDIRS {
        let dir = root.join(sub);
        fs::create_dir_all(&dir).map_err(|e| format!("не удалось создать {}: {e}", dir.display()))?;
    }

    let mut config = paths::load_or_create_config(&app)?;

    let already = config
        .libraries
        .iter()
        .any(|library| same_path(&library.root, &root));

    if already {
        return Err("это место уже добавлено".into());
    }

    let id = next_id(&config.libraries);

    let name = if name.trim().is_empty() {
        default_name(&root, &fallback)
    } else {
        name.trim().to_string()
    };

    config.libraries.push(Library {
        id: id.clone(),
        name,
        root: root.display().to_string(),
    });

    // Первое добавленное становится основным само: спрашивать об этом, когда
    // выбора ещё нет, незачем
    if config.primary_library.is_none() {
        config.primary_library = Some(id);
    }

    paths::write_config(&app, &config)?;

    Ok(config.libraries)
}

/// Убирает Место из списка.
///
/// Папку не трогает: в ней лежат программы, и удалить её молча по нажатию в
/// списке — потерять их. Удаление содержимого делается отдельно и осознанно.
#[tauri::command]
pub fn library_forget(app: AppHandle, id: String) -> Result<Vec<Library>, String> {
    let mut config = paths::load_or_create_config(&app)?;

    config.libraries.retain(|library| library.id != id);

    if config.primary_library.as_deref() == Some(id.as_str()) {
        config.primary_library = config.libraries.first().map(|library| library.id.clone());
    }

    paths::write_config(&app, &config)?;

    Ok(config.libraries)
}

/// Удаляет Место вместе с папкой.
///
/// В отличие от `library_forget`, стирает и содержимое — но только если внутри
/// ничего нет, кроме наших пустых подпапок. Место, в котором уже лежат
/// программы, так не удалить: для этого их сначала надо унести или удалить
/// осознанно, а не одним нажатием в списке.
#[tauri::command]
pub fn library_delete(app: AppHandle, id: String) -> Result<Vec<Library>, String> {
    let config = paths::load_or_create_config(&app)?;

    let Some(library) = config.libraries.iter().find(|library| library.id == id) else {
        return Err("такого места нет".into());
    };

    let root = PathBuf::from(&library.root);

    if root.is_dir() {
        let extra = foreign_entries(&root);

        if !extra.is_empty() {
            return Err(format!(
                "в папке есть содержимое: {}. Уберите место из списка или очистите папку сами",
                extra.join(", ")
            ));
        }

        fs::remove_dir_all(&root)
            .map_err(|e| format!("не удалось удалить {}: {e}", root.display()))?;
    }

    library_forget(app, id)
}

/// Переносит Место на другой диск.
///
/// Сначала пробуем переименовать — в пределах одного тома это мгновенно.
/// Между томами так нельзя, поэтому там идёт копирование с последующим
/// удалением исходного.
#[tauri::command]
pub async fn library_move(
    app: AppHandle,
    id: String,
    path: String,
    fallback: String,
) -> Result<(), String> {
    let mut config = paths::load_or_create_config(&app)?;

    let Some(library) = config.libraries.iter().find(|library| library.id == id) else {
        return Err("такого места нет".into());
    };

    let from = PathBuf::from(&library.root);
    let to = PathBuf::from(path.trim());

    guard_path(&to)?;

    if same_path(&from.display().to_string(), &to) {
        return Err("это то же самое место".into());
    }

    if to.exists() && foreign_entries(&to).iter().any(|_| true) {
        return Err("в выбранной папке уже что-то есть".into());
    }

    let source = from.clone();
    let target = to.clone();

    tauri::async_runtime::spawn_blocking(move || relocate(&source, &target))
        .await
        .map_err(|e| format!("перенос не завершён: {e}"))??;

    if let Some(library) = config.libraries.iter_mut().find(|library| library.id == id) {
        library.root = to.display().to_string();
        library.name = default_name(&to, &fallback);
    }

    paths::write_config(&app, &config)
}

/// Назначает Место основным.
#[tauri::command]
pub fn library_set_primary(app: AppHandle, id: String) -> Result<(), String> {
    let mut config = paths::load_or_create_config(&app)?;

    if !config.libraries.iter().any(|library| library.id == id) {
        return Err("такого места нет".into());
    }

    config.primary_library = Some(id);
    paths::write_config(&app, &config)
}

/// Переименовывает Место.
#[tauri::command]
pub fn library_rename(app: AppHandle, id: String, name: String) -> Result<(), String> {
    let mut config = paths::load_or_create_config(&app)?;

    let Some(library) = config.libraries.iter_mut().find(|library| library.id == id) else {
        return Err("такого места нет".into());
    };

    let name = name.trim();

    if name.is_empty() {
        return Err("имя не может быть пустым".into());
    }

    library.name = name.to_string();
    paths::write_config(&app, &config)
}

/// Диск, на котором можно завести Место.
#[derive(Serialize)]
pub struct Disk {
    /// «F:» — как человек его называет.
    pub letter: String,
    /// Метка тома. Пусто, если её не задавали.
    pub label: String,
    pub total: u64,
    pub free: u64,
    /// Съёмный: флешка или внешний диск. Такое Место переносится вместе с ним.
    pub removable: bool,
    /// Системный. Класть программы сюда можно, но обычно от этого и уходят.
    pub system: bool,
    /// Путь, который получится, если выбрать этот диск.
    pub path: String,
}

/// Диски, пригодные под Место.
///
/// Выбирают диск, а не папку: имя папки задаём мы сами и одинаково для всех —
/// так Место потом узнаётся на чужой машине по одному пути.
#[tauri::command]
pub fn library_disks() -> Vec<Disk> {
    let system = std::env::var("SystemDrive")
        .unwrap_or_else(|_| "C:".to_string())
        .to_uppercase();

    let mut disks = Vec::new();

    for letter in 'A'..='Z' {
        let root = format!("{letter}:\\");

        let Some(kind) = drive_kind(&root) else {
            continue;
        };

        // Сетевые и приводы отпадают: первое может исчезнуть посреди работы,
        // второе доступно только на чтение
        if !matches!(kind, DriveKind::Fixed | DriveKind::Removable) {
            continue;
        }

        let (total, free) = disk_space(&root);

        // Ноль означает, что носителя в приводе нет
        if total == 0 {
            continue;
        }

        disks.push(Disk {
            letter: format!("{letter}:"),
            label: volume_label(&root),
            total,
            free,
            removable: matches!(kind, DriveKind::Removable),
            system: format!("{letter}:") == system,
            path: format!("{root}QuickRapidX"),
        });
    }

    disks
}

/// Какой диск предложить по умолчанию.
///
/// Самый свободный несъёмный и несистемный: программы занимают много, класть их
/// на системный — ровно то, от чего человек уходит, а съёмный может оказаться
/// вынутым в самый нужный момент.
#[tauri::command]
pub fn library_suggest() -> String {
    let disks = library_disks();

    let best = disks
        .iter()
        .filter(|disk| !disk.system && !disk.removable)
        .max_by_key(|disk| disk.free)
        .or_else(|| disks.iter().max_by_key(|disk| disk.free));

    best.map(|disk| disk.path.clone()).unwrap_or_default()
}

/// Куда ставить программу: папка внутри основного Места.
///
/// Пусто, если Места нет вовсе — тогда установка идёт обычным путём, в систему.
#[tauri::command]
pub fn library_install_path(app: AppHandle, name: String) -> Result<String, String> {
    let config = paths::load_or_create_config(&app)?;

    let Some(id) = config.primary_library else {
        return Ok(String::new());
    };

    let Some(library) = config.libraries.iter().find(|library| library.id == id) else {
        return Ok(String::new());
    };

    // Имя папки берётся от названия программы, но чистится: в названиях бывают
    // двоеточия и косые черты, а это разделители пути
    let folder: String = name
        .trim()
        .chars()
        .map(|symbol| if r#"\/:*?"<>|"#.contains(symbol) { '-' } else { symbol })
        .collect();

    if folder.is_empty() {
        return Ok(String::new());
    }

    Ok(PathBuf::from(&library.root)
        .join("apps")
        .join(folder)
        .display()
        .to_string())
}

/// Появилась ли папка программы в Месте.
///
/// Ключ `--location` слушают не все установщики, и узнать это заранее нельзя.
/// Поэтому после установки просто смотрим, есть ли там что-нибудь.
#[tauri::command]
pub fn library_has(path: String) -> bool {
    let dir = PathBuf::from(path);

    dir.is_dir() && std::fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_some())
}

/// Папка «Загрузки» текущего пользователя, подпапка приложения.
///
/// Раньше путь собирался как `C:\Users\{имя}\Downloads` — и промахивался, если
/// профиль лежит не там или папка «Загрузки» перенесена, а перенести её
/// Windows позволяет. Здесь она спрашивается у системы.
#[tauri::command]
pub fn downloads_dir() -> Result<String, String> {
    let dir = downloads_root()?.join("QuickRapidX");

    fs::create_dir_all(&dir).map_err(|e| format!("не удалось создать {}: {e}", dir.display()))?;

    Ok(dir.display().to_string())
}

#[cfg(windows)]
fn downloads_root() -> Result<PathBuf, String> {
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{SHGetKnownFolderPath, FOLDERID_Downloads};

    // SAFETY: путь выделяет система, мы его копируем и тут же освобождаем тем
    // способом, которым он выдан.
    unsafe {
        let mut raw = std::ptr::null_mut();

        let hr = SHGetKnownFolderPath(
            &FOLDERID_Downloads,
            0,
            std::ptr::null_mut(),
            &mut raw,
        );

        if hr < 0 || raw.is_null() {
            return Err("система не сообщила путь к папке «Загрузки»".into());
        }

        let mut length = 0;
        while *raw.add(length) != 0 {
            length += 1;
        }

        let path = std::ffi::OsString::from_wide(std::slice::from_raw_parts(raw, length));
        CoTaskMemFree(raw as *const core::ffi::c_void);

        Ok(PathBuf::from(path))
    }
}

#[cfg(not(windows))]
fn downloads_root() -> Result<PathBuf, String> {
    Err("не поддерживаемая операционная система".into())
}

// ── Внутреннее ───────────────────────────────────────────────────────────

/// Что лежит в Месте помимо наших пустых подпапок.
///
/// Пустые `apps`, `data` и `shortcuts` не считаются содержимым: их создали мы
/// сами при заведении Места.
fn foreign_entries(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut extra = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        let ours = SUBDIRS.contains(&name.as_str())
            && fs::read_dir(entry.path()).is_ok_and(|mut inner| inner.next().is_none());

        if !ours {
            extra.push(name);
        }
    }

    extra
}

/// Переносит папку целиком.
fn relocate(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("не удалось создать {}: {e}", parent.display()))?;
    }

    // В пределах тома переименование мгновенно и не трогает данные
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }

    copy_tree(from, to)?;

    fs::remove_dir_all(from)
        .map_err(|e| format!("скопировано, но не удалось убрать {}: {e}", from.display()))
}

pub(crate) fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("не удалось создать {}: {e}", to.display()))?;

    let entries = fs::read_dir(from).map_err(|e| format!("не удалось прочитать {}: {e}", from.display()))?;

    for entry in entries.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());

        let Ok(kind) = entry.file_type() else {
            continue;
        };

        // Точки соединения ведут наружу — копировать по ним значит утащить
        // чужое дерево внутрь Места
        if kind.is_symlink() {
            continue;
        }

        if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .map_err(|e| format!("не удалось скопировать {}: {e}", source.display()))?;
        }
    }

    Ok(())
}

/// Не даёт завести Место там, где оно навредит.
///
/// Корень диска и системные папки отпадают: в первом случае подпапки Места
/// расползлись бы по всему тому, во втором — мы бы полезли туда, куда без прав
/// нельзя, и получили бы отказ уже после того, как обещали человеку папку.
fn guard_path(root: &Path) -> Result<(), String> {
    if root.parent().is_none() {
        return Err("нельзя выбрать корень диска — укажите папку внутри него".into());
    }

    let lower = root.display().to_string().to_lowercase();

    for forbidden in ["\\windows", "\\program files", "\\programdata"] {
        if lower.contains(forbidden) {
            return Err("нельзя выбрать системную папку".into());
        }
    }

    Ok(())
}

/// Сколько программ и сколько байт лежит в папке `apps`.
///
/// Программа — это подпапка первого уровня. Обход считает размер целиком;
/// отказы в доступе пропускаются, как и точки соединения: по ним обход ушёл бы
/// наружу и посчитал бы чужое.
fn measure(apps: &Path) -> (usize, u64) {
    let Ok(entries) = fs::read_dir(apps) else {
        return (0, 0);
    };

    let mut count = 0;
    let mut total = 0;

    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            count += 1;
            total += folder_size(&entry.path());
        }
    }

    (count, total)
}

fn folder_size(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };

            if kind.is_symlink() {
                continue;
            }

            if kind.is_dir() {
                stack.push(entry.path());
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }

    total
}

/// Род диска, который нас интересует.
enum DriveKind {
    Fixed,
    Removable,
    Other,
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Есть ли такой диск и какого он рода.
#[cfg(windows)]
fn drive_kind(root: &str) -> Option<DriveKind> {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;

    // SAFETY: строка завершена нулём и живёт до конца вызова.
    let kind = unsafe { GetDriveTypeW(wide(root).as_ptr()) };

    match kind {
        DRIVE_FIXED => Some(DriveKind::Fixed),
        DRIVE_REMOVABLE => Some(DriveKind::Removable),
        // 0 и 1 — диска нет; остальное (сеть, привод, RAM) нам не подходит
        0 | 1 => None,
        _ => Some(DriveKind::Other),
    }
}

/// Всего и свободно на томе. Нули, если спросить не вышло.
#[cfg(windows)]
fn disk_space(root: &str) -> (u64, u64) {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let path = wide(root);
    let mut free = 0u64;
    let mut total = 0u64;

    // SAFETY: строка завершена нулём, остальные указатели — на наши переменные.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut free,
            &mut total,
            std::ptr::null_mut(),
        )
    };

    if ok == 0 {
        (0, 0)
    } else {
        (total, free)
    }
}

/// Метка тома — то, как диск подписан в проводнике.
#[cfg(windows)]
fn volume_label(root: &str) -> String {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    let path = wide(root);
    let mut label = [0u16; 261];

    // SAFETY: буфер своей длины и есть, остальные поля нам не нужны.
    let ok = unsafe {
        GetVolumeInformationW(
            path.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };

    if ok == 0 {
        return String::new();
    }

    let length = label.iter().position(|c| *c == 0).unwrap_or(0);
    String::from_utf16_lossy(&label[..length])
}

#[cfg(not(windows))]
fn drive_kind(_root: &str) -> Option<DriveKind> {
    None
}

#[cfg(not(windows))]
fn disk_space(_root: &str) -> (u64, u64) {
    (0, 0)
}

#[cfg(not(windows))]
fn volume_label(_root: &str) -> String {
    String::new()
}

/// Метка тома или буква диска — как имя по умолчанию.
///
/// Запасное слово приходит доводом: имя ложится в настройки и остаётся там
/// навсегда, а русское «Место» в английском интерфейсе выглядело бы чужим.
fn default_name(root: &Path, fallback: &str) -> String {
    root.components()
        .next()
        .map(|first| first.as_os_str().to_string_lossy().trim_end_matches('\\').to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| fallback.trim().to_string())
}

/// Следующий свободный номер. Случайные идентификаторы здесь ни к чему: мест
/// единицы, а разбирать настройки глазами удобнее с «1», чем с UUID.
fn next_id(libraries: &[Library]) -> String {
    let max = libraries
        .iter()
        .filter_map(|library| library.id.parse::<u32>().ok())
        .max()
        .unwrap_or(0);

    (max + 1).to_string()
}

/// Сравнение путей без учёта регистра и хвостовой косой черты.
fn same_path(left: &str, right: &Path) -> bool {
    let normalize = |value: &str| {
        value
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
            .replace('/', "\\")
    };

    normalize(left) == normalize(&right.display().to_string())
}
