//! Поведение окна и запуска.
//!
//! Раньше эти настройки жили в трёх разных местах и ни одна не работала до
//! конца:
//!
//! * автозапуск включался подключаемым модулем, а его состояние интерфейс брал
//!   из localStorage — стоило убрать запись через диспетчер задач, и флажок
//!   продолжал показывать «включено»;
//! * `--start-minimized` дописывался в запись реестра, но никто его не читал:
//!   окно показывалось безусловно;
//! * «сворачивать в панель задач» писалось в localStorage, а обработчик
//!   закрытия прятал окно всегда, ни на что не глядя.
//!
//! Теперь правда одна: реестр — для автозапуска, `config.json` — для
//! остального, а интерфейс их читает, а не помнит.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};
use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};
use winreg::RegKey;

use crate::paths;

/// Ветка автозапуска текущего пользователя.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Имя записи. Такое же, каким пользовался подключаемый модуль автозапуска.
const RUN_NAME: &str = "QuickRapidX";

/// Признак «запустили из автозапуска»: только там он и появляется.
const MINIMIZED_FLAG: &str = "--start-minimized";

/// Последние известные размеры и положение окна.
///
/// Пишутся на каждое перемещение, а на диск уходят один раз при закрытии:
/// иначе перетаскивание окна означало бы сотню записей файла в секунду.
static GEOMETRY: Mutex<Option<(i32, i32, u32, u32)>> = Mutex::new(None);

/// Сводка для интерфейса. Собирается из реестра и файла настроек, а не из
/// памяти окна: это единственный способ заметить правку снаружи.
#[derive(serde::Serialize)]
pub struct Prefs {
    pub autostart: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub always_on_top: bool,
    pub remember_geometry: bool,
    pub remember_view: bool,
}

fn run_key() -> Result<RegKey, String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_ALL_ACCESS)
        .map_err(|e| format!("не удалось открыть ветку автозапуска: {e}"))
}

/// Текущая команда автозапуска, если запись есть.
fn run_value() -> Option<String> {
    run_key().ok()?.get_value::<String, &str>(RUN_NAME).ok()
}

/// Путь к своему exe в кавычках — с ним запись переживёт пробелы в пути.
fn own_command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("не удалось определить путь к программе: {e}"))?;

    Ok(format!("\"{}\"", exe.display()))
}

/// Читает всё разом.
#[tauri::command]
pub fn window_prefs_get(app: AppHandle) -> Result<Prefs, String> {
    let config = paths::load_or_create_config(&app)?;
    let value = run_value();

    Ok(Prefs {
        autostart: value.is_some(),
        // Флаг в записи реестра, а не отметка в файле: именно он решает,
        // покажется ли окно при входе в систему
        start_minimized: value.is_some_and(|v| v.contains(MINIMIZED_FLAG)),
        close_to_tray: config.close_to_tray,
        always_on_top: config.always_on_top,
        remember_geometry: config.remember_geometry,
        remember_view: config.remember_view,
    })
}

/// Включает или выключает автозапуск, сохраняя признак свёрнутого старта.
#[tauri::command]
pub fn window_prefs_set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    let key = run_key()?;

    if !on {
        // Записи может и не быть — это не ошибка
        let _ = key.delete_value(RUN_NAME);
    } else {
        let config = paths::load_or_create_config(&app)?;
        let mut command = own_command()?;

        if config.start_minimized {
            command.push(' ');
            command.push_str(MINIMIZED_FLAG);
        }

        key.set_value(RUN_NAME, &command)
            .map_err(|e| format!("не удалось записать автозапуск: {e}"))?;
    }

    let mut config = paths::load_or_create_config(&app)?;
    config.autostart = on;
    paths::write_config(&app, &config)
}

/// Ставит признак свёрнутого старта.
///
/// Раньше здесь было переключение вслепую: команда меняла флаг на
/// противоположный, не спрашивая, каким он должен стать. Стоило состоянию в
/// интерфейсе разойтись с реестром — и флажок начинал работать наоборот.
#[tauri::command]
pub fn window_prefs_set_start_minimized(app: AppHandle, on: bool) -> Result<(), String> {
    let mut config = paths::load_or_create_config(&app)?;
    config.start_minimized = on;
    paths::write_config(&app, &config)?;

    // Автозапуска нет — запись появится вместе с ним, уже с нужным флагом
    let Some(value) = run_value() else {
        return Ok(());
    };

    let base = value
        .replace(&format!(" {MINIMIZED_FLAG}"), "")
        .replace(MINIMIZED_FLAG, "")
        .trim()
        .to_string();

    let next = if on {
        format!("{base} {MINIMIZED_FLAG}")
    } else {
        base
    };

    run_key()?
        .set_value(RUN_NAME, &next)
        .map_err(|e| format!("не удалось обновить автозапуск: {e}"))
}

/// Прятать окно в трей вместо выхода при закрытии.
#[tauri::command]
pub fn window_prefs_set_close_to_tray(app: AppHandle, on: bool) -> Result<(), String> {
    let mut config = paths::load_or_create_config(&app)?;
    config.close_to_tray = on;
    paths::write_config(&app, &config)
}

/// Держать окно поверх остальных.
#[tauri::command]
pub fn window_prefs_set_always_on_top(app: AppHandle, on: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(on)
            .map_err(|e| format!("не удалось изменить порядок окон: {e}"))?;
    }

    let mut config = paths::load_or_create_config(&app)?;
    config.always_on_top = on;
    paths::write_config(&app, &config)
}

/// Запоминать положение и размер окна между запусками.
#[tauri::command]
pub fn window_prefs_set_remember_geometry(app: AppHandle, on: bool) -> Result<(), String> {
    let mut config = paths::load_or_create_config(&app)?;
    config.remember_geometry = on;

    if !on {
        config.geometry = None;
    }

    paths::write_config(&app, &config)
}

/// Возвращаться ли на тот же раздел после показа окна.
#[tauri::command]
pub fn window_prefs_set_remember_view(app: AppHandle, on: bool) -> Result<(), String> {
    let mut config = paths::load_or_create_config(&app)?;
    config.remember_view = on;
    paths::write_config(&app, &config)
}

/// Прятать ли окно при закрытии. Читается обработчиком закрытия.
pub fn close_to_tray(app: &AppHandle) -> bool {
    paths::load_or_create_config(app)
        .map(|config| config.close_to_tray)
        .unwrap_or(true)
}

/// Запускались ли из автозапуска со свёрнутым стартом.
pub fn started_minimized() -> bool {
    std::env::args().any(|arg| arg == MINIMIZED_FLAG)
}

/// Запоминает положение окна. Вызывается на перемещение и изменение размера.
pub fn note_geometry(window: &tauri::Window) {
    if window.label() != "main" {
        return;
    }

    let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };

    // Свёрнутое окно Windows отдаёт как 0×0 и уводит далеко за экран —
    // запоминать такое значит потерять настоящее положение
    if size.width == 0 || size.height == 0 {
        return;
    }

    /*
     * Пока идёт загрузка, окно стоит размером с заставку и только потом
     * вырастает до рабочего. Изменения размера приходят и в этот момент, и
     * раньше в память попадал именно размер заставки — 350×450, — а с ним
     * интерфейс не помещается вовсе.
     */
    if !crate::boot::is_done() {
        return;
    }

    if let Ok(mut slot) = GEOMETRY.lock() {
        *slot = Some((position.x, position.y, size.width, size.height));
    }
}

/// Сбрасывает запомненное положение на диск. Вызывается при закрытии.
pub fn flush_geometry(app: &AppHandle) {
    let Ok(mut config) = paths::load_or_create_config(app) else {
        return;
    };

    if !config.remember_geometry {
        return;
    }

    let Ok(slot) = GEOMETRY.lock() else {
        return;
    };

    let Some((x, y, width, height)) = *slot else {
        return;
    };

    config.geometry = Some(paths::Geometry { x, y, width, height });
    let _ = paths::write_config(app, &config);
}

/// Применяет сохранённые предпочтения к окну при старте.
pub fn restore(app: &AppHandle) {
    let Ok(config) = paths::load_or_create_config(app) else {
        return;
    };

    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if config.always_on_top {
        let _ = window.set_always_on_top(true);
    }

    if !config.remember_geometry {
        return;
    }

    let Some(geometry) = config.geometry else {
        return;
    };

    // Размер ставится до положения: иначе окно сначала прыгает в точку, а
    // потом растёт от неё, и на многомониторной раскладке может уехать
    let _ = window.set_size(PhysicalSize::new(geometry.width, geometry.height));
    let _ = window.set_position(PhysicalPosition::new(geometry.x, geometry.y));
}
