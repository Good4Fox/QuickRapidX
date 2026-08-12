//! Запуск в песочнице Windows.
//!
//! Третий способ изоляции, и самый надёжный по совместимости: внутри работает
//! настоящая Windows, поэтому программа ведёт себя ровно так, как привыкла.
//! Ставить ничего не нужно — компонент входит в состав системы.
//!
//! Плата за это честная и большая: песочница поднимается несколько секунд,
//! занимает под гигабайт памяти, и **всё внутри исчезает при закрытии** — ни
//! профиля, ни настроек, ни установленного. Это способ «открыть и забыть», а не
//! «держать браузер с отдельным профилем».
//!
//! Ещё одно ограничение самой системы: песочница работает в одном экземпляре.
//! Открыть в ней вторую программу, пока идёт первая, нельзя.
//!
//! Управляется файлом настроек `.wsb` — это XML, который система понимает
//! сама. Мы его собираем и открываем: пробрасываем папку программы внутрь и
//! просим запустить её при входе.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Что известно про песочницу Windows.
#[derive(Serialize)]
pub struct WSandbox {
    pub installed: bool,
}

/// Есть ли песочница в системе.
///
/// Компонент входит не во все издания — в домашнем его нет вовсе, — и включён
/// не всегда. Судим по самой программе: она появляется вместе с компонентом.
#[tauri::command]
pub fn wsandbox_status() -> WSandbox {
    WSandbox {
        installed: binary().is_some(),
    }
}

#[cfg(windows)]
fn binary() -> Option<PathBuf> {
    let root = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let path = PathBuf::from(root).join("System32").join("WindowsSandbox.exe");

    path.is_file().then_some(path)
}

#[cfg(not(windows))]
fn binary() -> Option<PathBuf> {
    None
}

/// Запускает программу в песочнице Windows.
///
/// `internet` — давать ли сети ход. Здесь это ровно тот же выбор, что и в нашей
/// среде: роль без права на интернет получает песочницу без сети.
///
/// `extra` — дополнительные папки, которые попросили открыть программе. Поле
/// для них в настройках изоляции есть давно, но песочница его не читала.
#[cfg(windows)]
pub fn launch(
    app: &tauri::AppHandle,
    exe: &str,
    args: &str,
    internet: bool,
    extra: &[String],
) -> Result<(), String> {
    let Some(_) = binary() else {
        return Err("песочница Windows не включена в этой системе".into());
    };

    let program = Path::new(exe);

    /*
     * Программы из Store в песочнице не запускаются, и это не поправимо.
     *
     * В `WindowsApps` лежат не программы, а точки перенаправления нулевого
     * размера: сама программа стоит пакетом, и запускается по имени пакета, а
     * не по файлу. Внутри песочницы этого пакета нет — там чистая система, —
     * поэтому пробрасывать туда точку бессмысленно: получится нулевой файл,
     * который ничего не откроет.
     *
     * Ровно это и выглядело как «поднимается сама песочница без программы».
     * Тот же запрет уже стоит у нашей среды, а сюда я его не поставил.
     */
    let store = program
        .components()
        .any(|part| part.as_os_str().eq_ignore_ascii_case("WindowsApps"));

    let stub = std::fs::metadata(program).map(|meta| meta.len() == 0).unwrap_or(false);

    if store || stub {
        return Err(concat!(
            "программа из Microsoft Store. Внутри песочницы её пакета нет — там ",
            "чистая система, — а в папке лежит не программа, а точка ",
            "перенаправления нулевого размера. Запускать внутри нечего"
        )
        .into());
    }

    let folder = program
        .parent()
        .ok_or_else(|| "у программы нет папки".to_string())?;

    let file = program
        .file_name()
        .ok_or_else(|| "не разобрать имя программы".to_string())?
        .to_string_lossy()
        .to_string();

    /*
     * Папка программы пробрасывается только на чтение.
     *
     * Внутри песочницы программе есть куда писать — там своя система целиком, —
     * а вот менять что-то у вас на диске ей незачем. Чтение и запуск при этом
     * остаются: этого хватает, чтобы она стартовала.
     */
    /*
     * Дополнительные папки.
     *
     * Одной папки программы хватает не всем: рядом бывают данные, лицензии,
     * готовые настройки. Каждая ложится внутрь под своим именем в `C:\Extra`.
     *
     * Положить их по тем же путям, что снаружи, нельзя: внутри песочницы
     * работает другой пользователь, и его папки зовутся иначе. Поэтому
     * программы, которые ищут строго свой прежний путь, проброс не спасёт — им
     * нужна установка внутри. Ровно на это и наткнулся Discord с Vencord: тот
     * подгружается по пути профиля хозяина машины, которого внутри нет.
     */
    let shared: String = extra
        .iter()
        .filter_map(|folder| {
            let path = Path::new(folder.trim());

            if folder.trim().is_empty() || !path.is_dir() {
                return None;
            }

            let name = path.file_name()?.to_string_lossy().to_string();

            Some(format!(
                "    <MappedFolder>\n      \
                   <HostFolder>{host}</HostFolder>\n      \
                   <SandboxFolder>C:\\Extra\\{name}</SandboxFolder>\n      \
                   <ReadOnly>true</ReadOnly>\n    \
                 </MappedFolder>\n",
                host = escape(&path.display().to_string()),
                name = escape(&name),
            ))
        })
        .collect();

    let body = format!(
        "<Configuration>\n  \
           <MappedFolders>\n    \
             <MappedFolder>\n      \
               <HostFolder>{host}</HostFolder>\n      \
               <SandboxFolder>C:\\QuickRapidX</SandboxFolder>\n      \
               <ReadOnly>true</ReadOnly>\n    \
             </MappedFolder>\n{shared}  \
           </MappedFolders>\n  \
           <Networking>{net}</Networking>\n  \
           <LogonCommand>\n    \
             <Command>{command}</Command>\n  \
           </LogonCommand>\n\
         </Configuration>\n",
        host = escape(&folder.display().to_string()),
        // Enable, а не Default: и то и другое включает сеть, но первое говорит
        // о намерении, а второе — «как система решит»
        net = if internet { "Enable" } else { "Disable" },
        /*
         * Запуск через `cmd`, и путь обязательно в кавычках.
         *
         * Без них имя с пробелом разрывалось надвое, команда выходила неверной,
         * и песочница поднималась пустой. Сказать об этом ей некому — окна
         * изнутри мы не видим, — и со стороны это выглядело ровно как
         * «запускается сама песочница без программы».
         *
         * `start` вместо прямого пути: он дожидается готовности рабочего стола
         * и не падает, если тот ещё не поднялся. Пустые кавычки после него
         * обязательны — первый его довод это заголовок окна, и без них
         * заголовком стал бы наш путь, а запускать было бы нечего.
         *
         * Имя папки внутри песочницы — не «Program», и это важно. Windows
         * считает существующий `C:\Program` опасным и встречает человека
         * предложением его переименовать: старый разбор путей читает
         * `C:\Program Files\…` как программу `C:\Program` с ключом `Files\…`.
         * Своё имя этой ловушки лишено.
         */
        command = escape(&match args.trim() {
            "" => format!("cmd.exe /c start \"\" \"C:\\QuickRapidX\\{file}\""),
            extra => format!("cmd.exe /c start \"\" \"C:\\QuickRapidX\\{file}\" {extra}"),
        }),
    );

    let plan = crate::paths::data_dir(app)?.join("sandbox.wsb");

    std::fs::write(&plan, body).map_err(|e| format!("не удалось собрать песочницу: {e}"))?;

    // Открываем файл настроек оболочкой: она и поднимает песочницу
    open(&plan)
}

#[cfg(not(windows))]
pub fn launch(
    _app: &tauri::AppHandle,
    _exe: &str,
    _args: &str,
    _internet: bool,
    _extra: &[String],
) -> Result<(), String> {
    Err("не поддерживаемая операционная система".into())
}

/// Отдаёт файл настроек оболочке.
#[cfg(windows)]
fn open(plan: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide: Vec<u16> = std::ffi::OsStr::new(&plan.display().to_string())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: строка завершена нулём и живёт до конца вызова.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if (result as isize) > 32 {
        return Ok(());
    }

    Err("песочница не открылась: возможно, уже запущена — она работает в одном экземпляре".into())
}

/// Готовит строку к вставке в XML.
///
/// Путь может содержать `&` — без замены файл настроек стал бы неразбираемым, и
/// песочница молча отказалась бы открываться.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── Проверка и починка ───────────────────────────────────────────────────
//
// Песочница отказывается запускаться чаще, чем хотелось бы, и всегда с
// невнятным «Не удалось инициализировать Windows Sandbox». За этим стоит
// небольшой набор вполне определённых причин, и почти все чинятся.
//
// Самая частая — `0x80070490`, «элемент не найден». Так система сообщает, что
// не нашла виртуальный коммутатор Hyper-V по умолчанию: без сети песочница
// поднимается, с сетью — нет. Второй частый случай — отключённая учётная запись
// `WDAGUtilityAccount`: под ней песочница и входит внутрь. Третий — остановленная
// служба вычислений Hyper-V.

/// Одна проверка.
#[derive(Serialize)]
pub struct Check {
    pub id: &'static str,
    /// Всё ли в порядке. `None` — узнать не удалось.
    pub ok: Option<bool>,
    pub detail: String,
}

/// Проверяет всё, что нужно песочнице, не требуя прав администратора.
#[tauri::command]
pub async fn wsandbox_check() -> Vec<Check> {
    tauri::async_runtime::spawn_blocking(checks)
        .await
        .unwrap_or_default()
}

#[cfg(windows)]
fn checks() -> Vec<Check> {
    let mut out = Vec::new();

    out.push(Check {
        id: "feature",
        ok: Some(binary().is_some()),
        detail: match binary() {
            Some(path) => path.display().to_string(),
            None => "компонент не установлен".into(),
        },
    });

    // Служба вычислений Hyper-V поднимает саму песочницу
    out.push(service("vmcompute", "compute"));

    // Служба узла Hyper-V — на ней держится виртуальный коммутатор
    out.push(service("HvHost", "hvhost"));

    /*
     * Коммутатор по умолчанию. Именно его отсутствие даёт `0x80070490`.
     *
     * Спрашиваем через список сетевых устройств, а не через средства Hyper-V:
     * те требуют прав администратора и установленного модуля, а адаптер виден
     * всем.
     */
    let switch = ask(
        "(Get-NetAdapter -Name 'vEthernet (Default Switch)' -ErrorAction SilentlyContinue) -ne $null",
    );

    out.push(Check {
        id: "switch",
        ok: switch.as_deref().map(|value| value.trim() == "True"),
        detail: match switch.as_deref().map(str::trim) {
            Some("True") => "коммутатор по умолчанию на месте".into(),
            Some(_) => "коммутатора по умолчанию нет — это и есть 0x80070490".into(),
            None => "проверить не удалось".into(),
        },
    });

    // Учётная запись, под которой песочница входит внутрь
    let account = ask(
        "(Get-LocalUser -Name 'WDAGUtilityAccount' -ErrorAction SilentlyContinue).Enabled",
    );

    /*
     * Сборка компонента против сборки системы.
     *
     * Самая коварная причина, и внешне её не видно: после обновления Windows до
     * новой сборки компонент песочницы остаётся от прежней. Числится
     * установленным, файлы на месте, службы работают — а поднять песочницу
     * система не может и говорит невнятное «Не удалось инициализировать»
     * с кодом 0x80070490.
     *
     * Проверяем прямо: у пакета компонента в имени стоит его сборка, у системы
     * своя. Расходятся — лечится переустановкой компонента.
     */
    let ours = ask(concat!(
        r"(Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\Packages' ",
        r"-ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -match 'DisposableClientVM' } | ",
        r"ForEach-Object { ($_.PSChildName -split '~')[-1] } | ForEach-Object { ($_ -split '\.')[2] } | ",
        r"Sort-Object -Unique) -join ','"
    ));

    let system = ask(
        r"(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').CurrentBuild",
    );

    let build = system.as_deref().map(str::trim).unwrap_or("").to_string();
    let packages = ours.as_deref().map(str::trim).unwrap_or("").to_string();

    out.push(Check {
        id: "build",
        ok: if build.is_empty() || packages.is_empty() {
            None
        } else {
            Some(packages.split(',').any(|value| value.trim() == build))
        },
        detail: if build.is_empty() || packages.is_empty() {
            "проверить не удалось".into()
        } else if packages.split(',').any(|value| value.trim() == build) {
            format!("компонент от сборки {build} — совпадает")
        } else {
            format!(
                "компонент от сборки {packages}, а система {build} — \
                 после обновления Windows он остался от прежней"
            )
        },
    });

    /*
     * Отложенная перезагрузка.
     *
     * Смотрим только ветки обслуживания и обновления. Список отложенных
     * переименований файлов сюда не годится: в нём подолгу лежит мусор от
     * прошлых обновлений, и он давал бы ложную тревогу на ровном месте.
     */
    let waiting = ask(concat!(
        r"$a = Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending'; ",
        r"$b = Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired'; ",
        r"$a -or $b"
    ));

    out.push(Check {
        id: "reboot",
        // Здесь наоборот: «в порядке» значит, что перезагрузка не нужна
        ok: waiting.as_deref().map(|value| value.trim() != "True"),
        detail: match waiting.as_deref().map(str::trim) {
            Some("True") => "система ждёт перезагрузки — до неё изменения не вступят в силу".into(),
            Some(_) => "перезагрузка не нужна".into(),
            None => "проверить не удалось".into(),
        },
    });

    out.push(Check {
        id: "account",
        ok: account.as_deref().map(|value| value.trim() == "True"),
        detail: match account.as_deref().map(str::trim) {
            Some("True") => "учётная запись песочницы включена".into(),
            Some("False") => "учётная запись WDAGUtilityAccount отключена".into(),
            _ => "учётной записи WDAGUtilityAccount нет".into(),
        },
    });

    out
}

#[cfg(not(windows))]
fn checks() -> Vec<Check> {
    Vec::new()
}

/// Состояние службы. Прав не требует.
#[cfg(windows)]
fn service(name: &str, id: &'static str) -> Check {
    let state = ask(&format!(
        "(Get-Service -Name '{name}' -ErrorAction SilentlyContinue).Status"
    ));

    Check {
        id,
        ok: state.as_deref().map(|value| value.trim() == "Running"),
        detail: match state.as_deref().map(str::trim) {
            Some("Running") => format!("служба {name} работает"),
            Some("") | None => format!("службы {name} нет"),
            Some(other) => format!("служба {name}: {other}"),
        },
    }
}

/// Короткий вопрос к оболочке. Пусто, если спросить не вышло.
#[cfg(windows)]
fn ask(script: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Чинит то, что чинится, — с правами администратора.
///
/// Порядок не случаен: сперва компонент, потом службы, потом коммутатор. Пока
/// не работает служба узла, коммутатор пересоздавать бессмысленно.
#[tauri::command]
pub fn wsandbox_repair() -> Result<(), String> {
    #[cfg(windows)]
    {
        /*
         * Перезапуск службы вычислений — то, что возвращает коммутатор по
         * умолчанию. Он пересоздаётся при её старте, и это официальный способ
         * лечения `0x80070490`; пересоздавать коммутатор вручную не нужно.
         *
         * Службу сетей контейнеров трогаем следом и только перезапуском: она
         * держит описания всех виртуальных сетей, включая чужие — Docker и WSL.
         * Сбрасывать её хранилище, как советуют в интернете, мы не станем: это
         * снесло бы и их сети заодно, а решаем мы не их задачу.
         */
        let steps = "\
            Enable-WindowsOptionalFeature -Online -FeatureName 'Containers-DisposableClientVM' -All -NoRestart -ErrorAction SilentlyContinue | Out-Null; \
            Enable-LocalUser -Name 'WDAGUtilityAccount' -ErrorAction SilentlyContinue; \
            Set-Service -Name 'HvHost' -StartupType Automatic -ErrorAction SilentlyContinue; \
            Set-Service -Name 'vmcompute' -StartupType Automatic -ErrorAction SilentlyContinue; \
            Start-Service -Name 'HvHost' -ErrorAction SilentlyContinue; \
            Restart-Service -Name 'hns' -Force -ErrorAction SilentlyContinue; \
            Restart-Service -Name 'vmcompute' -Force -ErrorAction SilentlyContinue";

        elevate(steps)?;

        Ok(())
    }

    #[cfg(not(windows))]
    {
        Err("не поддерживаемая операционная система".into())
    }
}

/// Переустанавливает компонент песочницы.
///
/// Тяжёлая починка — на случай, когда всё остальное в порядке, а песочница всё
/// равно не поднимается. Так бывает после обновления Windows до новой сборки:
/// компонент остаётся от прежней, числится установленным, файлы на месте, а
/// работать перестаёт.
///
/// Порядок обязателен именно такой. Сначала `RestoreHealth` — он подтягивает
/// с серверов обновления недостающие части хранилища компонентов для текущей
/// сборки; без него выключение и включение вернули бы тот же старый компонент.
/// И только потом сам компонент выключается и включается заново.
///
/// Занимает это минуты, а не секунды, и требует интернета. Окно оболочки при
/// этом не показывается: ход работы уходит в наш интерфейс событием `ws:step`,
/// а чужой чёрный терминал поверх программы — не наш способ разговаривать.
#[tauri::command]
pub async fn wsandbox_reinstall(app: tauri::AppHandle) -> Result<(), String> {
    let marks = crate::paths::data_dir(&app)?.join("sandbox-repair.log");

    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;

            use tauri::Emitter;

            // Прошлый след мешал бы: по нему мы приняли бы за ход работы то,
            // что осталось от позапрошлого раза
            let _ = std::fs::remove_file(&marks);

            /*
             * Ход работы читается из файла, а не из вывода процесса.
             *
             * Труба сюда не тянется: процесс с правами администратора нам не
             * дочерний — его создаёт система по запросу, и своих потоков
             * ввода-вывода он с нами не делит. Общий файл — единственное, что
             * пересекает эту границу.
             *
             * Дописываем построчно, а не перенаправлением вывода. Проверено:
             * и оболочка, и `cmd` открывают файл перенаправления монопольно —
             * прочитать его со стороны, пока работа идёт, нельзя вовсе, а
             * узнать что-то после конца работы уже незачем. `Add-Content`
             * закрывает файл после каждой строки, и он читается свободно.
             */
            let steps = format!(
                concat!(
                    r#"$m = "{marks}"; "#,
                    r#"function Mark($t) {{ Add-Content -LiteralPath $m -Value $t -Encoding utf8 }} "#,
                    r#"function Run($a) {{ & DISM.exe @a 2>&1 | ForEach-Object {{ Mark $_ }} }} "#,
                    r#"Mark "[qrx:1]"; "#,
                    r#"Run @("/Online", "/Cleanup-Image", "/RestoreHealth"); "#,
                    r#"Mark "[qrx:2]"; "#,
                    r#"Run @("/Online", "/Disable-Feature", "/FeatureName:Containers-DisposableClientVM", "/NoRestart"); "#,
                    r#"Mark "[qrx:3]"; "#,
                    r#"Run @("/Online", "/Enable-Feature", "/FeatureName:Containers-DisposableClientVM", "/All", "/NoRestart"); "#,
                    r#"Mark "[qrx:4]""#,
                ),
                marks = marks.display(),
            );

            let running = Arc::new(AtomicBool::new(true));

            let watcher = {
                let running = running.clone();
                let app = app.clone();
                let marks = marks.clone();

                std::thread::spawn(move || {
                    let mut last = Step {
                        stage: 0,
                        percent: -1.0,
                    };

                    while running.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(400));

                        /*
                         * Читаем байтами и с потерями.
                         *
                         * Внутри лежит вывод DISM на языке системы, и заранее
                         * неизвестно, в какой он кодировке: у оболочки старого
                         * поколения свои представления об этом. Строгое чтение
                         * споткнулось бы о первую же русскую букву и оборвало
                         * бы показ хода работы совсем — а метки шагов, ради
                         * которых всё и затевалось, состоят из одной латиницы
                         * и переживают любую потерю.
                         */
                        let Ok(raw) = std::fs::read(&marks) else {
                            continue;
                        };

                        let step = progress(&String::from_utf8_lossy(&raw));

                        // Шлём только изменения: то же самое дважды в секунду —
                        // это дёрганье интерфейса на ровном месте
                        if step.stage == last.stage && step.percent == last.percent {
                            continue;
                        }

                        last = step.clone();
                        let _ = app.emit("ws:step", step);
                    }
                })
            };

            let outcome = elevate(&steps);

            running.store(false, Ordering::Relaxed);
            let _ = watcher.join();

            outcome
        }

        #[cfg(not(windows))]
        {
            let _ = marks;
            Err("не поддерживаемая операционная система".into())
        }
    })
    .await
    .map_err(|e| format!("починка сорвалась: {e}"))?
}

/// Ход починки. Уходит в интерфейс событием `ws:step`.
#[derive(Clone, Serialize)]
pub struct Step {
    /// Какой шаг идёт: от 1 до 4. Ноль — ещё не начался.
    pub stage: u8,
    /// Сколько сделано на этом шаге. Отрицательное — система не сказала.
    pub percent: f32,
}

/// Разбирает след починки.
///
/// Шаг — по последней метке, которую успел поставить сценарий. Долю ищем только
/// после неё: у каждого шага полоска своя, и число от предыдущего показывало бы
/// чужой ход.
///
/// Доля есть не всегда, и это нормально. DISM рисует свою полоску поверх себя
/// же, без переводов строки, — до конца шага она может так и не появиться в
/// файле ни разу. Тогда наружу уходит `-1`, и полоса в окне просто ходит
/// туда-сюда: это честнее, чем показывать выдуманное число.
#[cfg(windows)]
fn progress(text: &str) -> Step {
    let stage = (1..=4u8)
        .rev()
        .find(|number| text.contains(&format!("[qrx:{number}]")))
        .unwrap_or(0);

    let tail = match text.rfind(&format!("[qrx:{stage}]")) {
        Some(at) => &text[at..],
        None => text,
    };

    Step {
        stage,
        percent: last_percent(tail),
    }
}

/// Последнее число перед знаком процента. Отрицательное — не нашлось.
#[cfg(windows)]
fn last_percent(text: &str) -> f32 {
    let symbols: Vec<char> = text.chars().collect();
    let mut found = -1.0;

    for (at, symbol) in symbols.iter().enumerate() {
        if *symbol != '%' {
            continue;
        }

        let mut start = at;

        while start > 0 && (symbols[start - 1].is_ascii_digit() || symbols[start - 1] == '.') {
            start -= 1;
        }

        if let Ok(value) = symbols[start..at].iter().collect::<String>().parse::<f32>() {
            found = value;
        }
    }

    found
}

/// Перезагружает машину.
///
/// Отдельной кнопкой и по прямому нажатию: смена компонента вступает в силу
/// только при загрузке, а сказать об этом строчкой в углу — значит оставить
/// человека доделывать вручную то, о чём он уже попросил.
#[tauri::command]
pub fn wsandbox_restart(reason: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        // Причину Windows показывает во весь экран перед перезагрузкой, и
        // читает её человек — поэтому она приходит из словаря, а не отсюда
        let reason = if reason.trim().is_empty() {
            "QuickRapidX".to_string()
        } else {
            reason
        };

        std::process::Command::new("shutdown")
            .args(["/r", "/t", "20", "/c", &reason])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("не удалось перезагрузить: {e}"))?;

        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = reason;
        Err("не поддерживаемая операционная система".into())
    }
}

/// Выполняет набор команд с правами администратора и ждёт конца.
///
/// Через `Start-Process -Verb RunAs`, потому что поднять права у себя процесс
/// не может — их даёт только новый процесс через запрос системы.
///
/// Сам набор уходит закодированным. Иначе он проходит две разборки подряд —
/// нашу оболочку и `Start-Process`, который собирает командную строку заново, —
/// и любая кавычка внутри разъезжается по дороге. Кодировка делает набор
/// сплошным словом из букв и цифр, разбирать в котором нечего.
#[cfg(windows)]
fn elevate(steps: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    use base64::Engine;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // Оболочка ждёт UTF-16 — тот же порядок байтов, что у самой Windows
    let wide: Vec<u8> = steps
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();

    let packed = base64::engine::general_purpose::STANDARD.encode(&wide);

    let script = format!(
        "Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait \
         -ArgumentList '-NoProfile', '-NonInteractive', '-EncodedCommand', '{packed}'"
    );

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("не удалось запросить права администратора: {e}"))?;

    if status.success() {
        return Ok(());
    }

    Err("права администратора не выданы".into())
}
