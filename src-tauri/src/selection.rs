//! Цвет прямоугольника выделения.
//!
//! Проводник рисует рамку выделения двумя системными цветами: заливка берётся
//! из `COLOR_HOTLIGHT`, обводка — из `COLOR_HIGHLIGHT`. В реестре они лежат в
//! `Control Panel\Colors` строками вида «0 102 204».
//!
//! Прежде приложение только писало в реестр — и требовало перезахода, потому
//! что запись сама по себе ничего не меняет: система читает эти значения при
//! входе. `SetSysColors` меняет цвета текущего сеанса и рассылает всем окнам
//! WM_SYSCOLORCHANGE, поэтому рамка перекрашивается сразу. Реестр всё равно
//! пишем — иначе цвет вернётся к прежнему после перезахода.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::paths;

/// Индексы системных цветов. Из WinUser.h.
///
/// Обе функции живут в Graphics::Gdi, а не в WindowsAndMessaging, как можно
/// подумать по названию.
const COLOR_HIGHLIGHT: i32 = 13;
const COLOR_HOTLIGHT: i32 = 26;

/// Подсветка в меню и списках.
///
/// Отдельное от `Hilight` значение, и по умолчанию оно синее даже там, где
/// `Hilight` уже перекрашен. Пишем его тем же цветом обводки: иначе часть
/// подсветок в системе остаётся прежней и спорит с выбранным цветом.
const COLOR_MENUHILIGHT: i32 = 29;

/// Цвет как три составляющих. В реестре — строка «R G B», в интерфейсе удобнее
/// числа: разбор строки в одном месте лучше, чем в каждом.
pub type Rgb = [u8; 3];

/// Пара цветов рамки выделения.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Swatch {
    pub fill: Rgb,
    pub stroke: Rgb,
}

fn parse(value: &str) -> Option<Rgb> {
    let parts: Vec<&str> = value.split_whitespace().collect();

    if parts.len() != 3 {
        return None;
    }

    let mut out = [0u8; 3];

    for (index, part) in parts.iter().enumerate() {
        out[index] = part.parse().ok()?;
    }

    Some(out)
}

fn format(color: Rgb) -> String {
    format!("{} {} {}", color[0], color[1], color[2])
}

/// Цвет из реестра, а при отсутствии — тот, что система использует сейчас.
///
/// Второй случай не редкость: пока цвет ни разу не меняли, значения в
/// `Control Panel\Colors` может не быть вовсе, и брать «по умолчанию» из
/// головы значило бы показать не то, что человек видит на экране.
fn current(name: &str, index: i32) -> Rgb {
    let from_registry = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Control Panel\Colors")
        .ok()
        .and_then(|key| key.get_value::<String, &str>(name).ok())
        .and_then(|value| parse(&value));

    if let Some(color) = from_registry {
        return color;
    }

    system_color(index)
}

#[cfg(windows)]
fn system_color(index: i32) -> Rgb {
    use windows_sys::Win32::Graphics::Gdi::GetSysColor;

    // SAFETY: вызов с числовым индексом, ничего не разыменовываем.
    let packed = unsafe { GetSysColor(index) };

    // COLORREF — это 0x00BBGGRR
    [
        (packed & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
    ]
}

#[cfg(not(windows))]
fn system_color(_index: i32) -> Rgb {
    [0, 120, 215]
}

/// Текущая пара цветов рамки выделения.
#[tauri::command]
pub fn selection_get() -> Swatch {
    Swatch {
        fill: current("HotTrackingColor", COLOR_HOTLIGHT),
        stroke: current("Hilight", COLOR_HIGHLIGHT),
    }
}

/// Ставит цвета: сразу в текущем сеансе и в реестр — на будущие.
#[tauri::command]
pub fn selection_apply(fill: Rgb, stroke: Rgb) -> Result<(), String> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(r"Control Panel\Colors")
        .map_err(|e| format!("не удалось открыть Control Panel\\Colors: {e}"))?;

    key.set_value("Hilight", &format(stroke))
        .map_err(|e| format!("не удалось записать Hilight: {e}"))?;

    key.set_value("HotTrackingColor", &format(fill))
        .map_err(|e| format!("не удалось записать HotTrackingColor: {e}"))?;

    key.set_value("MenuHilight", &format(stroke))
        .map_err(|e| format!("не удалось записать MenuHilight: {e}"))?;

    apply_live(fill, stroke)
}

#[cfg(windows)]
fn apply_live(fill: Rgb, stroke: Rgb) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::SetSysColors;

    let indexes = [COLOR_HIGHLIGHT, COLOR_HOTLIGHT, COLOR_MENUHILIGHT];
    let values = [pack(stroke), pack(fill), pack(stroke)];

    // SAFETY: длина совпадает с числом элементов обоих массивов, оба живут до
    // конца вызова.
    let ok = unsafe { SetSysColors(3, indexes.as_ptr(), values.as_ptr()) };

    if ok == 0 {
        return Err("система отклонила смену цветов".into());
    }

    Ok(())
}

#[cfg(not(windows))]
fn apply_live(_fill: Rgb, _stroke: Rgb) -> Result<(), String> {
    Err("не поддерживаемая операционная система".into())
}

#[cfg(windows)]
fn pack(color: Rgb) -> u32 {
    (color[0] as u32) | ((color[1] as u32) << 8) | ((color[2] as u32) << 16)
}

// ── Своя палитра ─────────────────────────────────────────────────────────

/// Сохранённые сочетания.
#[tauri::command]
pub fn selection_palette(app: AppHandle) -> Result<Vec<Swatch>, String> {
    Ok(paths::load_or_create_config(&app)?.selection_palette)
}

/// Добавляет сочетание в палитру.
///
/// Повторы отбрасываются: палитра нужна, чтобы переключаться между несколькими
/// вариантами, а не копить одинаковые.
#[tauri::command]
pub fn selection_palette_add(app: AppHandle, fill: Rgb, stroke: Rgb) -> Result<Vec<Swatch>, String> {
    let mut config = paths::load_or_create_config(&app)?;

    let exists = config
        .selection_palette
        .iter()
        .any(|swatch| swatch.fill == fill && swatch.stroke == stroke);

    if !exists {
        config.selection_palette.push(Swatch { fill, stroke });
        paths::write_config(&app, &config)?;
    }

    Ok(config.selection_palette)
}

/// Убирает сочетание по месту в списке.
#[tauri::command]
pub fn selection_palette_remove(app: AppHandle, index: usize) -> Result<Vec<Swatch>, String> {
    let mut config = paths::load_or_create_config(&app)?;

    if index < config.selection_palette.len() {
        config.selection_palette.remove(index);
        paths::write_config(&app, &config)?;
    }

    Ok(config.selection_palette)
}
