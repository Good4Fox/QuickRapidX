//! Удаление прошлой установки Windows.
//!
//! Отдельный модуль, а не пара строк в `recovery`, потому что задача совсем не
//! такая, какой выглядит. Замеры на живой машине сразу после обновления до
//! 25H2: **950 511 файлов, 457 484 папки, 50,5 ГБ**, и 88 папок, куда не
//! пускает даже перечисление — их владелец `TrustedInstaller`, а у
//! администратора там нет прав вовсе.
//!
//! Прежний способ — `takeown /r` и `icacls /t` на всё дерево, а потом
//! `rd /s /q` — это три полных прохода по полутора миллионам объектов. Каждый
//! из них печатает строку на объект, то есть ещё и журнал в сотни мегабайт.
//! На таком дереве это часы, и ровно поэтому «нажал, а оно висит»: часы без
//! единого признака жизни выглядят точно так же, как зависание.
//!
//! Здесь один проход. Права перехватываются не у всех подряд, а только там,
//! где удаление отказало, — то есть у тех самых 88 папок вместо полутора
//! миллионов объектов. Ход работы пишется в файл, и окно показывает
//! настоящие цифры: сколько удалено, сколько освобождено, сколько идёт.
//!
//! **Точки повторного разбора не раскрываются никогда.** Внутри лежат связи,
//! ведущие в живую систему: `Documents and Settings`, `Application Data`,
//! папки пользователей. Зайти в такую и продолжить обход значило бы удалить
//! работающую Windows. Проверяется это не по `is_symlink`, а по признаку
//! `FILE_ATTRIBUTE_REPARSE_POINT` — он честнее: `is_symlink` знает только два
//! вида меток из десятка, а нам нужны все.

use std::path::{Path, PathBuf};

/// Ключ запуска для дочернего процесса с правами администратора.
///
/// Значение — путь к файлу хода работы. Своих прав у нас нет, а поднять их
/// себе процесс не может: их даёт только новый процесс через запрос системы.
/// Поэтому мы запускаем сами себя с этим ключом.
pub const PURGE_FLAG: &str = "--purge-windows-old=";

/// Сколько сделано. Уходит в файл хода, оттуда — в окно.
#[derive(Default, Clone, Copy)]
pub struct Tally {
    /// Удалённых файлов и папок.
    pub objects: u64,
    /// Освобождено байт.
    pub bytes: u64,
    /// Сколько не поддалось даже после перехвата прав.
    pub failed: u64,
}

/// Папка прошлой установки на системном диске.
pub fn folder() -> PathBuf {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    PathBuf::from(format!("{drive}\\Windows.old"))
}

// ── Дочерний процесс ─────────────────────────────────────────────────────

/// Точка входа процесса с правами администратора.
///
/// Окно здесь не поднимается: процесс делает своё дело и завершается кодом.
/// Ноль — папки больше нет.
#[cfg(windows)]
pub fn run_standalone(log: &str) -> i32 {
    let root = folder();

    if !root.is_dir() {
        return 0;
    }

    /*
     * Права процесса.
     *
     * Членства в администраторах мало: чтобы забрать владение у
     * `TrustedInstaller`, нужна отдельная привилегия, и в токене она выключена,
     * даже когда есть. Включаем сами — иначе перехват прав молча не сработает,
     * и мы получим отказ там, где всё поправимо.
     */
    for name in [
        "SeTakeOwnershipPrivilege",
        "SeRestorePrivilege",
        "SeBackupPrivilege",
        "SeSecurityPrivilege",
    ] {
        enable(name);
    }

    let mut last = std::time::Instant::now();
    let plan = PathBuf::from(log);

    let tally = sweep(&root, &mut |tally| {
        // Раз в четверть секунды: чаще незачем, а реже уже заметно на глаз
        if last.elapsed() < std::time::Duration::from_millis(250) {
            return;
        }

        last = std::time::Instant::now();
        write_progress(&plan, tally, false);
    });

    write_progress(&plan, &tally, true);

    if root.is_dir() {
        1
    } else {
        0
    }
}

#[cfg(not(windows))]
pub fn run_standalone(_log: &str) -> i32 {
    1
}

/// Обход и удаление. Ход работы отдаётся наружу через `report`.
///
/// Стек вместо рекурсии намеренно: дерево прошлой установки уходит вглубь на
/// десятки уровней, а обходить его приходится сотнями тысяч шагов — на
/// рекурсии это упёрлось бы в стек потока.
///
/// Папка удаляется после своего содержимого, отсюда две пометки на стеке:
/// сперва зайти, потом убрать за собой.
#[cfg(windows)]
fn sweep(root: &Path, report: &mut impl FnMut(&Tally)) -> Tally {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

    enum Step {
        Enter(PathBuf),
        Leave(PathBuf),
    }

    let mut tally = Tally::default();
    let mut stack = vec![Step::Enter(root.to_path_buf())];

    while let Some(step) = stack.pop() {
        match step {
            Step::Enter(dir) => {
                stack.push(Step::Leave(dir.clone()));

                let listing = match std::fs::read_dir(&dir) {
                    Ok(listing) => Some(listing),
                    // Не пускает — забираем владение и пробуем ещё раз. Это те
                    // самые несколько десятков папок, ради которых прежний
                    // способ перебирал всё дерево целиком
                    Err(_) => {
                        seize(&dir);
                        std::fs::read_dir(&dir).ok()
                    }
                };

                let Some(listing) = listing else {
                    tally.failed += 1;
                    continue;
                };

                for entry in listing.flatten() {
                    let path = entry.path();

                    let Ok(meta) = entry.metadata() else {
                        tally.failed += 1;
                        continue;
                    };

                    /*
                     * Разбираем признаки сами, а не через `is_dir`.
                     *
                     * У точки соединения `is_dir` возвращает ложь: Rust считает
                     * её связью и папкой не признаёт. Убирать её при этом нужно
                     * именно как папку — снятием самой связи. Проверено: на
                     * `is_dir` связь не удалялась вовсе, а вместе с ней
                     * оставалось и всё, что её содержит.
                     */
                    let attributes = meta.file_attributes();
                    let directory = attributes & FILE_ATTRIBUTE_DIRECTORY != 0;

                    /*
                     * Связь удаляется как связь, и внутрь мы не заходим.
                     *
                     * Это главное правило всего модуля. `Windows.old\Users\…`
                     * полон точек, ведущих в живые папки пользователя; обход
                     * внутрь означал бы удаление работающей системы.
                     */
                    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        if erase(&path, directory) {
                            tally.objects += 1;
                        } else {
                            tally.failed += 1;
                        }

                        report(&tally);
                        continue;
                    }

                    if directory {
                        stack.push(Step::Enter(path));
                        continue;
                    }

                    // Размер берём до удаления: у удалённого файла его уже не
                    // спросить, а перечисление отдаёт его без обращения к диску
                    let size = meta.len();

                    if erase(&path, false) {
                        tally.objects += 1;
                        tally.bytes += size;
                    } else {
                        tally.failed += 1;
                    }

                    report(&tally);
                }
            }
            Step::Leave(dir) => {
                if erase(&dir, true) {
                    tally.objects += 1;
                } else {
                    tally.failed += 1;
                }

                report(&tally);
            }
        }
    }

    tally
}

/// Убирает один объект. Три попытки, каждая снимает свою помеху.
///
/// Порядок от дешёвого к дорогому: сперва просто удалить, потом снять
/// «только чтение» и прочие пометки, и лишь в последнюю очередь забрать
/// владение. Перехват прав — обращение к диспетчеру безопасности, и делать его
/// на каждый из миллиона файлов было бы тем же, от чего мы уходим.
#[cfg(windows)]
fn erase(path: &Path, directory: bool) -> bool {
    let drop = |path: &Path| {
        if directory {
            std::fs::remove_dir(path).is_ok()
        } else {
            std::fs::remove_file(path).is_ok()
        }
    };

    if drop(path) {
        return true;
    }

    plain(path);

    if drop(path) {
        return true;
    }

    seize(path);

    drop(path)
}

/// Снимает пометки «только чтение», «системный», «скрытый».
#[cfg(windows)]
fn plain(path: &Path) {
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_NORMAL};

    let target = wide(&path.display().to_string());

    // SAFETY: строка завершена нулём и живёт до конца вызова.
    unsafe {
        SetFileAttributesW(target.as_ptr(), FILE_ATTRIBUTE_NORMAL);
    }
}

/// Забирает владение и выдаёт администраторам полный доступ.
///
/// Именно в таком порядке: сменить список доступа у чужого объекта нельзя, а
/// вот стать его владельцем — можно, и владельцу список доступа менять
/// разрешено всегда. На этом и держится весь приём.
#[cfg(windows)]
fn seize(path: &Path) {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSidToSidW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
        NO_MULTIPLE_TRUSTEE, SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_GROUP, TRUSTEE_IS_SID,
    };
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    };

    const CONTAINER_INHERIT_ACE: u32 = 0x2;
    const OBJECT_INHERIT_ACE: u32 = 0x1;
    const GENERIC_ALL: u32 = 0x1000_0000;

    let target = wide(&path.display().to_string());

    // Встроенная группа администраторов. Строкой, а не сборкой по частям:
    // этот номер один и тот же в любой системе на свете
    let text = wide("S-1-5-32-544");

    // SAFETY: работа со списками доступа. Всё, что выдала система, освобождается
    // тем же способом, каким выдано.
    unsafe {
        let mut sid = std::ptr::null_mut();

        if ConvertStringSidToSidW(text.as_ptr(), &mut sid) == 0 || sid.is_null() {
            return;
        }

        SetNamedSecurityInfoW(
            target.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            sid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        /*
         * Список доступа пишется новый, а не дополняется прежним.
         *
         * Прежний мог запрещать нам всё явным запретом — а запрет сильнее
         * разрешения, и добавка к такому списку ничего бы не дала. Объект всё
         * равно идёт под снос, беречь в нём нечего.
         */
        let mut access: EXPLICIT_ACCESS_W = std::mem::zeroed();
        access.grfAccessPermissions = GENERIC_ALL;
        access.grfAccessMode = SET_ACCESS;
        access.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
        access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
        access.Trustee.TrusteeType = TRUSTEE_IS_GROUP;
        access.Trustee.ptstrName = sid as *mut u16;
        access.Trustee.MultipleTrusteeOperation = NO_MULTIPLE_TRUSTEE;

        let mut fresh: *mut ACL = std::ptr::null_mut();

        if SetEntriesInAclW(1, &access, std::ptr::null_mut(), &mut fresh) == 0 && !fresh.is_null() {
            SetNamedSecurityInfoW(
                target.as_ptr() as *mut u16,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fresh,
                std::ptr::null_mut(),
            );

            LocalFree(fresh as *mut core::ffi::c_void);
        }

        LocalFree(sid);
    }
}

/// Включает привилегию в токене процесса.
#[cfg(windows)]
fn enable(name: &str) {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let wanted = wide(name);

    // SAFETY: описатель токена закрывается на всех путях выхода.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();

        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return;
        }

        let mut id: LUID = std::mem::zeroed();

        if LookupPrivilegeValueW(std::ptr::null(), wanted.as_ptr(), &mut id) != 0 {
            let privileges = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: id,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };

            AdjustTokenPrivileges(
                token,
                0,
                &privileges,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }

        CloseHandle(token);
    }
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// ── Ход работы через файл ────────────────────────────────────────────────
//
// Труба между нами и процессом с правами администратора не тянется: он нам не
// дочерний, его создаёт система по запросу. Общий файл — единственное, что
// пересекает эту границу.

/// Пишет строку хода: `объекты байты отказы готово`.
///
/// Через временный файл с переименованием, а не поверх старого. Переименование
/// на месте система делает целиком, и читающий никогда не застанет строку
/// наполовину записанной — а поверх старого застал бы, и показал бы бессмыслицу.
pub fn write_progress(plan: &Path, tally: &Tally, done: bool) {
    let line = format!(
        "{} {} {} {}",
        tally.objects,
        tally.bytes,
        tally.failed,
        u8::from(done)
    );

    let temp = plan.with_extension("part");

    if std::fs::write(&temp, line).is_ok() {
        let _ = std::fs::rename(&temp, plan);
    }
}

// ── Проверки ─────────────────────────────────────────────────────────────

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Заводит точку соединения. Прав администратора не требует.
    fn junction(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &link.display().to_string(),
                &target.display().to_string(),
            ])
            .status()
            .expect("mklink не запустился");

        assert!(status.success(), "точка соединения не создана");
    }

    fn playground(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qrx-purge-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("не создать площадку");
        dir
    }

    /// Главная проверка всего модуля: связь наружу удаляется как связь, а то,
    /// куда она ведёт, остаётся нетронутым.
    ///
    /// Ошибка здесь означала бы удаление работающей системы: `Windows.old`
    /// полон таких связей — `Documents and Settings`, `Application Data`,
    /// папки пользователей.
    #[test]
    fn link_is_not_followed() {
        let base = playground("link");
        let outside = base.join("живое");
        let root = base.join("под-снос");

        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("важное.txt"), b"trogat nelzya").unwrap();

        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("свой.txt"), b"pod snos").unwrap();
        junction(&root.join("связь"), &outside);

        let tally = sweep(&root, &mut |_| {});

        assert!(!root.exists(), "папку не убрали: {tally:?}", tally = tally.objects);
        assert!(outside.is_dir(), "обход ушёл по связи и снёс чужую папку");
        assert!(
            outside.join("важное.txt").is_file(),
            "файл за связью не пережил удаление"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Помётки «только чтение» и «скрытый» удалению не мешают, а длинный путь
    /// не упирается в старое ограничение в 260 знаков.
    #[test]
    fn stubborn_and_deep_go_away() {
        let base = playground("deep");
        let root = base.join("под-снос");

        // Глубина набирается кусками по 40 знаков: цель — перевалить за 260,
        // на которых обрывается обычная работа с путями в Windows
        let mut deep = root.clone();
        for step in 0..9 {
            deep = deep.join(format!("уровень-{step}-абвгдеёжзийклмнопрстуфхц"));
        }

        std::fs::create_dir_all(&deep).unwrap();
        assert!(
            deep.display().to_string().len() > 260,
            "путь вышел коротким, проверка ничего не проверяет"
        );

        let locked = deep.join("только-чтение.txt");
        std::fs::write(&locked, b"x").unwrap();

        let mut rights = std::fs::metadata(&locked).unwrap().permissions();
        rights.set_readonly(true);
        std::fs::set_permissions(&locked, rights).unwrap();

        let tally = sweep(&root, &mut |_| {});

        assert!(!root.exists(), "глубокое дерево осталось");
        assert_eq!(tally.failed, 0, "что-то не поддалось");
        assert!(tally.objects >= 10, "объекты не посчитаны: {}", tally.objects);
        assert!(tally.bytes >= 1, "освобождённое место не посчитано");

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Строка хода переживает запись и чтение без потерь.
    #[test]
    fn progress_survives_the_round_trip() {
        let base = playground("progress");
        let plan = base.join("ход.log");

        let tally = Tally {
            objects: 950_511,
            bytes: 50_500_479_110,
            failed: 88,
        };

        write_progress(&plan, &tally, true);

        let (back, done) = read_progress(&plan).expect("строка хода не прочиталась");

        assert_eq!(back.objects, tally.objects);
        assert_eq!(back.bytes, tally.bytes);
        assert_eq!(back.failed, tally.failed);
        assert!(done);

        let _ = std::fs::remove_dir_all(&base);
    }
}

/// Разбирает строку хода. Пусто — читать пока нечего.
pub fn read_progress(plan: &Path) -> Option<(Tally, bool)> {
    let text = std::fs::read_to_string(plan).ok()?;
    let mut parts = text.split_whitespace();

    let objects = parts.next()?.parse().ok()?;
    let bytes = parts.next()?.parse().ok()?;
    let failed = parts.next()?.parse().ok()?;
    let done = parts.next()? == "1";

    Some((
        Tally {
            objects,
            bytes,
            failed,
        },
        done,
    ))
}
