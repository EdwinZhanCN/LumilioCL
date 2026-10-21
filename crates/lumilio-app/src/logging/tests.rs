use super::FileLog;
use log::{Level, Log, Record};

fn line(log: &FileLog, level: Level, target: &str, text: &str) {
    log.log(
        &Record::builder()
            .level(level)
            .target(target)
            .args(format_args!("{text}"))
            .build(),
    );
}

#[test]
fn the_launchers_notes_and_everyones_problems_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let current = dir.path().join("logs/launcher.log");
    let log = FileLog::new(
        current.clone(),
        dir.path().join("logs/launcher.1.log"),
        1 << 20,
    );
    line(&log, Level::Info, "lumilio_app::live", "started");
    line(&log, Level::Debug, "lumilio_app::live", "chatter");
    line(&log, Level::Info, "gpui::window", "a frame");
    line(&log, Level::Warn, "gpui::platform", "no display link");
    line(
        &log,
        Level::Error,
        "lumilio_ui::toast",
        "没有装上 Sodium: disk full",
    );
    let text = std::fs::read_to_string(&current).unwrap();
    assert!(text.contains("INFO  lumilio_app::live: started"), "{text}");
    assert!(
        text.contains("WARN  gpui::platform: no display link"),
        "{text}"
    );
    assert!(text.contains("没有装上 Sodium: disk full"), "{text}");
    assert!(
        !text.contains("chatter") && !text.contains("a frame"),
        "{text}"
    );
}

#[test]
fn a_full_log_starts_over_and_keeps_the_one_before() {
    let dir = tempfile::tempdir().unwrap();
    let current = dir.path().join("logs/launcher.log");
    let earlier = dir.path().join("logs/launcher.1.log");
    let log = FileLog::new(current.clone(), earlier.clone(), 200);
    for n in 0..20 {
        line(&log, Level::Warn, "lumilio_app", &format!("problem {n}"));
    }
    let now = std::fs::read_to_string(&current).unwrap();
    let before = std::fs::read_to_string(&earlier).unwrap();
    assert!(now.contains("problem 19"), "{now}");
    assert!(!now.contains("problem 0\n"), "{now}");
    assert!(before.contains("problem"), "{before}");
    let size = |path| std::fs::metadata(path).unwrap().len();
    assert!(size(&current) < 400 && size(&earlier) < 400);
}
