//! Корзина Windows: открыть, очистить, размер, лимит и «удалять мимо корзины».
//!
//! Перенесено из `app_basket.rs`. Отличия:
//! * `winapi` заменён на `windows-sys`;
//! * тома корзины перебираются, а не берутся по одному зашитому GUID
//!   `{6ffcef13-...}` — на другой машине такого ключа просто нет, и все
//!   настройки корзины отваливались с ошибкой открытия ключа реестра;
//! * предел и «мимо корзины» задаются диску, который выбрали, — как это и
//!   устроено в самой Windows. Прежде они писались во все записи реестра
//!   разом, а читались из первой попавшейся; записей же там остаётся от
//!   всякой когда-либо подключённой флешки — на этой машине семнадцать при
//!   четырёх дисках, и «первая» означала настройки давно забытого тома.

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

use crate::windows_cmd;

/// Ветка, где Windows держит настройки корзины по томам.
const BITBUCKET: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume";

/// Открывает корзину в проводнике.
#[tauri::command]
pub fn tray_basket_open() -> Result<(), String> {
    windows_cmd::console_command_start("powershell", "start shell:RecycleBinFolder")
}

/// Очищает корзину, спрашивая у оболочки ровно то, что просили.
///
/// Через `SHEmptyRecycleBin`, а не через `Clear-RecycleBin`. Дело не в
/// изяществе: у оболочки ровно три переключателя — подтверждение, звук и окно
/// хода, — и это те самые три, что MiniBin выносит в настройки. Через
/// PowerShell ни один из них недоступен: он умеет либо молча, либо с чужим
/// вопросом в консоли.
///
/// Работа идёт в отдельном потоке: с подтверждением и окном хода вызов
/// возвращается только после того, как человек ответит, а держать на этом
/// поток приложения нельзя.
pub fn empty(confirm: bool, sound: bool, progress: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::SHEmptyRecycleBinW;

        // Флаги здесь запрещающие: единица значит «не делать». Поэтому каждый
        // ставится, когда соответствующая возможность выключена
        const SHERB_NOCONFIRMATION: u32 = 0x1;
        const SHERB_NOPROGRESSUI: u32 = 0x2;
        const SHERB_NOSOUND: u32 = 0x4;

        let mut flags = 0;

        if !confirm {
            flags |= SHERB_NOCONFIRMATION;
        }
        if !progress {
            flags |= SHERB_NOPROGRESSUI;
        }
        if !sound {
            flags |= SHERB_NOSOUND;
        }

        // SAFETY: оба указателя нулевые — это «без окна-владельца» и «по всем
        // дискам», как и задумано.
        let hr = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), flags) };

        // Пустую корзину оболочка считает неожиданностью и отвечает ошибкой.
        // Для нас это не ошибка: просили опустошить — она пуста
        const E_UNEXPECTED: i32 = -2147418113;

        if hr == 0 || hr == 1 || hr == E_UNEXPECTED {
            return Ok(());
        }

        Err(format!("оболочка отказала: код {hr:#x}"))
    }

    #[cfg(not(windows))]
    {
        let _ = (confirm, sound, progress);
        Err("поддерживается только на Windows".into())
    }
}

/// Что сейчас в корзине.
#[derive(serde::Serialize, Clone, Copy, Default)]
pub struct BinState {
    pub bytes: u64,
    pub items: u64,
}

/// Опрашивает корзину по всем дискам.
pub fn state() -> BinState {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

        let mut info = SHQUERYRBINFO {
            cbSize: core::mem::size_of::<SHQUERYRBINFO>() as u32,
            i64Size: 0,
            i64NumItems: 0,
        };

        // SAFETY: структура заполнена, размер выставлен, путь нулевой — «по всем дискам».
        let hr = unsafe { SHQueryRecycleBinW(core::ptr::null(), &mut info) };

        if hr != 0 {
            return BinState::default();
        }

        BinState {
            bytes: info.i64Size as u64,
            items: info.i64NumItems as u64,
        }
    }

    #[cfg(not(windows))]
    BinState::default()
}

#[tauri::command]
pub fn tray_basket_state() -> BinState {
    state()
}

/// Предел корзины в байтах. Ноль — узнать не удалось.
///
/// Windows держит его в мегабайтах и по томам; нам нужен один общий, поэтому
/// складываем. Иначе на машине с двумя дисками полоса наполнения показывала бы
/// вдвое больше, чем есть.
///
/// Складываем по дискам, а не по записям реестра: записей там остаётся от
/// всякой некогда подключённой флешки, и сумма выходила в разы больше правды —
/// на этой машине семнадцать записей при четырёх дисках.
pub fn limit_bytes(_app: &tauri::AppHandle) -> u64 {
    total_limit_mb() as u64 * 1024 * 1024
}

/// Человекочитаемый размер. Нужен подписи значка в трее — там разметки нет.
///
/// Обозначения приходят доводом, а не зашиты здесь: подсказку значка человек
/// читает на своём языке, а прежние «Б/КБ/ГБ» оставались русскими на всех.
pub fn human(bytes: u64, units: &[String]) -> String {
    if units.is_empty() {
        return bytes.to_string();
    }

    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", units[0])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

/// Настройки корзины для интерфейса.
#[tauri::command]
pub fn bin_settings(app: tauri::AppHandle) -> crate::paths::Bin {
    crate::paths::load_or_create_config(&app)
        .map(|config| config.bin)
        .unwrap_or_default()
}

/// Сохраняет настройки и тут же приводит значок в трее в соответствие.
#[tauri::command]
pub fn bin_settings_save(app: tauri::AppHandle, bin: crate::paths::Bin) -> Result<(), String> {
    let mut config = crate::paths::load_or_create_config(&app)?;

    config.bin = bin;
    crate::paths::write_config(&app, &config)?;

    crate::bin_tray::refresh(&app);

    Ok(())
}

/// Значки корзины, какими их видит оболочка. Нужны интерфейсу для показа
/// того, что стоит по умолчанию.
#[tauri::command]
pub fn bin_shell_icons() -> Vec<String> {
    let (empty, full) = shell_icons();

    vec![empty, full]
}

/// Очищает корзину так, как настроено, — тем же путём, что и значок в трее.
#[tauri::command]
pub async fn bin_empty(app: tauri::AppHandle) -> Result<(), String> {
    let bin = bin_settings(app);

    tauri::async_runtime::spawn_blocking(move || empty(bin.confirm, bin.sound, bin.progress))
        .await
        .map_err(|e| format!("очистка сорвалась: {e}"))?
}

/// Значки корзины, которыми пользуется сама оболочка.
///
/// Берём их, а не рисуем свои: на рабочем столе корзина выглядит именно так, и
/// значок в трее должен совпадать с ней — включая случай, когда человек уже
/// подменил его сам.
///
/// Порядок веток не случаен: пользовательская подмена лежит в `HKCU` и
/// перекрывает общую, поэтому спрашиваем сначала её.
pub fn shell_icons() -> (String, String) {
    const CLSID: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon";
    const CLASSES: &str =
        r"CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon";

    let roots: [(winreg::HKEY, &str); 2] = [
        (HKEY_CURRENT_USER, CLSID),
        (winreg::enums::HKEY_CLASSES_ROOT, CLASSES),
    ];

    for (root, path) in roots {
        let Ok(key) = RegKey::predef(root).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };

        // Имена значений оболочка пишет то так, то так — и в разных ветках
        // по-разному. Спрашиваем оба написания, иначе на одной из машин
        // значок молча оказался бы нашим запасным
        let empty = key
            .get_value::<String, _>("empty")
            .or_else(|_| key.get_value::<String, _>("Empty"));

        let full = key
            .get_value::<String, _>("full")
            .or_else(|_| key.get_value::<String, _>("Full"));

        if let (Ok(empty), Ok(full)) = (empty, full) {
            return (expand(&empty), expand(&full));
        }
    }

    // Запасной вариант — те же номера ресурсов, что стоят у системы по
    // умолчанию. Пусто здесь означало бы значок приложения вместо корзины
    let root = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());

    (
        format!(r"{root}\System32\imageres.dll,-55"),
        format!(r"{root}\System32\imageres.dll,-54"),
    )
}

/// Разворачивает `%SystemRoot%` и прочие переменные в записи реестра.
fn expand(value: &str) -> String {
    let mut out = value.to_string();

    for name in ["SystemRoot", "WINDIR", "ProgramFiles", "LOCALAPPDATA"] {
        if let Ok(actual) = std::env::var(name) {
            out = out.replace(&format!("%{name}%"), &actual);
        }
    }

    out
}

/// Общий предел всех корзин в мегабайтах — для полосы наполнения в трее.
///
/// Полоса показывает корзину целиком, по всем дискам, и предел ей нужен такой
/// же общий: занятость `SHQueryRecycleBinW` считает по всем томам сразу.
#[tauri::command]
pub fn tray_basket_get_max_capacity() -> u32 {
    total_limit_mb()
}

/// Сумма пределов по всем дискам, в мегабайтах.
///
/// Отдельно от `bin_volumes`, потому что зовётся по таймеру: значку в трее и
/// полосе в панели нужен только предел, а опрашивать ради него корзину каждого
/// тома — работа впустую.
fn total_limit_mb() -> u32 {
    drive_roots()
        .iter()
        .filter_map(|root| volume_key(root, false).ok())
        .filter_map(|key| key.get_value::<u32, _>("MaxCapacity").ok())
        .sum()
}

/* --- Диски и их корзины ------------------------------------------------- */

/// Диск и то, как настроена его корзина.
///
/// Корзина в Windows не одна: у каждого тома своя папка `$Recycle.Bin`, и
/// предел с «мимо корзины» задаются тому, а не системе. Перенести её на
/// соседний диск нельзя — удалённое остаётся на том же томе, где лежало; иначе
/// удаление стало бы копированием через полдиска, а «восстановить в исходное
/// место» перестало бы что-то значить. Выбирается не место корзины, а диск,
/// чью корзину настраивают.
#[derive(serde::Serialize, Clone, Default)]
pub struct BinVolume {
    /// Корень тома: `C:\`.
    pub root: String,
    /// Метка тома — как диск подписан в проводнике.
    pub label: String,
    /// Размер диска в байтах.
    pub total: u64,
    /// Свободно на диске, байты.
    pub free: u64,
    /// Предел корзины в мегабайтах. Ноль — своего предела у тома нет.
    pub limit: u32,
    /// Удалять мимо корзины.
    pub nuke: bool,
    /// Занято корзиной этого тома, байты.
    pub used: u64,
    /// Сколько в ней лежит.
    pub items: u64,
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Имя тома в реестре корзины: `{6ffcef13-…}`.
///
/// Подключи в `BitBucket\Volume` названы GUID тома, а не буквой, и это не
/// прихоть: буква меняется от перетыкания диска, GUID — нет. Спрашиваем его у
/// системы, иначе пришлось бы гадать, какому диску какая запись, — а записей
/// там куда больше, чем дисков: остаются от всего, что когда-либо
/// подключалось.
#[cfg(windows)]
fn volume_guid(root: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;

    let mut name = [0u16; 64];

    // SAFETY: корень завершён нулём и живёт до конца вызова, буфер свой.
    let ok = unsafe {
        GetVolumeNameForVolumeMountPointW(wide(root).as_ptr(), name.as_mut_ptr(), name.len() as u32)
    };

    if ok == 0 {
        return None;
    }

    let length = name.iter().position(|c| *c == 0).unwrap_or(0);

    braces(&String::from_utf16_lossy(&name[..length]))
}

/// `\\?\Volume{…}\` → `{…}` — реестру нужна только скобочная середина.
fn braces(volume: &str) -> Option<String> {
    let start = volume.find('{')?;
    let end = volume.find('}')?;

    if end < start {
        return None;
    }

    Some(volume[start..=end].to_string())
}

/// Корни дисков, у которых бывает корзина.
///
/// Сетевые диски и приводы отсеиваются: корзины на них нет вовсе, а в списке
/// они выглядели бы настраиваемыми.
#[cfg(windows)]
fn drive_roots() -> Vec<String> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;

    // SAFETY: вызов без аргументов, отдаёт битовую маску занятых букв.
    let mask = unsafe { GetLogicalDrives() };

    (0..26u32)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| format!("{}:\\", (b'A' + bit as u8) as char))
        .filter(|root| {
            // SAFETY: строка завершена нулём и живёт до конца вызова.
            let kind = unsafe { GetDriveTypeW(wide(root).as_ptr()) };

            kind == DRIVE_FIXED || kind == DRIVE_REMOVABLE
        })
        .collect()
}

/// Всего и свободно на томе. Нули, если спросить не вышло.
#[cfg(windows)]
fn space(root: &str) -> (u64, u64) {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let path = wide(root);
    let mut free = 0u64;
    let mut total = 0u64;

    // SAFETY: строка завершена нулём, остальные указатели — на наши переменные.
    let ok =
        unsafe { GetDiskFreeSpaceExW(path.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) };

    if ok == 0 {
        (0, 0)
    } else {
        (total, free)
    }
}

/// Метка тома — то, как диск подписан в проводнике.
#[cfg(windows)]
fn label(root: &str) -> String {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    let path = wide(root);
    let mut name = [0u16; 261];

    // SAFETY: буфер своей длины и есть, остальные поля нам не нужны.
    let ok = unsafe {
        GetVolumeInformationW(
            path.as_ptr(),
            name.as_mut_ptr(),
            name.len() as u32,
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

    let length = name.iter().position(|c| *c == 0).unwrap_or(0);
    String::from_utf16_lossy(&name[..length])
}

/// Что лежит в корзине одного тома.
#[cfg(windows)]
fn volume_state(root: &str) -> BinState {
    use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

    let mut info = SHQUERYRBINFO {
        cbSize: core::mem::size_of::<SHQUERYRBINFO>() as u32,
        i64Size: 0,
        i64NumItems: 0,
    };

    let path = wide(root);

    // SAFETY: структура заполнена, размер выставлен, путь завершён нулём.
    let hr = unsafe { SHQueryRecycleBinW(path.as_ptr(), &mut info) };

    if hr != 0 {
        return BinState::default();
    }

    BinState {
        bytes: info.i64Size as u64,
        items: info.i64NumItems as u64,
    }
}

#[cfg(not(windows))]
fn volume_guid(_root: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
fn drive_roots() -> Vec<String> {
    Vec::new()
}

#[cfg(not(windows))]
fn space(_root: &str) -> (u64, u64) {
    (0, 0)
}

#[cfg(not(windows))]
fn label(_root: &str) -> String {
    String::new()
}

#[cfg(not(windows))]
fn volume_state(_root: &str) -> BinState {
    BinState::default()
}

/// Ключ настроек тома.
///
/// На запись ключ создаётся: у диска, с которого ещё ничего не удаляли, записи
/// нет, но задать ему предел заранее — законное желание, и отказ «нет такого
/// ключа» человеку ничего не объяснил бы.
fn volume_key(root: &str, write: bool) -> Result<RegKey, String> {
    let guid =
        volume_guid(root).ok_or_else(|| format!("не удалось узнать том диска {root}"))?;

    let path = format!(r"{BITBUCKET}\{guid}");
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    if write {
        hkcu.create_subkey(&path)
            .map(|(key, _)| key)
            .map_err(|e| format!("не удалось открыть настройки диска {root}: {e}"))
    } else {
        hkcu.open_subkey_with_flags(&path, KEY_READ)
            .map_err(|e| format!("нет настроек корзины для диска {root}: {e}"))
    }
}

/// Диски и то, как настроена корзина у каждого.
///
/// Читаем по дискам, а не по записям реестра: записей там вдесятеро больше —
/// они остаются от всякой некогда подключённой флешки, и «первая попавшаяся»
/// оказывалась настройками давно забытого тома.
#[tauri::command]
pub fn bin_volumes() -> Vec<BinVolume> {
    drive_roots()
        .into_iter()
        .map(|root| {
            let (total, free) = space(&root);
            let state = volume_state(&root);
            let key = volume_key(&root, false).ok();

            BinVolume {
                label: label(&root),
                total,
                free,
                limit: key
                    .as_ref()
                    .and_then(|key| key.get_value::<u32, _>("MaxCapacity").ok())
                    .unwrap_or(0),
                nuke: key
                    .as_ref()
                    .and_then(|key| key.get_value::<u32, _>("NukeOnDelete").ok())
                    .unwrap_or(0)
                    != 0,
                used: state.bytes,
                items: state.items,
                root,
            }
        })
        .collect()
}

/// Предел корзины одного диска, в мегабайтах.
#[tauri::command]
pub fn bin_volume_limit(root: String, megabytes: u32) -> Result<(), String> {
    if megabytes == 0 {
        return Err("предел должен быть больше нуля".into());
    }

    volume_key(&root, true)?
        .set_value("MaxCapacity", &megabytes)
        .map_err(|e| format!("не удалось записать предел для диска {root}: {e}"))
}

/// «Удалять мимо корзины» для одного диска.
#[tauri::command]
pub fn bin_volume_nuke(root: String, on: bool) -> Result<(), String> {
    let value: u32 = u32::from(on);

    volume_key(&root, true)?
        .set_value("NukeOnDelete", &value)
        .map_err(|e| format!("не удалось записать режим удаления для диска {root}: {e}"))
}

/// Сколько сейчас занято в корзине, в байтах.
#[tauri::command]
pub fn tray_basket_get_current_usage() -> Result<u64, String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

        let mut info = SHQUERYRBINFO {
            cbSize: core::mem::size_of::<SHQUERYRBINFO>() as u32,
            i64Size: 0,
            i64NumItems: 0,
        };

        // null вместо пути означает «по всем дискам».
        // SAFETY: структура заполнена, размер выставлен, указатель валиден.
        let hr = unsafe { SHQueryRecycleBinW(core::ptr::null(), &mut info) };

        if hr != 0 {
            return Err(format!("не удалось опросить корзину: код {hr}"));
        }

        Ok(info.i64Size as u64)
    }

    #[cfg(not(windows))]
    Err("поддерживается только на Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Имя тома система отдаёт с приставкой и хвостовой чертой, а реестр знает
    /// только скобки. Ошибись здесь на символ — и настройки уйдут в новый ключ,
    /// который Windows никогда не прочитает.
    #[test]
    fn guid_taken_out_of_volume_name() {
        assert_eq!(
            braces(r"\\?\Volume{6ffcef13-4d28-44af-991e-251460b61a48}\").as_deref(),
            Some("{6ffcef13-4d28-44af-991e-251460b61a48}")
        );
    }

    #[test]
    fn nothing_taken_from_a_name_without_braces() {
        assert!(braces(r"\\?\Volume").is_none());
        assert!(braces("").is_none());
        assert!(braces("}{").is_none());
    }

    /// Диски перечисляются с корнем — `GetVolumeNameForVolumeMountPointW`
    /// требует именно его и отказывает, если передать «C:» без черты.
    #[test]
    #[cfg(windows)]
    fn drives_come_with_a_root_slash() {
        for root in drive_roots() {
            assert!(root.ends_with(":\\"), "корень без черты: {root}");
        }
    }
}
