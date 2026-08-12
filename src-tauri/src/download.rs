//! Скачивание установщиков.
//!
//! Пункт «Скачать» задуман как корзина: человек отмечает нужное в каталоге, а
//! потом забирает всё разом. Ставить при этом ничего не нужно — установщики
//! просто складываются в папку, откуда их можно унести на другую машину или
//! запустить вручную.
//!
//! Два источника, потому что и записи бывают двух видов. У пакета winget есть
//! идентификатор — тогда качает сам winget, и берётся ровно тот файл, что
//! указан в проверенном манифесте. У записи с прямой ссылкой качать нечем,
//! кроме как самим; для этого берётся `curl`, который входит в состав Windows.

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;
use tauri::AppHandle;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Чем кончилась загрузка одной позиции.
#[derive(Serialize)]
pub struct Outcome {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

/// Куда складывать: «Загрузки/QuickRapidX».
fn target(app: &AppHandle) -> Result<PathBuf, String> {
    crate::library::downloads_dir().map(PathBuf::from).map_err(|e| {
        let _ = app;
        e
    })
}

/// Скачивает пакет winget, не устанавливая его.
#[tauri::command]
pub async fn download_package(app: AppHandle, id: String, name: String) -> Outcome {
    let dir = match target(&app) {
        Ok(dir) => dir,
        Err(e) => return Outcome { name, ok: false, message: e },
    };

    let result = tauri::async_runtime::spawn_blocking(move || {
        run(
            "winget",
            &[
                "download",
                "--id",
                &id,
                "--exact",
                "--download-directory",
                &dir.display().to_string(),
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        )
    })
    .await;

    match result {
        Ok(Ok(_)) => Outcome { name, ok: true, message: String::new() },
        Ok(Err(e)) => Outcome { name, ok: false, message: e },
        Err(e) => Outcome { name, ok: false, message: e.to_string() },
    }
}

/// Скачивает файл по прямой ссылке.
///
/// Только по HTTPS: подменить содержимое по открытому HTTP может кто угодно на
/// пути, а мы качаем то, что человек потом запустит.
#[tauri::command]
pub async fn download_link(app: AppHandle, url: String, name: String) -> Outcome {
    let url = url.trim().to_string();

    if !url.starts_with("https://") {
        return Outcome {
            name,
            ok: false,
            message: "ссылка не по HTTPS — скачивать по ней небезопасно".into(),
        };
    }

    let dir = match target(&app) {
        Ok(dir) => dir,
        Err(e) => return Outcome { name, ok: false, message: e },
    };

    let file = url
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty() && part.contains('.'))
        .unwrap_or("installer.exe")
        .split('?')
        .next()
        .unwrap_or("installer.exe")
        .to_string();

    let path = dir.join(file);

    let result = tauri::async_runtime::spawn_blocking(move || {
        run(
            "curl.exe",
            &[
                "--location",
                "--fail",
                "--silent",
                "--show-error",
                // Перенаправление на другой узел не должно уносить нас с HTTPS
                "--proto",
                "=https",
                "--output",
                &path.display().to_string(),
                &url,
            ],
        )
    })
    .await;

    match result {
        Ok(Ok(_)) => Outcome { name, ok: true, message: String::new() },
        Ok(Err(e)) => Outcome { name, ok: false, message: e },
        Err(e) => Outcome { name, ok: false, message: e.to_string() },
    }
}

#[cfg(windows)]
fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("{program} недоступен: {e}"))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail = stderr.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("");

    Err(if tail.is_empty() {
        format!("код {}", output.status.code().unwrap_or(-1))
    } else {
        tail.to_string()
    })
}

#[cfg(not(windows))]
fn run(_program: &str, _args: &[&str]) -> Result<String, String> {
    Err("не поддерживаемая операционная система".into())
}
