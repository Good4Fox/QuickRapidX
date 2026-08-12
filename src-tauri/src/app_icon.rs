//! Значок приложения.
//!
//! Меняется на ходу в трёх местах: окно (оно же панель задач), иконка в трее и
//! ярлыки запуска. Первые два умеет сама Tauri, третье — дело Windows: у
//! ярлыка переписывается `IconLocation`, после чего проводнику подаётся сигнал
//! обновить кэш значков. Без сигнала он показывает прежнюю картинку сколь
//! угодно долго — кэш переживает и перезапуск программы, и перезагрузку.
//!
//! Картинки вшиты в двоичный файл, а не читаются рядом с exe: значок ставится
//! до того, как окно появится на экране, и не должен зависеть от того, что
//! лежит в папке установки.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager};

use crate::paths;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Запуск вспомогательной оболочки без мелькающего окна консоли.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Один вариант значка: что показать в окне, в трее и на ярлыке.
struct Variant {
    id: &'static str,
    /// 256×256 — окно и панель задач.
    window: &'static [u8],
    /// 44×44 — трей: рисунок для мелкого размера, а не уменьшённый большой.
    tray: &'static [u8],
    /// Многоразмерный .ico — только ярлыки понимают этот формат.
    shortcut: &'static [u8],
}

macro_rules! variant {
    ($id:literal) => {
        Variant {
            id: $id,
            window: include_bytes!(concat!("../../src/lib/App/img/appicons/", $id, ".png")),
            tray: include_bytes!(concat!("../../src/lib/App/img/appicons/", $id, "_tray.png")),
            shortcut: include_bytes!(concat!("../../src/lib/App/img/appicons/", $id, ".ico")),
        }
    };
}

const VARIANTS: &[Variant] = &[
    variant!("gold"),
    variant!("star"),
    variant!("frost"),
    variant!("mint"),
    variant!("ember"),
    variant!("orchid"),
    variant!("mono"),
];

pub const DEFAULT: &str = "gold";

fn find(id: &str) -> &'static Variant {
    VARIANTS
        .iter()
        .find(|variant| variant.id == id)
        .unwrap_or(&VARIANTS[0])
}

/// Идентификаторы вариантов — интерфейс берёт по ним свои картинки.
#[tauri::command]
pub fn app_icon_list() -> Vec<&'static str> {
    VARIANTS.iter().map(|variant| variant.id).collect()
}

/// Выбранный сейчас вариант.
#[tauri::command]
pub fn app_icon_get(app: AppHandle) -> String {
    paths::load_or_create_config(&app)
        .map(|config| config.icon)
        .unwrap_or_else(|_| DEFAULT.to_string())
}

/// Ставит вариант и запоминает выбор.
///
/// `shortcuts` отделён от остального намеренно: правка ярлыков трогает файлы за
/// пределами приложения и заставляет проводник перечитать кэш значков, поэтому
/// делается только по явной просьбе.
#[tauri::command]
pub fn app_icon_set(app: AppHandle, id: String, shortcuts: bool) -> Result<(), String> {
    let variant = find(&id);

    apply(&app, variant.id)?;

    let mut config = paths::load_or_create_config(&app)?;
    config.icon = variant.id.to_string();
    paths::write_config(&app, &config)?;

    if shortcuts {
        let ico = write_ico(&app, variant)?;
        retarget_shortcuts(&ico);
        refresh_shell_icons();
    }

    // Знак стоит и внутри интерфейса — в полосе заголовка обоих окон и на
    // экране загрузки. Настройки открыты в одном окне, поэтому выбор
    // рассылается всем: иначе трей остался бы со старым знаком до перезапуска.
    let _ = app.emit("app:icon-changed", variant.id);

    Ok(())
}

/// Применяет значок к окну и трею. Вызывается и при старте.
pub fn apply(app: &AppHandle, id: &str) -> Result<(), String> {
    let variant = find(id);

    let window_icon =
        Image::from_bytes(variant.window).map_err(|e| format!("значок окна не прочитан: {e}"))?;

    if let Some(window) = app.get_webview_window("main") {
        window
            .set_icon(window_icon)
            .map_err(|e| format!("значок окна не поставлен: {e}"))?;
    }

    let tray_icon =
        Image::from_bytes(variant.tray).map_err(|e| format!("значок трея не прочитан: {e}"))?;

    if let Some(tray) = app.tray_by_id("quickrapidx") {
        tray.set_icon(Some(tray_icon))
            .map_err(|e| format!("значок трея не поставлен: {e}"))?;
    }

    Ok(())
}

/// Поднимает сохранённый выбор. Тихо: отказ значка не повод рушить запуск.
pub fn restore(app: &AppHandle) {
    let id = paths::load_or_create_config(app)
        .map(|config| config.icon)
        .unwrap_or_else(|_| DEFAULT.to_string());

    if id == DEFAULT {
        return;
    }

    if let Err(e) = apply(app, &id) {
        eprintln!("значок приложения не восстановлен: {e}");
    }
}

/// Кладёт .ico в каталог данных и отдаёт путь.
///
/// Путь должен быть постоянным: ярлык хранит его строкой и будет искать файл
/// там же после обновления программы.
fn write_ico(app: &AppHandle, variant: &Variant) -> Result<PathBuf, String> {
    let path = paths::data_dir(app)?.join("app-icon.ico");

    fs::write(&path, variant.shortcut).map_err(|e| format!("не удалось записать {}: {e}", path.display()))?;

    Ok(path)
}

/// Ярлыки, которые могут указывать на нашу программу.
#[cfg(windows)]
fn shortcut_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(dir) = std::env::var("USERPROFILE") {
        roots.push(PathBuf::from(&dir).join("Desktop"));
        roots.push(PathBuf::from(&dir).join("OneDrive").join("Desktop"));
    }

    if let Ok(dir) = std::env::var("APPDATA") {
        roots.push(
            PathBuf::from(dir)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    let mut found = Vec::new();

    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.to_lowercase().contains("quickrapidx"))
            {
                found.push(path);
            }
        }
    }

    found
}

/// Переписывает значок у найденных ярлыков.
///
/// Через WScript.Shell, а не через COM-интерфейс напрямую: тот же результат
/// тремя строками вместо ручной работы с IShellLink и IPersistFile.
#[cfg(windows)]
fn retarget_shortcuts(ico: &Path) {
    let targets = shortcut_candidates();

    if targets.is_empty() {
        return;
    }

    let ico = ico.display().to_string();

    for lnk in targets {
        let script = format!(
            "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); \
             $s.IconLocation = '{},0'; $s.Save()",
            lnk.display().to_string().replace('\'', "''"),
            ico.replace('\'', "''")
        );

        let result = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        if let Err(e) = result {
            eprintln!("значок ярлыка {} не изменён: {e}", lnk.display());
        }
    }
}

/// Просит проводник перечитать значки.
///
/// Без этого он продолжает рисовать прежний из своего кэша: тот живёт в
/// отдельном файле и не сбрасывается ни выходом из программы, ни перезагрузкой.
#[cfg(windows)]
fn refresh_shell_icons() {
    use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

    // Событие объявлено как i32, а константа приходит как u32 — приводим явно
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

#[cfg(not(windows))]
fn retarget_shortcuts(_ico: &Path) {}

#[cfg(not(windows))]
fn refresh_shell_icons() {}
