//! Наборы ярлыков.
//!
//! Смысл тот же, что у AppGroup: собрать несколько программ в один набор и
//! открывать их оттуда, не разыскивая по рабочему столу и меню «Пуск». Набор
//! «Работа», набор «Игры», набор «Записать видео» — что в него положат, то в
//! нём и будет.
//!
//! Запуск идёт через оболочку (`ShellExecuteW`), а не через создание процесса
//! напрямую. Это принципиально: в набор кладут не только `exe`, но и ярлыки
//! `lnk`, папки и ссылки, а понимает их всех только оболочка. Она же знает, что
//! делать с файлом, у которого нет своей программы.
//!
//! Хранится всё отдельным файлом рядом с настройками: набор — это не настройка,
//! а данные, которые человек собирает сам, и терять их вместе со сброшенным
//! `config.json` было бы обидно.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

/// Одна запись внутри набора.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shortcut {
    pub id: String,
    pub name: String,
    /// Что открывать: программа, ярлык, папка или ссылка.
    pub path: String,
    /// Ключи командной строки.
    #[serde(default)]
    pub args: String,
    /// Запускать от имени администратора.
    #[serde(default)]
    pub admin: bool,
    /// Запускать в нашей изоляции вместо обычного пуска.
    #[serde(default)]
    pub isolated: bool,
    /// Своя подсказка при наведении. Пусто — показывается путь.
    #[serde(default)]
    pub tip: String,
    /// Свой значок записи. Пусто — берётся из самого файла.
    ///
    /// Нужен там, где своего значка нет: у папок, у ссылок на страницы, у
    /// сценариев. Без него на их месте выводятся две буквы названия, и ряд
    /// значков рассыпается — глаз ищет картинку, а находит текст.
    #[serde(default)]
    pub icon: String,
    /// Вложенный набор: идентификатор другого набора вместо программы.
    ///
    /// Так набор «Работа» держит внутри «Разработку» и «Переписку», а панель
    /// заходит в них по нажатию. Программа и вложение — взаимоисключающие: у
    /// вложения нет пути, у программы нет этого поля.
    #[serde(default)]
    pub child: String,
}

/// Набор.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    /// Файл, у которого берётся значок набора. Пусто — значок соберётся из
    /// значков первых записей.
    #[serde(default)]
    pub icon: String,
    /// Своя иконка в системном трее, рядом с основной.
    #[serde(default = "yes")]
    pub tray: bool,
    /// Показывать в общей панели у основного значка.
    ///
    /// Отдельно от `tray` намеренно: значков в трее бывает много, и держать
    /// там все наборы захочет не каждый — а в общем списке место есть всегда.
    #[serde(default = "yes")]
    pub panel: bool,
    /// Показывать шапку в окне набора: значок, название и крестик.
    #[serde(default = "yes")]
    pub head: bool,
    /// Подписывать программы в окне набора.
    #[serde(default = "yes")]
    pub labels: bool,
    /// Ставить программы столбцом, а не строкой.
    ///
    /// По умолчанию строкой: панель задач чаще всего внизу, и окно набора
    /// растёт от неё вбок, не заслоняя экран. Но она бывает и сбоку, и тогда
    /// столбец ложится по ней ровнее.
    #[serde(default)]
    pub vertical: bool,
    #[serde(default)]
    pub items: Vec<Shortcut>,
}

fn yes() -> bool {
    true
}

/// Чем кончился запуск.
#[derive(Serialize)]
pub struct Started {
    pub ok: bool,
    /// Сколько записей удалось открыть.
    pub count: usize,
    pub message: String,
}

/// Все наборы по порядку.
#[tauri::command]
pub fn group_list(app: AppHandle) -> Vec<Group> {
    read(&app)
}

/// Наборы, которым положена своя иконка в трее.
pub fn for_tray(app: &AppHandle) -> Vec<Group> {
    read(app).into_iter().filter(|group| group.tray).collect()
}

/// Один набор целиком — панель трея спрашивает его по нажатию на значок.
#[tauri::command]
pub fn group_one(app: AppHandle, id: String) -> Option<Group> {
    read(&app).into_iter().find(|group| group.id == id)
}

/// Что нужно, чтобы вытащить набор из окна перетаскиванием.
#[derive(Serialize)]
pub struct Draggable {
    /// Сам файл ярлыка — его и переносят.
    pub file: String,
    /// Картинка под курсором во время переноса.
    pub icon: String,
}

/// Готовит ярлык набора и картинку к нему, ничего не открывая.
///
/// Нужен перетаскиванию: чтобы вытащить набор на панель задач, файл должен
/// существовать до начала переноса — переносят ведь именно его.
#[tauri::command]
pub fn group_shortcut_file(app: AppHandle, id: String) -> Result<Draggable, String> {
    #[cfg(windows)]
    {
        let Some(group) = read(&app).into_iter().find(|group| group.id == id) else {
            return Err("такого набора нет".into());
        };

        files_for(&app, &group, false)
    }

    #[cfg(not(windows))]
    {
        let _ = (app, id);
        Err("не поддерживаемая операционная система".into())
    }
}

/// Готовит ярлык и картинку набора.
///
/// `force` — переписать, даже если файлы на месте. Без него готовое отдаётся
/// как есть: запись ярлыка идёт через оболочку, это сотни миллисекунд, а
/// перетаскивание начинается в тот же миг, когда нажали кнопку мыши, — ждать
/// там нечего.
#[cfg(windows)]
fn files_for(app: &AppHandle, group: &Group, force: bool) -> Result<Draggable, String> {
    let folder = crate::paths::data_dir(app)?.join("shortcuts");

    fs::create_dir_all(&folder)
        .map_err(|e| format!("не удалось создать {}: {e}", folder.display()))?;

    let base = safe_name(&group.name);
    let lnk = folder.join(format!("{base}.lnk"));
    let png = folder.join(format!("{base}.png"));

    if force || !lnk.is_file() {
        write_shortcut(&lnk, group)?;
    }

    // Картинка не обязательна: без неё перенос идёт с курсором по умолчанию, и
    // это лучше, чем отказ из-за значка
    if force || !png.is_file() {
        let _ = crate::icons::write_png(&icon_source(group), 64, &png);
    }

    Ok(Draggable {
        file: lnk.display().to_string(),
        icon: if png.is_file() {
            png.display().to_string()
        } else {
            String::new()
        },
    })
}

/// Переписывает ярлыки всех наборов.
///
/// Вызывается вместе с обновлением значков в трее — то есть тогда, когда
/// что-то из показываемого изменилось. К моменту, когда за плитку возьмутся
/// мышью, файл уже готов.
#[cfg(windows)]
pub fn rebuild_shortcuts(app: &AppHandle) {
    let app = app.clone();

    std::thread::spawn(move || {
        for group in read(&app) {
            let _ = files_for(&app, &group, true);
        }
    });
}

#[cfg(not(windows))]
pub fn rebuild_shortcuts(_app: &AppHandle) {}

/// Делает копию набора.
#[tauri::command]
pub fn group_duplicate(app: AppHandle, id: String, suffix: String) -> Result<String, String> {
    let mut groups = read(&app);

    let Some(source) = groups.iter().find(|group| group.id == id).cloned() else {
        return Err("такого набора нет".into());
    };

    // Новые опознавательные номера у самого набора и у каждой записи: иначе
    // копия и original указывали бы на одни и те же настройки изоляции
    let copy = Group {
        id: fresh_id(),
        // Приставка приходит из словаря: имя копии ложится в настройки и в
        // подсказку значка трея, и остаётся там навсегда
        name: format!("{} {}", source.name, suffix.trim()),
        icon: source.icon.clone(),
        tray: false,
        panel: source.panel,
        head: source.head,
        labels: source.labels,
        vertical: source.vertical,
        items: source
            .items
            .iter()
            .map(|item| Shortcut {
                id: fresh_id(),
                ..item.clone()
            })
            .collect(),
    };

    let id = copy.id.clone();

    groups.push(copy);
    write(&app, &groups)?;
    crate::tray::refresh(&app);

    Ok(id)
}

/// Новый опознавательный номер.
///
/// Случайности здесь не нужно: достаточно, чтобы номера не повторялись. Берём
/// время с точностью до наносекунды и счётчик — двух одинаковых не выйдет даже
/// в цикле копирования.
fn fresh_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);

    format!("{stamp:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Готовит ярлык набора и открывает проводник с ним выделенным.
///
/// Закрепить на панели задач своими силами нельзя. Windows закрыла глагол
/// `taskbarpin` для программ ещё в десятке; обходные пути через
/// `User Pinned\TaskBar` и реестр `Taskband` ломаются от обновления к
/// обновлению и в одиннадцатой версии не работают. Проверено по исходникам
/// AppGroup: там тоже нет ни закрепления, ни попытки его подделать — их
/// `TaskbarManager` только правит уже закреплённые ярлыки, а руководство
/// предлагает перетащить значок или закрепить его из проводника.
///
/// Из двух честных путей выбран второй, он короче: мы делаем ярлык и открываем
/// папку с ним выделенным. Остаётся один правый клик — «Закрепить на панели
/// задач». Перетаскивать и разыскивать файл не нужно.
///
/// Ярлык ведёт на нас с ключом `--group=<набор>`: запуск открывает панель с
/// этим набором и больше ничего не делает.
#[tauri::command]
pub fn group_pin(app: AppHandle, id: String) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let Some(group) = read(&app).into_iter().find(|group| group.id == id) else {
            return Err("такого набора нет".into());
        };

        // Своя папка, а не рабочий стол: ярлыков наберётся по числу наборов, и
        // засорять ими стол ради одного закрепления не за что
        let folder = crate::paths::data_dir(&app)?.join("shortcuts");

        fs::create_dir_all(&folder)
            .map_err(|e| format!("не удалось создать {}: {e}", folder.display()))?;

        let lnk = folder.join(format!("{}.lnk", safe_name(&group.name)));

        write_shortcut(&lnk, &group)?;

        // Проводник с выделенным файлом: дальше остаётся правый клик
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", lnk.display()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();

        Ok(lnk.display().to_string())
    }

    #[cfg(not(windows))]
    {
        let _ = (app, id);
        Err("не поддерживаемая операционная система".into())
    }
}

/// Подтягивает уже закреплённые ярлыки под нынешний вид наборов.
///
/// Это единственное, что делает с панелью задач AppGroup, и делает
/// справедливо: закреплённый ярлык — обычный файл в
/// `User Pinned\TaskBar`, и переименованный набор или смена значка иначе
/// оставили бы на панели прежнюю картинку до конца времён.
///
/// Правятся только значок и подпись. Переименовывать сам файл не станем: имя
/// закреплённого ярлыка — то, за что держится панель задач, и подменять его
/// ради косметики значит рисковать чужим закреплением.
///
/// Один вызов PowerShell на всё разом, а не на файл: список наборов уезжает
/// временным файлом в JSON, чтобы ничего не пришлось вклеивать в текст команды.
/// Запускает подтягивание в стороне от главного потока.
///
/// Внутри — вызов PowerShell с ожиданием, полсекунды с лишним. На старте
/// приложения это ровно столько же задержки перед появлением окна, а работа
/// сама по себе не срочная: закреплённый ярлык подождёт.
#[cfg(windows)]
pub fn sync_pinned(app: &AppHandle) {
    let app = app.clone();

    std::thread::spawn(move || sync_pinned_now(&app));
}

#[cfg(windows)]
fn sync_pinned_now(app: &AppHandle) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let groups = read(app);

    if groups.is_empty() {
        return;
    }

    let map: Vec<(String, String, String)> = groups
        .iter()
        .map(|group| (group.id.clone(), group.name.clone(), icon_source(group)))
        .collect();

    let Ok(body) = serde_json::to_string(&map) else {
        return;
    };

    let Ok(dir) = crate::paths::data_dir(app) else {
        return;
    };

    let list = dir.join("pinned.tmp.json");

    if fs::write(&list, body).is_err() {
        return;
    }

    let script = format!(
        r#"
$dir = Join-Path $env:APPDATA 'Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar'
if (!(Test-Path $dir)) {{ exit }}
$map = @{{}}
foreach ($row in (Get-Content -Raw -LiteralPath '{list}' | ConvertFrom-Json)) {{ $map[$row[0]] = $row }}
$sh = New-Object -ComObject WScript.Shell
foreach ($file in Get-ChildItem -LiteralPath $dir -Filter *.lnk) {{
  $s = $sh.CreateShortcut($file.FullName)
  if ($s.Arguments -match '--group=(\S+)') {{
    $row = $map[$Matches[1]]
    if ($row) {{
      $s.Description = $row[1]
      if ($row[2]) {{ $s.IconLocation = $row[2] + ',0' }}
      $s.Save()
    }}
  }}
}}
"#,
        list = list.display().to_string().replace('\'', "''"),
    );

    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status();

    let _ = fs::remove_file(&list);

    // Просим проводник перечитать значки: свой кэш он иначе держит долго
    // SAFETY: обращение без параметров-указателей на наши данные.
    unsafe {
        use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

#[cfg(not(windows))]
pub fn sync_pinned(_app: &AppHandle) {}

/// Закрепляет набор в меню «Пуск».
///
/// В отличие от панели задач, это система разрешает: у ярлыка есть действие
/// «Закрепить в меню „Пуск“», и его можно вызвать. Проверено перечислением
/// действий у настоящего файла — там есть закрепление в «Пуске» и нет
/// закрепления на панели задач.
///
/// Название действия переведено, поэтому ищем по частям слов сразу на трёх
/// языках. Не нашли — честно говорим об этом, а не делаем вид, что получилось.
#[tauri::command]
pub fn group_pin_start(app: AppHandle, id: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let Some(group) = read(&app).into_iter().find(|group| group.id == id) else {
            return Err("такого набора нет".into());
        };

        let bundle = files_for(&app, &group, true)?;
        let file = std::path::PathBuf::from(&bundle.file);

        let folder = file
            .parent()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();

        let name = file
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();

        let quote = |value: &str| value.replace('\'', "''");

        // Слова подобраны так, чтобы попасть в «Закрепить в меню „Пуск“»,
        // «Pin to Start» и «スタートにピン留めする» и не задеть соседние
        // действия вроде «Закрепить на панели быстрого доступа»
        let script = format!(
            "$sh = New-Object -ComObject Shell.Application; \
             $item = $sh.Namespace('{folder}').ParseName('{name}'); \
             if (-not $item) {{ exit 2 }} \
             $verb = $item.Verbs() | Where-Object {{ \
               $n = $_.Name -replace '&',''; \
               $n -like '*меню*Пуск*' -or $n -like '*to Start*' -or $n -like '*スタート*' }} | \
               Select-Object -First 1; \
             if (-not $verb) {{ exit 3 }} \
             $verb.DoIt()",
            folder = quote(&folder),
            name = quote(&name),
        );

        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("не удалось обратиться к оболочке: {e}"))?;

        match status.code() {
            Some(0) => Ok(()),
            Some(3) => Err("система не предлагает закрепление в «Пуске» для этого файла".into()),
            _ => Err("закрепить не удалось".into()),
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (app, id);
        Err("не поддерживаемая операционная система".into())
    }
}

/// Имя файла из названия набора.
fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|symbol| {
            if symbol.is_alphanumeric() || symbol == ' ' || symbol == '-' || symbol == '_' {
                symbol
            } else {
                '_'
            }
        })
        .take(64)
        .collect();

    let trimmed = cleaned.trim().to_string();

    if trimmed.is_empty() {
        "Набор".to_string()
    } else {
        trimmed
    }
}

/// Значок, которым подписывается ярлык набора.
#[cfg(windows)]
fn icon_source(group: &Group) -> String {
    if !group.icon.trim().is_empty() {
        return group.icon.clone();
    }

    group
        .items
        .first()
        .map(|item| item.path.clone())
        .unwrap_or_else(|| {
            std::env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        })
}

/// Пишет ярлык через WScript.Shell.
///
/// Через оболочку, а не COM напрямую: тот же результат тремя строками вместо
/// ручной работы с IShellLink и IPersistFile.
#[cfg(windows)]
fn write_shortcut(lnk: &Path, group: &Group) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let me = std::env::current_exe()
        .map_err(|e| format!("свой путь не определён: {e}"))?
        .display()
        .to_string();

    // Одинарные кавычки внутри PowerShell удваиваются — иначе строка оборвётся
    // посреди пути и остаток уйдёт в команду
    let quote = |value: &str| value.replace('\'', "''");

    let script = format!(
        // WindowStyle 7 — «свёрнутым и без переключения на него». В отладочной
        // сборке приложение консольное, и без этого запуск с ярлыка выкидывал
        // бы на экран чёрное окно
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); \
         $s.TargetPath = '{}'; $s.Arguments = '{}{}'; $s.IconLocation = '{},0'; \
         $s.WindowStyle = 7; $s.Description = '{}'; $s.Save()",
        quote(&lnk.display().to_string()),
        quote(&me),
        crate::tray::GROUP_FLAG,
        quote(&group.id),
        quote(&icon_source(group)),
        quote(&group.name),
    );

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("ярлык не создан: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("ярлык не создан".into())
    }
}

/// Создаёт набор или заменяет существующий.
///
/// Порядок записей внутри — тот, что пришёл: интерфейс распоряжается им сам.
#[tauri::command]
pub fn group_save(app: AppHandle, group: Group) -> Result<(), String> {
    if group.name.trim().is_empty() {
        return Err("у набора нет названия".into());
    }

    let mut groups = read(&app);

    match groups.iter_mut().find(|item| item.id == group.id) {
        Some(existing) => *existing = group,
        None => groups.push(group),
    }

    write(&app, &groups)?;
    crate::tray::refresh(&app);

    Ok(())
}

/// Убирает набор.
#[tauri::command]
pub fn group_remove(app: AppHandle, id: String) -> Result<(), String> {
    let mut groups = read(&app);

    groups.retain(|group| group.id != id);

    write(&app, &groups)?;
    crate::tray::refresh(&app);

    Ok(())
}

/// Меняет порядок наборов.
#[tauri::command]
pub fn group_reorder(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    let groups = read(&app);

    // Пришедший порядок — главный, но потерять при этом ничего нельзя:
    // неупомянутое дописывается в конец как было
    let mut sorted: Vec<Group> = ids
        .iter()
        .filter_map(|id| groups.iter().find(|group| &group.id == id).cloned())
        .collect();

    for group in groups {
        if !sorted.iter().any(|item| item.id == group.id) {
            sorted.push(group);
        }
    }

    write(&app, &sorted)?;
    crate::tray::refresh(&app);

    Ok(())
}

/// Открывает одну запись набора.
#[tauri::command]
pub fn group_launch(app: AppHandle, group: String, item: String) -> Started {
    let Some(shortcut) = find(&app, &group, &item) else {
        return Started {
            ok: false,
            count: 0,
            message: "такой записи нет".into(),
        };
    };

    match start(&app, &shortcut) {
        Ok(()) => Started {
            ok: true,
            count: 1,
            message: String::new(),
        },
        Err(message) => Started {
            ok: false,
            count: 0,
            message,
        },
    }
}

/// Открывает весь набор разом.
///
/// Осечка на одной записи не отменяет остальные: набор из десяти программ не
/// должен молчать целиком из-за одной переставленной.
#[tauri::command]
pub fn group_launch_all(app: AppHandle, id: String) -> Started {
    let Some(group) = read(&app).into_iter().find(|group| group.id == id) else {
        return Started {
            ok: false,
            count: 0,
            message: "такого набора нет".into(),
        };
    };

    let mut count = 0;
    let mut failed: Vec<String> = Vec::new();

    for shortcut in &group.items {
        // Вложенные наборы «Открыть всё» не разворачивает: за одним нажатием
        // должно стоять понятное число программ, а не всё дерево целиком
        if !shortcut.child.trim().is_empty() {
            continue;
        }

        match start(&app, shortcut) {
            Ok(()) => count += 1,
            Err(_) => failed.push(shortcut.name.clone()),
        }
    }

    Started {
        ok: failed.is_empty(),
        count,
        message: if failed.is_empty() {
            String::new()
        } else {
            format!("не открылось: {}", failed.join(", "))
        },
    }
}

// ── Внутреннее ───────────────────────────────────────────────────────────

fn path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(crate::paths::data_dir(app)?.join("groups.json"))
}

fn read(app: &AppHandle) -> Vec<Group> {
    let Ok(file) = path(app) else {
        return Vec::new();
    };

    let Ok(raw) = fs::read_to_string(file) else {
        return Vec::new();
    };

    serde_json::from_str(&raw).unwrap_or_default()
}

/// Пишет через временный файл.
///
/// Прямая запись поверх оставляет обрезанный файл, если её прервать на
/// середине, — а вместе с ним пропадают все наборы разом.
fn write(app: &AppHandle, groups: &[Group]) -> Result<(), String> {
    let file = path(app)?;
    let temp = file.with_extension("json.tmp");

    let body =
        serde_json::to_string_pretty(groups).map_err(|e| format!("наборы не собраны: {e}"))?;

    fs::write(&temp, body).map_err(|e| format!("наборы не записаны: {e}"))?;

    fs::rename(&temp, &file).map_err(|e| format!("наборы не сохранены: {e}"))
}

fn find(app: &AppHandle, group: &str, item: &str) -> Option<Shortcut> {
    read(app)
        .into_iter()
        .find(|each| each.id == group)?
        .items
        .into_iter()
        .find(|each| each.id == item)
}

/// Открывает запись.
fn start(app: &AppHandle, shortcut: &Shortcut) -> Result<(), String> {
    // Вложенный набор — не программа: открывать его нечем, в него заходят
    if !shortcut.child.trim().is_empty() {
        return Err("это вложенный набор".into());
    }

    let target = shortcut.path.trim();

    if target.is_empty() {
        return Err("не указано, что открывать".into());
    }

    if shortcut.isolated {
        // Изоляция — про запуск программы, а не про открытие папки или ссылки:
        // ей нужен файл, который она поместит в контейнер.
        //
        // Своих прав набор не хранит. Если для этой же программы права уже
        // заведены в каталоге, берём их: «в изоляции» должно означать одно и
        // то же, откуда бы её ни запустили. Нет записи — запуск без прав, то
        // есть самый строгий.
        /*
         * Права ищутся сначала по самой записи набора, потом по программе.
         *
         * У записи они свои: одна и та же программа в двух наборах может быть
         * заведена по-разному — рабочий браузер с сетью, а «почитать» без неё.
         * Если у записи ничего не задано, берутся права из каталога для этого
         * же файла: заводить их дважды не нужно.
         */
        let profile = crate::container::profile_named(app, &shortcut.id)
            .or_else(|| crate::container::profile_for_exe(app, target));

        let launched = crate::container::container_launch(
            app.clone(),
            target.to_string(),
            profile.as_ref().map(|it| it.capabilities.clone()).unwrap_or_default(),
            profile.as_ref().map(|it| it.folders.clone()).unwrap_or_default(),
            Some(shortcut.args.clone()),
            profile.as_ref().map(|it| it.env.clone()),
            profile.as_ref().map(|it| it.engine.clone()),
            profile.as_ref().map(|it| it.sandbox.clone()),
        );

        return if launched.ok {
            Ok(())
        } else {
            Err(launched.message)
        };
    }

    open(target, &shortcut.args, shortcut.admin)
}

#[cfg(windows)]
fn open(target: &str, args: &str, admin: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let file = wide(target);
    let args = args.trim();
    let parameters = wide(args);

    // Рабочая папка — та, где лежит программа: без неё многие ищут свои файлы
    // рядом с нами, а не рядом с собой
    let folder = Path::new(target)
        .parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default();
    let directory = wide(&folder);

    // «runas» — та самая просьба о правах, на которую Windows покажет запрос.
    // Обычный запуск идёт без глагола: оболочка сама выберет, что делать с
    // ярлыком, папкой или ссылкой
    let verb = wide("runas");

    // SAFETY: все строки завершены нулём и живут до конца вызова.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            if admin {
                verb.as_ptr()
            } else {
                std::ptr::null()
            },
            file.as_ptr(),
            if args.is_empty() {
                std::ptr::null()
            } else {
                parameters.as_ptr()
            },
            if folder.is_empty() {
                std::ptr::null()
            } else {
                directory.as_ptr()
            },
            SW_SHOWNORMAL,
        )
    };

    // Оболочка отвечает числом; всё, что не больше 32, — код ошибки
    let code = result as isize;

    if code > 32 {
        return Ok(());
    }

    Err(match code {
        2 => "файла нет на месте".to_string(),
        3 => "папки нет на месте".to_string(),
        5 => "нет доступа".to_string(),
        // 1223 — человек отказался дать права в запросе Windows
        1223 => "запуск от администратора отменён".to_string(),
        other => format!("оболочка отказала: ошибка {other}"),
    })
}

#[cfg(not(windows))]
fn open(_target: &str, _args: &str, _admin: bool) -> Result<(), String> {
    Err("не поддерживаемая операционная система".into())
}
