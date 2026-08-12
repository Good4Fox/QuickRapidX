//! Изоляция запуска — то же по смыслу, что Knox у Samsung.
//!
//! Knox не перехватывает обращения к файлам. Он заводит **отдельный профиль
//! пользователя**, а разграничение держит само ядро Android: свои каталоги
//! данных, свои экземпляры приложений, правила SELinux. Работает это потому,
//! что многопользовательская изоляция встроена в систему.
//!
//! У Windows есть встроенный механизм того же рода — AppContainer, тот самый,
//! которым изолированы приложения из Store. Он даёт:
//!
//! * свой SID, от имени которого работает процесс;
//! * доступ только к тому, что этому SID выдано явно, — остальное закрыто
//!   правами файловой системы, а не подменой путей;
//! * свою папку данных внутри профиля;
//! * сеть только при выданной возможности — без неё соединений не будет вовсе.
//!
//! Прав администратора всё это не требует, драйвера тоже: границу держит ядро
//! Windows.
//!
//! Чего оно не даёт: незаметности. Программа видит, что работает в
//! AppContainer, — это открытый признак её маркёра доступа. Скрывать его
//! нечем без драйвера, и обещать этого мы не станем.
//!
//! Важное ограничение по применимости: обычные настольные программы внутри
//! AppContainer работают не все. Тот, кто рассчитывает на полный доступ к
//! системе, просто откажется запускаться. Поэтому запуск в изоляции —
//! отдельное действие, а не единственный способ.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Что вышло из попытки запуска.
#[derive(Serialize)]
pub struct Launch {
    pub ok: bool,
    pub message: String,
}

/// Имя контейнера. Одно на приложение: контейнер и есть наше Место.
const NAME: &str = "QuickRapidX.Space";

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Заводит контейнер, если его ещё нет, и отдаёт его SID строкой.
///
/// Повторный вызов не ошибка: система вернёт отказ «уже есть», и мы просто
/// спросим готовый SID.
#[tauri::command]
pub fn container_ensure() -> Result<String, String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Isolation::CreateAppContainerProfile;

        let name = wide(NAME);
        let display = wide("QuickRapidX");
        let about = wide("Изолированный запуск программ QuickRapidX");

        let mut sid = std::ptr::null_mut();

        // SAFETY: все строки завершены нулём и живут до конца вызова.
        let hr = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                display.as_ptr(),
                about.as_ptr(),
                std::ptr::null(),
                0,
                &mut sid,
            )
        };

        // 0x800700B7 — «уже существует». Это не отказ, а обычный второй запуск
        if hr < 0 && hr != 0x8007_00B7u32 as i32 {
            return Err(format!("не удалось завести контейнер: 0x{hr:08X}"));
        }

        if !sid.is_null() {
            let text = sid_to_string(sid);
            free_sid(sid);
            return text;
        }

        derive_sid()
    }

    #[cfg(not(windows))]
    {
        Err("не поддерживаемая операционная система".into())
    }
}

/// Убирает контейнер вместе с его папкой данных.
#[tauri::command]
pub fn container_remove() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Isolation::DeleteAppContainerProfile;

        let name = wide(NAME);

        // SAFETY: строка завершена нулём и живёт до конца вызова.
        let hr = unsafe { DeleteAppContainerProfile(name.as_ptr()) };

        if hr < 0 {
            return Err(format!("не удалось убрать контейнер: 0x{hr:08X}"));
        }

        Ok(())
    }

    #[cfg(not(windows))]
    {
        Err("не поддерживаемая операционная система".into())
    }
}

/// Открывает контейнеру доступ к папке.
///
/// Без этого он не увидит даже ту программу, которую ему предстоит запустить:
/// AppContainer по умолчанию не имеет прав нигде, кроме своей папки данных.
/// Права выдаются файловой системой — это и есть граница, а не подмена путей.
#[tauri::command]
pub fn container_grant(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        grant(Path::new(&path))
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        Err("не поддерживаемая операционная система".into())
    }
}

/// Возможность, которую можно выдать программе.
///
/// Это известные SID из состава Windows, а не наша выдумка: те же самые
/// возможности запрашивают приложения из Store. Каждая — отдельное право, и
/// невыданное отсутствует у процесса вовсе, а не просто запрещено правилом.
#[derive(Serialize)]
pub struct Capability {
    pub id: &'static str,
    pub sid: &'static str,
}

/// Набор возможностей. Устройства (камера, микрофон) сюда не входят: их SID
/// вычисляются хешированием имени, и без отдельной работы их не собрать.
pub const CAPABILITIES: &[Capability] = &[
    Capability { id: "internet", sid: "S-1-15-3-1" },
    Capability { id: "internet_server", sid: "S-1-15-3-2" },
    Capability { id: "private_network", sid: "S-1-15-3-3" },
    Capability { id: "documents", sid: "S-1-15-3-4" },
    Capability { id: "pictures", sid: "S-1-15-3-5" },
    Capability { id: "videos", sid: "S-1-15-3-6" },
    Capability { id: "music", sid: "S-1-15-3-7" },
    Capability { id: "removable", sid: "S-1-15-3-10" },
];

/// Что вообще можно выдать — интерфейс строит список по этому ответу.
#[tauri::command]
pub fn container_capabilities() -> Vec<&'static str> {
    CAPABILITIES.iter().map(|capability| capability.id).collect()
}

/// Роль программы — готовый набор возможностей под привычную задачу.
///
/// Идея взята у Bottles: там среда заводится выбором «игра / приложение / своё»,
/// а не перечислением библиотек. Разбираться в правах доступа не обязан никто,
/// а вот сказать «это браузер» может каждый. Набор при этом остаётся видимым:
/// выбор роли просто отмечает те же возможности, что доступны вручную.
#[derive(Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub capabilities: &'static [&'static str],
}

pub const PRESETS: &[Preset] = &[
    // Браузеру нужен выход наружу и папка загрузок
    Preset {
        id: "browser",
        capabilities: &["internet", "documents"],
    },
    // Общение: сеть плюс то, что обычно отправляют собеседнику
    Preset {
        id: "messenger",
        capabilities: &["internet", "pictures", "documents"],
    },
    // Просмотр и прослушивание — без сети вовсе
    Preset {
        id: "viewer",
        capabilities: &["pictures", "videos", "music"],
    },
    // Работа с документами: файлы да, сеть нет
    Preset {
        id: "office",
        capabilities: &["documents"],
    },
    // Игра: сеть наружу и по домашней сети, файлы ни к чему
    Preset {
        id: "game",
        capabilities: &["internet", "private_network"],
    },
    // Ничего. Самый строгий и самый безопасный вариант
    Preset {
        id: "offline",
        capabilities: &[],
    },
    /*
     * Всё, что вообще можно выдать.
     *
     * Заведена не для безопасности, а ради того, чтобы программа просто
     * запустилась. Наша среда на AppContainer отказывает обычным настольным
     * программам в самых неожиданных местах, и человеку, который хочет
     * попробовать изоляцию, а не изучать её, нужен вариант «сними всё, что
     * снимается». Граница при этом остаётся: своя файловая система, свой
     * реестр, чужие окна и процессы по-прежнему недоступны.
     */
    Preset {
        id: "free",
        capabilities: &[
            "internet",
            "internet_server",
            "private_network",
            "documents",
            "pictures",
            "videos",
            "music",
            "removable",
        ],
    },
];

/// Готовые роли — интерфейс строит выбор по этому ответу.
#[tauri::command]
pub fn container_presets() -> &'static [Preset] {
    PRESETS
}

/// Запускает программу в изоляции.
///
/// `capabilities` — что ей разрешено: браузеру нужен интернет, просмотрщику
/// изображений — библиотека изображений, а большинству не нужно ничего.
/// Невыданное отсутствует у процесса совсем.
///
/// `folders` — папки, которые ей открывают дополнительно. Программе всегда
/// открывается её собственная папка; всё прочее — только по списку.
///
/// `args` и `env` — ключи командной строки и добавка к окружению. Нужны там,
/// где программу заводят особым образом: своим профилем, своей папкой данных,
/// отключённым ускорением. Окружение наследуется от нас и дополняется, а не
/// заменяется: без системных переменных не заработает почти ничто.
#[tauri::command]
pub fn container_launch(
    app: tauri::AppHandle,
    exe: String,
    capabilities: Vec<String>,
    folders: Vec<String>,
    args: Option<String>,
    env: Option<Vec<crate::paths::EnvVar>>,
    engine: Option<String>,
    sandbox: Option<String>,
) -> Launch {
    #[cfg(windows)]
    {
        let args = args.unwrap_or_default();
        let env = env.unwrap_or_default();

        /*
         * Чем изолировать — выбор человека.
         *
         * Sandboxie надёжнее: у неё драйвер ядра, он подменяет обращения к
         * файлам и реестру, и программа внутри живёт как в обычной системе.
         * Наша среда на AppContainer не требует ничего ставить, но обычная
         * настольная программа то и дело упирается в то, чего AppContainer не
         * пускает.
         */
        let chosen = engine.as_deref().unwrap_or("own");

        if chosen == "wsandbox" {
            // Ярлык разворачиваем и здесь: песочница получит настоящую
            // программу, а не ссылку на неё
            let (target, extra) = match resolve(Path::new(&exe)) {
                Some((path, own)) => (path, own),
                None => (exe.clone(), String::new()),
            };

            let joined = match (extra.trim(), args.trim()) {
                ("", rest) => rest.to_string(),
                (first, "") => first.to_string(),
                (first, rest) => format!("{first} {rest}"),
            };

            // Право на сеть здесь то же самое, что и у нашей среды: роль без
            // интернета получает песочницу без сети
            let internet = capabilities.iter().any(|value| value == "internet");

            return match crate::wsandbox::launch(&app, &target, &joined, internet, &folders) {
                Ok(()) => Launch {
                    ok: true,
                    message: String::new(),
                },
                Err(message) => Launch { ok: false, message },
            };
        }

        if chosen == "sandboxie" {
            // Ярлык разворачиваем и здесь: Sandboxie его поймёт, но ключи из
            // него тогда потеряются
            let (target, extra) = match resolve(Path::new(&exe)) {
                Some((path, own)) => (path, own),
                None => (exe.clone(), String::new()),
            };

            let joined = match (extra.trim(), args.trim()) {
                ("", rest) => rest.to_string(),
                (first, "") => first.to_string(),
                (first, rest) => format!("{first} {rest}"),
            };

            return match crate::sandboxie::launch(
                &target,
                &joined,
                sandbox.as_deref().unwrap_or_default(),
            ) {
                Ok(()) => Launch {
                    ok: true,
                    message: String::new(),
                },
                Err(message) => Launch { ok: false, message },
            };
        }

        match launch(Some(&app), &exe, &capabilities, &folders, &args, &env) {
            Ok(()) => Launch {
                ok: true,
                message: String::new(),
            },
            Err(message) => Launch { ok: false, message },
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (app, exe, capabilities, folders, args, env, engine, sandbox);
        Launch {
            ok: false,
            message: "не поддерживаемая операционная система".into(),
        }
    }
}

/// Настройки изоляции для программы. Пусто — ничего не выдано.
#[tauri::command]
pub fn container_profile(app: tauri::AppHandle, target: String) -> crate::paths::Isolation {
    crate::paths::load_or_create_config(&app)
        .ok()
        .and_then(|config| {
            config
                .isolation
                .into_iter()
                .find(|profile| profile.target == target)
        })
        .unwrap_or(crate::paths::Isolation {
            target,
            preset: String::new(),
            engine: String::new(),
            sandbox: String::new(),
            capabilities: Vec::new(),
            folders: Vec::new(),
            exe: String::new(),
            args: String::new(),
            env: Vec::new(),
        })
}

/// Все настройки разом.
///
/// Список каталога спрашивает их один раз, а не по записи: иначе на каждую
/// строку уходил бы отдельный вызов, а читается всё равно один и тот же файл.
#[tauri::command]
pub fn container_profiles(app: tauri::AppHandle) -> Vec<crate::paths::Isolation> {
    crate::paths::load_or_create_config(&app)
        .map(|config| config.isolation)
        .unwrap_or_default()
}

/// Настройки по опознавательному номеру записи.
///
/// Пустой набор прав от «ничего не задано» отличается: первое значит «запускать
/// без прав», второе — «посмотри в другом месте».
pub fn profile_named(app: &tauri::AppHandle, target: &str) -> Option<crate::paths::Isolation> {
    crate::paths::load_or_create_config(app)
        .ok()?
        .isolation
        .into_iter()
        .find(|profile| profile.target == target)
}

/// Настройки по пути к файлу.
///
/// Наборы ярлыков знают программу по пути, а не по записи каталога. Если для
/// этого же файла права уже заведены — берём их, чтобы «в изоляции» из набора
/// и из каталога означало одно и то же.
pub fn profile_for_exe(app: &tauri::AppHandle, exe: &str) -> Option<crate::paths::Isolation> {
    let wanted = exe.trim().to_lowercase();

    crate::paths::load_or_create_config(app)
        .ok()?
        .isolation
        .into_iter()
        .find(|profile| profile.exe.trim().to_lowercase() == wanted)
}

/// Запоминает настройки изоляции программы.
#[tauri::command]
pub fn container_set_profile(
    app: tauri::AppHandle,
    profile: crate::paths::Isolation,
) -> Result<(), String> {
    let mut config = crate::paths::load_or_create_config(&app)?;

    match config
        .isolation
        .iter_mut()
        .find(|item| item.target == profile.target)
    {
        Some(existing) => *existing = profile,
        None => config.isolation.push(profile),
    }

    crate::paths::write_config(&app, &config)
}

/// Запускает программу с её собственными настройками.
///
/// `exe` можно не передавать: если за записью уже закреплён файл, берётся он.
#[tauri::command]
pub fn container_run(app: tauri::AppHandle, target: String, exe: Option<String>) -> Launch {
    let profile = container_profile(app.clone(), target);

    let path = exe
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| profile.exe.clone());

    if path.trim().is_empty() {
        return Launch {
            ok: false,
            message: "для этой записи не выбран файл программы".into(),
        };
    }

    container_launch(
        app,
        path,
        profile.capabilities,
        profile.folders,
        Some(profile.args),
        Some(profile.env),
        Some(profile.engine),
        Some(profile.sandbox),
    )
}

// ── Запуск по ярлыку ─────────────────────────────────────────────────────
//
// Bottles умеет положить программу из своей среды ярлыком на рабочий стол:
// дальше она запускается как обычная, и о самом Bottles вспоминать не нужно.
// Здесь так же — ярлык ведёт на нас с ключом `--isolate=<запись>`, мы
// поднимаем программу в изоляции и завершаемся, не показав окна.

/// Ключ командной строки, по которому узнаётся запуск из ярлыка.
pub const ISOLATE_FLAG: &str = "--isolate=";

/// Запускает программу из ярлыка и возвращает управление сразу.
///
/// Tauri здесь не поднята: настройки читаются с диска напрямую. Ошибку показать
/// некуда — окна нет, — поэтому она уходит окном сообщения системы.
pub fn run_standalone(target: &str) {
    let Some(config) = crate::paths::load_config_standalone() else {
        report("настройки не прочитаны");
        return;
    };

    let Some(profile) = config
        .isolation
        .into_iter()
        .find(|item| item.target == target)
    else {
        report("для этого ярлыка нет настроек изоляции");
        return;
    };

    if profile.exe.trim().is_empty() {
        report("в настройках не выбран файл программы");
        return;
    }

    #[cfg(windows)]
    {
        // Среда могла быть убрана после того, как ярлык создали
        let _ = container_ensure();

        if let Err(message) = launch(
            None,
            &profile.exe,
            &profile.capabilities,
            &profile.folders,
            &profile.args,
            &profile.env,
        ) {
            report(&message);
        }
    }
}

/// Кладёт ярлык изолированного запуска на рабочий стол.
///
/// Через WScript.Shell, а не через COM напрямую: тот же результат тремя
/// строками вместо ручной работы с IShellLink и IPersistFile — так же, как
/// сделано с значками ярлыков.
#[tauri::command]
pub fn container_shortcut(
    app: tauri::AppHandle,
    target: String,
    name: String,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let profile = container_profile(app, target.clone());

        if profile.exe.trim().is_empty() {
            return Err("сначала выберите файл программы".into());
        }

        let desktop = PathBuf::from(std::env::var("USERPROFILE").map_err(|_| "профиль не найден")?)
            .join("Desktop");

        if !desktop.is_dir() {
            return Err("рабочий стол не найден".into());
        }

        let lnk = desktop.join(format!("{}.lnk", safe_name(&name)?));

        let me = std::env::current_exe()
            .map_err(|e| format!("свой путь не определён: {e}"))?
            .display()
            .to_string();

        // Одинарные кавычки внутри PowerShell удваиваются — иначе строка
        // оборвётся посреди пути и остаток уйдёт в команду
        let quote = |value: &str| value.replace('\'', "''");

        let script = format!(
            "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); \
             $s.TargetPath = '{}'; $s.Arguments = '{}{}'; $s.IconLocation = '{},0'; \
             $s.Description = 'QuickRapidX'; $s.Save()",
            quote(&lnk.display().to_string()),
            quote(&me),
            ISOLATE_FLAG,
            quote(&target),
            quote(&profile.exe),
        );

        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("ярлык не создан: {e}"))?;

        if !status.success() {
            return Err("ярлык не создан".into());
        }

        Ok(lnk.display().to_string())
    }

    #[cfg(not(windows))]
    {
        let _ = (app, target, name);
        Err("не поддерживаемая операционная система".into())
    }
}

/// Показывает ошибку окном системы: при запуске из ярлыка писать её некуда.
fn report(message: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

        let text = wide(message);
        let title = wide("QuickRapidX");

        // SAFETY: обе строки завершены нулём и живут до конца вызова.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    #[cfg(not(windows))]
    eprintln!("{message}");
}

// ── Снимки среды ─────────────────────────────────────────────────────────
//
// Ещё одна мысль от Bottles: там у среды есть версии, и перед тем как поставить
// внутрь что-то сомнительное, делается снимок, а если сломалось — откат. У нас
// откатывать есть что: у контейнера своя папка данных, и всё, что программа
// пишет «в систему», на деле ложится туда. Снимок — её копия, откат — возврат
// копии на место.
//
// Это же и есть перенос среды на другую машину: папка складывается один раз, а
// разворачивается где угодно.

/// Снимок папки данных контейнера.
#[derive(Serialize)]
pub struct Snapshot {
    pub name: String,
    /// Сколько занимает, байт.
    pub size: u64,
    /// Когда сделан, секунд от начала эпохи. Формат даты — дело интерфейса.
    pub made: u64,
}

/// Заведена ли среда и где она.
///
/// SID выводится из имени и существует всегда — по нему о среде не судить.
/// Признак — папка данных: её создаёт система при заведении контейнера.
#[derive(Serialize)]
pub struct Status {
    pub on: bool,
    pub sid: String,
    pub data: String,
}

/// Состояние среды на момент открытия экрана.
#[tauri::command]
pub fn container_status() -> Status {
    #[cfg(windows)]
    {
        let sid = derive_sid().unwrap_or_default();
        let data = data_dir().unwrap_or_default();
        let on = !data.is_empty() && Path::new(&data).is_dir();

        Status { on, sid, data }
    }

    #[cfg(not(windows))]
    {
        Status {
            on: false,
            sid: String::new(),
            data: String::new(),
        }
    }
}

/// Где лежит папка данных контейнера.
///
/// Это не наша выдумка и не подменённый путь: система сама заводит контейнеру
/// каталог внутри профиля и сама же говорит, где он.
#[tauri::command]
pub fn container_data() -> Result<String, String> {
    #[cfg(windows)]
    {
        data_dir()
    }

    #[cfg(not(windows))]
    {
        Err("не поддерживаемая операционная система".into())
    }
}

/// Складывает нынешнее состояние среды в снимок.
#[tauri::command]
pub async fn container_snapshot(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let store = snapshots_dir(&app)?;
    let safe = safe_name(&name)?;
    let target = store.join(&safe);

    if target.exists() {
        return Err("снимок с таким именем уже есть".into());
    }

    let source = container_data()?;

    tauri::async_runtime::spawn_blocking(move || {
        crate::library::copy_tree(Path::new(&source), &target)?;
        Ok(safe)
    })
    .await
    .map_err(|e| format!("снимок не сделан: {e}"))?
}

/// Какие снимки уже есть.
#[tauri::command]
pub fn container_snapshots(app: tauri::AppHandle) -> Result<Vec<Snapshot>, String> {
    let store = snapshots_dir(&app)?;

    let Ok(entries) = std::fs::read_dir(&store) else {
        return Ok(Vec::new());
    };

    let mut list: Vec<Snapshot> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            let path = entry.path();

            let made = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_secs())
                .unwrap_or(0);

            Snapshot {
                name: entry.file_name().to_string_lossy().to_string(),
                size: tree_size(&path),
                made,
            }
        })
        .collect();

    // Свежие сверху: откатываются обычно к последнему
    list.sort_by(|a, b| b.made.cmp(&a.made));

    Ok(list)
}

/// Возвращает среду к состоянию снимка.
///
/// Нынешнее содержимое папки данных при этом пропадает — поэтому перед
/// откатом интерфейс спрашивает, а само действие отдельной кнопкой.
#[tauri::command]
pub async fn container_snapshot_restore(
    app: tauri::AppHandle,
    name: String,
) -> Result<(), String> {
    let source = snapshots_dir(&app)?.join(safe_name(&name)?);

    if !source.is_dir() {
        return Err("такого снимка нет".into());
    }

    let target = container_data()?;

    tauri::async_runtime::spawn_blocking(move || {
        let target = Path::new(&target);

        // Оговорка на случай, если система однажды вернёт не то: чистим только
        // то, что и правда лежит в каталоге контейнеров
        if !target.to_string_lossy().contains("Packages") {
            return Err("папка контейнера определена неверно — откат отменён".into());
        }

        // Убирается содержимое, а не сама папка: её права выданы контейнеру при
        // заведении, и новая, созданная нами, была бы для него закрыта
        clear_dir(target)?;

        crate::library::copy_tree(&source, target)
    })
    .await
    .map_err(|e| format!("откат не завершён: {e}"))?
}

/// Убирает снимок.
#[tauri::command]
pub fn container_snapshot_forget(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let path = snapshots_dir(&app)?.join(safe_name(&name)?);

    if !path.is_dir() {
        return Ok(());
    }

    std::fs::remove_dir_all(&path).map_err(|e| format!("снимок не убран: {e}"))
}

/// Выкладывает снимок наружу — на флешку или в Место.
///
/// Это и есть перенос среды на другую машину: там снимок вносится обратно и
/// разворачивается. Ничего сверх копии папки здесь не происходит.
#[tauri::command]
pub async fn container_snapshot_export(
    app: tauri::AppHandle,
    name: String,
    folder: String,
) -> Result<String, String> {
    let safe = safe_name(&name)?;
    let source = snapshots_dir(&app)?.join(&safe);

    if !source.is_dir() {
        return Err("такого снимка нет".into());
    }

    let target = PathBuf::from(&folder).join(format!("QuickRapidX {safe}"));

    if target.exists() {
        return Err("там уже есть папка с таким именем".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        crate::library::copy_tree(&source, &target)?;
        Ok(target.display().to_string())
    })
    .await
    .map_err(|e| format!("выгрузка не завершена: {e}"))?
}

/// Вносит снимок с другой машины.
#[tauri::command]
pub async fn container_snapshot_import(
    app: tauri::AppHandle,
    folder: String,
    fallback: String,
) -> Result<String, String> {
    let source = PathBuf::from(&folder);

    if !source.is_dir() {
        return Err("папки нет".into());
    }

    let name = source
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        // Запасное имя приходит из словаря: оно становится именем папки на
        // диске и подписью в списке, то есть остаётся у человека навсегда
        .unwrap_or_else(|| fallback.trim().to_string());

    let safe = safe_name(&name)?;
    let target = snapshots_dir(&app)?.join(&safe);

    if target.exists() {
        return Err("снимок с таким именем уже есть".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        crate::library::copy_tree(&source, &target)?;
        Ok(safe)
    })
    .await
    .map_err(|e| format!("загрузка не завершена: {e}"))?
}

fn snapshots_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let path = crate::paths::data_dir(app)?.join("snapshots");

    std::fs::create_dir_all(&path).map_err(|e| format!("не удалось создать {}: {e}", path.display()))?;

    Ok(path)
}

/// Имя снимка становится именем папки, поэтому пропускаем только безобидное.
fn safe_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("имя снимка не указано".into());
    }

    let cleaned: String = trimmed
        .chars()
        .map(|symbol| {
            if symbol.is_alphanumeric() || symbol == ' ' || symbol == '-' || symbol == '_' {
                symbol
            } else {
                '_'
            }
        })
        .collect();

    Ok(cleaned.chars().take(64).collect())
}

/// Убирает всё внутри папки, оставляя саму папку.
fn clear_dir(path: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let inside = entry.path();

        let removed = match entry.file_type() {
            Ok(kind) if kind.is_dir() => std::fs::remove_dir_all(&inside),
            _ => std::fs::remove_file(&inside),
        };

        removed.map_err(|e| format!("не удалось убрать {}: {e}", inside.display()))?;
    }

    Ok(())
}

fn tree_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };

    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => tree_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|meta| meta.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

#[cfg(windows)]
fn data_dir() -> Result<String, String> {
    use windows_sys::Win32::Security::Isolation::GetAppContainerFolderPath;
    use windows_sys::Win32::System::Com::CoTaskMemFree;

    let sid = derive_sid()?;
    let wide_sid = wide(&sid);
    let mut raw = std::ptr::null_mut();

    // SAFETY: строка завершена нулём; выданный системой буфер освобождается тем
    // способом, который она предписывает.
    unsafe {
        let hr = GetAppContainerFolderPath(wide_sid.as_ptr(), &mut raw);

        if hr < 0 || raw.is_null() {
            return Err(format!("папка среды не найдена: 0x{hr:08X}"));
        }

        let mut length = 0;
        while *raw.add(length) != 0 {
            length += 1;
        }

        let text = String::from_utf16_lossy(std::slice::from_raw_parts(raw, length));

        CoTaskMemFree(raw as *const core::ffi::c_void);

        Ok(text)
    }
}

// ── Внутреннее ───────────────────────────────────────────────────────────

#[cfg(windows)]
fn derive_sid() -> Result<String, String> {
    use windows_sys::Win32::Security::Isolation::DeriveAppContainerSidFromAppContainerName;

    let name = wide(NAME);
    let mut sid = std::ptr::null_mut();

    // SAFETY: строка завершена нулём и живёт до конца вызова.
    let hr = unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) };

    if hr < 0 || sid.is_null() {
        return Err(format!("не удалось получить SID контейнера: 0x{hr:08X}"));
    }

    let text = sid_to_string(sid);
    free_sid(sid);
    text
}

#[cfg(windows)]
fn sid_to_string(sid: *mut core::ffi::c_void) -> Result<String, String> {
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;

    let mut raw = std::ptr::null_mut();

    // SAFETY: SID выдан системой и жив; строку освобождаем сразу после копии.
    unsafe {
        if ConvertSidToStringSidW(sid, &mut raw) == 0 || raw.is_null() {
            return Err("SID контейнера не удалось прочитать".into());
        }

        let mut length = 0;
        while *raw.add(length) != 0 {
            length += 1;
        }

        let text = String::from_utf16_lossy(std::slice::from_raw_parts(raw, length));

        windows_sys::Win32::Foundation::LocalFree(raw as *mut core::ffi::c_void);

        Ok(text)
    }
}

#[cfg(windows)]
fn free_sid(sid: *mut core::ffi::c_void) {
    // SAFETY: SID выдан CreateAppContainerProfile и освобождается им же
    // предписанным способом.
    unsafe {
        windows_sys::Win32::Security::FreeSid(sid);
    }
}

/// Добавляет контейнеру право читать и выполнять в папке.
#[cfg(windows)]
fn grant(path: &Path) -> Result<(), String> {
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
        GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_WELL_KNOWN_GROUP,
    };
    use windows_sys::Win32::Security::Isolation::DeriveAppContainerSidFromAppContainerName;
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    };

    const CONTAINER_INHERIT_ACE: u32 = 0x2;
    const OBJECT_INHERIT_ACE: u32 = 0x1;
    const GENERIC_READ_EXECUTE: u32 = 0x8000_0000 | 0x2000_0000;

    if !path.exists() {
        return Err("папки нет".into());
    }

    let target = wide(&path.display().to_string());
    let name = wide(NAME);

    // SAFETY: работа со списками доступа. Каждый выданный системой блок
    // освобождается тем способом, которым выдан.
    unsafe {
        let mut sid = std::ptr::null_mut();

        if DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) < 0 || sid.is_null() {
            return Err("SID контейнера не получен".into());
        }

        let mut old: *mut ACL = std::ptr::null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();

        let read = GetNamedSecurityInfoW(
            target.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut old,
            std::ptr::null_mut(),
            &mut descriptor,
        );

        if read != 0 {
            free_sid(sid);
            return Err(format!("права папки не прочитаны: {read}"));
        }

        let mut access: EXPLICIT_ACCESS_W = std::mem::zeroed();
        access.grfAccessPermissions = GENERIC_READ_EXECUTE;
        access.grfAccessMode = GRANT_ACCESS;
        // Наследование: право должно распространиться на всё внутри папки
        access.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
        access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
        access.Trustee.TrusteeType = TRUSTEE_IS_WELL_KNOWN_GROUP;
        access.Trustee.ptstrName = sid as *mut u16;
        access.Trustee.MultipleTrusteeOperation = NO_MULTIPLE_TRUSTEE;

        let mut updated: *mut ACL = std::ptr::null_mut();
        let merged = SetEntriesInAclW(1, &access, old, &mut updated);

        if merged != 0 {
            windows_sys::Win32::Foundation::LocalFree(descriptor);
            free_sid(sid);
            return Err(format!("список доступа не собран: {merged}"));
        }

        let written = SetNamedSecurityInfoW(
            target.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            updated,
            std::ptr::null_mut(),
        );

        windows_sys::Win32::Foundation::LocalFree(updated as *mut core::ffi::c_void);
        windows_sys::Win32::Foundation::LocalFree(descriptor);
        free_sid(sid);

        if written != 0 {
            return Err(format!("права не выданы: {written}"));
        }

        Ok(())
    }
}

/// Запускает процесс с маркёром доступа контейнера.
#[cfg(windows)]
/// Запускает процесс с маркёром доступа контейнера.
///
/// `app` — кому сообщить, чем кончился запуск. Пусто при запуске с ярлыка: там
/// приложение ещё не поднято, и слушать некому.
fn launch(
    app: Option<&tauri::AppHandle>,
    exe: &str,
    capabilities: &[String],
    folders: &[String],
    args: &str,
    env: &[crate::paths::EnvVar],
) -> Result<(), String> {
    use windows_sys::Win32::Security::Isolation::DeriveAppContainerSidFromAppContainerName;
    use windows_sys::Win32::Security::{SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES};
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
        PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTUPINFOEXW,
    };

    const SE_GROUP_ENABLED: u32 = 0x4;

    /*
     * Ярлык — не программа.
     *
     * Создание процесса умеет запускать только настоящий исполняемый файл, а на
     * `.lnk` отвечает ошибкой 193 «не приложение Win32»: разворачивать ярлыки
     * умеет оболочка, но она не даёт задать маркёр доступа контейнера. Поэтому
     * разворачиваем сами и запускаем уже цель.
     *
     * Ключи из ярлыка приписываются к нашим спереди: они часть того, чем
     * программа является, а наши — добавка к запуску.
     */
    let (exe, args) = match resolve(Path::new(exe)) {
        Some((target, extra)) => {
            let joined = match (extra.trim(), args.trim()) {
                ("", rest) => rest.to_string(),
                (first, "") => first.to_string(),
                (first, rest) => format!("{first} {rest}"),
            };

            (target, joined)
        }
        None => (exe.to_string(), args.to_string()),
    };

    let exe = exe.as_str();
    let args = args.as_str();

    let path = Path::new(exe);

    if !path.is_file() {
        return Err("такого файла нет".into());
    }

    /*
     * Отказываем заранее только тому, что запустить нельзя в принципе.
     *
     * Раньше здесь стояла проверка «расширение должно быть exe» — и она рубила
     * верные случаи: ярлык, чью цель не удалось прочитать, ярлык на ярлык,
     * программу без привычного расширения. Гадать за систему не нужно: пусть
     * пробует, а если откажет — код ошибки скажет точнее любой нашей догадки.
     */
    if path
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case("url"))
    {
        return Err("это ссылка, а не программа: изоляции нечего в неё поместить".into());
    }

    /*
     * Приложение из Store в нашу изоляцию не поместить.
     *
     * Оно уже работает в своём контейнере, и вся его работа с данными — от
     * `ApplicationData.Current` до баз настроек — опирается на удостоверение
     * пакета. Запущенное под нашим маркёром доступа, оно это удостоверение
     * теряет и падает на первом же обращении к своим данным: «Отказано в
     * доступе, error trying to open the explicit application data handle».
     *
     * Обойти это нельзя: удостоверение пакета выдаёт система, а не мы. Такие
     * приложения Windows изолирует и без нас — тем же механизмом.
     */
    if path
        .components()
        .any(|part| part.as_os_str().eq_ignore_ascii_case("WindowsApps"))
    {
        return Err("это приложение из Store: Windows уже держит его в своей изоляции,                     и поместить его ещё и в нашу нельзя"
            .into());
    }

    let folder = path
        .parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default();

    /*
     * Среда заводится сама, при первом же запуске.
     *
     * Раньше её надо было включить руками, и забывший об этом получал ошибку 2
     * «файл не найден» — от попытки выдать права папке контейнера, которого нет.
     * Заведение идемпотентно: система ответит «уже есть», и мы просто пойдём
     * дальше.
     */
    container_ensure()?;

    /*
     * Права на папку программы — попытка, а не условие.
     *
     * Контейнер по умолчанию не имеет прав нигде, и обычно их надо выдать. Но
     * список доступа папки вроде `Program Files` нам менять не позволено — она
     * не наша, — и попытка возвращала отказ 5, обрывая запуск. При этом Windows
     * многим таким папкам уже выдала права всем пакетам приложений, и программа
     * прекрасно стартует без нашего вмешательства.
     *
     * Поэтому пробуем и идём дальше: если прав и правда не хватит, об этом
     * скажет сам запуск, и скажет точнее.
     */
    let folder_rights = grant(Path::new(&folder));

    // Всё остальное — только по списку. Молча открыть лишнее значило бы
    // сделать изоляцию видимостью
    for extra in folders {
        if extra.trim().is_empty() {
            continue;
        }

        let _ = grant(Path::new(extra));
    }

    // Путь в кавычках, ключи следом как есть: они идут в командную строку самой
    // программы, а не в оболочку, — разбирать их некому и подставлять некуда
    let mut command = wide(&match args.trim() {
        "" => format!("\"{exe}\""),
        extra => format!("\"{exe}\" {extra}"),
    });

    let directory = wide(&folder);
    let name = wide(NAME);

    /*
     * Свой профиль внутри среды.
     *
     * Это то, обо что спотыкались почти все программы. Обычная настольная
     * программа при запуске лезет за своим профилем в `%APPDATA%` и
     * `%LOCALAPPDATA%` — а контейнеру туда писать нельзя, права там наши, не
     * его. Firefox из-за этого не мог завести профиль, остальные просто висли
     * или закрывались, не сумев создать свои файлы.
     *
     * Даём каждой программе собственную папку внутри среды и подсовываем её
     * этими же переменными. Писать туда контейнер вправе — это его земля, — а
     * настоящий профиль остаётся нетронутым: программа в изоляции не видит ни
     * ваших закладок, ни ваших входов.
     *
     * Заданное человеком не трогаем: его переменные добавляются поверх наших.
     */
    let mut block = env_block(&with_workspace(exe, env));

    // SAFETY: список свойств собирается и удаляется здесь же, все указатели
    // живут до конца вызова.
    unsafe {
        let mut sid = std::ptr::null_mut();

        if DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) < 0 || sid.is_null() {
            return Err("SID контейнера не получен".into());
        }

        // Выданные возможности. Невыданное отсутствует у процесса вовсе — это
        // не правило брандмауэра, а отсутствие права у маркёра доступа
        let mut granted: Vec<SID_AND_ATTRIBUTES> = Vec::new();

        for wanted in capabilities {
            let Some(known) = CAPABILITIES.iter().find(|item| item.id == wanted) else {
                continue;
            };

            if let Some(mut attribute) = capability_sid(known.sid) {
                attribute.Attributes = SE_GROUP_ENABLED;
                granted.push(attribute);
            }
        }

        let mut security: SECURITY_CAPABILITIES = std::mem::zeroed();
        security.AppContainerSid = sid;

        if !granted.is_empty() {
            security.Capabilities = granted.as_mut_ptr();
            security.CapabilityCount = granted.len() as u32;
        }

        let mut size = 0;
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);

        let mut buffer = vec![0u8; size];
        let list = buffer.as_mut_ptr() as *mut _;

        if InitializeProcThreadAttributeList(list, 1, 0, &mut size) == 0 {
            free_sid(sid);
            return Err("список свойств процесса не создан".into());
        }

        let updated = UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            &mut security as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        if updated == 0 {
            DeleteProcThreadAttributeList(list);
            free_sid(sid);
            return Err("свойства изоляции не заданы".into());
        }

        let mut startup: STARTUPINFOEXW = std::mem::zeroed();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = list;

        let mut info: PROCESS_INFORMATION = std::mem::zeroed();

        let started = CreateProcessW(
            std::ptr::null(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            block.as_mut_ptr() as *const core::ffi::c_void,
            directory.as_ptr(),
            &mut startup.StartupInfo,
            &mut info,
        );

        DeleteProcThreadAttributeList(list);
        free_sid(sid);

        // SID возможностей выданы ConvertStringSidToSidW и живут до сюда: маркёр
        // доступа процесс получил при создании и в этой памяти больше не
        // нуждается
        for attribute in &granted {
            windows_sys::Win32::Foundation::LocalFree(attribute.Sid);
        }

        if started == 0 {
            let code = windows_sys::Win32::Foundation::GetLastError();

            return Err(match code {
                2 => "программа не найдена по этому пути".to_string(),
                5 if folder_rights.is_err() => {
                    "не удалось открыть контейнеру доступ к папке программы: она не ваша, и права                      на неё меняет только администратор. Перенесите программу к себе — например, в                      наше Место — и попробуйте снова"
                        .to_string()
                }
                5 => "программа отказалась работать в изоляции: нет доступа".to_string(),
                // 193 — «не приложение Win32». Обычно это ярлык или документ,
                // но сюда мы их уже не пускаем, значит файл и правда не тот
                193 => "это не исполняемый файл".to_string(),
                740 => "программе нужны права администратора, а в изоляции их нет".to_string(),
                _ => format!("запуск не удался: ошибка {code}"),
            });
        }

        /*
         * Смотрим, чем кончится запуск.
         *
         * Программа в изоляции умирает молча: сообщить о нехватке прав ей
         * нечем — окна ещё нет, а вывод некуда девать. Со стороны это выглядит
         * как «ничего не произошло» или «повисло». Наблюдатель ждёт несколько
         * секунд и говорит, что случилось на самом деле: код выхода, если
         * процесс умер, или «жив, но окна не показал», если завис.
         *
         * Дескриптор процесса уходит наблюдателю и закрывается там: закрыть его
         * здесь значило бы потерять и код выхода, и саму возможность ждать.
         */
        watch(app.cloned(), info.hProcess, info.dwProcessId, exe.to_string());
        windows_sys::Win32::Foundation::CloseHandle(info.hThread);

        Ok(())
    }
}

/// Разворачивает ярлык в путь до программы и её ключи.
///
/// Через оболочку, а не через COM напрямую: тот же результат тремя строками
/// вместо ручной работы с IShellLink и IPersistFile — так же, как сделано с
/// созданием ярлыков.
///
/// Возвращает пусто, если это не ярлык или прочитать его не вышло: тогда путь
/// берётся как есть.
#[cfg(windows)]
fn resolve(path: &Path) -> Option<(String, String)> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    if !path
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case("lnk"))
    {
        return None;
    }

    // Одинарные кавычки внутри PowerShell удваиваются — иначе строка оборвётся
    // посреди пути
    let quoted = path.display().to_string().replace('\'', "''");

    // Вывод переводим в UTF-8: по умолчанию оболочка пишет в кодовой странице
    // консоли, и путь с кириллицей приходил бы битым
    let script = format!(
        "[Console]::OutputEncoding = [Text.Encoding]::UTF8;          $s = (New-Object -ComObject WScript.Shell).CreateShortcut('{quoted}');          Write-Output $s.TargetPath; Write-Output $s.Arguments"
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();

    let target = lines.next()?.trim().to_string();

    /*
     * Пустая цель — признак ярлыка на приложение из Store.
     *
     * У такого ярлыка вместо пути лежит удостоверение пакета, и запускает его
     * оболочка, а не мы. Возвращаем пусто: дальше путь останется прежним, и
     * запуск честно скажет, что это не программа.
     */
    if target.is_empty() {
        return None;
    }

    Some((target, lines.next().unwrap_or("").trim().to_string()))
}

/// Ждёт первых секунд жизни изолированного процесса и говорит, чем кончилось.
///
/// Отдельным потоком: ожидание блокирующее, а запуск должен вернуться сразу.
#[cfg(windows)]
fn watch(
    app: Option<tauri::AppHandle>,
    process: windows_sys::Win32::Foundation::HANDLE,
    pid: u32,
    exe: String,
) {
    use tauri::Emitter;
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

    /// Сколько ждать. Дольше — и человек успеет решить, что всё в порядке.
    const PATIENCE: u32 = 6000;

    let process = process as usize;

    std::thread::spawn(move || {
        let process = process as windows_sys::Win32::Foundation::HANDLE;

        // SAFETY: дескриптор наш, закрывается здесь же ровно один раз.
        unsafe {
            let waited = WaitForSingleObject(process, PATIENCE);

            let verdict = if waited == WAIT_OBJECT_0 {
                let mut code = 0u32;
                GetExitCodeProcess(process, &mut code);

                Some(match code {
                    0 => "программа закрылась сама, ничего не сообщив".to_string(),
                    // 0xC0000022 — отказано в доступе на уровне ядра. Самый
                    // частый конец для программы, которой не хватило прав
                    0xC000_0022 => "программе отказано в доступе внутри изоляции".to_string(),
                    0xC000_0135 | 0xC000_0139 => {
                        "программе не хватило системных библиотек внутри изоляции".to_string()
                    }
                    other => format!("программа завершилась с кодом 0x{other:08X}"),
                })
            } else if has_window(pid) {
                None
            } else {
                Some("программа запустилась, но окна так и не показала".to_string())
            };

            CloseHandle(process);

            if let Some(text) = verdict {
                // Не в журнал, а человеку: молчание — худшее, что можно выдать
                // на «программа не открылась»
                if let Some(app) = &app {
                    let _ = app.emit("iso:report", (exe.clone(), text.clone()));
                }

                eprintln!("изоляция: {exe} — {text}");
            }
        }
    });
}

#[cfg(not(windows))]
fn watch(_app: Option<tauri::AppHandle>, _process: (), _pid: u32, _exe: String) {}

/// Есть ли у процесса видимое окно.
#[cfg(windows)]
fn has_window(pid: u32) -> bool {
    // BOOL живёт в core, а не в Foundation
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Search {
        pid: u32,
        found: bool,
    }

    unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> BOOL {
        // SAFETY: указатель наш, живёт до конца перечисления.
        let search = unsafe { &mut *(data as *mut Search) };

        let mut owner = 0u32;
        unsafe { GetWindowThreadProcessId(window, &mut owner) };

        if owner == search.pid && unsafe { IsWindowVisible(window) } != 0 {
            search.found = true;
            return 0;
        }

        1
    }

    let mut search = Search { pid, found: false };

    // SAFETY: перечисление синхронное, ссылка жива всё это время.
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as isize);
    }

    search.found
}

/// Добавляет к окружению свой профиль внутри среды.
///
/// Папка у каждой программы своя: две программы, пишущие в один профиль, мешали
/// бы друг другу так же, как мешают в обычной системе. Имя берётся от файла —
/// понятное глазом, когда заглядываешь в среду.
///
/// Уже заданные человеком переменные не перебиваются: они идут после наших и
/// побеждают.
#[cfg(windows)]
fn with_workspace(exe: &str, env: &[crate::paths::EnvVar]) -> Vec<crate::paths::EnvVar> {
    let Ok(root) = data_dir() else {
        return env.to_vec();
    };

    let stem = Path::new(exe)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".into());

    let safe: String = stem
        .chars()
        .map(|symbol| if symbol.is_alphanumeric() { symbol } else { '_' })
        .take(48)
        .collect();

    let home = Path::new(&root).join("apps").join(safe);

    // Папки создаём заранее: программа, которой не дали их создать, обычно
    // просто закрывается, не объяснив причины
    for part in ["Roaming", "Local", "LocalLow", "Temp"] {
        let _ = std::fs::create_dir_all(home.join(part));
    }

    let mut ours = vec![
        crate::paths::EnvVar {
            name: "USERPROFILE".into(),
            value: home.display().to_string(),
        },
        crate::paths::EnvVar {
            name: "APPDATA".into(),
            value: home.join("Roaming").display().to_string(),
        },
        crate::paths::EnvVar {
            name: "LOCALAPPDATA".into(),
            value: home.join("Local").display().to_string(),
        },
        crate::paths::EnvVar {
            name: "TEMP".into(),
            value: home.join("Temp").display().to_string(),
        },
        crate::paths::EnvVar {
            name: "TMP".into(),
            value: home.join("Temp").display().to_string(),
        },
    ];

    ours.extend_from_slice(env);
    ours
}

#[cfg(not(windows))]
fn with_workspace(_exe: &str, env: &[crate::paths::EnvVar]) -> Vec<crate::paths::EnvVar> {
    env.to_vec()
}

/// Собирает окружение для процесса: наше плюс добавленное.
///
/// Заменять окружение целиком нельзя — без PATH, TEMP и SystemRoot не заведётся
/// почти ничто. Поэтому берётся наше и правится поверх: одноимённая переменная
/// перекрывается, остальные остаются.
///
/// Формат блока задан системой: пары `имя=значение`, каждая завершена нулём,
/// и ещё один ноль в конце. Пустой блок означал бы окружение без единой
/// переменной, поэтому при пустой добавке блок всё равно собирается из нашего.
#[cfg(windows)]
fn env_block(extra: &[crate::paths::EnvVar]) -> Vec<u16> {
    let mut vars: Vec<(String, String)> = std::env::vars().collect();

    for item in extra {
        let name = item.name.trim();

        // Имя без знака равенства: иначе пара распалась бы не там, где нужно
        if name.is_empty() || name.contains('=') {
            continue;
        }

        match vars
            .iter_mut()
            .find(|(existing, _)| existing.eq_ignore_ascii_case(name))
        {
            Some(found) => found.1 = item.value.clone(),
            None => vars.push((name.to_string(), item.value.clone())),
        }
    }

    let mut block: Vec<u16> = Vec::new();

    for (name, value) in vars {
        block.extend(format!("{name}={value}").encode_utf16());
        block.push(0);
    }

    block.push(0);
    block
}

/// Возможность как SID.
#[cfg(windows)]
fn capability_sid(sid: &str) -> Option<windows_sys::Win32::Security::SID_AND_ATTRIBUTES> {
    use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows_sys::Win32::Security::SID_AND_ATTRIBUTES;

    let text = wide(sid);
    let mut sid = std::ptr::null_mut();

    // SAFETY: строка завершена нулём; SID остаётся жить до конца запуска и
    // освобождается системой вместе с процессом.
    unsafe {
        if ConvertStringSidToSidW(text.as_ptr(), &mut sid) == 0 || sid.is_null() {
            return None;
        }

        let mut attribute: SID_AND_ATTRIBUTES = std::mem::zeroed();
        attribute.Sid = sid;

        Some(attribute)
    }
}
