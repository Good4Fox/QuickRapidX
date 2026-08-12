//! Хранилище пользовательских записей о программах.
//!
//! Перенесено из `app_json.rs` прошлой версии: тот же файл `app_data.json`,
//! та же схема из пяти списков, те же имена команд и параметров.
//!
//! Отличия:
//! * файл лежит в `app_data_dir()/apps`, а не по относительному пути
//!   `home/save/app/` — раньше он оказывался рядом с текущим рабочим
//!   каталогом, то есть в разном месте в зависимости от способа запуска;
//! * чтение и запись собраны в двух функциях вместо повторения в каждой
//!   команде;
//! * `editor_person_in_json` больше не затирает `id` номером строки
//!   (`person.id = linene_id.to_string()`) — идентификатор записи сохраняется.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

use crate::paths;

/// Одна запись о программе.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub site: String,
    pub download_plus: String,
    pub download: String,
    pub programm_ico: String,
    pub programm_name: String,
    pub programm_description: String,
}

/// Содержимое `app_data.json`: пять списков по категориям.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    pub app_dops: Vec<Person>,
    #[serde(default)]
    pub app_games_dops: Vec<Person>,
    #[serde(default)]
    pub app_game_launcher_dops: Vec<Person>,
    #[serde(default)]
    pub app_dev_dops: Vec<Person>,
    #[serde(default)]
    pub app_ai_dops: Vec<Person>,
}

impl Data {
    /// Имена списков в том же порядке, в котором их ждёт интерфейс.
    const LISTS: [&'static str; 5] = [
        "app_dops",
        "app_games_dops",
        "app_game_launcher_dops",
        "app_dev_dops",
        "app_ai_dops",
    ];

    /// Список по имени категории.
    fn list_mut(&mut self, name: &str) -> Result<&mut Vec<Person>, String> {
        match name {
            "app_dops" => Ok(&mut self.app_dops),
            "app_games_dops" => Ok(&mut self.app_games_dops),
            "app_game_launcher_dops" => Ok(&mut self.app_game_launcher_dops),
            "app_dev_dops" => Ok(&mut self.app_dev_dops),
            "app_ai_dops" => Ok(&mut self.app_ai_dops),
            other => Err(format!("неизвестная категория: {other}")),
        }
    }

    fn list(&self, name: &str) -> Result<&Vec<Person>, String> {
        match name {
            "app_dops" => Ok(&self.app_dops),
            "app_games_dops" => Ok(&self.app_games_dops),
            "app_game_launcher_dops" => Ok(&self.app_game_launcher_dops),
            "app_dev_dops" => Ok(&self.app_dev_dops),
            "app_ai_dops" => Ok(&self.app_ai_dops),
            other => Err(format!("неизвестная категория: {other}")),
        }
    }

    /// Все пять списков в том же порядке, в котором их ждёт интерфейс.
    fn as_arrays(&self) -> Result<Vec<Value>, String> {
        [
            serde_json::to_value(&self.app_dops),
            serde_json::to_value(&self.app_games_dops),
            serde_json::to_value(&self.app_game_launcher_dops),
            serde_json::to_value(&self.app_dev_dops),
            serde_json::to_value(&self.app_ai_dops),
        ]
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("не удалось собрать JSON: {e}"))
    }
}

fn file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(paths::data_dir(app)?.join("apps").join("app_data.json"))
}

/// Читает хранилище. Отсутствующий или повреждённый файл даёт пустые списки.
fn load(app: &AppHandle) -> Result<Data, String> {
    let path = file_path(app)?;

    let Ok(raw) = fs::read_to_string(&path) else {
        return Ok(Data::default());
    };

    match serde_json::from_str(&raw) {
        Ok(data) => Ok(data),
        Err(err) => {
            // Битый файл не должен ронять вкладку: уводим его в сторону,
            // чтобы данные можно было достать руками, и начинаем с пустого.
            let backup = path.with_extension("broken.json");
            let _ = fs::rename(&path, &backup);
            eprintln!("app_data.json повреждён ({err}), сохранён как {}", backup.display());
            Ok(Data::default())
        }
    }
}

/// Пишет хранилище, создавая каталог при необходимости.
fn save(app: &AppHandle, data: &Data) -> Result<(), String> {
    let path = file_path(app)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("не удалось создать {}: {e}", parent.display()))?;
    }

    let body = serde_json::to_string_pretty(data).map_err(|e| format!("не удалось собрать JSON: {e}"))?;

    // Пишем рядом и переименовываем: запись поверх живого файла рвала бы все
    // пять списков разом, если бы оборвалась на середине
    let temp = path.with_extension("json.tmp");

    fs::write(&temp, body).map_err(|e| format!("не удалось записать app_data.json: {e}"))?;

    fs::rename(&temp, &path).map_err(|e| format!("не удалось заменить app_data.json: {e}"))
}

/// Переносит каталог из файла прежней версии.
///
/// У старого приложения он лежал рядом с исполняемым файлом — в
/// `home/save/app/app_data.json`, — и при переезде на каталог данных остался
/// там. Схема та же, поэтому достаточно прочитать и слить.
///
/// Совпадения по названию пропускаются: перенос можно повторить, не получив
/// каталог из двойных записей.
#[tauri::command]
pub fn import_catalog(app: AppHandle, path: String) -> Result<usize, String> {
    let raw = fs::read_to_string(&path).map_err(|e| format!("не удалось прочитать {path}: {e}"))?;

    let incoming: Data =
        serde_json::from_str(&raw).map_err(|e| format!("файл не похож на каталог: {e}"))?;

    let mut data = load(&app)?;
    let mut added = 0;

    for name in Data::LISTS {
        let source = incoming.list(name)?.clone();
        let target = data.list_mut(name)?;

        let known: std::collections::HashSet<String> = target
            .iter()
            .map(|person| person.programm_name.to_lowercase())
            .collect();

        for person in source {
            if known.contains(&person.programm_name.to_lowercase()) {
                continue;
            }

            target.push(person);
            added += 1;
        }
    }

    save(&app, &data)?;

    Ok(added)
}

/// Отдаёт все пять списков одной строкой JSON.
#[tauri::command]
pub fn json_data_info(app: AppHandle) -> Result<String, String> {
    let data = load(&app)?;

    serde_json::to_string_pretty(&data.as_arrays()?)
        .map_err(|e| format!("не удалось собрать JSON: {e}"))
}

/// Сохраняет запись: добавляет новую или заменяет существующую.
///
/// Адресация по `id`, а не по номеру строки. Прежние `editor_person_in_json` и
/// `remove_person_in_json` брали позицию в массиве — и это работало, пока
/// список показывался целиком в исходном порядке. С поиском и сортировкой
/// позиция на экране перестаёт совпадать с позицией в файле, и правка попадает
/// не в ту запись.
#[tauri::command]
pub fn catalog_save(app: AppHandle, category: String, entry: Person) -> Result<(), String> {
    if entry.programm_name.trim().is_empty() {
        return Err("у записи нет названия".into());
    }

    let mut data = load(&app)?;
    let list = data.list_mut(&category)?;

    match list.iter_mut().find(|person| person.id == entry.id) {
        Some(existing) => *existing = entry,
        None => list.push(entry),
    }

    save(&app, &data)
}

/// Убирает запись по идентификатору.
#[tauri::command]
pub fn catalog_remove(app: AppHandle, category: String, id: String) -> Result<(), String> {
    let mut data = load(&app)?;
    let list = data.list_mut(&category)?;

    let before = list.len();
    list.retain(|person| person.id != id);

    if list.len() == before {
        return Err("такой записи нет".into());
    }

    save(&app, &data)
}

/// Переносит запись в другую категорию.
#[tauri::command]
pub fn catalog_move(app: AppHandle, from: String, to: String, id: String) -> Result<(), String> {
    if from == to {
        return Ok(());
    }

    let mut data = load(&app)?;

    let source = data.list_mut(&from)?;
    let Some(index) = source.iter().position(|person| person.id == id) else {
        return Err("такой записи нет".into());
    };

    let person = source.remove(index);
    data.list_mut(&to)?.push(person);

    save(&app, &data)
}

/// Добавляет запись в категорию.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn create_json_data_info(
    app: AppHandle,
    linene_id: Option<&str>,
    linenenew_site: Option<&str>,
    linenenew_download_plus: Option<&str>,
    linenenew_download: Option<&str>,
    linenenew_programm_ico: Option<&str>,
    linenenew_programm_name: Option<&str>,
    linenenew_programm_description: Option<&str>,
    linenjname: Option<&str>,
) -> Result<String, String> {
    // Незаполненная форма — не ошибка, просто ничего не делаем.
    let (
        Some(id),
        Some(site),
        Some(download_plus),
        Some(download),
        Some(programm_ico),
        Some(programm_name),
        Some(programm_description),
        Some(category),
    ) = (
        linene_id,
        linenenew_site,
        linenenew_download_plus,
        linenenew_download,
        linenenew_programm_ico,
        linenenew_programm_name,
        linenenew_programm_description,
        linenjname,
    )
    else {
        return Ok(String::new());
    };

    let mut data = load(&app)?;

    data.list_mut(category)?.push(Person {
        id: id.to_string(),
        site: site.to_string(),
        download_plus: download_plus.to_string(),
        download: download.to_string(),
        programm_ico: programm_ico.to_string(),
        programm_name: programm_name.to_string(),
        programm_description: programm_description.to_string(),
    });

    save(&app, &data)?;
    Ok("New person added".to_string())
}

/// Изменяет запись по номеру строки в категории.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn editor_person_in_json(
    app: AppHandle,
    linene_id: i32,
    linenenew_site: &str,
    linenenew_download_plus: &str,
    linenenew_download: &str,
    linenenew_programm_ico: &str,
    linenenew_programm_name: &str,
    linenenew_programm_description: &str,
    linenjname: &str,
) -> Result<String, String> {
    let mut data = load(&app)?;
    let list = data.list_mut(linenjname)?;

    let Some(person) = list.get_mut(linene_id as usize) else {
        return Err(format!("запись с номером {linene_id} не найдена"));
    };

    // `id` намеренно не трогаем: раньше сюда писался номер строки, и
    // идентификатор записи терялся при первом же редактировании.
    person.site = linenenew_site.to_string();
    person.download_plus = linenenew_download_plus.to_string();
    person.download = linenenew_download.to_string();
    person.programm_ico = linenenew_programm_ico.to_string();
    person.programm_name = linenenew_programm_name.to_string();
    person.programm_description = linenenew_programm_description.to_string();

    save(&app, &data)?;
    Ok("Person updated successfully".to_string())
}

/// Удаляет запись по номеру строки в категории.
#[tauri::command]
pub fn remove_person_in_json(
    app: AppHandle,
    linenumber: usize,
    structure_name: &str,
) -> Result<String, String> {
    let mut data = load(&app)?;
    let list = data.list_mut(structure_name)?;

    if linenumber >= list.len() {
        return Err(format!("неверный номер записи: {linenumber}"));
    }

    list.remove(linenumber);
    save(&app, &data)?;

    Ok(format!("Success: Removed person at index {linenumber}"))
}

/// Ищет записи в категории по одному из полей.
#[tauri::command]
pub fn search_person_in_json(
    app: AppHandle,
    search_value: &str,
    linenjname: &str,
    search_field: Option<&str>,
) -> Result<String, String> {
    let data = load(&app)?;
    let list = data.list(linenjname)?;

    let field = search_field.unwrap_or("programm_name");
    let needle = search_value.to_lowercase();

    let found: Vec<&Person> = list
        .iter()
        .filter(|person| {
            let value = match field {
                "id" => &person.id,
                "site" => &person.site,
                "download_plus" => &person.download_plus,
                "download" => &person.download,
                "programm_ico" => &person.programm_ico,
                "programm_name" => &person.programm_name,
                "programm_description" => &person.programm_description,
                _ => return false,
            };

            value.to_lowercase().contains(&needle)
        })
        .collect();

    if found.is_empty() {
        return Err(format!("по запросу «{search_value}» в {linenjname} ничего не найдено"));
    }

    serde_json::to_string_pretty(&found).map_err(|e| format!("не удалось собрать JSON: {e}"))
}
