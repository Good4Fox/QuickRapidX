//! Запуск команд и терминала, проверка прав администратора.
//!
//! Перенесено из `windows_function.rs` прошлой версии. Отличия:
//! ручные `extern "system"` объявления заменены на `windows-sys`, а чтение
//! вывода команды уехало в блокирующий поток — раньше оно висело на
//! исполнителе async и подмораживало интерфейс на долгих командах.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use tauri::{Emitter, Window};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Не показывать консольное окно у порождённого процесса.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Запускает программу с аргументом `-Command`, не показывая консоль.
#[cfg(windows)]
pub fn console_command_start(program: &str, args: &str) -> Result<(), String> {
    Command::new(program)
        .arg("-Command")
        .arg(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Запущено ли приложение с правами администратора.
#[tauri::command]
pub fn windows_pars_admin() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::{
            AllocateAndInitializeSid, CheckTokenMembership, FreeSid, SID_IDENTIFIER_AUTHORITY,
        };

        // Значения из winnt.h. Задаём literal'ами: в windows-sys они разложены
        // по разным модулям и переезжают между версиями.
        const SECURITY_NT_AUTHORITY: [u8; 6] = [0, 0, 0, 0, 0, 5];
        const SECURITY_BUILTIN_DOMAIN_RID: u32 = 0x20;
        const DOMAIN_ALIAS_RID_ADMINS: u32 = 0x220;

        unsafe {
            let authority = SID_IDENTIFIER_AUTHORITY {
                Value: SECURITY_NT_AUTHORITY,
            };
            let mut admins = core::ptr::null_mut();

            // Собираем SID группы «Администраторы» и спрашиваем, состоит ли в
            // ней текущий токен. null вместо токена означает «токен текущего
            // потока» — отдельно открывать его не нужно.
            if AllocateAndInitializeSid(
                &authority,
                2,
                SECURITY_BUILTIN_DOMAIN_RID,
                DOMAIN_ALIAS_RID_ADMINS,
                0,
                0,
                0,
                0,
                0,
                0,
                &mut admins,
            ) == 0
            {
                return false;
            }

            let mut is_member = 0;
            let ok = CheckTokenMembership(core::ptr::null_mut(), admins, &mut is_member);

            FreeSid(admins);

            ok != 0 && is_member != 0
        }
    }

    #[cfg(not(windows))]
    {
        false
    }
}

/// Выполняет команду, построчно отправляя вывод в окно событиями
/// `command-output` и `command-error`.
#[tauri::command]
pub async fn run_command(cmd: String, window: Window) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = spawn_shell(&cmd).map_err(|e| format!("не удалось запустить команду: {e}"))?;

        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines() {
                let line = line.map_err(|e| format!("ошибка чтения вывода: {e}"))?;
                window
                    .emit("command-output", line)
                    .map_err(|e| format!("не удалось отправить событие: {e}"))?;
            }
        }

        if let Some(stderr) = child.stderr.take() {
            for line in BufReader::new(stderr).lines() {
                let line = line.map_err(|e| format!("ошибка чтения ошибок: {e}"))?;
                window
                    .emit("command-error", line)
                    .map_err(|e| format!("не удалось отправить событие: {e}"))?;
            }
        }

        // Дожидаемся завершения, иначе процесс остаётся зомби
        let _ = child.wait();

        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("поток команды упал: {e}"))?
}

fn spawn_shell(cmd: &str) -> std::io::Result<std::process::Child> {
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", cmd])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    }

    #[cfg(not(windows))]
    {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
}

/// Открывает отдельное окно терминала с командой.
#[tauri::command]
pub fn open_terminal_command(
    command: String,
    as_admin: bool,
    use_powershell: bool,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        let result = if as_admin {
            Command::new("powershell")
                .arg("-Command")
                .arg(format!(
                    "Start-Process powershell -ArgumentList '-NoExit', '-Command', '{command}' -Verb RunAs"
                ))
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
        } else {
            // Своё окно этой оболочке не нужно: она только вызывает `start`,
            // а видимый терминал открывает уже он. Без флага поверх нужного
            // окна мелькало чёрное — она и была единственным местом в ядре,
            // где запуск процесса не гасил своё окно
            let mut shell = Command::new("cmd");
            shell.creation_flags(CREATE_NO_WINDOW);

            if use_powershell {
                shell.arg("/c").arg(format!("start powershell -NoExit -Command {command}"));
            } else {
                shell.arg("/c").arg(format!("start cmd /K {command}"));
            }
            shell.spawn()
        };

        result
            .map(|_| "Терминал запущен с командой.".to_string())
            .map_err(|e| format!("ошибка запуска терминала: {e}"))
    }

    #[cfg(not(windows))]
    {
        let _ = (command, as_admin, use_powershell);
        Err("не поддерживаемая операционная система".into())
    }
}

/// Открывает путь тем, чем его открывает оболочка.
///
/// Своя команда вместо плагина открытия. У того путь проверяется по списку
/// разрешённых, список задаётся в правах приложения — и там, где он не совпал,
/// плагин отказывает **молча**: кнопка нажимается, ничего не происходит, в
/// журнал ничего не попадает. Мы на это наступили дважды: сперва список был
/// пуст, потом образец в нём не сошёлся с путями Windows.
///
/// Здесь проверять нечего: путь приходит из наших же списков — Мест хранения,
/// найденных программ, каталога изоляции, — а не из внешнего мира.
#[tauri::command]
pub fn shell_open(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let target = path.trim();

        if target.is_empty() {
            return Err("путь не указан".into());
        }

        let wide: Vec<u16> = std::ffi::OsStr::new(target)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: строка завершена нулём и живёт до конца вызова.
        let code = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };

        // Оболочка отвечает описателем больше 32 при успехе — так задумано в
        // самом вызове, это не наша выдумка
        if (code as isize) > 32 {
            return Ok(());
        }

        Err(format!("оболочка отказала: код {}", code as isize))
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        Err("не поддерживаемая операционная система".into())
    }
}
