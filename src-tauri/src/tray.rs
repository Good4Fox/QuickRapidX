//! Иконки в системном трее и всплывающая панель.
//!
//! Основная иконка: левый клик — показать/спрятать главное окно, правый —
//! панель быстрых действий (окно `tray`) у курсора.
//!
//! Наборы ярлыков получают по собственной иконке рядом. Это и есть весь смысл
//! затеи: щёлкнул по значку набора — открылся список того, что в нём лежит.
//! Общая панель со списком наборов такого не даёт: до нужного набора всё равно
//! пришлось бы добираться в два шага.
//!
//! Иконки наборов заводятся заново при каждом изменении списка: добавить или
//! убрать одну по месту нельзя — состав известен только целиком.

use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Rect};

/// Для какого набора панель открыта сейчас. Пусто — общий список.
static OPENED_FOR: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

/// Когда и для какого набора панель спряталась сама, потеряв фокус.
///
/// Клик по значку сначала уводит фокус с панели — она прячется, — и только
/// потом до нас доходит событие клика. Без этой отметки обработчик видел бы
/// «панель скрыта» и тут же открывал её заново: закрыть панель кликом по
/// значку было невозможно.
static PANEL_AUTOHIDDEN: LazyLock<Mutex<Option<(Instant, String)>>> =
    LazyLock::new(|| Mutex::new(None));

/// В пределах этого времени клик считается тем самым, что и закрыл панель.
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// Когда панель в последний раз показали.
///
/// Сразу после показа Windows может прислать запоздалое `Focused(false)` —
/// окно ведь до этого фокуса не имело. Обработчик принимал его за «пользователь
/// щёлкнул мимо» и прятал панель в тот же миг: она мигала и пропадала.
static PANEL_SHOWN_AT: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

/// Сколько после показа не реагировать на потерю фокуса.
const SHOW_GRACE: Duration = Duration::from_millis(400);

/// Панель показали только что — потеря фокуса сейчас ничего не значит.
fn shown_just_now() -> bool {
    PANEL_SHOWN_AT
        .lock()
        .ok()
        .and_then(|slot| *slot)
        .is_some_and(|at| at.elapsed() < SHOW_GRACE)
}

/// Прячет панель по уходу фокуса — если это не отголосок её же показа.
pub fn on_panel_focus_lost(window: &tauri::Window) {
    if shown_just_now() || !window.is_visible().unwrap_or(false) {
        return;
    }

    if let Some(panel) = window.get_webview_window(window.label()) {
        crate::doze::tuck(&panel);
    } else {
        let _ = window.hide();
    }

    note_panel_autohidden();
}

/// Отмечает, что панель спряталась по потере фокуса, и для какого набора.
///
/// Раньше отметка ставилась только когда курсор стоял над значком: так
/// отличали «щёлкнули по значку» от «щёлкнули мимо». Признак оказался
/// ненадёжным — события наведения приходят не всегда, особенно если значок
/// убран в переполнение за стрелочку. Стоило им не прийти, и панель переставала
/// закрываться: она пряталась по потере фокуса, а пришедший следом щелчок
/// открывал её заново.
///
/// Теперь отметка ставится всегда, а различает случаи набор: запрет на
/// открытие действует только для того значка, чьё содержимое панель и
/// показывала. Щелчок по другому значку переносит панель к нему.
///
/// Плата за это — 300 мс: если закрыть панель щелчком по рабочему столу и в тот
/// же миг ткнуть в значок, он не сработает. Промах в такое окно куда безобиднее
/// панели, которая не закрывается вовсе.
pub fn note_panel_autohidden() {
    let opened = OPENED_FOR
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default();

    if let Ok(mut slot) = PANEL_AUTOHIDDEN.lock() {
        *slot = Some((Instant::now(), opened));
    }
}

/// Панель только что закрылась щелчком по этому же значку?
///
/// Отметка снимается в любом случае: она относится к одному щелчку и на
/// следующий влиять не должна.
fn just_autohidden(group: &str) -> bool {
    let Ok(mut slot) = PANEL_AUTOHIDDEN.lock() else {
        return false;
    };

    let taken = slot.take();

    matches!(taken, Some((at, ref who)) if at.elapsed() < REOPEN_GUARD && who == group)
}

/// Зазор между краем рабочей области и панелью, в логических пикселях.
const GAP: f64 = 8.0;

/// Логический размер окна трея. Должен совпадать с tauri.conf.json.
const PANEL_WIDTH: f64 = 390.0;
const PANEL_HEIGHT: f64 = 490.0;

/// Начальный размер окна набора — до того, как оно скажет свой настоящий.
///
/// Постоянного размера у него нет: окно подгоняется под содержимое, как это
/// сделано и в AppGroup, где ширина набирается из плиток и числа колонок, а не
/// задаётся числом. Набор из двух ярлыков и набор из двадцати — разные окна.
const GROUP_WIDTH: f64 = 140.0;
const GROUP_HEIGHT: f64 = 90.0;

/// Куда прижимать окно набора: прямоугольник значка и точка курсора.
///
/// Запоминается при открытии, потому что подгонка размера приходит позже —
/// когда содержимое отрисовалось, — и место надо пересчитать заново.
static ANCHOR: LazyLock<Mutex<Option<(PhysicalPosition<f64>, Rect)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Какое окно показывать: набор открывается в своём.
///
/// Панель трея — ящик с разделами: корзина, общий список, нижний ряд
/// переходов. Набор открывают ради одного действия. Делить одно окно на две
/// такие задачи значит либо тащить в набор лишнее, либо ломать панель.
fn panel_label(group: &str) -> &'static str {
    if group.is_empty() {
        "tray"
    } else {
        "group"
    }
}

/// Приставка к идентификатору иконки набора.
///
/// По ней обработчик отличает набор от основной иконки, а обновление — свои
/// иконки от чужих.
const GROUP_PREFIX: &str = "qrx.group.";

/// Какой набор показывать в панели. Пусто — общий список.
///
/// Окно узнаёт это событием: панель одна на все наборы, и открывается она то
/// на одном, то на другом.
const OPEN_EVENT: &str = "tray:group";

/// Ключ командной строки у ярлыка набора.
pub const GROUP_FLAG: &str = "--group=";

/// Набор, который просили открыть при холодном запуске.
///
/// Ярлык с панели задач может застать приложение выключенным. Тогда панель
/// нельзя показать сразу: загрузка ещё идёт, а главное окно в конце забирает
/// фокус — панель тут же спряталась бы, приняв это за щелчок мимо. Поэтому
/// просьба откладывается до конца загрузки.
static PENDING: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

/// Запоминает набор из командной строки, если он там есть.
pub fn note_pending_group() {
    let asked = std::env::args()
        .skip(1)
        .find_map(|arg| arg.strip_prefix(GROUP_FLAG).map(str::to_string));

    if let Some(group) = asked {
        set_pending_group(group);
    }
}

/// Откладывает просьбу до конца загрузки.
pub fn set_pending_group(group: String) {
    if let Ok(mut slot) = PENDING.lock() {
        *slot = Some(group);
    }
}

/// Забирает отложенную просьбу. Второй раз вернёт пусто.
pub fn take_pending_group() -> Option<String> {
    PENDING.lock().ok().and_then(|mut slot| slot.take())
}

/// Прямоугольник панели задач.
///
/// Спрашивается у самой оболочки, а не считается по разнице между экраном и
/// рабочей областью: панель, скрывающаяся автоматически, из рабочей области не
/// вычитается вовсе, и разница вышла бы нулевой.
#[cfg(windows)]
fn taskbar() -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    use windows_sys::Win32::UI::Shell::{SHAppBarMessage, ABM_GETTASKBARPOS, APPBARDATA};

    // SAFETY: структура заполняется целиком, размер проставлен как требуется.
    unsafe {
        let mut data: APPBARDATA = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<APPBARDATA>() as u32;

        if SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) == 0 {
            return None;
        }

        let rect = data.rc;

        // Размеры без знака: прямоугольник от оболочки приходит со знаком, но
        // отрицательной ширины у панели задач быть не может
        Some((
            PhysicalPosition::new(rect.left, rect.top),
            PhysicalSize::new(
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            ),
        ))
    }
}

#[cfg(not(windows))]
fn taskbar() -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    None
}

/// Открывает панель с набором рядом с его ярлыком на панели задач.
///
/// Прямоугольника кнопки на панели задач система не отдаёт, поэтому опорой
/// служит сама панель задач: по её краю окно и встаёт, а место вдоль панели
/// берётся по курсору — там, куда человек только что нажал.
///
/// Почему не по курсору целиком: нажатие сопровождается рывком мыши, и окно,
/// поставленное по её точке, уезжало то выше, то ниже кнопки. Панель задач
/// стоит на месте, и от неё окно не пляшет.
pub fn open_group_at_cursor(app: &AppHandle, group: String) {
    let cursor = app
        .cursor_position()
        .unwrap_or(PhysicalPosition { x: 0.0, y: 0.0 });

    let anchor = match taskbar() {
        // Вдоль панели — по курсору, поперёк — по самой панели
        Some((bar, size)) if size.width >= size.height => Rect {
            position: tauri::Position::Physical(PhysicalPosition::new(cursor.x as i32, bar.y)),
            size: tauri::Size::Physical(PhysicalSize::new(1, size.height)),
        },

        // Панель сбоку: поперёк берём её ширину, вдоль — курсор
        Some((bar, size)) => Rect {
            position: tauri::Position::Physical(PhysicalPosition::new(bar.x, cursor.y as i32)),
            size: tauri::Size::Physical(PhysicalSize::new(size.width, 1)),
        },

        // Панели не нашлось — остаётся точка курсора
        None => Rect {
            position: tauri::Position::Physical(PhysicalPosition::new(
                cursor.x as i32,
                cursor.y as i32,
            )),
            size: tauri::Size::Physical(PhysicalSize::new(1, 1)),
        },
    };

    open_for(app, group, cursor, anchor);
}

/// Заводит иконки наборов заново.
///
/// Вызывается при запуске и после каждой правки списка. Старые снимаются все:
/// узнать, какая из них относится к изменённому набору, дешевле пересозданием,
/// чем разбором того, что поменялось.
pub fn refresh(app: &AppHandle) {
    let groups = crate::groups::for_tray(app);

    /*
     * Пересоздавать значки на каждую мелочь нельзя.
     *
     * Windows считает вернувшийся значок новым и убирает его в переполнение —
     * за стрелочку. Человек вытащил набор на видное место, переименовал в нём
     * одну программу — и значок снова спрятался. Поэтому сначала смотрим,
     * изменилось ли то, что трей вообще показывает: состав, названия, значки.
     */
    let now = signature(&groups);

    if LAST_BUILT.lock().is_ok_and(|before| *before == now) {
        return;
    }

    // Снимаем прежние. Идентификаторы известны только по приставке, поэтому
    // проходим по нынешнему составу — иконка снятого набора иначе осталась бы
    // висеть до перезапуска
    for id in taken() {
        app.remove_tray_by_id(&id);
    }

    let mut ids = Vec::new();

    for group in groups {
        let id = format!("{GROUP_PREFIX}{}", group.id);

        let icon = group_icon(app, &group);

        let built = TrayIconBuilder::with_id(id.clone())
            .icon(icon)
            .tooltip(&group.name)
            .on_tray_icon_event(on_group_event)
            .build(app);

        match built {
            Ok(_) => ids.push(id),
            Err(e) => eprintln!("иконка набора «{}» не заведена: {e}", group.name),
        }
    }

    if let Ok(mut slot) = GROUP_ICONS.lock() {
        *slot = ids;
    }

    if let Ok(mut slot) = LAST_BUILT.lock() {
        *slot = now;
    }

    // Заодно подтягиваем закреплённые на панели задач ярлыки: у них тот же
    // значок и та же подпись, и меняются они по тем же причинам
    crate::groups::sync_pinned(app);

    // И готовим файлы ярлыков заранее: их тащат мышью, а запись через оболочку
    // занимает сотни миллисекунд — в момент захвата ждать нечего
    crate::groups::rebuild_shortcuts(app);
}

/// То, от чего зависит вид трея: состав, названия и значки.
///
/// Содержимое наборов сюда не входит намеренно — кроме первой записи, у
/// которой берётся значок, если своего у набора нет.
fn signature(groups: &[crate::groups::Group]) -> String {
    groups
        .iter()
        .map(|group| {
            let source = group
                .items
                .first()
                .map(|item| item.path.as_str())
                .unwrap_or("");

            format!("{}\u{1}{}\u{1}{}\u{1}{}", group.id, group.name, group.icon, source)
        })
        .collect::<Vec<_>>()
        .join("\u{2}")
}

/// Идентификаторы заведённых иконок наборов.
static GROUP_ICONS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Слепок того, из чего трей собран сейчас.
static LAST_BUILT: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

fn taken() -> Vec<String> {
    GROUP_ICONS.lock().map(|slot| slot.clone()).unwrap_or_default()
}

/// Значок набора: из указанного файла, иначе из первой записи, иначе наш.
fn group_icon(app: &AppHandle, group: &crate::groups::Group) -> tauri::image::Image<'static> {
    let source = if !group.icon.trim().is_empty() {
        group.icon.clone()
    } else {
        group
            .items
            .first()
            .map(|item| item.path.clone())
            .unwrap_or_default()
    };

    if !source.trim().is_empty() {
        // 32 — размер, в котором Windows рисует значок в панели; больше значит
        // отдать системе картинку, которую она тут же уменьшит
        if let Some((pixels, side)) = crate::icons::raster(&source, 32) {
            return tauri::image::Image::new_owned(pixels, side, side);
        }
    }

    fallback_icon(app)
}

fn fallback_icon(app: &AppHandle) -> tauri::image::Image<'static> {
    tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))
        .map(|image| image.to_owned())
        .unwrap_or_else(|_| {
            // Пиксели копируются, а не заимствуются: картинка окна живёт не
            // дольше ссылки на приложение, а иконка нужна трею и после выхода
            // отсюда
            app.default_window_icon()
                .map(|image| {
                    tauri::image::Image::new_owned(
                        image.rgba().to_vec(),
                        image.width(),
                        image.height(),
                    )
                })
                .expect("в сборке нет иконки окна по умолчанию")
        })
}

/// Клик по значку набора — открыть его список у этого же значка.
fn on_group_event(tray: &TrayIcon, event: TrayIconEvent) {
    let app = tray.app_handle();

    let id = tray.id().as_ref().to_string();

    let Some(group) = id.strip_prefix(GROUP_PREFIX).map(str::to_string) else {
        return;
    };

    // Правый клик по набору — то же, что левый: своего меню у него нет, а
    // молчать на половину нажатий было бы странно
    let TrayIconEvent::Click {
        button_state: MouseButtonState::Up,
        position,
        rect,
        ..
    } = event
    else {
        return;
    };

    open_for(app, group, position, rect);
}

pub fn build(app: &App) -> tauri::Result<()> {
    // Отдельная иконка под трей: она нарисована под мелкий размер и читается
    // в системной панели лучше, чем уменьшенная иконка приложения.
    let icon = match tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")) {
        Ok(image) => image,
        Err(e) => {
            eprintln!("не удалось прочитать icons/tray.png ({e}), берём иконку окна");
            app.default_window_icon()
                .cloned()
                .expect("в сборке нет иконки окна по умолчанию")
        }
    };

    TrayIconBuilder::with_id("quickrapidx")
        .icon(icon)
        .tooltip("QuickRapidX")
        .on_tray_icon_event(on_event)
        .build(app)?;

    Ok(())
}

fn on_event(tray: &TrayIcon, event: TrayIconEvent) {
    let app = tray.app_handle();

    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            // Левый клик тоже гасит отметку: иначе правый клик сразу после него
            // приняли бы за «тот самый, что закрыл панель», и он бы не сработал.
            let _ = just_autohidden("");
            toggle_main(app);
        }

        TrayIconEvent::Click {
            button: MouseButton::Right,
            button_state: MouseButtonState::Up,
            position,
            rect,
            ..
        } => toggle_panel(app, position, rect),

        _ => {}
    }
}

/// Показывает главное окно, если оно скрыто, и прячет, если показано.
fn toggle_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    // Свёрнутое окно считаем скрытым: клик по иконке должно его разворачивать,
    // а не прятать ещё раз.
    if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false) {
        crate::doze::tuck(&window);
        return;
    }

    /*
     * Через общую дверь, а не `show()` напрямую.
     *
     * Спрятанное окно спит, и одного показа ему мало: спящее ничего не рисует,
     * и окно появлялось пустым — со стороны это выглядело как «не открывается
     * вовсе». Именно этот путь я и не разбудил, когда заводил сон: крестик
     * ведёт через обработчик закрытия, а значок в трее — сюда.
     */
    let _ = window.unminimize();
    crate::doze::rouse(&window);
    let _ = window.set_focus();
}

/// Открывает настройки корзины — в главном окне.
///
/// Не в панели трея: настроек там нет вовсе. Панель показывает состояние и два
/// действия, а всё, что настраивается, живёт в окне программы.
pub fn open_bin_settings(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let _ = window.unminimize();
    crate::doze::rouse(&window);
    let _ = window.set_focus();

    // Событие отправляем после показа: подписка живёт в разметке окна, и до
    // его появления слушать некому
    let _ = window.emit("app:open-settings", "SettingsBin");
}

/// Показывает панель трея рядом с самой иконкой.
///
/// Опорой служит прямоугольник иконки из события, а не курсор и не рабочая
/// область монитора. Курсор в момент клика находится внутри панели задач, а
/// рабочая область её не исключает, если панель задач скрывается автоматически:
/// Windows отдаёт всю высоту экрана. Прямоугольник иконки верен в обоих случаях
/// и заодно говорит, у какого края экрана стоит панель задач.
fn toggle_panel(app: &AppHandle, cursor: PhysicalPosition<f64>, icon: Rect) {
    open_for(app, String::new(), cursor, icon);
}

/// Открывает панель у значка. Пустой `group` — общий список.
///
/// Повторный клик по тому же значку закрывает панель; клик по другому —
/// переносит её к нему и показывает другой набор, а не закрывает.
fn open_for(app: &AppHandle, group: String, cursor: PhysicalPosition<f64>, icon: Rect) {
    let label = panel_label(&group);

    let Some(window) = app.get_webview_window(label) else {
        return;
    };

    // Переход между общим списком и набором меняет окно: прежнее надо убрать,
    // иначе оно останется висеть поверх нового
    for other in ["tray", "group"] {
        if other == label {
            continue;
        }

        if let Some(stale) = app.get_webview_window(other) {
            crate::doze::tuck(&stale);
        }
    }

    let same = OPENED_FOR
        .lock()
        .ok()
        .map(|slot| *slot == group)
        .unwrap_or(false);

    // Отметка о самозакрытии относится ровно к одному щелчку, поэтому
    // снимается всегда — а вот запрещает открытие только своему значку
    let closed_by_this = just_autohidden(&group);

    if window.is_visible().unwrap_or(false) {
        if same {
            crate::doze::tuck(&window);
            return;
        }

        // Другой значок — панель просто переезжает
    } else if closed_by_this {
        // Панель уже закрылась сама от этого же клика — открывать её заново
        // нельзя, иначе она мигает и остаётся на экране.
        return;
    }

    if let Ok(mut slot) = OPENED_FOR.lock() {
        *slot = group.clone();
    }

    /*
     * Будим до события, а не перед показом.
     *
     * Окно набора считает свой размер само: получив событие, оно перерисовывает
     * содержимое и меряет его через два кадра отрисовки. Спящее окно кадров не
     * рисует — замер не состоялся бы вовсе, и окно вылезло бы прежнего размера,
     * а потом на глазах прыгнуло.
     */
    crate::doze::wake(&window);

    // Окно узнаёт, что показывать, событием: панель одна на все наборы
    let _ = app.emit(OPEN_EVENT, group);

    let monitor = window
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        show_panel(&window);
        return;
    };

    let scale = monitor.scale_factor();

    // Скрытое окно не всегда сообщает размер в физических пикселях, поэтому
    // считаем сами: логический размер из конфигурации × масштаб монитора.
    let (logical_width, logical_height) = if label == "group" {
        (GROUP_WIDTH, GROUP_HEIGHT)
    } else {
        (PANEL_WIDTH, PANEL_HEIGHT)
    };

    let width = (logical_width * scale).round() as i32;
    let height = (logical_height * scale).round() as i32;
    let gap = (GAP * scale).round() as i32;

    let icon_pos: PhysicalPosition<i32> = icon.position.to_physical(scale);
    let icon_size: PhysicalSize<i32> = icon.size.to_physical(scale);

    let screen_pos = monitor.position();
    let screen_size = monitor.size();
    let screen_left = screen_pos.x;
    let screen_top = screen_pos.y;
    let screen_right = screen_left + screen_size.width as i32;
    let screen_bottom = screen_top + screen_size.height as i32;

    let (x, y) = beside(
        (icon_pos, icon_size),
        (width, height),
        (screen_left, screen_top, screen_right, screen_bottom),
        gap,
    );

    // Размер задаётся явно: окно ни разу не показывали, и полагаться на то,
    // что оно уже нужного размера, нельзя
    let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));

    if label == "group" {
        if let Ok(mut slot) = ANCHOR.lock() {
            *slot = Some((cursor, icon));
        }
    }

    let place = PhysicalPosition::new(x, y);

    let _ = window.set_position(place);
    show_panel(&window);

    /*
     * И ещё раз после показа.
     *
     * Окну, которое ни разу не показывали, Windows назначает место сама, и
     * положение, выставленное до показа, она может перебить своим — окно
     * появлялось где-то вверху экрана вместо места у значка. Повтор после
     * показа стоит дёшево и снимает вопрос.
     */
    let _ = window.set_position(place);
}

/// Скругление окна набора, в логических пикселях.
///
/// Должно совпадать с `border-radius` у `.qrx_pop`: одна и та же дуга рисуется
/// разметкой и вырезается у окна, и разойтись им нельзя.
const GROUP_RADIUS: f64 = 14.0;

/// Вырезает у окна скруглённые углы.
///
/// Без этого окно остаётся прямоугольным, а скруглена только панель внутри —
/// на углах и видно, что одно не совпадает с другим. Прозрачность здесь не
/// помощник: она зависит от того, как отработает композитор, а область окна
/// задаётся системе прямо и работает одинаково везде.
///
/// Вызывать после каждой смены размера: область задаётся в пикселях и вместе с
/// окном сама не растёт.
fn round_window(window: &tauri::WebviewWindow, radius: f64) {
    #[cfg(windows)]
    {
        // SetWindowRgn живёт в Gdi, хотя и объявлена в user32
        use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};

        let (Ok(handle), Ok(size)) = (window.hwnd(), window.outer_size()) else {
            return;
        };

        let scale = window.scale_factor().unwrap_or(1.0);

        // CreateRoundRectRgn ждёт ширину и высоту дуги, а не радиус
        let arc = (radius * scale * 2.0).round() as i32;

        // Правый и нижний края у области не включаются, отсюда +1
        let region = unsafe {
            CreateRoundRectRgn(
                0,
                0,
                size.width as i32 + 1,
                size.height as i32 + 1,
                arc,
                arc,
            )
        };

        if region.is_null() {
            return;
        }

        // SAFETY: область передаётся системе во владение — освобождать её нам
        // уже нельзя, и удалять её здесь было бы ошибкой.
        unsafe {
            SetWindowRgn(handle.0 as _, region, 1);
        }
    }
}

/// Просит Windows не скруглять окно самой.
///
/// У окна нет рамки, а углы ему всё равно скругляет система — своим радиусом,
/// поверх нашего. Две скруглённые формы одна в другой и дают неряшливые уголки
/// по краям: наша золотая дуга не совпадает с системной обрезкой.
///
/// Отказ ничего не ломает: на Windows 10 такого свойства нет, и окно останется
/// прямоугольным — то есть ровно тем, чем и было.
fn square_corners(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        };

        let Ok(handle) = window.hwnd() else {
            return;
        };

        let value = DWMWCP_DONOTROUND;

        // SAFETY: указатель ведёт на переменную в стеке, живую до конца вызова.
        unsafe {
            DwmSetWindowAttribute(
                handle.0 as _,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &value as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&value) as u32,
            );
        }
    }
}

/// Где встать окну рядом со значком.
///
/// По горизонтали — по середине значка, а не по его краю: значок в трее и
/// кнопка на панели задач узкие, и окно, выровненное краем, выглядит съехавшим
/// в сторону. По вертикали — с той стороны, где есть место: значок в нижней
/// половине экрана значит панель задач снизу, и раскрываться надо вверх.
///
/// К положению курсора расчёт не привязан вовсе. Щелчок по значку — это ещё и
/// рывок мышью, и окно, поставленное по курсору, уезжало следом за ним.
fn beside(
    anchor: (PhysicalPosition<i32>, PhysicalSize<i32>),
    window: (i32, i32),
    screen: (i32, i32, i32, i32),
    gap: i32,
) -> (i32, i32) {
    let (position, size) = anchor;
    let (width, height) = window;
    let (left, top, right, bottom) = screen;

    let middle = position.x + size.width / 2;
    let x = middle - width / 2;

    let y = if position.y > (top + bottom) / 2 {
        position.y - height - gap
    } else {
        position.y + size.height + gap
    };

    (
        x.clamp(left + gap, (right - width - gap).max(left)),
        y.clamp(top + gap, (bottom - height - gap).max(top)),
    )
}

/// С какой стороны экрана стоит панель задач.
///
/// Нужна окну набора: всплывашка скрытых значков у Windows поднимается **от**
/// панели задач, и наше окно должно появляться так же. Куда именно — знает
/// только ядро: у окна нет ни положения панели, ни своего места на экране.
///
/// Отвечаем словом, а не числами: окну нужна сторона, а не координаты, и
/// разбирать их на его стороне значило бы повторить эту же логику второй раз.
#[tauri::command]
pub fn tray_edge() -> String {
    let Some((position, size)) = taskbar() else {
        return "bottom".into();
    };

    // Панель задач — полоса: она либо шире, чем выше, либо наоборот. По этому
    // и различаем, вдоль какой стороны она лежит
    if size.width >= size.height {
        if position.y > 0 {
            "bottom".into()
        } else {
            "top".into()
        }
    } else if position.x > 0 {
        "right".into()
    } else {
        "left".into()
    }
}

/// Показывает панель и отмечает момент показа.
fn show_panel(window: &tauri::WebviewWindow) {
    if let Ok(mut slot) = PANEL_SHOWN_AT.lock() {
        *slot = Some(Instant::now());
    }

    // Панель спит, пока скрыта, — будим её вместе с показом
    crate::doze::rouse(window);
    let _ = window.set_focus();

    // После показа: до него у окна ещё нет настоящего изображения, и система
    // применяет своё скругление заново
    square_corners(window);

    // Своя дуга вместо системной — у окна набора она заметно меньше
    if window.label() == "group" {
        round_window(window, GROUP_RADIUS);
    }
}

/// Какой набор панель показывает сейчас.
///
/// Окно спрашивает это при появлении. Одного события мало: оно уходит один
/// раз, и окно, чей интерфейс ещё не подписался — а при первом открытии он
/// только грузится, — его не получит. Тогда окно оставалось ни с чем и честно
/// сообщало, что набора нет.
#[tauri::command]
pub fn tray_opened_group() -> String {
    OPENED_FOR.lock().map(|slot| slot.clone()).unwrap_or_default()
}


/// Подгоняет окно набора под содержимое и ставит его на место заново.
///
/// Размер приходит из окна: сколько плиток поместилось в ряд и сколько вышло
/// рядов, знает только разметка. Ядру остаётся применить это и пересчитать
/// место — от прямоугольника значка, у которого окно и должно стоять.
#[tauri::command]
pub fn tray_fit_group(app: AppHandle, width: f64, height: f64) {
    let Some(window) = app.get_webview_window("group") else {
        return;
    };

    let Some((cursor, icon)) = ANCHOR.lock().ok().and_then(|slot| *slot) else {
        return;
    };

    let monitor = window
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return;
    };

    let scale = monitor.scale_factor();

    // Разумные границы: окно не должно ни схлопнуться в точку, ни занять экран.
    // Нижний предел мал намеренно — набор из двух программ и должен быть с
    // ладонь, а не с прежние две сотни пикселей впустую
    let width = (width.clamp(90.0, 900.0) * scale).round() as i32;
    let height = (height.clamp(50.0, 760.0) * scale).round() as i32;
    let gap = (GAP * scale).round() as i32;

    let icon_pos: PhysicalPosition<i32> = icon.position.to_physical(scale);
    let icon_size: PhysicalSize<i32> = icon.size.to_physical(scale);

    let screen_pos = monitor.position();
    let screen_size = monitor.size();
    let screen_left = screen_pos.x;
    let screen_top = screen_pos.y;
    let screen_right = screen_left + screen_size.width as i32;
    let screen_bottom = screen_top + screen_size.height as i32;

    let (x, y) = beside(
        (icon_pos, icon_size),
        (width, height),
        (screen_left, screen_top, screen_right, screen_bottom),
        gap,
    );

    let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));
    let _ = window.set_position(PhysicalPosition::new(x, y));

    // Область окна задана в пикселях и вместе с ним не растёт — вырезаем заново
    round_window(&window, GROUP_RADIUS);
}
