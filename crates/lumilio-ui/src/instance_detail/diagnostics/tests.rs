use super::helpers::crash_for_session;
use super::helpers::files_matching;
use super::helpers::join_path;
use super::helpers::least_level;
use super::helpers::parent_path;
use lumilio_core::{FileEntry, LogLevel};

#[test]
fn paths_join_and_climb_one_folder_at_a_time() {
    assert_eq!(join_path("", "config"), "config");
    assert_eq!(join_path("config", "sodium.json"), "config/sodium.json");
    assert_eq!(parent_path("config/sodium"), "config");
    assert_eq!(parent_path("config"), "");
    assert_eq!(parent_path(""), "");
}

#[test]
fn the_file_search_matches_names_ignoring_case_and_empty_shows_all() {
    let entry = |name: &str| FileEntry {
        name: name.into(),
        is_dir: false,
        size: 0,
        modified: 0,
    };
    let entries = [
        entry("Sodium.json"),
        entry("options.txt"),
        entry("sodium-extra.toml"),
    ];
    assert_eq!(files_matching(&entries, "").len(), 3);
    assert_eq!(files_matching(&entries, " SODIUM ").len(), 2);
    assert!(files_matching(&entries, "zzz").is_empty());
}

#[test]
fn a_crash_report_belongs_to_the_session_it_was_written_in() {
    let report = |name: &str, modified| lumilio_core::CrashReport {
        file_name: name.into(),
        modified,
    };
    let reports = [
        report("before.txt", 90),
        report("during.txt", 150),
        report("end.txt", 198),
        report("late.txt", 400),
    ];
    // A session from 100 to 200.
    assert_eq!(
        crash_for_session(&reports, 100, 100).map(|r| r.file_name.as_str()),
        Some("end.txt"),
        "the one nearest the end"
    );
    assert!(crash_for_session(&reports, 1000, 50).is_none());
    assert_eq!(
        crash_for_session(&[report("a.txt", 250)], 100, 100).map(|r| r.file_name.as_str()),
        Some("a.txt"),
        "a little after the end still counts"
    );
    assert!(crash_for_session(&[report("a.txt", 400)], 100, 100).is_none());
}

#[test]
fn a_level_choice_shows_that_level_and_worse() {
    assert_eq!(least_level(0), None);
    assert_eq!(least_level(1), Some(LogLevel::Error));
    assert_eq!(least_level(2), Some(LogLevel::Warn));
    assert_eq!(least_level(3), Some(LogLevel::Info));
}
