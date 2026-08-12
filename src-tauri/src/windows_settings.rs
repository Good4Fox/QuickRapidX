//! Системные настройки Windows: питание, реестр, цвет рамки выделения.
//!
//! Перенесено из `windows_settings.rs` прошлой версии. Отличия:
//! `winapi` (не поддерживается с 2020) заменён на `windows-sys`, `winreg`
//! поднят с 0.10 до 0.56, а завершение сеанса, выключение и перезагрузка идут
//! через `shutdown.exe` вместо `ExitWindowsEx` с ручной выдачей привилегии
//! SE_SHUTDOWN — так же, как это уже делал `windows_logoff` в оригинале.

use std::process::Command;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
use winreg::RegKey;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Запускает системную утилиту, не показывая консольное окно.
fn run_hidden(program: &str, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Открывает «Параметры» Windows.
#[tauri::command]
pub fn windows_open_settings() -> Result<(), String> {
    run_hidden("cmd", &["/c", "start", "", "ms-settings:"])
}

/// Открывает проводник на «Этот компьютер».
///
/// Через `explorer.exe` с оболочечной папкой, а не по пути вроде `C:\`:
/// буква системного диска не у всех одна и та же, а этот адрес есть всегда.
#[tauri::command]
pub fn windows_open_explorer() -> Result<(), String> {
    run_hidden("explorer", &["shell:MyComputerFolder"])
}

/// Блокирует рабочую станцию.
#[tauri::command]
pub fn windows_lock_workstation() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Shutdown::LockWorkStation;

        // SAFETY: вызов без аргументов, ничего не разыменовываем.
        let ok = unsafe { LockWorkStation() };
        if ok == 0 {
            return Err("не удалось заблокировать рабочую станцию".into());
        }
        Ok(())
    }

    #[cfg(not(windows))]
    Err("поддерживается только на Windows".into())
}

/// Переводит компьютер в сон.
#[tauri::command]
pub fn windows_sleep() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::SetSuspendState;

        // Первый аргумент false — именно сон, а не гибернация.
        // SAFETY: примитивные аргументы, указателей нет.
        let ok = unsafe { SetSuspendState(false, false, false) };
        if !ok {
            return Err("не удалось перейти в спящий режим".into());
        }
        Ok(())
    }

    #[cfg(not(windows))]
    Err("поддерживается только на Windows".into())
}

/// Выключает компьютер.
#[tauri::command]
pub fn windows_shutdown() -> Result<(), String> {
    run_hidden("shutdown", &["/s", "/t", "0"])
}

/// Перезагружает компьютер.
#[tauri::command]
pub fn windows_restart() -> Result<(), String> {
    run_hidden("shutdown", &["/r", "/t", "0"])
}

/// Завершает сеанс пользователя.
#[tauri::command]
pub fn windows_logoff() -> Result<(), String> {
    run_hidden("shutdown", &["/l"])
}

/// Меняет цвет рамки выделения проводника.
///
/// Значения — строки вида «0 102 204», как их вводит экран настроек.
#[tauri::command]
pub fn windows_update_square_box(
    windows_settings_color_fill: &str,
    windows_settings_color_stroke: &str,
) -> Result<(), String> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(r"Control Panel\Colors")
        .map_err(|e| format!("не удалось открыть Control Panel\\Colors: {e}"))?;

    key.set_value("Hilight", &windows_settings_color_stroke)
        .map_err(|e| format!("не удалось записать Hilight: {e}"))?;

    key.set_value("HotTrackingColor", &windows_settings_color_fill)
        .map_err(|e| format!("не удалось записать HotTrackingColor: {e}"))?;

    Ok(())
}

/// Читает значение «Flags» у ключа в HKEY_CURRENT_USER.
#[tauri::command]
pub fn regedit_get_keys_flags(hkey_path: &str) -> Result<String, String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(hkey_path, KEY_READ)
        .map_err(|e| format!("не удалось открыть ключ: {e}"))?
        .get_value::<String, _>("Flags")
        .map_err(|e| format!("не удалось прочитать 'Flags': {e}"))
}

/// Записывает значение «Flags» в ключ HKEY_CURRENT_USER.
#[tauri::command]
pub fn regedit_set_keys_flags(hkey_path: &str, value: &str) -> Result<(), String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(hkey_path, KEY_WRITE)
        .map_err(|e| format!("не удалось открыть ключ: {e}"))?
        .set_value("Flags", &value)
        .map_err(|e| format!("не удалось записать 'Flags': {e}"))
}

/// Включена ли гибернация.
#[tauri::command]
pub fn regedit_get_keys_hibernate() -> Result<u32, String> {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control\Power", KEY_READ)
        .map_err(|e| e.to_string())?
        .get_value::<u32, _>("HibernateEnabled")
        .map_err(|e| e.to_string())
}

/// Включает или выключает гибернацию. Требует прав администратора.
#[tauri::command]
pub fn regedit_set_keys_hibernate(state: String) -> Result<String, String> {
    let on = match state.as_str() {
        "on" => true,
        "off" => false,
        _ => return Err("неверное состояние: ожидается 'on' или 'off'".into()),
    };

    let output = {
        let mut cmd = Command::new("powercfg");
        cmd.args(["/h", state.as_str()]);

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        cmd.output().map_err(|e| e.to_string())?
    };

    if !output.status.success() {
        // powercfg без прав администратора возвращает ненулевой код —
        // раньше это молча считалось успехом.
        return Err(format!(
            "powercfg вернул ошибку: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(format!("Hibernate {}", if on { "enabled" } else { "disabled" }))
}

