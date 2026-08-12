//! Значки программ.
//!
//! Список программ без значков читается плохо: глаз ищет знакомую картинку, а
//! не строку. Значок лежит внутри самого исполняемого файла как ресурс, и
//! достать его можно только средствами Windows.
//!
//! Сначала пробуем `ExtractIconExW` — она берёт значок прямо из файла. Если не
//! вышло (у файла его нет, или путь ведёт на ярлык или папку), спрашиваем
//! оболочку через `SHGetFileInfoW`: та вернёт хотя бы тот значок, которым
//! Windows рисует этот файл в проводнике.
//!
//! Наружу уходит готовая картинка ссылкой `data:`, а не сырые пиксели.
//!
//! Сырые пришлось бы сериализовать массивом чисел: значок 56×56 — это 12 544
//! байта, в JSON около пятидесяти килобайт, и на списке в несколько сотен
//! программ выходят десятки мегабайт. Собрать их в ядре и разобрать в окне
//! стоит секунд, и всё это время окно не отвечает на нажатия. PNG того же
//! значка — пара килобайт, и рисовать его на холсте уже не нужно.

/// Достаёт значок файла готовой ссылкой `data:image/png;base64,...`.
#[tauri::command]
pub async fn icon_extract(path: String, size: Option<u32>) -> Result<String, String> {
    // Ограничение сверху: значок в файле редко бывает крупнее, дальше пойдёт
    // растягивание, а данных прибавится вчетверо
    let size = size.unwrap_or(48).clamp(16, 256) as i32;

    tauri::async_runtime::spawn_blocking(move || {
        let image = load(&path, size)?;

        encode(&image.pixels, image.width)
    })
    .await
    .map_err(|e| format!("значок не получен: {e}"))?
}

/*
 * Ключ значка в общем хранилище.
 *
 * Наружу уходит не картинка, а ключ: по нему окно строит ссылку своего
 * протокола, и дальше картинку тянет уже движок. Строка `data:` означала бы
 * килобайты в памяти окна на каждый значок и заново на каждое окно.
 */

/// Отдаёт ключ значка, добывая его при первой надобности.
#[tauri::command]
pub async fn icon_key(
    app: tauri::AppHandle,
    path: String,
    size: Option<u32>,
) -> Result<String, String> {
    let size = size.unwrap_or(48).clamp(16, 256);
    let root = crate::iconpack::root(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let key = crate::iconpack::key_of(&path, size);

        // Уже лежит — обращаться к оболочке незачем, это самое медленное здесь
        if crate::iconpack::get(&root, key).is_some() {
            return Ok(format!("{key:016x}"));
        }

        let image = load(&path, size as i32)?;
        let body = pack(&image.pixels, image.width)?;

        crate::iconpack::put(&root, key, &body)?;

        Ok(format!("{key:016x}"))
    })
    .await
    .map_err(|e| format!("значок не получен: {e}"))?
}

/// Упаковывает пиксели в PNG.
fn pack(rgba: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let mut raw = Vec::new();

    {
        let mut encoder = png::Encoder::new(&mut raw, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("значок не упакован: {e}"))?;

        writer
            .write_image_data(rgba)
            .map_err(|e| format!("значок не упакован: {e}"))?;
    }

    Ok(raw)
}

/// Значок в виде пикселей RGBA и его сторона.
///
/// Нужен трею: иконке в системной панели картинка отдаётся сырой, а не
/// упакованной, — упаковывать её в PNG, чтобы система тут же распаковала, было
/// бы работой ради работы.
pub fn raster(path: &str, size: u32) -> Option<(Vec<u8>, u32)> {
    let image = load(path, size.clamp(16, 256) as i32).ok()?;

    Some((image.pixels, image.width))
}

/// Растрированный значок. Наружу не уходит — только в кодировщик.
///
/// Значок всегда квадратный, поэтому сторона одна.
struct IconImage {
    width: u32,
    /// RGBA без сжатия.
    pixels: Vec<u8>,
}

/// Кладёт значок файла рядом, картинкой PNG.
///
/// Нужен перетаскиванию: под курсором во время переноса показывается картинка,
/// и берётся она файлом, а не строкой.
pub fn write_png(source: &str, size: u32, dest: &std::path::Path) -> Result<(), String> {
    let image = load(source, size.clamp(16, 256) as i32)?;
    let mut raw = Vec::new();

    {
        let mut encoder = png::Encoder::new(&mut raw, image.width, image.width);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("значок не упакован: {e}"))?;

        writer
            .write_image_data(&image.pixels)
            .map_err(|e| format!("значок не упакован: {e}"))?;
    }

    std::fs::write(dest, raw).map_err(|e| format!("значок не записан: {e}"))
}

/// Упаковывает пиксели в PNG и оборачивает ссылкой.
fn encode(rgba: &[u8], size: u32) -> Result<String, String> {
    use base64::Engine;

    let mut raw = Vec::new();

    {
        let mut encoder = png::Encoder::new(&mut raw, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("значок не упакован: {e}"))?;

        writer
            .write_image_data(rgba)
            .map_err(|e| format!("значок не упакован: {e}"))?;
    }

    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&raw)
    ))
}

/// Делит запись значка на файл и номер внутри него.
///
/// Windows пишет их через запятую — `imageres.dll,-55`, — и именно так оболочка
/// хранит собственные значки корзины: в реестре у неё не файл, а файл с
/// номером. Раньше мы отдавали всю строку как путь, и такой значок не
/// находился вовсе.
///
/// Запятая бывает и в имени файла, поэтому хвост считается номером только
/// тогда, когда он целое число, а то, что перед ним, — существующий файл.
/// Отрицательный номер система понимает как опознавательный номер ресурса, а
/// не как порядковый, — на этом и держится вся запись оболочки.
fn split_index(path: &str) -> (String, i32) {
    let Some(at) = path.rfind(',') else {
        return (path.to_string(), 0);
    };

    let (head, tail) = path.split_at(at);

    let Ok(index) = tail[1..].trim().parse::<i32>() else {
        return (path.to_string(), 0);
    };

    let file = head.trim();

    if !std::path::Path::new(file).is_file() {
        return (path.to_string(), 0);
    }

    (file.to_string(), index)
}

#[cfg(windows)]
fn load(path: &str, size: i32) -> Result<IconImage, String> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows_sys::Win32::UI::Shell::{ExtractIconExW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL};

    let path = path.trim();

    if path.is_empty() {
        return Err("путь не указан".into());
    }

    let (file, index) = split_index(path);

    let wide: Vec<u16> = std::ffi::OsStr::new(&file)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: дальше работа с GDI и оболочкой. Значок и объекты рисования
    // освобождаются в конце; ранних возвратов между созданием и освобождением
    // нет.
    unsafe {
        // Значок из самого файла — он точнее, чем общий значок типа файла
        let mut large = std::ptr::null_mut();
        let taken = ExtractIconExW(wide.as_ptr(), index, &mut large, std::ptr::null_mut(), 1);

        let icon = if taken > 0 && !large.is_null() {
            large
        } else {
            // Ярлык, папка или файл без своего значка — спрашиваем оболочку
            let mut info: SHFILEINFOW = std::mem::zeroed();

            let ok = SHGetFileInfoW(
                wide.as_ptr(),
                0,
                &mut info,
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );

            if ok == 0 || info.hIcon.is_null() {
                return Err("у файла нет значка".into());
            }

            info.hIcon
        };

        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);

        let mut header: BITMAPINFO = std::mem::zeroed();
        header.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        header.bmiHeader.biWidth = size;
        // Отрицательная высота — строки сверху вниз, как ожидает холст
        header.bmiHeader.biHeight = -size;
        header.bmiHeader.biPlanes = 1;
        header.bmiHeader.biBitCount = 32;
        header.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &header, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);

        if bitmap.is_null() || bits.is_null() {
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            DestroyIcon(icon);
            return Err("не удалось подготовить холст".into());
        }

        let previous = SelectObject(dc, bitmap as HGDIOBJ);
        let count = size as usize * size as usize * 4;

        std::ptr::write_bytes(bits as *mut u8, 0, count);

        let drawn = DrawIconEx(
            dc,
            0,
            0,
            icon,
            size,
            size,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        );

        let pixels = if drawn == 0 {
            Vec::new()
        } else {
            to_rgba(std::slice::from_raw_parts(bits as *const u8, count))
        };

        SelectObject(dc, previous);
        DeleteObject(bitmap as HGDIOBJ);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        DestroyIcon(icon);

        if pixels.is_empty() {
            return Err("значок не удалось нарисовать".into());
        }

        Ok(IconImage {
            width: size as u32,
            pixels,
        })
    }
}

#[cfg(not(windows))]
fn load(_path: &str, _size: i32) -> Result<IconImage, String> {
    Err("не поддерживаемая операционная система".into())
}

/// Переводит пиксели GDI в порядок, понятный холсту.
///
/// GDI держит их как BGRA с домноженной на прозрачность краской, холст ждёт
/// RGBA с обычной. Без обратного деления полупрозрачные края значка выглядели
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

        let restore = |value: u8| ((value as u32 * 255) / a as u32).min(255) as u8;

        out.extend_from_slice(&[restore(r), restore(g), restore(b), a]);
    }

    out
}
