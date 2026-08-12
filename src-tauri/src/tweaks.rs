//! Специальные возможности и питание.
//!
//! Прежде залипание клавиш переключалось записью в реестр — и требовало
//! перезахода, потому что система читает эти значения при входе. Здесь оно
//! идёт через `SystemParametersInfo`: та меняет настройку в текущем сеансе,
//! сама пишет её в профиль и рассылает окнам оповещение. Перезаход не нужен.
//!
//! Заодно ушёл магический «510». В реестре это набор признаков одним числом:
//! доступна ли возможность, работает ли горячее сочетание, спрашивать ли
//! подтверждение. Разбирать его по битам вручную незачем — структура
//! `STICKYKEYS` называет их по именам.

use serde::Serialize;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

#[cfg(windows)]
use windows_sys::Win32::UI::Accessibility::{FILTERKEYS, STICKYKEYS, TOGGLEKEYS};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETFILTERKEYS,
    SPI_GETSTICKYKEYS, SPI_GETTOGGLEKEYS, SPI_SETFILTERKEYS, SPI_SETSTICKYKEYS, SPI_SETTOGGLEKEYS,
};

/// Признак «горячее сочетание работает» — общий у всех трёх возможностей.
///
/// Это он включает приглашение «нажать Shift пять раз», «удерживать правый
/// Shift восемь секунд» и «удерживать Num Lock пять секунд». Выключают обычно
/// именно его, а не саму возможность: она нужна тем, кому нужна, а всплывающее
/// окно мешает всем остальным.
#[cfg(windows)]
const HOTKEYACTIVE: u32 = 0x0000_0004;

/// Сводка по разделу.
#[derive(Serialize)]
pub struct Tweaks {
    /// Приглашение по пятикратному Shift.
    pub sticky_hotkey: bool,
    /// Приглашение по удержанию правого Shift.
    pub filter_hotkey: bool,
    /// Приглашение по удержанию Num Lock.
    pub toggle_hotkey: bool,
    /// Гибернация включена.
    pub hibernate: bool,
    /// Быстрый запуск включён. Без гибернации он не работает.
    pub fast_startup: bool,
    /// Размер файла гибернации, байты. Ноль — файла нет.
    pub hiberfil: u64,
    /// Есть ли права администратора: без них питание не переключить.
    pub admin: bool,
}

/// Читает всё разом.
#[tauri::command]
pub fn tweaks_get() -> Tweaks {
    Tweaks {
        sticky_hotkey: sticky_hotkey(),
        filter_hotkey: filter_hotkey(),
        toggle_hotkey: toggle_hotkey(),
        hibernate: hibernate_enabled(),
        fast_startup: fast_startup_enabled(),
        hiberfil: hiberfil_size(),
        admin: crate::windows_cmd::windows_pars_admin(),
    }
}

// ── Специальные возможности ──────────────────────────────────────────────

#[cfg(windows)]
macro_rules! read_flags {
    ($struct:ty, $get:expr) => {{
        // SAFETY: размер передаётся из самой структуры, указатель на неё живёт
        // до конца выражения.
        unsafe {
            let mut data: $struct = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<$struct>() as u32;

            let ok = SystemParametersInfoW(
                $get,
                data.cbSize,
                &mut data as *mut _ as *mut core::ffi::c_void,
                0,
            );

            if ok == 0 {
                None
            } else {
                Some(data.dwFlags)
            }
        }
    }};
}

#[cfg(windows)]
macro_rules! write_flags {
    ($struct:ty, $get:expr, $set:expr, $on:expr) => {{
        // SAFETY: та же структура читается и пишется обратно, размер её же.
        unsafe {
            let mut data: $struct = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<$struct>() as u32;

            if SystemParametersInfoW(
                $get,
                data.cbSize,
                &mut data as *mut _ as *mut core::ffi::c_void,
                0,
            ) == 0
            {
                return Err("не удалось прочитать текущее состояние".into());
            }

            if $on {
                data.dwFlags |= HOTKEYACTIVE;
            } else {
                data.dwFlags &= !HOTKEYACTIVE;
            }

            // UPDATEINIFILE сохраняет настройку в профиль, SENDCHANGE сообщает
            // окнам. Без первого она забудется при выходе, без второго часть
            // программ продолжит считать по-старому.
            let ok = SystemParametersInfoW(
                $set,
                data.cbSize,
                &mut data as *mut _ as *mut core::ffi::c_void,
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            );

            if ok == 0 {
                return Err("система отклонила изменение".into());
            }
        }

        Ok(())
    }};
}

#[cfg(windows)]
fn sticky_hotkey() -> bool {
    read_flags!(STICKYKEYS, SPI_GETSTICKYKEYS).is_some_and(|flags| flags & HOTKEYACTIVE != 0)
}

#[cfg(windows)]
fn filter_hotkey() -> bool {
    read_flags!(FILTERKEYS, SPI_GETFILTERKEYS).is_some_and(|flags| flags & HOTKEYACTIVE != 0)
}

#[cfg(windows)]
fn toggle_hotkey() -> bool {
    read_flags!(TOGGLEKEYS, SPI_GETTOGGLEKEYS).is_some_and(|flags| flags & HOTKEYACTIVE != 0)
}

/// Приглашение по пятикратному Shift.
#[tauri::command]
pub fn tweaks_set_sticky(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        write_flags!(STICKYKEYS, SPI_GETSTICKYKEYS, SPI_SETSTICKYKEYS, on)
    }

    #[cfg(not(windows))]
    {
        let _ = on;
        Err("не поддерживаемая операционная система".into())
    }
}

/// Приглашение по удержанию правого Shift.
#[tauri::command]
pub fn tweaks_set_filter(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        write_flags!(FILTERKEYS, SPI_GETFILTERKEYS, SPI_SETFILTERKEYS, on)
    }

    #[cfg(not(windows))]
    {
        let _ = on;
        Err("не поддерживаемая операционная система".into())
    }
}

/// Приглашение по удержанию Num Lock.
#[tauri::command]
pub fn tweaks_set_toggle(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        write_flags!(TOGGLEKEYS, SPI_GETTOGGLEKEYS, SPI_SETTOGGLEKEYS, on)
    }

    #[cfg(not(windows))]
    {
        let _ = on;
        Err("не поддерживаемая операционная система".into())
    }
}

#[cfg(not(windows))]
fn sticky_hotkey() -> bool {
    false
}

#[cfg(not(windows))]
fn filter_hotkey() -> bool {
    false
}

#[cfg(not(windows))]
fn toggle_hotkey() -> bool {
    false
}

// ── Питание ──────────────────────────────────────────────────────────────

/// Гибернация включена.
fn hibernate_enabled() -> bool {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control\Power", KEY_READ)
        .ok()
        .and_then(|key| key.get_value::<u32, &str>("HibernateEnabled").ok())
        .is_some_and(|value| value == 1)
}

/// Быстрый запуск включён.
///
/// Он держится на гибернации: при выключении система усыпляет ядро в тот же
/// файл. Выключишь гибернацию — быстрый запуск отключится вместе с ней, поэтому
/// в интерфейсе он подчинён ей.
fn fast_startup_enabled() -> bool {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Power",
            KEY_READ,
        )
        .ok()
        .and_then(|key| key.get_value::<u32, &str>("HiberbootEnabled").ok())
        .is_some_and(|value| value == 1)
}

/// Размер файла гибернации.
///
/// Он занимает заметную часть системного диска — обычно от трети до половины
/// оперативной памяти, — и это главный довод выключить гибернацию. Показать
/// цифру честнее, чем предлагать выбор вслепую.
fn hiberfil_size() -> u64 {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());

    std::fs::metadata(format!("{drive}\\hiberfil.sys"))
        .map(|meta| meta.len())
        .unwrap_or(0)
}

/// Переключает гибернацию.
///
/// Через `powercfg`, а не записью в реестр: утилита ещё и создаёт или удаляет
/// сам файл гибернации, чего запись значения не делает. Нужны права
/// администратора.
#[tauri::command]
pub fn tweaks_set_hibernate(on: bool) -> Result<(), String> {
    run_powercfg(&["/hibernate", if on { "on" } else { "off" }])
}

/// Переключает быстрый запуск.
#[tauri::command]
pub fn tweaks_set_fast_startup(on: bool) -> Result<(), String> {
    if on && !hibernate_enabled() {
        return Err("быстрый запуск работает только вместе с гибернацией".into());
    }

    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Power",
            winreg::enums::KEY_SET_VALUE,
        )
        .map_err(|e| format!("нужны права администратора: {e}"))?;

    key.set_value("HiberbootEnabled", &if on { 1u32 } else { 0u32 })
        .map_err(|e| format!("не удалось записать значение: {e}"))
}

#[cfg(windows)]
fn run_powercfg(args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = Command::new("powercfg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("не удалось запустить powercfg: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    // powercfg пишет отказ в кодировке консоли — разбирать её незачем, причина
    // у отказа почти всегда одна
    Err("нужны права администратора".into())
}

#[cfg(not(windows))]
fn run_powercfg(_args: &[&str]) -> Result<(), String> {
    Err("не поддерживаемая операционная система".into())
}
