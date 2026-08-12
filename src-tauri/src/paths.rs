//! Рабочие каталоги и файл настроек.
//!
//! Всё живёт в каталоге данных приложения, который выдаёт сама Tauri
//! (`%APPDATA%\QuickRapidX` на Windows). Относительных путей здесь нет
//! намеренно: они создавали бы папки рядом с текущим рабочим каталогом,
//! то есть в случайном месте — зависит от того, откуда запустили exe.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Подкаталоги, которые должны существовать к моменту старта интерфейса.
const SUBDIRS: [&str; 5] = ["apps", "apps/icons", "automation", "logs", "snapshots"];

/// Положение и размер окна между запусками.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Переменная окружения, выдаваемая программе при запуске.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

/// Настройки изоляции одной программы.
///
/// Каждой нужно своё: браузеру интернет, просмотрщику — библиотека
/// изображений, большинству не нужно ничего. Общий набор на всех означал бы
/// либо запрет того, что программе необходимо, либо разрешение того, что ей ни
/// к чему.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Isolation {
    /// К чему относится: идентификатор записи каталога или путь к файлу.
    pub target: String,
    /// Выбранная роль. Хранится, чтобы интерфейс показал, что отмечено; на
    /// запуск влияют только `capabilities` — роль лишь заполняет их.
    #[serde(default)]
    pub preset: String,
    /// Чем изолировать: `own` — наша среда на AppContainer, `sandboxie` —
    /// установленная Sandboxie. Пусто равно `own`.
    #[serde(default)]
    pub engine: String,
    /// Имя песочницы Sandboxie. Пусто — её `DefaultBox`.
    #[serde(default)]
    pub sandbox: String,
    /// Выданные возможности — по именам из container::CAPABILITIES.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Папки, открытые программе дополнительно к её собственной.
    #[serde(default)]
    pub folders: Vec<String>,
    /// Что запускать. У записи каталога своего пути к файлу нет — он
    /// указывается здесь один раз и дальше работает кнопкой в списке.
    #[serde(default)]
    pub exe: String,
    /// Ключи командной строки.
    #[serde(default)]
    pub args: String,
    /// Добавка к окружению процесса.
    #[serde(default)]
    pub env: Vec<EnvVar>,
}

/// Настройки корзины: свой значок в трее и его поведение.
///
/// Собрано по образцу MiniBin — той самой мелочи, ради которой её и ставят:
/// корзина видна в трее, меняет вид по наполнению и очищается одним нажатием.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bin {
    /// Показывать отдельный значок корзины в трее.
    #[serde(default)]
    pub tray: bool,
    /// Пять ступеней наполнения вместо двух «пусто/полно».
    #[serde(default = "yes")]
    pub steps: bool,
    /// Что делает нажатие по значку: `open` или `empty`.
    #[serde(default = "open_action")]
    pub click: String,
    /// Спрашивать подтверждение перед очисткой.
    #[serde(default = "yes")]
    pub confirm: bool,
    /// Звук очистки.
    #[serde(default = "yes")]
    pub sound: bool,
    /// Окно хода очистки.
    #[serde(default = "yes")]
    pub progress: bool,
    /// Свои значки на пять ступеней: пусто, четверть, половина, три четверти,
    /// полная. Пустая строка — берём у оболочки.
    #[serde(default)]
    pub icons: Vec<String>,
}

fn yes() -> bool {
    true
}

fn open_action() -> String {
    "open".to_string()
}

impl Default for Bin {
    fn default() -> Self {
        Self {
            tray: false,
            steps: true,
            click: open_action(),
            confirm: true,
            sound: true,
            progress: true,
            icons: vec![String::new(); 5],
        }
    }
}

/// Настройки приложения. Пишутся в `config.json` рядом с данными.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Версия схемы — понадобится, когда состав полей начнёт меняться.
    pub version: u32,
    pub theme: String,
    pub language: String,
    /// Прятать окно в трей вместо выхода при закрытии.
    pub close_to_tray: bool,
    /// Стартовать свёрнутым (для автозапуска).
    pub start_minimized: bool,
    /// Запускаться вместе с Windows.
    #[serde(default)]
    pub autostart: bool,
    /// Держать окно поверх остальных.
    #[serde(default)]
    pub always_on_top: bool,
    /// Запоминать положение и размер окна.
    #[serde(default)]
    pub remember_geometry: bool,
    /// Возвращаться на тот же раздел, на котором окно спрятали.
    ///
    /// По умолчанию выключено: окно прячут крестиком из любого места, и
    /// открывать его потом на настройках, куда заходили один раз, — не то,
    /// чего ждут. Кому нужно наоборот — включает.
    #[serde(default)]
    pub remember_view: bool,
    /// Запомненное положение. Пусто, пока окно ни разу не закрывали.
    #[serde(default)]
    pub geometry: Option<Geometry>,
    /// Сохранённые сочетания цветов рамки выделения.
    #[serde(default)]
    pub selection_palette: Vec<crate::selection::Swatch>,
    /// Места — хранилища программ, которыми владеет приложение.
    #[serde(default)]
    pub libraries: Vec<crate::library::Library>,
    /// Основное Место: туда попадает то, для чего не выбрали другое.
    #[serde(default)]
    pub primary_library: Option<String>,
    /// Настройки изоляции по программам.
    #[serde(default)]
    pub isolation: Vec<Isolation>,
    /// Выбранный вариант значка приложения.
    ///
    /// serde(default) обязателен: поле добавлено позже, и без него старый
    /// config.json перестал бы разбираться — а значит уехал бы в
    /// config.broken.json вместе со всеми прочими настройками.
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Корзина: свой значок в трее и его повадки.
    #[serde(default)]
    pub bin: Bin,
}

fn default_icon() -> String {
    "gold".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            theme: "dark".into(),
            language: "ru".into(),
            close_to_tray: true,
            start_minimized: false,
            autostart: false,
            always_on_top: false,
            remember_geometry: false,
            remember_view: false,
            geometry: None,
            selection_palette: Vec::new(),
            libraries: Vec::new(),
            primary_library: None,
            isolation: Vec::new(),
            icon: default_icon(),
            bin: Bin::default(),
        }
    }
}

/// Имя каталога данных. То же, что `identifier` в `tauri.conf.json`, — по нему
/// Tauri и складывает свой `app_data_dir`.
const IDENTIFIER: &str = "com.good4fox.quickrapidx";

/// Настройки без запущенного приложения.
///
/// Нужны ровно одному месту — запуску по ярлыку: там окно не поднимается и
/// Tauri не стартует вовсе, а прочитать настройки надо. Путь собирается тот же,
/// что выдал бы `app_data_dir`.
pub fn load_config_standalone() -> Option<Config> {
    let path = PathBuf::from(std::env::var("APPDATA").ok()?)
        .join(IDENTIFIER)
        .join("config.json");

    let raw = fs::read_to_string(path).ok()?;

    serde_json::from_str::<Config>(&raw).ok()
}

/// Корневой каталог данных приложения.
pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("не удалось определить каталог данных: {e}"))
}

/// Создаёт каталог данных и все подкаталоги. Идемпотентно.
pub fn ensure_dirs(app: &AppHandle) -> Result<PathBuf, String> {
    let root = data_dir(app)?;

    for sub in SUBDIRS {
        let path = root.join(sub);
        fs::create_dir_all(&path).map_err(|e| format!("не удалось создать {}: {e}", path.display()))?;
    }

    Ok(root)
}

/// Путь к файлу настроек.
pub fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("config.json"))
}

/// Читает настройки, а при первом запуске — создаёт файл со значениями по умолчанию.
///
/// Битый или несовместимый `config.json` не роняет загрузку: он уходит в
/// `config.broken.json`, а на его место встаёт свежий файл по умолчанию.
pub fn load_or_create_config(app: &AppHandle) -> Result<Config, String> {
    let path = config_path(app)?;

    if !path.exists() {
        let config = Config::default();
        write_config(app, &config)?;
        return Ok(config);
    }

    let raw = fs::read_to_string(&path).map_err(|e| format!("не удалось прочитать config.json: {e}"))?;

    match serde_json::from_str::<Config>(&raw) {
        Ok(config) => Ok(config),
        Err(err) => {
            let backup = path.with_file_name("config.broken.json");
            let _ = fs::rename(&path, &backup);
            eprintln!("config.json повреждён ({err}), заменён на значения по умолчанию");

            let config = Config::default();
            write_config(app, &config)?;
            Ok(config)
        }
    }
}

/// Сохраняет настройки на диск.
pub fn write_config(app: &AppHandle, config: &Config) -> Result<(), String> {
    let path = config_path(app)?;
    let body = serde_json::to_string_pretty(config).map_err(|e| format!("не удалось собрать config.json: {e}"))?;

    fs::write(&path, body).map_err(|e| format!("не удалось записать config.json: {e}"))
}
