//! Корзина отдельным значком в трее.
//!
//! Сделано по образцу MiniBin — той самой мелочи, ради которой её ставят.
//! Оттуда взято всё, что там есть: значок меняется по наполнению, нажатие
//! настраивается на «открыть» или «очистить», у очистки три отдельных
//! переключателя (подтверждение, звук, окно хода), а под каждую ступень
//! наполнения можно подставить свой значок.
//!
//! Своего у нас два, и оба — про честность к системе. Первое: значки по
//! умолчанию не рисуются нами, а берутся у самой оболочки, из той же записи
//! реестра, откуда их берёт рабочий стол. Значит, в трее корзина выглядит
//! ровно так же, как на столе, — включая случай, когда человек подменил её
//! значок сам. Второе: три переключателя очистки не изображаются нами, а
//! отдаются `SHEmptyRecycleBin` — это её собственные флаги, и ведёт она себя
//! в точности как проводник.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::LazyLock;
use std::time::Duration;

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

/// Опознавательный номер значка. Свой, не из набора наборов.
const BIN_ID: &str = "qrx.bin";

const OPEN_ITEM: &str = "qrx.bin.open";
const EMPTY_ITEM: &str = "qrx.bin.empty";
const SETTINGS_ITEM: &str = "qrx.bin.settings";

/// Ступень наполнения, показанная сейчас. Чтобы не трогать значок зря.
///
/// Windows перерисовывает значок на каждое присвоение, и делать это дважды в
/// секунду при неизменной корзине — мигание на ровном месте.
static SHOWN: AtomicU8 = AtomicU8::new(u8::MAX);

/// Что показано в подписи сейчас: объём и число предметов.
///
/// Отдельно от ступени, и это не мелочь. Ступень меняется редко — четверть
/// корзины надо набрать, — а подпись говорит точный размер, и он меняется от
/// каждого удалённого файла. Пока обновление висело на одной ступени, подпись
/// подолгу показывала вчерашний размер.
static TOLD: LazyLock<std::sync::Mutex<(u64, u64)>> =
    LazyLock::new(|| std::sync::Mutex::new((u64::MAX, u64::MAX)));

/// Идёт ли уже наблюдение. Поток нужен один на всё приложение.
static WATCHING: AtomicBool = AtomicBool::new(false);

/// Слепок настроек, при которых значок собран. По нему видно, надо ли пересобирать.
static BUILT: LazyLock<std::sync::Mutex<String>> =
    LazyLock::new(|| std::sync::Mutex::new(String::new()));

/// Заводит или снимает значок корзины по нынешним настройкам.
pub fn refresh(app: &AppHandle) {
    let bin = settings(app);

    if !bin.tray {
        app.remove_tray_by_id(BIN_ID);

        if let Ok(mut slot) = BUILT.lock() {
            slot.clear();
        }

        SHOWN.store(u8::MAX, Ordering::Relaxed);
        return;
    }

    let now = signature(&bin);

    if BUILT.lock().is_ok_and(|before| *before == now) {
        // Настройки те же — но наполнение могло измениться, и значок надо
        // подтянуть без пересоздания: пересозданный Windows считает новым и
        // прячет за стрелочку переполнения
        paint(app, true);
        return;
    }

    app.remove_tray_by_id(BIN_ID);
    SHOWN.store(u8::MAX, Ordering::Relaxed);

    let state = crate::basket::state();
    let step = level(app, &state, &bin);

    let built = TrayIconBuilder::with_id(BIN_ID)
        .icon(icon_for(app, &bin, step))
        .tooltip(hint(app, &state))
        .menu(&menu(app))
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu)
        .on_tray_icon_event(on_click)
        .build(app);

    match built {
        Ok(_) => {
            SHOWN.store(step, Ordering::Relaxed);

            if let Ok(mut slot) = BUILT.lock() {
                *slot = now;
            }

            watch(app.clone());
        }
        Err(e) => eprintln!("значок корзины не заведён: {e}"),
    }
}

/// То, от чего зависит вид значка. Наполнение сюда не входит: оно меняется
/// само по себе, и пересобирать значок под него нельзя.
fn signature(bin: &crate::paths::Bin) -> String {
    format!("{}\u{1}{}\u{1}{}", bin.steps, bin.click, bin.icons.join("\u{2}"))
}

/// Пересобирает меню под сменившийся язык.
///
/// Пункты меню рисует система по тем строкам, что ей отдали при заведении, —
/// сами они не переведутся. Вызывается из общей рассылки о смене языка, там же,
/// где окна перечитывают свои словари.
pub fn relabel(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(BIN_ID) else {
        return;
    };

    let _ = tray.set_menu(Some(menu(app)));

    // Подпись тоже на выбранном языке — заставляем перечитать её заново
    if let Ok(mut told) = TOLD.lock() {
        *told = (u64::MAX, u64::MAX);
    }

    paint(app, false);
}

fn settings(app: &AppHandle) -> crate::paths::Bin {
    crate::paths::load_or_create_config(app)
        .map(|config| config.bin)
        .unwrap_or_default()
}

/// Наблюдение за корзиной.
///
/// Опросом, а не подпиской на извещения оболочки. Подписка требует своего окна
/// с очередью сообщений и снимается при перезапуске проводника — ради значка,
/// который меняется раз в час, это лишняя машинерия. Опрос же стоит одного
/// обращения к кэшу оболочки: `SHQueryRecycleBin` не обходит папку, а читает
/// её сводку.
fn watch(app: AppHandle) {
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1500));

        // Значок сняли — наблюдать не за чем, но поток оставляем: включат
        // обратно, и он подхватит без перезапуска приложения
        if app.tray_by_id(BIN_ID).is_some() {
            paint(&app, false);
        }
    });
}

/// Подтягивает вид значка под нынешнее наполнение.
///
/// `force` — обновить подпись даже при той же ступени: при пересборке она
/// могла остаться от прошлого состояния.
fn paint(app: &AppHandle, force: bool) {
    let Some(tray) = app.tray_by_id(BIN_ID) else {
        return;
    };

    let bin = settings(app);
    let state = crate::basket::state();
    let step = level(app, &state, &bin);

    // Картинка — только при смене ступени: Windows перерисовывает значок на
    // каждое присвоение, и делать это раз в полторы секунды впустую значит
    // заставить его мигать
    if force || SHOWN.load(Ordering::Relaxed) != step {
        let _ = tray.set_icon(Some(icon_for(app, &bin, step)));
        SHOWN.store(step, Ordering::Relaxed);
    }

    // Подпись — при любом изменении содержимого: она называет точный размер, и
    // он меняется от каждого удалённого файла, а ступень при этом стоит на месте
    let now = (state.bytes, state.items);
    let stale = TOLD.lock().map(|told| *told != now).unwrap_or(true);

    if force || stale {
        let _ = tray.set_tooltip(Some(hint(app, &state)));

        if let Ok(mut told) = TOLD.lock() {
            *told = now;
        }
    }
}

/// Ступень наполнения: 0 — пусто, 4 — полна.
///
/// Считается от предела, который назначен корзине в самой Windows, — того же,
/// что показан в панели. Брать за предел размер диска было бы неверно: корзина
/// перестаёт принимать файлы задолго до его конца.
///
/// Непустая корзина никогда не показывается пустой, даже когда занято доли
/// процента: весь смысл значка в том, чтобы это было видно.
fn level(app: &AppHandle, state: &crate::basket::BinState, bin: &crate::paths::Bin) -> u8 {
    if state.items == 0 {
        return 0;
    }

    if !bin.steps {
        return 4;
    }

    let limit = crate::basket::limit_bytes(app);

    if limit == 0 {
        return 4;
    }

    let share = state.bytes as f64 / limit as f64;

    match share {
        s if s >= 0.75 => 4,
        s if s >= 0.5 => 3,
        s if s >= 0.25 => 2,
        _ => 1,
    }
}

fn hint(_app: &AppHandle, state: &crate::basket::BinState) -> String {
    let words = labels();

    if state.items == 0 {
        return words.empty;
    }

    format!(
        "{}: {} · {} {}",
        words.title,
        crate::basket::human(state.bytes, &words.units),
        state.items,
        words.items
    )
}

/// Слова для трея, как их прислал интерфейс.
///
/// Присланные, а не свои. Переводы приложения живут в `language.ts` — там же,
/// где все прочие строки, — и второй копии у них быть не должно: она разошлась
/// бы с первой на первой же правке.
///
/// Прошлая попытка так и сделала: маленькая таблица здесь и выбор по
/// `config.language`. Не работало вовсе — язык хранится в `localStorage`
/// окна, а в настройки его не пишет никто, и выбор всегда падал на значение
/// по умолчанию.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct Labels {
    /// Подпись пустой корзины.
    pub empty: String,
    /// Заголовок подписи у непустой.
    pub title: String,
    /// Единица счёта предметов.
    pub items: String,
    pub open: String,
    pub clear: String,
    pub settings: String,
    /// Обозначения размера от байтов до терабайтов — пять штук по порядку.
    ///
    /// Приходят оттуда же, откуда все прочие слова. Прежде они были зашиты
    /// здесь кириллицей, и подсказка значка говорила «1,5 ГБ» на любом языке.
    pub units: Vec<String>,
}

impl Default for Labels {
    /// Запасные — на первую секунду, пока ни одно окно не поднялось.
    ///
    /// По-английски, потому что таков язык приложения по умолчанию: именно его
    /// выбирает `i18n`, когда в хранилище ничего нет. Живут они ровно до
    /// первой присылки и после неё не возвращаются.
    fn default() -> Self {
        Self {
            empty: "The recycle bin is empty".into(),
            title: "Recycle bin".into(),
            items: "items".into(),
            open: "Open the recycle bin".into(),
            clear: "Empty the recycle bin".into(),
            settings: "Recycle bin settings".into(),
            units: ["B", "KB", "MB", "GB", "TB"].map(Into::into).into(),
        }
    }
}

static LABELS: LazyLock<std::sync::Mutex<Labels>> =
    LazyLock::new(|| std::sync::Mutex::new(Labels::default()));

fn labels() -> Labels {
    LABELS.lock().map(|slot| slot.clone()).unwrap_or_default()
}

/// Принимает переводы от интерфейса.
///
/// Зовётся при каждом показе окна и при каждой смене языка — реактивно, из
/// общей обвязки всех окон. Одинаковое приходит часто, поэтому сравниваем:
/// пересобирать меню на каждое появление окна незачем.
#[tauri::command]
pub fn bin_set_labels(app: AppHandle, labels: Labels) {
    let changed = LABELS
        .lock()
        .map(|mut slot| {
            let differs = *slot != labels;
            *slot = labels;
            differs
        })
        .unwrap_or(false);

    if changed {
        relabel(&app);
    }
}

/// Значок под ступень: свой, если назначен, иначе собранный из значков оболочки.
fn icon_for(
    app: &AppHandle,
    bin: &crate::paths::Bin,
    step: u8,
) -> tauri::image::Image<'static> {
    // 32 — размер, в котором Windows рисует значок в панели
    const SIDE: u32 = 32;

    if let Some(own) = bin.icons.get(step as usize).map(String::as_str) {
        if !own.trim().is_empty() {
            if let Some((pixels, side)) = crate::icons::raster(own, SIDE) {
                return tauri::image::Image::new_owned(pixels, side, side);
            }
        }
    }

    let (empty, full) = crate::basket::shell_icons();

    // Крайние ступени — значки оболочки как есть: в трее корзина должна
    // выглядеть точно так же, как на рабочем столе
    let source = if step == 0 { &empty } else { &full };

    let Some((pixels, side)) = crate::icons::raster(source, SIDE) else {
        return fallback(app);
    };

    if step == 0 || step == 4 {
        return tauri::image::Image::new_owned(pixels, side, side);
    }

    tauri::image::Image::new_owned(with_gauge(pixels, side, step), side, side)
}

/// Дорисовывает полоску наполнения понизу значка.
///
/// Своей картинки под четверть и половину у нас нет и взяться ей неоткуда:
/// оболочка знает только «пусто» и «полно». Рисовать три собственные корзины
/// значило бы поставить рядом с системными свои, нарисованные иначе, — в трее
/// это сразу видно.
///
/// Поэтому промежуточные ступени — тот же значок полной корзины и мера под
/// ним. Читается она и в шестнадцати точках: глаз ловит длину полоски, а не
/// рисунок.
fn with_gauge(mut pixels: Vec<u8>, side: u32, step: u8) -> Vec<u8> {
    // Золото приложения — то же, что на полосах в окне
    const INK: [u8; 3] = [221, 170, 29];

    let side = side as usize;
    let thickness = (side / 8).max(2);
    let filled = side * step as usize / 4;

    for row in side.saturating_sub(thickness)..side {
        for column in 0..side {
            let at = (row * side + column) * 4;

            if at + 3 >= pixels.len() {
                continue;
            }

            // Незанятая часть меры — тёмная подложка, иначе на светлом фоне
            // полоска обрывается в никуда и читается как обрезанный значок
            let (ink, alpha) = if column < filled {
                (INK, 255u8)
            } else {
                ([0, 0, 0], 90u8)
            };

            pixels[at] = ink[0];
            pixels[at + 1] = ink[1];
            pixels[at + 2] = ink[2];
            pixels[at + 3] = alpha;
        }
    }

    pixels
}

fn fallback(app: &AppHandle) -> tauri::image::Image<'static> {
    tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))
        .map(|image| image.to_owned())
        .unwrap_or_else(|_| {
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

/// Меню правого нажатия. Ровно те же три пункта, что у MiniBin.
fn menu(app: &AppHandle) -> Menu<tauri::Wry> {
    let words = labels();

    let open = MenuItem::with_id(app, OPEN_ITEM, &words.open, true, None::<&str>);
    let empty = MenuItem::with_id(app, EMPTY_ITEM, &words.clear, true, None::<&str>);
    let settings = MenuItem::with_id(app, SETTINGS_ITEM, &words.settings, true, None::<&str>);

    let split = PredefinedMenuItem::separator(app);

    let items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> =
        match (open, empty, split, settings) {
            (Ok(open), Ok(empty), Ok(split), Ok(settings)) => vec![
                Box::new(open),
                Box::new(empty),
                Box::new(split),
                Box::new(settings),
            ],
            _ => Vec::new(),
        };

    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|item| item.as_ref()).collect();

    Menu::with_items(app, &refs).expect("меню корзины не собралось")
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        OPEN_ITEM => open(app),
        EMPTY_ITEM => clear(app),
        SETTINGS_ITEM => crate::tray::open_bin_settings(app),
        _ => {}
    }
}

/// Нажатие по значку — то, что назначено в настройках.
fn on_click(tray: &TrayIcon, event: TrayIconEvent) {
    let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    else {
        return;
    };

    let app = tray.app_handle();

    if settings(app).click == "empty" {
        clear(app);
    } else {
        open(app);
    }
}

fn open(app: &AppHandle) {
    if let Err(e) = crate::basket::tray_basket_open() {
        eprintln!("корзина не открылась: {e}");
    }

    // Ход мог измениться сразу — но проводник пересчитывает не мгновенно
    let app = app.clone();

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        paint(&app, true);
    });
}

fn clear(app: &AppHandle) {
    let bin = settings(app);
    let app = app.clone();

    /*
     * В отдельном потоке, и это обязательно.
     *
     * С подтверждением и окном хода оболочка возвращает управление только
     * после ответа человека. На потоке приложения это означало бы застывший
     * интерфейс всё время, пока висит вопрос.
     */
    std::thread::spawn(move || {
        if let Err(e) = crate::basket::empty(bin.confirm, bin.sound, bin.progress) {
            eprintln!("корзина не очистилась: {e}");
        }

        std::thread::sleep(Duration::from_millis(400));
        paint(&app, true);
    });
}
