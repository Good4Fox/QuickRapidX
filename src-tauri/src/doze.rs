//! Сон скрытых окон.
//!
//! Каждому окну верхнего уровня Chromium даёт свой процесс отрисовки, и в
//! замере на этой машине три наших окна стоили 223 МБ из 576. Свести их в один
//! процесс нельзя: ни `--process-per-site`, ни `--renderer-process-limit`, ни
//! отключение изоляции сайтов этого не делают — проверено по документации
//! Microsoft, а не по советам.
//!
//! Зато WebView2 умеет усыплять окно целиком. Разметка при этом сохраняется —
//! просыпается окно тем же, каким уснуло, — а вот таймеры, анимации и работа
//! процесса отрисовки останавливаются, и система получает право забрать его
//! память.
//!
//! Плата честная: пробуждение не мгновенное. Но окна трея и набора живут
//! скрытыми часами и показываются на секунды, так что платим мы редко, а
//! экономим постоянно.
//!
//! Порядок вызовов обязателен и в обе стороны разный. Уснуть можно только
//! невидимому окну — сначала невидимость, потом сон. Проснуться — наоборот:
//! сначала разбудить, потом показать, иначе окно нарисует пустоту.

/// Усыпляет окно. Звать после того, как окно спрятано.
#[cfg(windows)]
pub fn sleep(window: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    };
    use webview2_com::TrySuspendCompletedHandler;
    use windows::core::Interface;

    let _ = window.with_webview(|platform| {
        // SAFETY: обращения к COM-интерфейсу окна из его собственного потока —
        // именно там `with_webview` и выполняет замыкание.
        unsafe {
            let controller = platform.controller();

            // Невидимость первой: спящим окно становится только после неё
            if controller.SetIsVisible(false).is_err() {
                return;
            }

            let Ok(core) = controller.CoreWebView2() else {
                return;
            };

            let Ok(sleepy) = core.cast::<ICoreWebView2_3>() else {
                return;
            };

            /*
             * Ответ нас не интересует, но обработчик обязателен.
             *
             * Сон — дело лучших намерений: он не удаётся, пока в окне идёт
             * загрузка или проигрывается звук, и сообщает об этом как раз сюда.
             * Ничего страшного в отказе нет — окно просто останется бодрым до
             * следующего раза.
             */
            let done = TrySuspendCompletedHandler::create(Box::new(|_result, _ok| Ok(())));

            let _ = sleepy.TrySuspend(&done);

            /*
             * И отдельно просим уложиться в меньшее.
             *
             * Это не то же, что сон, и одно другому не мешает. Сон
             * останавливает работу окна; здесь мы говорим, что памяти ему
             * теперь нужно поменьше, — WebView2 сбрасывает свои запасы и жмёт
             * кучу сценариев. Складывается со сном и работает, даже когда тот
             * не удался.
             *
             * Обратно само не вернётся: Microsoft это оговаривает прямо, и
             * возвращать обычный уровень при пробуждении приходится самим —
             * иначе окно останется поджатым и на экране.
             */
            if let Ok(thrifty) = core.cast::<ICoreWebView2_19>() {
                let _ = thrifty.SetMemoryUsageTargetLevel(
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
                );
            }
        }
    });
}

/// Будит окно. Звать до того, как окно показано.
#[cfg(windows)]
pub fn wake(window: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows::core::Interface;

    let _ = window.with_webview(|platform| {
        // SAFETY: то же, что и выше.
        unsafe {
            let controller = platform.controller();

            /*
             * Сначала разбудить, потом показать.
             *
             * Одной видимости мало: спящее окно её не заметит и нарисует
             * пустоту. Пробуждение же безобидно для бодрого — на нём оно просто
             * ничего не делает, поэтому проверять состояние незачем.
             */
            if let Ok(core) = controller.CoreWebView2() {
                if let Ok(sleepy) = core.cast::<ICoreWebView2_3>() {
                    let _ = sleepy.Resume();
                }

                // Поджатие обратно само не отпускает — возвращаем вручную,
                // иначе окно останется экономным и на экране
                if let Ok(thrifty) = core.cast::<ICoreWebView2_19>() {
                    let _ = thrifty.SetMemoryUsageTargetLevel(
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
                    );
                }
            }

            let _ = controller.SetIsVisible(true);
        }
    });
}

#[cfg(not(windows))]
pub fn sleep(_window: &tauri::WebviewWindow) {}

#[cfg(not(windows))]
pub fn wake(_window: &tauri::WebviewWindow) {}

/// Отдаёт рабочий набор чуть погодя.
///
/// Не сразу: усыплённое окно освобождает свои страницы не мгновенно, и сброс в
/// тот же миг застал бы их ещё занятыми. Пара секунд — и отдавать уже есть что.
///
/// Отдельным потоком, потому что звать это из обработчика скрытия нельзя: тот
/// работает в потоке окна, а окно в это время ещё прячется.
#[cfg(windows)]
fn trim_later() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(2));
        trim();
    });
}

#[cfg(not(windows))]
fn trim_later() {}

/// Прячет окно и укладывает его спать.
///
/// Одна дверь на все скрытия — и из ядра, и из разметки. Раньше окна прятались
/// в семнадцати местах, двенадцать из которых звали `hide()` прямо из
/// интерфейса, минуя ядро; повесить сон на каждое значило бы держать
/// семнадцать копий одного правила и однажды забыть про восемнадцатое.
#[tauri::command]
pub fn window_doze(window: tauri::WebviewWindow) {
    tuck(&window);
}

/// То же, но из ядра: у него окно уже под рукой.
pub fn tuck(window: &tauri::WebviewWindow) {
    let _ = window.hide();
    sleep(window);

    // Окно ушло — самое время отдать системе то, что за ним держалось
    trim_later();
}

/// Будит окно и показывает его.
pub fn rouse(window: &tauri::WebviewWindow) {
    wake(window);
    let _ = window.show();
}

// ── Сброс рабочего набора ────────────────────────────────────────────────

/// Когда сбрасывали в последний раз.
///
/// Сброс не бесплатен: страницы возвращаются обратно по мере обращения, и
/// делать его часто — значит гонять их туда-сюда. Раз в полминуты и только по
/// уходу в простой.
#[cfg(windows)]
static TRIMMED: std::sync::LazyLock<std::sync::Mutex<Option<std::time::Instant>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

#[cfg(windows)]
const TRIM_GUARD: std::time::Duration = std::time::Duration::from_secs(30);

/// Отдаёт системе рабочий набор — свой и всех окон.
///
/// Что это на самом деле. Страницы не выбрасываются, а переводятся в резерв:
/// система получает право забрать их под чужие нужды, но пока они не
/// понадобились никому — они остаются в памяти, и возврат стоит нуля. На диск
/// они уедут только если памяти станет по-настоящему мало, и тогда это ровно
/// то, что система сделала бы сама, просто чуть раньше.
///
/// Поэтому и звать это можно **только по уходу в простой**: главное окно ушло
/// в трей, всплывающие уснули. Пока с программой работают, сброс означал бы
/// возврат тех же страниц через секунду — работу ради красивой цифры.
///
/// Честная оговорка: обязательство системы перед приложением этим не
/// уменьшается. Меняется то, что показывают диспетчер задач и всё, что смотрит
/// на рабочий набор.
#[cfg(windows)]
pub fn trim() {
    // Без подкачки сброс делать незачем — см. `paging_on`
    if !paging_on() {
        return;
    }

    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Memory::SetProcessWorkingSetSizeEx;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcess, PROCESS_SET_QUOTA,
    };

    {
        let Ok(mut slot) = TRIMMED.lock() else {
            return;
        };

        if slot.is_some_and(|at| at.elapsed() < TRIM_GUARD) {
            return;
        }

        *slot = Some(std::time::Instant::now());
    }

    // SAFETY: описатели закрываются сразу после использования; неудача любого
    // вызова означает лишь, что этот процесс не поджался.
    unsafe {
        let ours = GetCurrentProcessId();

        // Оба предела в «максимум» — так система понимает просьбу как «сбрось
        // рабочий набор до нуля». Способ документированный, не самодельный
        SetProcessWorkingSetSizeEx(GetCurrentProcess(), usize::MAX, usize::MAX, 0);

        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);

        if snapshot.is_null() {
            return;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                /*
                 * Только свои дети.
                 *
                 * Проверка по родителю обязательна: на машине бывает и второе
                 * приложение на WebView2 — в замерах оно нашлось сразу, — и
                 * поджимать чужие процессы мы не вправе.
                 */
                if entry.th32ParentProcessID == ours {
                    let handle = OpenProcess(PROCESS_SET_QUOTA, 0, entry.th32ProcessID);

                    if !handle.is_null() {
                        SetProcessWorkingSetSizeEx(handle, usize::MAX, usize::MAX, 0);
                        windows_sys::Win32::Foundation::CloseHandle(handle);
                    }

                    // Внуки: у WebView2 процессы отрисовки заводит его главный
                    // процесс, а не мы
                    trim_children(entry.th32ProcessID);
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        windows_sys::Win32::Foundation::CloseHandle(snapshot);
    }
}

/// Поджимает детей указанного процесса. Один уровень — глубже не нужно.
#[cfg(windows)]
unsafe fn trim_children(parent: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Memory::SetProcessWorkingSetSizeEx;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA};

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);

        if snapshot.is_null() {
            return;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32ParentProcessID == parent {
                    let handle = OpenProcess(PROCESS_SET_QUOTA, 0, entry.th32ProcessID);

                    if !handle.is_null() {
                        SetProcessWorkingSetSizeEx(handle, usize::MAX, usize::MAX, 0);
                        CloseHandle(handle);
                    }
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
    }
}

#[cfg(not(windows))]
pub fn trim() {}

/// Включена ли в системе подкачка.
///
/// Проверка нужна не ради безопасности — без подкачки сброс тоже безвреден.
/// Дело в том, что он тогда почти ничего и не делает: страницы уходят в резерв,
/// но выселить их системе некуда, и при первой же нехватке памяти она вернёт их
/// обратно. Получилась бы работа ради работы, да ещё и с лишними отказами
/// страниц у человека, который подкачку выключил намеренно — обычно как раз
/// ради отзывчивости.
///
/// Смотрим в настройки, а не в файл: файла может не быть на месте в момент
/// проверки, а «управляется системой» означает, что он появится по надобности.
#[cfg(windows)]
fn paging_on() -> bool {
    use std::sync::OnceLock;

    /// Спрашивается один раз: подкачку не включают и не выключают на ходу —
    /// это требует перезагрузки.
    static ANSWER: OnceLock<bool> = OnceLock::new();

    *ANSWER.get_or_init(|| {
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
        use winreg::RegKey;

        let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management",
            KEY_READ,
        ) else {
            // Не смогли спросить — считаем, что подкачка есть: так по
            // умолчанию у подавляющего большинства
            return true;
        };

        /*
         * Значение многострочное: по строке на файл подкачки. Выключенная
         * подкачка оставляет список пустым — или с одной пустой строкой, что то
         * же самое.
         *
         * Автоматическое управление списка не опустошает: там стоит запись вида
         * `?:\pagefile.sys 0 0`, где нули означают «размер выберет система».
         * Поэтому отдельной проверки на него не нужно.
         */
        let files: Vec<String> = key.get_value("PagingFiles").unwrap_or_default();

        files.iter().any(|line| !line.trim().is_empty())
    })
}

#[cfg(not(windows))]
fn paging_on() -> bool {
    false
}
