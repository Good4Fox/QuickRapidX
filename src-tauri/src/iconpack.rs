//! Хранилище значков: один файл вместо тысячи.
//!
//! Первым заходом значки клались отдельными PNG в подпапку. На машине с
//! сотнями программ и шестью размерами это тысячи мелких файлов — а мелкий
//! файл на NTFS дорог не своим содержимым, а записью в каталоге: открытие
//! каждого стоит обращения к файловой системе, и папку потом ещё и чистить.
//!
//! Здесь один файл и указатель в памяти. Устройство простейшее из работающих —
//! дозапись в конец:
//!
//! ```text
//! QRXICON1                 подпись, 8 байт
//! ┌──────────────────────┐
//! │ ключ      8 байт     │  отпечаток пути, размера и времени правки
//! │ длина     4 байта    │
//! │ картинка  N байт     │  готовый PNG
//! └──────────────────────┘  и так далее, до конца файла
//! ```
//!
//! Дозапись в конец выбрана не от простоты. При обновлении программы её значок
//! меняется, ключ меняется вместе с ним, и старая запись остаётся в файле
//! мусором. Переписывать файл целиком ради этого — значит на каждое обновление
//! перекладывать мегабайты; вместо этого мусор копится и вычищается разом,
//! когда его становится больше половины.
//!
//! Указатель строится один раз при первом обращении: файл читается насквозь, и
//! дальше любой значок берётся одним чтением по смещению. Совпал ключ дважды —
//! верна последняя запись, поэтому обход идёт от начала и просто перезаписывает
//! прежнее место.
//!
//! Наружу значки уходят **не строками**, а по своему протоколу `qrxicon`.
//! Строка `data:` в разметке означает килобайты в памяти окна на каждый значок
//! и заново на каждое окно; ссылка же отдаётся движку, а он сам решает, что
//! держать, — у него для этого свой склад, который он же и чистит.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Подпись файла. Меняется вместе с устройством записи.
const MAGIC: &[u8; 8] = b"QRXICON1";

/// Где лежит запись: смещение картинки и её длина.
type Spot = (u64, u32);

struct Store {
    file: PathBuf,
    index: HashMap<u64, Spot>,
    /// Сколько байт занято записями, которые уже перекрыты новыми.
    stale: u64,
}

static STORE: OnceLock<Mutex<Option<Store>>> = OnceLock::new();

fn cell() -> &'static Mutex<Option<Store>> {
    STORE.get_or_init(|| Mutex::new(None))
}

/// Открывает хранилище, читая указатель, если этого ещё не делали.
fn open(root: &Path) -> Result<MutexGuard<'static, Option<Store>>, String> {
    let mut slot = cell().lock().map_err(|_| "хранилище занято".to_string())?;

    if slot.is_some() {
        return Ok(slot);
    }

    let file = root.join("icons.pack");

    let _ = std::fs::create_dir_all(root);

    let mut index = HashMap::new();
    let mut stale = 0u64;

    if let Ok(mut handle) = std::fs::File::open(&file) {
        let mut sign = [0u8; 8];

        // Чужой или испорченный файл не разбираем: заведём заново поверх
        if handle.read_exact(&mut sign).is_ok() && &sign == MAGIC {
            let mut at = MAGIC.len() as u64;

            loop {
                let mut head = [0u8; 12];

                if handle.read_exact(&mut head).is_err() {
                    break;
                }

                let key = u64::from_le_bytes(head[0..8].try_into().unwrap());
                let len = u32::from_le_bytes(head[8..12].try_into().unwrap());

                let body = at + 12;

                if handle.seek(SeekFrom::Start(body + len as u64)).is_err() {
                    break;
                }

                // Тот же ключ второй раз — прежняя запись становится мусором
                if let Some((_, old)) = index.insert(key, (body, len)) {
                    stale += old as u64 + 12;
                }

                at = body + len as u64;
            }
        }
    }

    *slot = Some(Store { file, index, stale });

    Ok(slot)
}

/// Достаёт картинку по ключу.
pub fn get(root: &Path, key: u64) -> Option<Vec<u8>> {
    let slot = open(root).ok()?;
    let store = slot.as_ref()?;

    let (at, len) = *store.index.get(&key)?;

    let mut handle = std::fs::File::open(&store.file).ok()?;

    handle.seek(SeekFrom::Start(at)).ok()?;

    let mut body = vec![0u8; len as usize];

    handle.read_exact(&mut body).ok()?;

    Some(body)
}

/// Кладёт картинку под ключ, дописывая в конец.
pub fn put(root: &Path, key: u64, body: &[u8]) -> Result<(), String> {
    let mut slot = open(root)?;

    let Some(store) = slot.as_mut() else {
        return Err("хранилище не открылось".into());
    };

    let fresh = !store.file.exists();

    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&store.file)
        .map_err(|e| format!("хранилище не открылось: {e}"))?;

    if fresh {
        handle
            .write_all(MAGIC)
            .map_err(|e| format!("подпись не записана: {e}"))?;
    }

    let at = handle
        .seek(SeekFrom::End(0))
        .map_err(|e| format!("конец не найден: {e}"))?;

    let len = body.len() as u32;

    handle
        .write_all(&key.to_le_bytes())
        .and_then(|_| handle.write_all(&len.to_le_bytes()))
        .and_then(|_| handle.write_all(body))
        .map_err(|e| format!("значок не записан: {e}"))?;

    if let Some((_, old)) = store.index.insert(key, (at + 12, len)) {
        store.stale += old as u64 + 12;
    }

    let size = at + 12 + len as u64;

    // Мусора стало больше половины — самое время переложить файл начисто
    if store.stale * 2 > size && size > 1024 * 1024 {
        let _ = compact(store);
    }

    Ok(())
}

/// Перекладывает файл начисто, оставляя только живые записи.
///
/// Через временный файл с переименованием: обрыв посреди перекладки не должен
/// оставить хранилище наполовину переписанным — тогда потерялось бы всё, а не
/// один мусорный кусок.
fn compact(store: &mut Store) -> Result<(), String> {
    let temp = store.file.with_extension("part");

    let mut source =
        std::fs::File::open(&store.file).map_err(|e| format!("хранилище не открылось: {e}"))?;

    let mut out =
        std::fs::File::create(&temp).map_err(|e| format!("временный файл не создан: {e}"))?;

    out.write_all(MAGIC)
        .map_err(|e| format!("подпись не записана: {e}"))?;

    let mut moved: HashMap<u64, Spot> = HashMap::with_capacity(store.index.len());
    let mut at = MAGIC.len() as u64;

    for (key, (from, len)) in store.index.iter() {
        let mut body = vec![0u8; *len as usize];

        if source.seek(SeekFrom::Start(*from)).is_err() || source.read_exact(&mut body).is_err() {
            continue;
        }

        out.write_all(&key.to_le_bytes())
            .and_then(|_| out.write_all(&len.to_le_bytes()))
            .and_then(|_| out.write_all(&body))
            .map_err(|e| format!("значок не переложен: {e}"))?;

        moved.insert(*key, (at + 12, *len));
        at += 12 + *len as u64;
    }

    drop(out);
    drop(source);

    std::fs::rename(&temp, &store.file).map_err(|e| format!("хранилище не заменено: {e}"))?;

    store.index = moved;
    store.stale = 0;

    Ok(())
}

/// Отпечаток пути, размера и времени правки.
///
/// Время обязательно: программу обновляют, значок в ней меняется, а путь
/// остаётся прежним — без него мы бы вечно показывали позавчерашнюю картинку.
///
/// Свёртка своя, FNV-1a: нужен не стойкий хеш, а различимый ключ, и тянуть
/// ради этого зависимость незачем.
pub fn key_of(path: &str, size: u32) -> u64 {
    let stamp = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|gap| gap.as_secs())
        .unwrap_or(0);

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;

    let mut eat = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    };

    eat(path.to_lowercase().as_bytes());
    eat(&size.to_le_bytes());
    eat(&stamp.to_le_bytes());

    hash
}

/// Каталог, в котором лежит хранилище.
pub fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(crate::paths::data_dir(app)?.join("apps"))
}
