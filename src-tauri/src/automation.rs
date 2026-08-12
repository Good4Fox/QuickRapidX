//! Список пользовательских сценариев автоматизации.
//!
//! Перенесено из `app_automation.rs`. Два отличия: каталог берётся из
//! `app_data_dir()`, а не относительно текущего рабочего каталога (раньше
//! список зависел от того, откуда запустили exe), и наружу отдаётся список
//! имён, а не одна склеенная строка — фронтенд и так обходил её как список.

use std::fs;

use tauri::AppHandle;

use crate::paths;

/// Расширения, которые считаем сценариями.
const SCRIPT_EXT: [&str; 3] = ["py", "bat", "vbs"];

#[tauri::command]
pub fn check_automation_folders(app: AppHandle) -> Result<Vec<String>, String> {
    let dir = paths::data_dir(&app)?.join("automation");

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        // Каталога ещё нет — это не ошибка, просто пусто
        Err(_) => return Ok(Vec::new()),
    };

    let mut names = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        let is_script = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| SCRIPT_EXT.contains(&ext.to_lowercase().as_str()));

        if !is_script {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            names.push(name.to_string());
        }
    }

    names.sort();
    Ok(names)
}
