//! Последовательность загрузки приложения.
//!
//! Окно загрузки не крутит собственных таймеров: каждый шаг здесь делает
//! настоящую работу и сам сообщает о себе событием. Если шаг встанет —
//! полоса честно замрёт на нём, а не доедет до ста процентов сама.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize};

use crate::paths;

/// Сколько минимум показывать шаг. Без этого быстрые шаги мелькают и
/// загрузка выглядит дёрганой.
const MIN_STEP: Duration = Duration::from_millis(240);

/// Пауза между «готово» и ростом окна — на ней проигрывается затухание.
const HANDOFF: Duration = Duration::from_millis(280);

/// Рабочий размер окна. Открывается оно размером с заставку и вырастает сюда.
const WORK_WIDTH: f64 = 1280.0;
const WORK_HEIGHT: f64 = 730.0;

/// Защита от повторного запуска: окно загрузки может перемонтироваться
/// при hot reload в dev-режиме и позвать `boot_start` второй раз.
static STARTED: AtomicBool = AtomicBool::new(false);

/// Загрузка завершена.
///
/// События рассылаются один раз и без повтора: окно, чей интерфейс ещё не
/// успел подписаться, ничего не получит и будет ждать вечно. Главное окно
/// грузит весь CSS приложения и вполне может не успеть за время загрузки
/// ядра — поэтому состояние должно быть ещё и опрашиваемым.
static DONE: AtomicBool = AtomicBool::new(false);

/// Отработала ли загрузка. Окно спрашивает это при монтировании.
pub fn is_done() -> bool {
    DONE.load(Ordering::SeqCst)
}

#[derive(Clone, Serialize)]
struct Progress {
    index: usize,
    total: usize,
    key: &'static str,
    percent: u32,
}

#[derive(Clone, Serialize)]
struct Failure {
    key: &'static str,
    message: String,
}

/// Один шаг загрузки: машинный ключ и сама работа.
///
/// Подписи здесь нет намеренно. Прежде рядом с ключом лежала готовая русская
/// строка, она же и показывалась, — а экран загрузки человек видит при каждом
/// запуске, и на английском с японским он оставался русским. Теперь ядро
/// присылает только ключ, а подпись к нему берёт словарь.
struct Step {
    key: &'static str,
    run: fn(&AppHandle) -> Result<(), String>,
}

const STEPS: [Step; 5] = [
    Step {
        key: "paths",
        run: step_paths,
    },
    Step {
        key: "config",
        run: step_config,
    },
    Step {
        key: "system",
        run: step_system,
    },
    Step {
        key: "modules",
        run: step_modules,
    },
    Step {
        key: "interface",
        run: step_interface,
    },
];

fn step_paths(app: &AppHandle) -> Result<(), String> {
    paths::ensure_dirs(app).map(|_| ())
}

fn step_config(app: &AppHandle) -> Result<(), String> {
    paths::load_or_create_config(app).map(|_| ())
}

fn step_system(app: &AppHandle) -> Result<(), String> {
    // Версия webview — первое, что понадобится в отчётах об ошибках.
    match tauri::webview_version() {
        Ok(version) => println!("webview2: {version}"),
        Err(e) => eprintln!("не удалось определить версию webview: {e}"),
    }
    println!("сборка: {}", app.package_info().version);
    Ok(())
}

fn step_modules(_app: &AppHandle) -> Result<(), String> {
    // Задел под реестр модулей (очистка, реестр, диски). Пока пусто.
    Ok(())
}

fn step_interface(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// Запускает загрузку в фоновом потоке.
///
/// Главный поток не блокируется — в старой версии здесь стоял
/// `thread::sleep(5s)` прямо в `setup()`, и окно всё это время не отвечало.
pub fn start(app: AppHandle) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    tauri::async_runtime::spawn_blocking(move || {
        let total = STEPS.len();

        for (i, step) in STEPS.iter().enumerate() {
            let index = i + 1;
            let began = Instant::now();

            // Подпись показываем до работы, чтобы пользователь видел, на чём стоим.
            let _ = app.emit(
                "boot:progress",
                Progress {
                    index,
                    total,
                    key: step.key,
                    percent: ((index - 1) * 100 / total) as u32,
                },
            );

            if let Err(message) = (step.run)(&app) {
                eprintln!("загрузка прервана на шаге {}: {message}", step.key);
                let _ = app.emit(
                    "boot:failed",
                    Failure {
                        key: step.key,
                        message,
                    },
                );
                return;
            }

            if let Some(rest) = MIN_STEP.checked_sub(began.elapsed()) {
                std::thread::sleep(rest);
            }

            let _ = app.emit(
                "boot:progress",
                Progress {
                    index,
                    total,
                    key: step.key,
                    percent: (index * 100 / total) as u32,
                },
            );
        }

        // Флаг ставим до рассылки: окно, которое подпишется позже, спросит
        // состояние и увидит, что загрузка уже прошла.
        DONE.store(true, Ordering::SeqCst);
        let _ = app.emit("boot:ready", ());

        std::thread::sleep(HANDOFF);
        finish(&app);
    });
}

/// Разворачивает окно с размера заставки до рабочего.
///
/// Раньше здесь закрывалось отдельное окно-заставки и показывалось главное.
/// На этом стыке и появлялись артефакты: маленькое окно ещё не успевало
/// закрыться, а большое уже показывалось — было видно то пустую заставку, то
/// сразу содержимое приложения. Теперь окно одно и просто меняет размер.
fn finish(app: &AppHandle) {
    let start_minimized = paths::load_or_create_config(app)
        .map(|c| c.start_minimized)
        .unwrap_or(false);

    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    // Сначала размер, потом ограничение: обратный порядок не дал бы окну
    // вырасти, пока минимум ещё не поднят.
    let _ = window.set_size(LogicalSize::new(WORK_WIDTH, WORK_HEIGHT));
    let _ = window.set_min_size(Some(LogicalSize::new(WORK_WIDTH, WORK_HEIGHT)));

    /*
     * Запомненное положение возвращается здесь, а не при подготовке окна.
     *
     * Раньше его восстанавливали раньше, до загрузки, — и эти же две строки
     * тут же затирали его рабочим размером и центром. Настройка «запоминать
     * положение окна» из-за этого не делала ничего вовсе: окно каждый раз
     * вставало посередине.
     *
     * Размер берётся только если он не меньше рабочего: в запись мог попасть
     * размер окна загрузки, а с ним интерфейс не помещается.
     */
    let remembered = paths::load_or_create_config(app)
        .ok()
        .filter(|config| config.remember_geometry)
        .and_then(|config| config.geometry);

    match remembered {
        Some(geometry) => {
            let scale = window.scale_factor().unwrap_or(1.0);
            let least = LogicalSize::new(WORK_WIDTH, WORK_HEIGHT).to_physical::<u32>(scale);

            if geometry.width >= least.width && geometry.height >= least.height {
                let _ = window.set_size(PhysicalSize::new(geometry.width, geometry.height));
            }

            let _ = window.set_position(PhysicalPosition::new(geometry.x, geometry.y));
        }
        None => {
            let _ = window.center();
        }
    }

    /*
     * Запуск с ярлыка набора: показать надо панель, а не окно.
     *
     * Открывается она здесь, в самом конце загрузки, а не при старте: главное
     * окно забирает фокус, и панель, показанная раньше, тут же спряталась бы,
     * приняв это за щелчок мимо себя.
     */
    if let Some(group) = crate::tray::take_pending_group() {
        crate::tray::open_group_at_cursor(app, group);
        return;
    }

    if !start_minimized {
        let _ = window.set_focus();
    }
}
