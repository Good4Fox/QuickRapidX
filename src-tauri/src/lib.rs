//! QuickRapidX — ядро приложения.
//!
//! Окна объявлены в `tauri.conf.json` и создаются скрытыми. Показывает их
//! `setup`: сначала окно загрузки, а главное — когда отработает `boot`.

mod app_icon;
mod automation;
mod basket;
mod bin_tray;
mod boot;
mod container;
mod cursor;
mod download;
mod doze;
mod groups;
mod iconpack;
mod icons;
mod inventory;
mod library;
mod paths;
mod purge;
mod recovery;
mod sandboxie;
mod selection;
mod store;
mod system;
mod tray;
mod tweaks;
mod window_fx;
mod window_prefs;
mod wsandbox;
mod winget;
mod windows_cmd;
mod windows_settings;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, WindowEvent};

/// Взведён на время осознанного выхода.
///
/// Крестик главного окна прячет его в трей, то есть отменяет закрытие. Без
/// этого флага тот же обработчик отменял бы и закрытие при выходе — и
/// приложение осталось бы висеть в памяти.
static QUITTING: AtomicBool = AtomicBool::new(false);

/// Сигнал от окна загрузки: интерфейс отрисован, можно начинать.
///
/// Загрузка стартует отсюда, а не из `setup`, чтобы первые события прогресса
/// не ушли в пустоту — окно к этому моменту уже слушает.
#[tauri::command]
fn boot_start(app: AppHandle) {
    boot::start(app);
}

/// Отработала ли загрузка ядра.
///
/// Окно спрашивает это при монтировании: событие `boot:ready` рассылается
/// один раз, и окно, чей интерфейс ещё не подписался, его не получит.
#[tauri::command]
fn boot_status() -> bool {
    boot::is_done()
}

/// Показать главное окно (кнопка из панели трея).
#[tauri::command]
fn show_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        doze::rouse(&window);
        let _ = window.set_focus();
    }
}

/// Показать/спрятать главное окно — вызывается из панели трея.
#[tauri::command]
fn app_tray_popen(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            hide_main(&window);
            return;
        }

        let _ = window.unminimize();
        doze::rouse(&window);
        let _ = window.set_focus();
    }
}

/// Прячет главное окно и сообщает ему об этом.
///
/// Сообщение нужно самому окну: спрятали его из настроек, а показать потом
/// надо с главной — иначе программа возвращается туда, куда заходили один раз.
/// Решает окно, а не ядро: настройка «возвращаться на тот же раздел» живёт
/// рядом с прочими настройками окна, и читать её дважды незачем.
fn hide_main(window: &tauri::WebviewWindow) {
    // Через общую дверь: она же и усыпляет. Главное окно — самый крупный
    // процесс отрисовки из трёх, и висеть ему бодрым в трее незачем
    doze::tuck(window);
    let _ = window.emit("app:main-hidden", ());
}

/// Разослать окнам сигнал перечитать язык и тему.
///
/// Раньше это делалось через `window.eval("location.reload()")` — окна
/// перезагружались целиком, теряя своё состояние. Теперь окна просто
/// перечитывают localStorage по событию.
#[tauri::command]
fn app_update_labels(app: AppHandle) {
    let _ = app.emit("app:labels-updated", ());

    // Подписи корзины в трее переведутся сами: окно, получив это событие,
    // перечитает словарь и пришлёт их новыми через `bin_set_labels`
}

/// Полный выход из приложения.
#[tauri::command]
fn app_quit(app: AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    app.exit(0);
}

/// Прячет консольное окно процесса, если оно есть.
///
/// В отладочной сборке приложение остаётся консольным: `windows_subsystem`
/// проставляется только в релизе, иначе в разработке пропал бы вывод. Из-за
/// этого запуск с ярлыка выводил на экран чёрное окно консоли — даже когда сам
/// процесс тут же завершался, отдав просьбу работающему экземпляру.
///
/// В релизе консоли нет вовсе, и вызов ничего не делает.
fn hide_console() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::GetConsoleWindow;
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

        // SAFETY: обращения без параметров-указателей; окна может не быть, и
        // тогда ShowWindow просто отвечает отказом.
        unsafe {
            let console = GetConsoleWindow();

            if !console.is_null() {
                ShowWindow(console, SW_HIDE);
            }
        }
    }
}

/// Разрешает работающему экземпляру поднять своё окно поверх.
///
/// Windows не даёт произвольному процессу забирать передний план: право есть
/// только у того, кого запустили последним. Ярлык запускает нас, значит право
/// у **этого** процесса, а показать окно должен уже работающий — и его вызов
/// `set_focus` тихо ни к чему не приводил. Окно появлялось, но фокуса не
/// получало и закрывалось от первого же щелчка мимо.
///
/// `ASFW_ANY` передаёт право кому угодно. Живёт оно до ближайшей смены
/// переднего плана, то есть ровно на время передачи.
fn allow_foreground_handoff() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

        // SAFETY: вызов без параметров-указателей; отказ ничего не ломает.
        unsafe {
            AllowSetForegroundWindow(ASFW_ANY);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    /*
     * Ярлык изолированного запуска ведёт на нас же, но с ключом. Тогда окно не
     * поднимается вовсе: программа стартует в изоляции, и процесс завершается.
     *
     * Проверка стоит до сборки приложения намеренно — иначе поднялся бы весь
     * интерфейс ради одного запуска, а при включённом единственном экземпляре
     * ключ ушёл бы уже работающему окну.
     */
    if let Some(target) = std::env::args()
        .skip(1)
        .find_map(|arg| arg.strip_prefix(container::ISOLATE_FLAG).map(str::to_string))
    {
        hide_console();
        container::run_standalone(&target);
        return;
    }

    /*
     * Удаление прошлой установки Windows. Тот же приём и по той же причине:
     * этот процесс поднят с правами администратора и делает одну работу.
     *
     * Поднимать ради неё весь интерфейс нельзя вдвойне: он оказался бы вторым
     * экземпляром, да ещё и с правами администратора — а окно с такими правами
     * перестаёт принимать перетаскивание файлов из проводника.
     */
    if let Some(log) = std::env::args()
        .skip(1)
        .find_map(|arg| arg.strip_prefix(purge::PURGE_FLAG).map(str::to_string))
    {
        hide_console();
        std::process::exit(purge::run_standalone(&log));
    }

    // Запуск с ярлыка набора: этот процесс либо отдаст просьбу работающему
    // экземпляру и завершится, либо поднимет приложение. В обоих случаях
    // мелькать консолью он не должен
    if std::env::args().any(|arg| arg.starts_with(tray::GROUP_FLAG)) {
        hide_console();
        allow_foreground_handoff();
    }

    let mut builder = tauri::Builder::default();

    /*
     * Свой протокол для значков: `qrxicon://localhost/<ключ>`.
     *
     * Значки идут окну ссылкой, а не строкой. Строка `data:` живёт в памяти
     * окна и заводится заново в каждом — три окна означали бы три копии одного
     * значка. По ссылке же картинку тянет и держит сам движок, у него для
     * этого свой склад, который он же и чистит.
     *
     * Ответ асинхронный: чтение идёт из общего файла-хранилища, и держать на
     * этом поток окна нельзя — он же рисует.
     */
    builder = builder.register_asynchronous_uri_scheme_protocol("qrxicon", |ctx, request, responder| {
        let app = ctx.app_handle().clone();

        // Ключ — последняя часть пути. Всё прочее нас не касается: чужого сюда
        // не попадает, протокол свой
        let key = request
            .uri()
            .path()
            .trim_start_matches('/')
            .to_string();

        std::thread::spawn(move || {
            let body = iconpack::root(&app)
                .ok()
                .and_then(|root| u64::from_str_radix(&key, 16).ok().map(|id| (root, id)))
                .and_then(|(root, id)| iconpack::get(&root, id));

            let answer = match body {
                Some(bytes) => tauri::http::Response::builder()
                    .header("Content-Type", "image/png")
                    // Значок под этим ключом не меняется никогда: ключ считан
                    // из времени правки файла, и правка даёт новый ключ
                    .header("Cache-Control", "public, max-age=31536000, immutable")
                    .body(bytes),
                None => tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new()),
            };

            if let Ok(answer) = answer {
                responder.respond(answer);
            }
        });
    });

    /*
     * Передача запуска уже работающему экземпляру. Всегда, в любой сборке.
     *
     * Плагин работает так: **первый** процесс заводит общесистемный объект, а
     * поздние по нему узнают, что копия уже есть. Значит, регистрировать его по
     * условию бессмысленно: если у работающего экземпляра плагина нет,
     * узнавать поздним не по чему, и ярлык набора поднимал вторую копию.
     * Именно это и происходило, пока условие стояло.
     *
     * Прежняя оговорка была про разработку: `tauri dev` отдавал бы запуск
     * висящему экземпляру и молча выходил. Но `tauri dev` и так завершает
     * прежний процесс перед новым запуском, а поведение приложения не должно
     * различаться между сборками — иначе такие ошибки и всплывают у
     * пользователя, а не на сборке.
     */
    builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        // Ярлык набора с панели задач: приложение уже работает, и запуск
        // должен открыть его панель, а не поднять главное окно
        if let Some(group) = args
            .iter()
            .find_map(|arg| arg.strip_prefix(tray::GROUP_FLAG).map(str::to_string))
        {
            // Пока загрузка не кончилась, показывать панель рано: в конце
            // загрузки главное окно забирает фокус, и панель спряталась бы,
            // приняв это за щелчок мимо себя. Откладываем на тот же путь,
            // которым идёт холодный запуск
            if boot::is_done() {
                tray::open_group_at_cursor(app, group);
            } else {
                tray::set_pending_group(group);
            }

            return;
        }

        // Второй запуск не поднимает второй экземпляр, а возвращает первый.
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            doze::rouse(&window);
            let _ = window.set_focus();
        }
    }));

    builder
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            boot_start,
            boot_status,
            show_main_window,
            app_tray_popen,
            app_update_labels,
            app_icon::app_icon_list,
            app_icon::app_icon_get,
            app_icon::app_icon_set,
            app_quit,
            automation::check_automation_folders,
            windows_cmd::windows_pars_admin,
            windows_cmd::run_command,
            windows_cmd::open_terminal_command,
            tray::tray_opened_group,
            tray::tray_edge,
            tray::tray_fit_group,
            groups::group_list,
            groups::group_one,
            groups::group_save,
            groups::group_remove,
            groups::group_reorder,
            groups::group_pin,
            groups::group_shortcut_file,
            groups::group_pin_start,
            groups::group_duplicate,
            groups::group_launch,
            groups::group_launch_all,
            sandboxie::sandboxie_status,
            wsandbox::wsandbox_status,
            wsandbox::wsandbox_check,
            wsandbox::wsandbox_repair,
            wsandbox::wsandbox_reinstall,
            wsandbox::wsandbox_restart,
            container::container_ensure,
            container::container_remove,
            container::container_grant,
            container::container_launch,
            container::container_capabilities,
            container::container_presets,
            container::container_profile,
            container::container_profiles,
            container::container_set_profile,
            container::container_run,
            container::container_shortcut,
            container::container_data,
            container::container_status,
            container::container_snapshot,
            container::container_snapshots,
            container::container_snapshot_restore,
            container::container_snapshot_forget,
            container::container_snapshot_export,
            container::container_snapshot_import,
            windows_cmd::shell_open,
            icons::icon_extract,
            icons::icon_key,
            download::download_package,
            download::download_link,
            inventory::inventory_scan,
            winget::winget_version,
            winget::winget_search,
            winget::winget_show,
            winget::winget_installed,
            winget::winget_install,
            winget::winget_upgrade,
            winget::winget_export,
            winget::winget_import,
            library::library_list,
            library::library_add,
            library::library_forget,
            library::library_delete,
            library::library_move,
            library::library_set_primary,
            library::library_rename,
            library::library_suggest,
            library::library_disks,
            library::downloads_dir,
            library::library_install_path,
            library::library_has,
            tweaks::tweaks_get,
            tweaks::tweaks_set_sticky,
            tweaks::tweaks_set_filter,
            tweaks::tweaks_set_toggle,
            tweaks::tweaks_set_hibernate,
            tweaks::tweaks_set_fast_startup,
            selection::selection_get,
            selection::selection_apply,
            selection::selection_palette,
            selection::selection_palette_add,
            selection::selection_palette_remove,
            cursor::cursor_default,
            recovery::recovery_start,
            recovery::recovery_open_log,
            recovery::recovery_windows_old,
            recovery::recovery_windows_old_size,
            recovery::recovery_remove_windows_old,
            recovery::recovery_open_settings,
            windows_settings::windows_open_settings,
            windows_settings::windows_open_explorer,
            windows_settings::windows_lock_workstation,
            windows_settings::windows_sleep,
            windows_settings::windows_shutdown,
            windows_settings::windows_restart,
            windows_settings::windows_logoff,
            windows_settings::windows_update_square_box,
            windows_settings::regedit_get_keys_flags,
            windows_settings::regedit_set_keys_flags,
            windows_settings::regedit_get_keys_hibernate,
            windows_settings::regedit_set_keys_hibernate,
            window_prefs::window_prefs_get,
            window_prefs::window_prefs_set_autostart,
            window_prefs::window_prefs_set_start_minimized,
            window_prefs::window_prefs_set_close_to_tray,
            window_prefs::window_prefs_set_always_on_top,
            window_prefs::window_prefs_set_remember_geometry,
            window_prefs::window_prefs_set_remember_view,
            store::json_data_info,
            store::import_catalog,
            store::catalog_save,
            store::catalog_remove,
            store::catalog_move,
            store::create_json_data_info,
            store::editor_person_in_json,
            store::remove_person_in_json,
            store::search_person_in_json,
            basket::tray_basket_open,
            basket::tray_basket_get_max_capacity,
            basket::bin_volumes,
            basket::bin_volume_limit,
            basket::bin_volume_nuke,
            basket::tray_basket_get_current_usage,
            basket::tray_basket_state,
            basket::bin_settings,
            basket::bin_settings_save,
            basket::bin_shell_icons,
            basket::bin_empty,
            bin_tray::bin_set_labels,
            doze::window_doze,
            system::system_info,
        ])
        .setup(|app| {
            // Выбор папки для Места: вбивать путь руками — не дело
            let _ = app.handle().plugin(tauri_plugin_dialog::init());

            let _ = app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ));

            tray::build(app)?;
            // Наборы получают по своей иконке рядом с основной: это их
            // единственный способ попасть на глаза, окно для них не нужно
            tray::refresh(app.handle());
            // Корзина — свой значок рядом с ними, если её попросили. Заводится
            // после наборов: порядок в панели соответствует порядку заведения
            bin_tray::refresh(app.handle());
            /*
             * Всплывающие окна укладываем спать сразу.
             *
             * Они объявлены скрытыми и создаются такими при запуске — то есть
             * ни разу не проходят через скрытие и без этого остались бы бодрыми
             * навсегда, хотя человек их, может, и не откроет за весь сеанс.
             *
             * С задержкой: разметка внутри них ещё грузится, а уснувшее на
             * полпути окно осталось бы недостроенным до первого показа.
             */
            {
                let waker = app.handle().clone();

                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(6));

                    for label in ["tray", "group"] {
                        if let Some(window) = waker.get_webview_window(label) {
                            doze::sleep(&window);
                        }
                    }

                    // Уснули — отдаём системе то, что за ними держалось
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    doze::trim();
                });
            }

            // Ярлык набора мог застать приложение выключенным — просьба
            // запоминается и исполняется в конце загрузки
            tray::note_pending_group();
            // Значок ставится сразу после трея: к этому моменту есть и окно,
            // и иконка в панели, а окно ещё не показано пользователю
            app_icon::restore(app.handle());
            window_fx::setup(app.handle());
            window_prefs::restore(app.handle());

            // Окно показывается сразу — оно же и есть экран загрузки.
            // Кроме запуска из автозапуска со свёрнутым стартом: там окна не
            // должно быть видно вовсе, программа живёт в трее.
            if !window_prefs::started_minimized() {
                if let Some(main) = app.get_webview_window("main") {
                    main.show()?;
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Панель трея ведёт себя как всплывающее меню: ушёл фокус — закрылась.
            //
            // Отмечаем момент: клик по иконке трея сначала уводит отсюда фокус,
            // и только потом доходит до обработчика клика. Без отметки тот
            // увидел бы скрытую панель и открыл её заново — закрыть панель
            // кликом по иконке было бы нельзя.
            WindowEvent::Focused(false) if window.label() == "tray" || window.label() == "group" => {
                tray::on_panel_focus_lost(window);
            }

            // Сворачивание Windows отдаёт как изменение размера в 0×0.
            WindowEvent::Resized(size) => {
                window_fx::note_resize(window, size.width, size.height);
                window_prefs::note_geometry(window);
            }

            WindowEvent::Moved(_) => {
                window_prefs::note_geometry(window);
            }

            // Крестик главного окна прячет его в трей — но только если так
            // велено в настройках. Прежде окно пряталось всегда, и флажок
            // «сворачивать в панель задач» ни на что не влиял.
            // При осознанном выходе закрытие не отменяем — иначе не выйти.
            WindowEvent::CloseRequested { api, .. }
                if window.label() == "main" && !QUITTING.load(Ordering::SeqCst) =>
            {
                let app = window.app_handle();
                window_prefs::flush_geometry(app);

                if window_prefs::close_to_tray(app) {
                    api.prevent_close();

                    // Через помощника: он же сообщает окну, что оно спряталось
                    if let Some(main) = app.get_webview_window("main") {
                        hide_main(&main);
                    }
                } else {
                    QUITTING.store(true, Ordering::SeqCst);
                    app.exit(0);
                }
            }

            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("не удалось запустить приложение");
}
