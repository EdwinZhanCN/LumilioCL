use lumilio_core::FileEntry;

use super::helpers::{
    crash_for_session, files_matching, join_path, output_for_session, parent_path,
};

#[test]
fn the_launch_output_belongs_only_to_the_session_that_wrote_it() {
    let entry = |name: &str, modified| FileEntry {
        name: name.into(),
        is_dir: false,
        size: 0,
        modified,
    };
    let files = [
        entry("debug.log", 150),
        entry(lumilio_core::GAME_OUTPUT_LOG, 700),
    ];
    // The last launch began at 100 and installed for ten minutes first.
    assert_eq!(
        output_for_session(&files, 100, true).map(|file| file.name.as_str()),
        Some(lumilio_core::GAME_OUTPUT_LOG)
    );
    // An earlier session: the file has been written over since.
    assert!(output_for_session(&files, 10, false).is_none());
    // The last launch failed before the game started; the file is older.
    assert!(output_for_session(&files, 800, true).is_none());
    assert!(output_for_session(&files[..1], 100, true).is_none());
}

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
