//! Курсор пользователя для предпросмотра.
//!
//! В предпросмотре рамки выделения раньше лежала нарисованная стрелка из
//! набора значков. Она не имела отношения к тому, что человек видит на экране:
//! курсор в Windows меняется схемой, бывает крупнее, другой формы и с
//! анимацией.
//!
//! Здесь берётся настоящий курсор. Путь к файлу лежит в
//! `HKCU\Control Panel\Cursors\Arrow`; пусто — значит схема стандартная, и
//! курсор берётся у самой системы.
//!
//! Растрируем через GDI, а не разбором формата: `.cur` — это ICO с точкой
//! привязки, `.ani` — контейнер RIFF с набором кадров внутри, и оба пришлось бы
//! декодировать вручную. `DrawIconEx` умеет и то и другое, а заодно отдаёт
//! нужный кадр анимации по номеру.
//!
//! Наружу уходит не картинка, а сырые пиксели: кодировать PNG в ядре значило бы
//! тянуть ради этого целую библиотеку, а интерфейсу всё равно рисовать их на
//! холсте.

use serde::Serialize;

/// Растрированный курсор.
#[derive(Serialize)]
pub struct CursorImage {
    pub width: u32,
    pub height: u32,
    /// Точка привязки — та, которой курсор «указывает».
    ///
    /// У стрелки это кончик, у крестика — середина, у руки — палец. Без неё
    /// картинку некуда поставить: выделение тянется именно от этой точки, а не
    /// от угла изображения.
    pub hot_x: u32,
    pub hot_y: u32,
    /// Кадры по порядку, каждый — RGBA без сжатия.
    pub frames: Vec<Vec<u8>>,
    /// Задержка между кадрами, миллисекунды. Для статичного курсора — 0.
    pub delay: u32,
}

/// Отдаёт текущий курсор «стрелка» кадрами.
#[tauri::command]
pub fn cursor_default() -> Result<CursorImage, String> {
    #[cfg(windows)]
    {
        load()
    }

    #[cfg(not(windows))]
    {
        Err("не поддерживаемая операционная система".into())
    }
}

#[cfg(windows)]
fn load() -> Result<CursorImage, String> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetObjectW,
        ReleaseDC, SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        HGDIOBJ,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyCursor, DrawIconEx, GetIconInfo, LoadCursorFromFileW, LoadCursorW, ICONINFO,
        DI_NORMAL, IDC_ARROW,
    };
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    /// Сколько кадров пробуем снять у анимации, прежде чем сдаться.
    const MAX_FRAMES: u32 = 64;

    /// Запасной размер: столько берём, если у курсора не удалось спросить свой.
    const FALLBACK: i32 = 32;

    // Путь к своему файлу курсора. Пусто или ключа нет — схема стандартная.
    let custom = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Control Panel\Cursors")
        .ok()
        .and_then(|key| key.get_value::<String, &str>("Arrow").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // SAFETY: дальше идёт работа с GDI. Каждый созданный объект удаляется в
    // конце функции; ранних возвратов между созданием и удалением нет.
    unsafe {
        let cursor = match &custom {
            Some(path) => {
                // Путь может содержать %SystemRoot% — раскрываем сами:
                // LoadCursorFromFileW переменные окружения не понимает
                let expanded = expand(path);
                let wide: Vec<u16> = std::ffi::OsStr::new(&expanded)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();

                let handle = LoadCursorFromFileW(wide.as_ptr());

                if handle.is_null() {
                    LoadCursorW(std::ptr::null_mut(), IDC_ARROW)
                } else {
                    handle
                }
            }
            None => LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        };

        if cursor.is_null() {
            return Err("не удалось получить курсор".into());
        }

        // Размер и точка привязки — у самого курсора. Рисовать в чужой размер
        // значило бы промахнуться привязкой: она задана в точках оригинала.
        let mut icon: ICONINFO = std::mem::zeroed();
        let has_info = GetIconInfo(cursor, &mut icon) != 0;

        let mut size = FALLBACK;
        let (hot_x, hot_y) = if has_info {
            let mut bitmap: BITMAP = std::mem::zeroed();
            let source = if icon.hbmColor.is_null() {
                icon.hbmMask
            } else {
                icon.hbmColor
            };

            let read = GetObjectW(
                source as HGDIOBJ,
                std::mem::size_of::<BITMAP>() as i32,
                &mut bitmap as *mut _ as *mut core::ffi::c_void,
            );

            if read != 0 && bitmap.bmWidth > 0 {
                size = bitmap.bmWidth;

                // У чёрно-белого курсора маска вдвое выше: сверху прозрачность,
                // снизу сам рисунок
                let height = if icon.hbmColor.is_null() {
                    bitmap.bmHeight / 2
                } else {
                    bitmap.bmHeight
                };

                if height > 0 && height < size {
                    size = height;
                }
            }

            if !icon.hbmColor.is_null() {
                DeleteObject(icon.hbmColor as HGDIOBJ);
            }
            if !icon.hbmMask.is_null() {
                DeleteObject(icon.hbmMask as HGDIOBJ);
            }

            (icon.xHotspot, icon.yHotspot)
        } else {
            (0, 0)
        };

        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);

        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = size;
        // Отрицательная высота — строки сверху вниз, как ожидает холст
        info.bmiHeader.biHeight = -size;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);

        if bitmap.is_null() || bits.is_null() {
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            let _ = DestroyCursor(cursor);
            return Err("не удалось подготовить холст".into());
        }

        let previous = SelectObject(dc, bitmap as HGDIOBJ);

        let pixels = size as usize * size as usize * 4;
        let mut frames: Vec<Vec<u8>> = Vec::new();

        for step in 0..MAX_FRAMES {
            // Холст между кадрами обнуляется: DrawIconEx рисует поверх, и без
            // очистки кадры накладывались бы друг на друга
            std::ptr::write_bytes(bits as *mut u8, 0, pixels);

            let ok = DrawIconEx(
                dc,
                0,
                0,
                cursor,
                size,
                size,
                step,
                std::ptr::null_mut(),
                DI_NORMAL,
            );

            if ok == 0 {
                // Кадры кончились — у статичного курсора это происходит сразу
                // после первого
                break;
            }

            let raw = std::slice::from_raw_parts(bits as *const u8, pixels);
            let frame = to_rgba(raw);

            // Анимация зациклена: как только кадр повторил первый, дальше пойдёт
            // тот же круг
            if frames.first().is_some_and(|first| *first == frame) {
                break;
            }

            frames.push(frame);
        }

        SelectObject(dc, previous);
        DeleteObject(bitmap as HGDIOBJ);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        let _ = DestroyCursor(cursor);

        if frames.is_empty() {
            return Err("курсор не удалось нарисовать".into());
        }

        Ok(CursorImage {
            width: size as u32,
            height: size as u32,
            hot_x,
            hot_y,
            // Частота кадров у анимации хранится в самом файле, до неё через
            // DrawIconEx не добраться. 1/10 секунды — шаг, которым Windows
            // отмеряет анимацию курсора по умолчанию.
            delay: if frames.len() > 1 { 100 } else { 0 },
            frames,
        })
    }
}

/// Переводит пиксели GDI в порядок, понятный холсту.
///
/// GDI хранит их как BGRA с домноженной на прозрачность краской, холст ждёт
/// RGBA с обычной. Без обратного деления полупрозрачные края курсора выглядели
/// бы темнее, чем есть.
#[cfg(windows)]
fn to_rgba(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());

    for chunk in raw.chunks_exact(4) {
        let (b, g, r, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);

        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }

        let unpremultiply = |value: u8| -> u8 {
            let restored = (value as u32 * 255) / a as u32;
            restored.min(255) as u8
        };

        out.extend_from_slice(&[unpremultiply(r), unpremultiply(g), unpremultiply(b), a]);
    }

    out
}

/// Раскрывает переменные окружения в пути вида `%SystemRoot%\cursors\arrow.cur`.
#[cfg(windows)]
fn expand(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;

    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];

        let Some(end) = tail.find('%') else {
            out.push('%');
            rest = tail;
            continue;
        };

        let name = &tail[..end];
        out.push_str(&std::env::var(name).unwrap_or_else(|_| format!("%{name}%")));
        rest = &tail[end + 1..];
    }

    out.push_str(rest);
    out
}
