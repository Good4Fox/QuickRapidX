//! Восстановление Windows.
//!
//! Задачи (`sfc`, `DISM`, `chkdsk`, сброс служб) требуют прав администратора и
//! идут долго — от минуты до часа. Раньше такие команды открывали отдельную
//! консоль: человек видел ход работы, но получал поверх окна чёрный терминал и
//! никакой связи с приложением.
//!
//! Здесь консоли нет. Задача запускается скрыто с повышением прав, её вывод
//! перенаправляется в журнал, а приложение читает оттуда проценты и шлёт их в
//! интерфейс. Полный текст журнала открывается по кнопке — разбирать его в
//! приложении незачем: `sfc` при перенаправлении пишет в UTF-16, `DISM` и
//! `chkdsk` — в кодировке консоли, и на разных языках системы это разные
//! кодировки. Проценты же всегда ASCII и читаются одинаково.
//!
//! Набор команд закреплён здесь, а не приходит из интерфейса: строка оттуда
//! означала бы возможность выполнить с правами администратора что угодно.

use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::paths;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Идёт ли уже задача. Две сразу запускать нельзя: `DISM` и `sfc` берут
/// одну и ту же блокировку обслуживания образа и мешают друг другу.
static BUSY: AtomicBool = AtomicBool::new(false);

/// Задача восстановления: ключ и команда для `cmd`.
struct Task {
    id: &'static str,
    command: &'static str,
}

/// `/English` у DISM — не украшательство: без него утилита печатает проценты
/// в языке системы, но с ним журнал остаётся читаемым в любой локали.
const TASKS: &[Task] = &[
    Task {
        id: "sfc",
        command: "sfc /scannow",
    },
    Task {
        id: "dism_check",
        command: "DISM /Online /Cleanup-Image /CheckHealth /English",
    },
    Task {
        id: "dism_scan",
        command: "DISM /Online /Cleanup-Image /ScanHealth /English",
    },
    Task {
        id: "dism_restore",
        command: "DISM /Online /Cleanup-Image /RestoreHealth /English",
    },
    Task {
        id: "chkdsk",
        command: "chkdsk %SystemDrive% /scan",
    },
    Task {
        id: "cleanup",
        command: "DISM /Online /Cleanup-Image /StartComponentCleanup /English",
    },
    Task {
        id: "windows_update",
        command: "net stop wuauserv & net stop bits & net stop cryptsvc \
                  & ren %SystemRoot%\\SoftwareDistribution SoftwareDistribution.old \
                  & ren %SystemRoot%\\System32\\catroot2 catroot2.old \
                  & net start cryptsvc & net start bits & net start wuauserv",
    },
    Task {
        id: "network",
        command: "netsh winsock reset & netsh int ip reset",
    },
];

fn find(id: &str) -> Option<&'static Task> {
    TASKS.iter().find(|task| task.id == id)
}

#[derive(Clone, Serialize)]
struct Progress {
    id: String,
    /// Доля выполнения, 0–100. Пусто, пока утилита не напечатала ни одной.
    percent: Option<u32>,
    /// Сколько идёт задача, секунды.
    elapsed: u64,
}

#[derive(Clone, Serialize)]
struct Finished {
    id: String,
    code: i32,
    log: String,
}

/// Путь к журналу задачи.
fn log_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(paths::data_dir(app)?.join("logs").join(format!("recovery-{id}.log")))
}

/// Запускает задачу восстановления.
#[tauri::command]
pub fn recovery_start(app: AppHandle, id: String) -> Result<(), String> {
    let task = find(&id).ok_or_else(|| format!("неизвестная задача: {id}"))?;

    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("другая задача восстановления уже идёт".into());
    }

    let log = match log_path(&app, task.id) {
        Ok(path) => path,
        Err(e) => {
            BUSY.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    // Журнал от прошлого запуска мешал бы считать проценты: в нём остались
    // свои, и первые секунды показывались бы они
    let _ = std::fs::remove_file(&log);

    let child = spawn_elevated(task.command, &log);

    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            BUSY.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    let task_id = task.id.to_string();

    // Ждём завершения в отдельном потоке и попутно читаем журнал: команда
    // может идти час, а ответ интерфейсу нужен сразу
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut last_percent = None;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    BUSY.store(false, Ordering::SeqCst);

                    let _ = app.emit(
                        "recovery:finished",
                        Finished {
                            id: task_id.clone(),
                            code: status.code().unwrap_or(-1),
                            log: log.display().to_string(),
                        },
                    );
                    return;
                }
                Ok(None) => {}
                Err(_) => {
                    BUSY.store(false, Ordering::SeqCst);
                    return;
                }
            }

            let percent = read_percent(&log);

            if percent != last_percent || started.elapsed().as_secs() % 2 == 0 {
                last_percent = percent;

                let _ = app.emit(
                    "recovery:progress",
                    Progress {
                        id: task_id.clone(),
                        percent,
                        elapsed: started.elapsed().as_secs(),
                    },
                );
            }

            std::thread::sleep(Duration::from_millis(600));
        }
    });

    Ok(())
}

/// Открывает журнал задачи в связанной программе.
#[tauri::command]
pub fn recovery_open_log(app: AppHandle, id: String) -> Result<(), String> {
    let log = log_path(&app, &id)?;

    if !log.exists() {
        return Err("журнал ещё не создан".into());
    }

    let mut cmd = Command::new("cmd");
    cmd.args(["/c", "start", "", &log.display().to_string()]);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Последняя доля выполнения из журнала.
///
/// Утилиты печатают ход одной строкой, возвращая каретку, поэтому разбираем и
/// по `\r`, и по `\n`. Кодировка при этом не важна: `sfc` пишет в UTF-16, где
/// каждый знак ASCII идёт через нулевой байт, — поэтому сначала выкидываем
/// нули, а цифры и знак процента остаются на месте в обеих кодировках.
fn read_percent(log: &PathBuf) -> Option<u32> {
    let mut file = std::fs::File::open(log).ok()?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw).ok()?;

    let ascii: Vec<u8> = raw.into_iter().filter(|byte| *byte != 0).collect();
    let text = String::from_utf8_lossy(&ascii);

    let mut last = None;
    let bytes = text.as_bytes();

    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'%' {
            continue;
        }

        // Отступаем назад по цифрам, пропуская дробную часть вида «12.5%»
        let mut start = index;
        let mut seen_digit = false;

        while start > 0 {
            let previous = bytes[start - 1];

            if previous.is_ascii_digit() {
                seen_digit = true;
                start -= 1;
            } else if (previous == b'.' || previous == b',') && seen_digit {
                start -= 1;
            } else {
                break;
            }
        }

        if !seen_digit {
            continue;
        }

        let number = &text[start..index];
        let whole = number.split(['.', ',']).next().unwrap_or("");

        if let Ok(value) = whole.parse::<u32>() {
            if value <= 100 {
                last = Some(value);
            }
        }
    }

    last
}

/// Запускает команду скрыто и с повышением прав, направляя вывод в журнал.
///
/// Промежуточный `powershell` нужен по двум причинам: только `Start-Process
/// -Verb RunAs` показывает запрос прав, и только он умеет `-Wait` — по его
/// завершению и понятно, что задача закончилась. Само окно скрыто.
#[cfg(windows)]
fn spawn_elevated(command: &str, log: &PathBuf) -> Result<std::process::Child, String> {
    let inner = format!("{command} > \"{}\" 2>&1", log.display());

    let script = format!(
        "Start-Process cmd.exe -Verb RunAs -WindowStyle Hidden -Wait \
         -ArgumentList '/c', '{}'",
        inner.replace('\'', "''")
    );

    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("не удалось запросить права администратора: {e}"))
}

#[cfg(not(windows))]
fn spawn_elevated(_command: &str, _log: &PathBuf) -> Result<std::process::Child, String> {
    Err("не поддерживаемая операционная система".into())
}

// ── Прошлая установка Windows ────────────────────────────────────────────

/// Что известно о прошлой установке Windows.
#[derive(Serialize)]
pub struct WindowsOld {
    pub present: bool,
    pub path: String,
}

/// Папка прошлой установки на системном диске.
///
/// Диск берётся из окружения, а не пишется как `C:`: система стоит не у всех на
/// одной и той же букве.
fn windows_old_path() -> PathBuf {
    crate::purge::folder()
}

/// Есть ли прошлая установка Windows.
#[tauri::command]
pub fn recovery_windows_old() -> WindowsOld {
    let path = windows_old_path();

    WindowsOld {
        present: path.is_dir(),
        path: path.display().to_string(),
    }
}

/// Сколько занимает прошлая установка, байты.
///
/// Считается обходом дерева: у папки нет заранее известного размера, а внутри
/// сотни тысяч файлов — поэтому команда асинхронная и уходит в отдельный поток.
/// Отказы в доступе пропускаются: часть подпапок закрыта даже для
/// администратора, и падать из-за них незачем — итог всё равно оценочный.
#[tauri::command]
pub async fn recovery_windows_old_size() -> u64 {
    tauri::async_runtime::spawn_blocking(|| folder_size(&windows_old_path()))
        .await
        .unwrap_or(0)
}

fn folder_size(root: &PathBuf) -> u64 {
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;

    #[cfg(windows)]
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    let mut total = 0u64;
    let mut stack = vec![root.clone()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };

            /*
             * Точки повторного разбора не раскрываются.
             *
             * Внутри лежат связи в живую систему, и обход по ним посчитал бы её
             * вместе с папкой. Судим по признаку, а не по `is_symlink`: тот
             * знает лишь два вида меток из десятка, и остальные — облачные
             * файлы, дедупликацию — пропустил бы внутрь.
             */
            #[cfg(windows)]
            if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                continue;
            }

            #[cfg(not(windows))]
            if meta.is_symlink() {
                continue;
            }

            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }

    total
}

/// Ход удаления прошлой установки. Отдельно от `Progress`: доли здесь мало —
/// нужны сами цифры, иначе полоса без движения выглядит зависанием.
#[derive(Clone, Serialize)]
struct Purge {
    objects: u64,
    bytes: u64,
    failed: u64,
    elapsed: u64,
}

/// Удаляет прошлую установку Windows.
///
/// Обычным удалением она не берётся: владелец части файлов — TrustedInstaller,
/// и администратора туда не пускают даже на перечисление. Прежде здесь стояли
/// `takeown /r`, `icacls /t` и `rd /s /q` — три полных прохода по дереву. На
/// живой машине это 950 511 файлов и 457 484 папки, то есть часы работы, три
/// строки журнала на каждый объект и ни одного признака жизни в окне.
///
/// Теперь проход один, и делает его наш же процесс с правами администратора
/// (см. `purge`): права перехватываются только там, где удаление отказало, а
/// ход работы идёт в окно живыми цифрами.
///
/// Штатный путь — «Очистка диска» с пунктом «Предыдущие установки Windows», но
/// он появляется не всегда и молчит о том, почему не сработал.
#[tauri::command]
pub fn recovery_remove_windows_old(app: AppHandle) -> Result<(), String> {
    let path = windows_old_path();

    if !path.is_dir() {
        return Err("папка прошлой установки не найдена".into());
    }

    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("другая задача восстановления уже идёт".into());
    }

    let log = log_path(&app, "windows_old")?;
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::write(&log, "0 0 0 0");

    let started = match spawn_purge(&log) {
        Ok(child) => child,
        Err(e) => {
            BUSY.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    let mut child = started;

    std::thread::spawn(move || {
        let began = Instant::now();

        loop {
            let over = matches!(child.try_wait(), Ok(Some(_)) | Err(_));

            if let Some((tally, done)) = crate::purge::read_progress(&log) {
                let _ = app.emit(
                    "recovery:purge",
                    Purge {
                        objects: tally.objects,
                        bytes: tally.bytes,
                        failed: tally.failed,
                        elapsed: began.elapsed().as_secs(),
                    },
                );

                if done && over {
                    break;
                }
            }

            if over {
                break;
            }

            std::thread::sleep(Duration::from_millis(400));
        }

        BUSY.store(false, Ordering::SeqCst);

        // Судим по самой папке, а не по коду возврата: код мог бы соврать в
        // любую сторону, а «есть или нет» — это то, ради чего всё затевалось
        let code = i32::from(windows_old_path().is_dir());

        let _ = app.emit(
            "recovery:finished",
            Finished {
                id: "windows_old".to_string(),
                code,
                log: log.display().to_string(),
            },
        );
    });

    Ok(())
}

/// Запускает нас же с правами администратора и ключом удаления.
///
/// Себя, а не `cmd`: удаление написано у нас, и только своим кодом можно
/// перехватывать права точечно и считать сделанное. Окно при этом не
/// поднимается — ключ разбирается до сборки приложения.
#[cfg(windows)]
fn spawn_purge(log: &PathBuf) -> Result<std::process::Child, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("не найти собственный путь: {e}"))?
        .display()
        .to_string();

    let script = format!(
        "Start-Process -FilePath '{exe}' -Verb RunAs -WindowStyle Hidden -Wait \
         -ArgumentList '{flag}{log}'",
        exe = exe.replace('\'', "''"),
        flag = crate::purge::PURGE_FLAG,
        log = log.display().to_string().replace('\'', "''"),
    );

    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("не удалось запросить права администратора: {e}"))
}

#[cfg(not(windows))]
fn spawn_purge(_log: &PathBuf) -> Result<std::process::Child, String> {
    Err("не поддерживаемая операционная система".into())
}

/// Открывает раздел восстановления в «Параметрах».
///
/// Переустановка поверх с сохранением файлов («Устранение неполадок с помощью
/// Центра обновления Windows») запускается только оттуда: у неё нет ни ключа
/// реестра, ни команды — это действие самой страницы параметров.
#[tauri::command]
pub fn recovery_open_settings() -> Result<(), String> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/c", "start", "", "ms-settings:recovery"]);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}
