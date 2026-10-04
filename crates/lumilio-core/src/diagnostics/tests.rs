use super::*;
use crate::instance::InstanceSettings;

#[test]
fn levels_come_from_the_bracket_groups_and_continuations_inherit() {
    assert_eq!(
        log_level("[21:04:11] [main/INFO]: Loading"),
        Some(LogLevel::Info)
    );
    assert_eq!(
        log_level("[21:04:11] [Render thread/WARN] [net.minecraft/]: x"),
        Some(LogLevel::Warn)
    );
    assert_eq!(
        log_level("[21:04:11] [Server thread/FATAL]: x"),
        Some(LogLevel::Error)
    );
    assert_eq!(log_level("[21:04:11] [ERROR]: x"), Some(LogLevel::Error));
    assert_eq!(
        log_level("[21:04:11] [main/TRACE]: x"),
        Some(LogLevel::Debug)
    );
    assert_eq!(log_level("\tat net.minecraft.Foo(Foo.java:1)"), None);
    assert_eq!(log_level("[not a level] plain"), None);
    assert_eq!(log_level("a [main/ERROR] mid-line mention"), None);

    let text = "boot line\n[1] [main/INFO]: fine\n[2] [main/ERROR]: boom\n\tat a.B(B.java)\n[3] [main/INFO]: ok";
    let lines = log_lines(text);
    let levels: Vec<_> = lines.iter().map(|line| line.level).collect();
    assert_eq!(
        levels,
        [
            LogLevel::Info,
            LogLevel::Info,
            LogLevel::Error,
            LogLevel::Error,
            LogLevel::Info
        ]
    );
}

#[test]
fn filtering_keeps_the_level_and_the_search_together() {
    let text = "[1] [main/INFO]: loaded sodium\n[2] [main/WARN]: Sodium is old\n[3] [main/ERROR]: crash\n\tat sodium.Thing";
    let lines = log_lines(text);
    assert_eq!(filter_log(&lines, None, "").len(), 4);
    assert_eq!(filter_log(&lines, Some(LogLevel::Warn), "").len(), 3);
    assert_eq!(filter_log(&lines, Some(LogLevel::Error), "").len(), 2);
    let found = filter_log(&lines, Some(LogLevel::Warn), "SODIUM");
    assert_eq!(
        found.len(),
        2,
        "case-insensitive, trace line inherits error"
    );
    assert!(filter_log(&lines, None, "nothing like this").is_empty());
}

fn record(loader: Loader, installed: bool) -> InstanceRecord {
    InstanceRecord {
        id: "a".into(),
        name: "A".into(),
        game_version: "1.21.1".into(),
        loader,
        loader_version: None,
        favorite: false,
        created_at: 0,
        last_played: None,
        play_seconds: 0,
        installed,
        settings: InstanceSettings::default(),
    }
}

fn java(major: u32) -> JavaRuntime {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("j");
    fs::create_dir_all(home.join("bin")).unwrap();
    fs::write(home.join("bin/java"), "").unwrap();
    fs::write(home.join("release"), format!("JAVA_VERSION=\"{major}\"")).unwrap();
    // Only the parsed values are used after the temp dir goes away.
    JavaRuntime::inspect(&home).unwrap()
}

fn item(name: &str, enabled: bool) -> ContentItem {
    ContentItem {
        file_name: name.into(),
        display_name: name.into(),
        enabled,
        size: 1,
        modified: 0,
        is_directory: false,
    }
}

fn meta(id: &str, loader: &'static str) -> Option<ModMetadata> {
    Some(ModMetadata {
        id: id.into(),
        name: None,
        version: None,
        loader,
    })
}

fn healthy<'a>(instance: &'a InstanceRecord, runtimes: &'a [JavaRuntime]) -> Facts<'a> {
    Facts {
        instance,
        launchable_loaders: &[Loader::Vanilla, Loader::Fabric],
        runtimes,
        required_java: Some(21),
        has_account: true,
        damaged_files: Some(0),
        mods: &[],
        max_memory_mb: Some(4096),
        last_session: Some(SessionOutcome::Clean),
    }
}

#[test]
fn a_healthy_installed_instance_has_no_problems() {
    let instance = record(Loader::Vanilla, true);
    let runtimes = [java(21)];
    assert!(diagnose(&healthy(&instance, &runtimes)).is_empty());
}

#[test]
fn blocking_problems_are_errors_and_sort_first() {
    let instance = record(Loader::Forge, false);
    let mut facts = healthy(&instance, &[]);
    facts.has_account = false;
    facts.damaged_files = Some(3);
    let problems = diagnose(&facts);
    let kinds: Vec<_> = problems
        .iter()
        .map(|p| (p.severity, p.kind.clone()))
        .collect();
    assert_eq!(kinds[0].0, Severity::Error);
    assert!(kinds.contains(&(
        Severity::Error,
        ProblemKind::LoaderUnsupported(Loader::Forge)
    )));
    assert!(kinds.contains(&(Severity::Error, ProblemKind::NoAccount)));
    assert!(kinds.contains(&(Severity::Error, ProblemKind::NoJava { required: Some(21) })));
    assert!(kinds.contains(&(Severity::Info, ProblemKind::NotInstalled)));
    assert!(kinds.contains(&(Severity::Warning, ProblemKind::DamagedFiles { count: 3 })));
    assert!(problems.windows(2).all(|w| w[0].severity >= w[1].severity));
}

#[test]
fn an_older_java_is_not_enough_but_a_newer_one_is() {
    let instance = record(Loader::Vanilla, true);
    let old = [java(17)];
    assert!(
        diagnose(&healthy(&instance, &old))
            .iter()
            .any(|p| matches!(p.kind, ProblemKind::NoJava { .. }))
    );
    let newer = [java(25)];
    assert!(diagnose(&healthy(&instance, &newer)).is_empty());
}

#[test]
fn mod_problems_ignore_disabled_files_and_unreadable_metadata() {
    let instance = record(Loader::Fabric, true);
    let runtimes = [java(21)];
    let mods = vec![
        (item("a.jar", true), meta("sodium", "fabric")),
        (item("a-old.jar", true), meta("sodium", "fabric")),
        (item("forge-only.jar", true), meta("jei", "forge")),
        (item("off.jar", false), meta("jei", "forge")),
        (item("mystery.jar", true), None),
    ];
    let mut facts = healthy(&instance, &runtimes);
    facts.mods = &mods;
    let kinds: Vec<_> = diagnose(&facts).into_iter().map(|p| p.kind).collect();
    assert!(kinds.contains(&ProblemKind::DuplicateMods {
        ids: vec!["sodium".into()]
    }));
    assert!(kinds.contains(&ProblemKind::WrongLoaderMods {
        names: vec!["forge-only.jar".into()]
    }));
}

#[test]
fn memory_and_last_session_warnings() {
    let instance = record(Loader::Vanilla, true);
    let runtimes = [java(21)];
    let mut facts = healthy(&instance, &runtimes);
    facts.max_memory_mb = Some(512);
    facts.last_session = Some(SessionOutcome::Crashed);
    let kinds: Vec<_> = diagnose(&facts).into_iter().map(|p| p.kind).collect();
    assert!(kinds.contains(&ProblemKind::LowMemory { max_mb: 512 }));
    assert!(kinds.contains(&ProblemKind::LastSessionFailed(SessionOutcome::Crashed)));
    facts.max_memory_mb = None;
    facts.last_session = Some(SessionOutcome::Stopped);
    assert!(
        diagnose(&facts).is_empty(),
        "no limit set and a user-stopped run are not problems"
    );
}

#[test]
fn the_log_tail_starts_on_a_line_boundary() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_latest_log(dir.path(), 100).unwrap(), None);
    fs::create_dir_all(dir.path().join("logs")).unwrap();
    fs::write(
        dir.path().join("logs/latest.log"),
        "line one\nline two\nline three\n",
    )
    .unwrap();
    assert_eq!(
        read_latest_log(dir.path(), 1000).unwrap().unwrap(),
        "line one\nline two\nline three\n"
    );
    // 15 bytes back lands mid "line two"; the partial line is dropped.
    assert_eq!(
        read_latest_log(dir.path(), 15).unwrap().unwrap(),
        "line three\n"
    );
}

#[test]
fn crash_reports_list_newest_first_and_read_by_name_only() {
    let dir = tempfile::tempdir().unwrap();
    assert!(list_crash_reports(dir.path()).unwrap().is_empty());
    let folder = dir.path().join("crash-reports");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("crash-b.txt"), "second").unwrap();
    fs::write(folder.join("crash-a.txt"), "first").unwrap();
    fs::write(folder.join("notes.md"), "x").unwrap();
    let reports = list_crash_reports(dir.path()).unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(
        read_crash_report(dir.path(), "crash-a.txt", 100).unwrap(),
        "first"
    );
    assert_eq!(
        read_crash_report(dir.path(), "crash-a.txt", 3).unwrap(),
        "fir"
    );
    assert!(read_crash_report(dir.path(), "../x", 10).is_err());
}

#[test]
fn known_failures_are_recognized_once_each_in_order() {
    let text = "Exception in thread main java.lang.OutOfMemoryError: Java heap space\n\
                java.lang.UnsupportedClassVersionError: bad\n\
                Mixin apply failed x\nMixin apply failed y\n";
    assert_eq!(
        analyze(text),
        [
            CrashHint::OutOfMemory,
            CrashHint::JavaTooOld,
            CrashHint::ModConflict
        ]
    );
    assert!(analyze("all good").is_empty());
    assert_eq!(
        analyze("Unrecognized option: -XX:Foo"),
        [CrashHint::BadJvmOption]
    );
}

#[test]
fn listing_files_is_confined_to_the_game_directory() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(game.join("config")).unwrap();
    fs::write(game.join("options.txt"), "12345").unwrap();
    fs::write(game.join("Alpha.txt"), "1").unwrap();
    fs::write(dir.path().join("secret.txt"), "no").unwrap();
    let root = list_dir(&game, "").unwrap();
    let names: Vec<_> = root.iter().map(|e| (e.name.as_str(), e.is_dir)).collect();
    assert_eq!(
        names,
        [
            ("config", true),
            ("Alpha.txt", false),
            ("options.txt", false)
        ]
    );
    assert_eq!(root[2].size, 5);
    assert!(list_dir(&game, "config/").unwrap().is_empty());
    for bad in ["..", "config/../..", "/etc", "a\\b", "C:"] {
        assert!(list_dir(&game, bad).is_err(), "{bad}");
    }
}
