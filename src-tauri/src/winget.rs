//! Витрина winget.
//!
//! Машиночитаемого вывода у `winget search` нет — только таблица, и подписи
//! колонок в ней переведены: «Имя / ИД / Версия / Совпадение / Источник».
//! Разбирать её по двум и более пробелам нельзя: длинное значение вплотную
//! подходит к соседней колонке, и строка распадается то на пять полей, то на
//! четыре — проверено на живом выводе.
//!
//! Поэтому колонки берутся по их положению в строке заголовка: где начинается
//! очередная непробельная последовательность, там и граница. Сами подписи при
//! этом не читаются, так что язык системы роли не играет. Ширины winget
//! подбирает под содержимое, поэтому положения считаются заново на каждый
//! вызов, а не запоминаются.

use std::process::Command;

use serde::Serialize;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Пакет в выдаче.
#[derive(Serialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
}

/// Есть ли winget и какой версии.
#[tauri::command]
pub fn winget_version() -> Result<String, String> {
    let output = run(&["--version"])?;

    Ok(output.trim().to_string())
}

/// Ищет пакеты по строке.
#[tauri::command]
pub async fn winget_search(query: String) -> Result<Vec<Package>, String> {
    let query = query.trim().to_string();

    if query.is_empty() {
        return Ok(Vec::new());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let output = run(&[
            "search",
            &query,
            "--disable-interactivity",
            "--accept-source-agreements",
        ])?;

        Ok(parse_table(&output))
    })
    .await
    .map_err(|e| format!("поиск не завершён: {e}"))?
}

/// Что из установленного знает winget — по нему видно, что можно обновить.
#[tauri::command]
pub async fn winget_installed() -> Result<Vec<Package>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let output = run(&["list", "--disable-interactivity", "--accept-source-agreements"])?;

        Ok(parse_table(&output))
    })
    .await
    .map_err(|e| format!("список не получен: {e}"))?
}

/// Подробности пакета.
#[derive(Serialize, Default)]
pub struct Details {
    pub publisher: String,
    pub publisher_url: String,
    pub homepage: String,
    /// Откуда качается установщик. Главный проверяемый признак: в winget
    /// встречаются копии, выложенные посторонними, и отличает их именно
    /// источник загрузки, а не название пакета.
    pub installer_url: String,
    pub license: String,
}

/// Спрашивает подробности у winget.
///
/// Вывод — список «подпись: значение», и подписи переведены. Поэтому значения
/// берутся не по подписи, а по виду: ссылка — то, что начинается с http.
/// Издателя приходится брать по порядку строк, других зацепок нет.
#[tauri::command]
pub async fn winget_show(id: String) -> Result<Details, String> {
    let id = id.trim().to_string();

    if id.is_empty() {
        return Err("не указан пакет".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let output = run(&[
            "show",
            "--id",
            &id,
            "--exact",
            "--disable-interactivity",
            "--accept-source-agreements",
        ])?;

        Ok(parse_details(&output))
    })
    .await
    .map_err(|e| format!("сведения не получены: {e}"))?
}

fn parse_details(output: &str) -> Details {
    let mut details = Details::default();
    let mut urls: Vec<String> = Vec::new();
    // Сведения об установщике winget печатает с отступом — по нему и отличаем
    // ссылку на установщик от ссылок на сайт, лицензию и политику
    let mut installer_urls: Vec<String> = Vec::new();

    for line in output.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };

        let value = value.trim();

        if value.is_empty() {
            continue;
        }

        // Ссылка узнаётся по себе, а не по переведённой подписи
        if value.starts_with("http://") || value.starts_with("https://") {
            // Подпись обрезана по первому двоеточию, поэтому у ссылки в
            // значении осталась только часть после «https» — собираем заново
            let full = line.split_once(": ").map(|(_, v)| v.trim()).unwrap_or(value);

            if line.starts_with(' ') || line.starts_with('\t') {
                installer_urls.push(full.to_string());
            } else {
                urls.push(full.to_string());
            }

            continue;
        }

        let label = label.trim().to_lowercase();

        if details.publisher.is_empty() && (label.contains("издател") || label.contains("publisher")) {
            details.publisher = value.to_string();
        }

        if details.license.is_empty() && (label.contains("лиценз") || label.contains("license")) {
            details.license = value.to_string();
        }
    }

    // Первые две ссылки без отступа — сайт издателя и домашняя страница;
    // дальше идут лицензия и политика, они нам не нужны
    details.publisher_url = urls.first().cloned().unwrap_or_default();
    details.homepage = urls.get(1).cloned().unwrap_or_default();
    details.installer_url = installer_urls.first().cloned().unwrap_or_default();

    details
}

/// Ставит пакет.
///
/// `location` — папка внутри Места. winget передаёт её установщику ключом
/// `--location`, но послушают его не все: у одних установщиков папка назначения
/// вообще не настраивается, у других настраивается только через свои ключи.
/// Портативные пакеты (`portable`) кладутся туда всегда — они и есть то, что
/// переносится потом целиком.
///
/// Проверить заранее, послушается ли установщик, нельзя: это выясняется только
/// по факту. Поэтому после установки интерфейс смотрит, появилась ли папка.
#[tauri::command]
pub async fn winget_install(id: String, location: Option<String>) -> Result<String, String> {
    package_command(id, "install", location).await
}

/// Обновляет пакет.
#[tauri::command]
pub async fn winget_upgrade(id: String) -> Result<String, String> {
    package_command(id, "upgrade", None).await
}

async fn package_command(
    id: String,
    verb: &'static str,
    location: Option<String>,
) -> Result<String, String> {
    let id = id.trim().to_string();

    if id.is_empty() {
        return Err("не указан пакет".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let mut args: Vec<String> = vec![
            verb.to_string(),
            "--id".into(),
            id,
            "--exact".into(),
            "--silent".into(),
            "--disable-interactivity".into(),
            "--accept-package-agreements".into(),
            "--accept-source-agreements".into(),
        ];

        if let Some(path) = location.filter(|value| !value.trim().is_empty()) {
            args.push("--location".into());
            args.push(path);
            // Установка для пользователя не требует прав администратора и не
            // лезет в Program Files — для установки в своё Место это то, что
            // нужно. Пакет без такой области winget поставит как умеет.
            args.push("--scope".into());
            args.push("user".into());
        }

        let borrowed: Vec<&str> = args.iter().map(|value| value.as_str()).collect();
        run(&borrowed)
    })
    .await
    .map_err(|e| format!("установка не завершена: {e}"))?
}

/// Записывает набор установленного в файл.
///
/// Это готовый способ «сохранить состав и восстановить на чистой системе»:
/// формат у winget свой, машиночитаемый, и его же понимает `import`.
#[tauri::command]
pub async fn winget_export(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run(&[
            "export",
            "--output",
            &path,
            "--accept-source-agreements",
            "--disable-interactivity",
        ])
    })
    .await
    .map_err(|e| format!("выгрузка не завершена: {e}"))?
}

/// Ставит всё из файла набора.
#[tauri::command]
pub async fn winget_import(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run(&[
            "import",
            "--import-file",
            &path,
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
            // Уже стоящее и отсутствующее в источниках не должно обрывать
            // восстановление на середине
            "--ignore-unavailable",
            "--ignore-versions",
        ])
    })
    .await
    .map_err(|e| format!("восстановление не завершено: {e}"))?
}

// ── Внутреннее ───────────────────────────────────────────────────────────

#[cfg(windows)]
fn run(args: &[&str]) -> Result<String, String> {
    let output = Command::new("winget")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("winget недоступен: {e}"))?;

    // Кодировка вывода зависит от языка системы. Читаем как есть: имена могут
    // пострадать, но идентификаторы, версии и положения колонок — только ASCII
    let text = String::from_utf8_lossy(&output.stdout).to_string();

    if output.status.success() {
        return Ok(text);
    }

    let code = output.status.code().unwrap_or(-1);
    let tail = text.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("");

    Err(format!("winget завершился с кодом {code}. {tail}"))
}

#[cfg(not(windows))]
fn run(_args: &[&str]) -> Result<String, String> {
    Err("не поддерживаемая операционная система".into())
}

/// Разбирает таблицу winget по положениям колонок из её заголовка.
fn parse_table(output: &str) -> Vec<Package> {
    let lines: Vec<&str> = output.lines().collect();

    // Заголовок — строка прямо перед линией из дефисов
    let Some(divider) = lines
        .iter()
        .position(|line| line.trim_start().starts_with("---") && line.trim().len() > 10)
    else {
        return Vec::new();
    };

    if divider == 0 {
        return Vec::new();
    }

    let starts = column_starts(lines[divider - 1]);

    // Меньше трёх колонок — это не та таблица: нужны имя, идентификатор, версия
    if starts.len() < 3 {
        return Vec::new();
    }

    let mut packages = Vec::new();

    for line in lines.iter().skip(divider + 1) {
        if line.trim().is_empty() {
            continue;
        }

        let cells = slice_columns(line, &starts);

        let id = cells.get(1).cloned().unwrap_or_default();

        // Идентификатор пакета не содержит пробелов. Строки вроде «найдено 20
        // пакетов» этой проверки не проходят и отсеиваются сами
        if id.is_empty() || id.contains(' ') {
            continue;
        }

        packages.push(Package {
            name: cells.first().cloned().unwrap_or_default(),
            id,
            version: cells.get(2).cloned().unwrap_or_default(),
            source: cells.last().cloned().unwrap_or_default(),
        });
    }

    packages
}

/// Где начинается каждая колонка — по строке заголовка.
fn column_starts(header: &str) -> Vec<usize> {
    let chars: Vec<char> = header.chars().collect();
    let mut starts = Vec::new();
    let mut inside = false;

    for (index, symbol) in chars.iter().enumerate() {
        if symbol.is_whitespace() {
            inside = false;
            continue;
        }

        if !inside {
            starts.push(index);
            inside = true;
        }
    }

    starts
}

/// Режет строку по границам колонок.
fn slice_columns(line: &str, starts: &[usize]) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut cells = Vec::with_capacity(starts.len());

    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(chars.len());

        if *start >= chars.len() {
            cells.push(String::new());
            continue;
        }

        let piece: String = chars[*start..end.min(chars.len())].iter().collect();
        cells.push(piece.trim().to_string());
    }

    cells
}
