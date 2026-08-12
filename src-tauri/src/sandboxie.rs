//! Запуск через Sandboxie.
//!
//! Наша изоляция построена на AppContainer — том, что даёт сама Windows без
//! драйвера и без прав администратора. Граница там настоящая, но применимость
//! узкая: обычная настольная программа то и дело упирается в то, чего
//! AppContainer не пускает, и виснет или закрывается.
//!
//! Sandboxie решает ту же задачу драйвером ядра: он подменяет обращения к
//! файлам и реестру, и программа внутри живёт как в обычной системе, только
//! пишет в песочницу. Отсюда и разница в надёжности.
//!
//! Встроить её к себе нельзя и не нужно. Это отдельная программа с драйвером,
//! и распространяется она под GPL — включать её в наш состав значило бы взять
//! на себя обязательства этой лицензии. Поэтому мы просто находим уже
//! установленную и отдаём запуск ей: вызов чужой программы её частью нас не
//! делает, а весит это ровно ноль.

use std::path::PathBuf;

use serde::Serialize;

/// Что удалось узнать про установленную Sandboxie.
#[derive(Serialize)]
pub struct Sandboxie {
    pub installed: bool,
    /// Путь к `Start.exe` — через неё и запускают.
    pub path: String,
}

/// Где искать `Start.exe`, если реестр промолчал.
#[cfg(windows)]
const GUESSES: &[&str] = &[
    r"C:\Program Files\Sandboxie-Plus\Start.exe",
    r"C:\Program Files\Sandboxie\Start.exe",
    r"C:\Program Files (x86)\Sandboxie-Plus\Start.exe",
    r"C:\Program Files (x86)\Sandboxie\Start.exe",
];

/// Ищет установленную Sandboxie.
#[tauri::command]
pub fn sandboxie_status() -> Sandboxie {
    match find() {
        Some(path) => Sandboxie {
            installed: true,
            path: path.display().to_string(),
        },
        None => Sandboxie {
            installed: false,
            path: String::new(),
        },
    }
}

/// Путь к `Start.exe`.
#[cfg(windows)]
pub fn find() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    // Сначала спрашиваем систему: пути установки бывают какими угодно, а
    // догадки годятся только как запасной вариант
    let keys = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Sandboxie"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Sandboxie-Plus"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sandboxie-Plus",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sandboxie-Plus",
        ),
    ];

    for (root, path) in keys {
        let Ok(key) = RegKey::predef(root).open_subkey(path) else {
            continue;
        };

        for name in ["InstallLocation", "InstallPath", "Path"] {
            let Ok(value) = key.get_value::<String, _>(name) else {
                continue;
            };

            let candidate = PathBuf::from(value.trim_matches('"')).join("Start.exe");

            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    GUESSES
        .iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
}

#[cfg(not(windows))]
pub fn find() -> Option<PathBuf> {
    None
}

/// Запускает программу в песочнице Sandboxie.
///
/// `box_name` — имя песочницы. Пусто означает `DefaultBox`: он есть в любой
/// установке, а своя песочница должна быть заведена в самой Sandboxie — создать
/// её со стороны нельзя.
#[cfg(windows)]
pub fn launch(exe: &str, args: &str, box_name: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Some(start) = find() else {
        return Err("Sandboxie не найдена на этой машине".into());
    };

    let mut command = std::process::Command::new(start);

    let chosen = box_name.trim();

    command.arg(format!("/box:{}", if chosen.is_empty() { "DefaultBox" } else { chosen }));

    // Дальше идёт сама программа и её ключи: всё, что после, Start.exe
    // передаёт ей как есть
    command.arg(exe);

    for piece in split_args(args) {
        command.arg(piece);
    }

    if let Some(folder) = std::path::Path::new(exe).parent() {
        command.current_dir(folder);
    }

    command
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("не удалось обратиться к Sandboxie: {e}"))
}

#[cfg(not(windows))]
pub fn launch(_exe: &str, _args: &str, _box_name: &str) -> Result<(), String> {
    Err("не поддерживаемая операционная система".into())
}

/// Делит строку ключей на отдельные слова, уважая кавычки.
///
/// Отдавать всю строку одним куском нельзя: она уехала бы программе как один
/// длинный ключ. А делить по пробелам без оглядки на кавычки — значит разорвать
/// путь с пробелом надвое.
fn split_args(line: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut quoted = false;

    for symbol in line.chars() {
        match symbol {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    pieces.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        pieces.push(current);
    }

    pieces
}
