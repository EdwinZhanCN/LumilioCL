use super::super::Section;
use super::data::Arrived;
use super::data::Confirm;
use super::data::Data;
use super::labels::change_label;
use super::labels::duration_label;
use super::labels::export_notice;
use super::labels::outcome_label;
use super::labels::size_label;
use super::overview::ProblemAction;
use super::overview::problem_action;
use super::overview::problem_text;
use super::overview::recent_changes;
use super::overview::recent_sessions;
use super::worlds::ordered_worlds;
use super::worlds::playing_world;
use lumilio_core::{
    ChangeKind, HistoryEvent, HistoryRead, Problem, ProblemKind, ProjectKind, SessionOutcome,
    Severity, SnapshotInfo, SnapshotScope, WorldInfo,
};

fn world(folder: &str, name: &str, played: Option<i64>) -> WorldInfo {
    WorldInfo {
        folder: folder.into(),
        name: name.into(),
        last_played_ms: played,
        game_version: None,
        hardcore: false,
        has_icon: false,
        damaged: false,
        lock_touched_ms: None,
    }
}

#[test]
fn worlds_are_searched_by_name_or_folder_and_sorted_by_play_time_or_name() {
    let worlds = vec![
        world("a", "Zeta", Some(30)),
        world("b", "alpha", Some(10)),
        world("c-folder", "Mid", None),
    ];
    let names = |shown: Vec<&WorldInfo>| -> Vec<String> {
        shown.iter().map(|world| world.name.clone()).collect()
    };
    assert_eq!(
        names(ordered_worlds(&worlds, "", 0)),
        ["Zeta", "alpha", "Mid"]
    );
    assert_eq!(
        names(ordered_worlds(&worlds, "", 1)),
        ["alpha", "Mid", "Zeta"]
    );
    assert_eq!(names(ordered_worlds(&worlds, "ALP", 0)), ["alpha"]);
    assert_eq!(names(ordered_worlds(&worlds, " c-fol ", 0)), ["Mid"]);
    assert!(ordered_worlds(&worlds, "nothing", 0).is_empty());
}

#[test]
fn an_export_says_what_was_listed_and_what_was_carried() {
    let path = std::path::Path::new("/tmp/pack.mrpack");
    let report = lumilio_core::ExportReport {
        linked: 3,
        bundled: 2,
        lookup_failed: false,
        size: 10,
    };
    let text = export_notice(&report, path);
    assert!(text.contains("/tmp/pack.mrpack") && text.contains("3 个文件按地址列出"));
    assert!(text.contains("2 个放在包里"));
    let offline = lumilio_core::ExportReport {
        linked: 0,
        bundled: 5,
        lookup_failed: true,
        size: 10,
    };
    let text = export_notice(&offline, path);
    assert!(text.contains("没能连上 Modrinth") && text.contains("5 个文件"));
}

#[test]
fn a_confirmation_names_what_it_will_remove_and_tolerates_a_list_not_loaded() {
    let worlds = [world("w1", "Skyblock", Some(1))];
    let snapshots = [SnapshotInfo {
        id: "s1".into(),
        label: "before update".into(),
        scope: SnapshotScope::Full,
        created: 1,
        size: 10,
    }];
    let (title, _, ok) = Confirm::DeleteWorld("w1".into()).words(&worlds, &snapshots);
    assert!(title.contains("Skyblock") && ok == "删除");
    let (title, description, ok) = Confirm::RestoreSnapshot("s1".into()).words(&worlds, &snapshots);
    assert!(title.contains("before update") && ok == "恢复");
    assert!(description.contains("自动存一份"));
    let (title, _, _) = Confirm::DeleteSnapshot("s1".into()).words(&worlds, &snapshots);
    assert!(title.contains("before update"));
    // Not loaded: it still asks, by folder or in general terms.
    assert!(
        Confirm::DeleteWorld("x".into())
            .words(&[], &[])
            .0
            .contains('x')
    );
    assert!(
        Confirm::DeleteSnapshot("?".into())
            .words(&[], &[])
            .0
            .contains("这份快照")
    );
}

#[test]
fn every_problem_with_something_to_do_names_it() {
    use lumilio_core::Loader;
    let cases = [
        (ProblemKind::NotInstalled, ProblemAction::Install),
        (
            ProblemKind::LoaderUnsupported(Loader::Forge),
            ProblemAction::ChangeRuntime,
        ),
        (
            ProblemKind::NoJava { required: Some(21) },
            ProblemAction::InstallJava(Some(21)),
        ),
        (ProblemKind::NoAccount, ProblemAction::Accounts),
        (
            ProblemKind::DamagedFiles { count: 3 },
            ProblemAction::Repair,
        ),
        (
            ProblemKind::WrongLoaderMods { names: vec![] },
            ProblemAction::Content,
        ),
        (
            ProblemKind::DuplicateMods { ids: vec![] },
            ProblemAction::Content,
        ),
        (
            ProblemKind::LowMemory { max_mb: 512 },
            ProblemAction::Settings(3),
        ),
        (
            ProblemKind::LastSessionFailed(SessionOutcome::Crashed),
            ProblemAction::Logs,
        ),
    ];
    for (kind, action) in cases {
        let (found, label) = problem_action(&kind).unwrap();
        assert_eq!(found, action);
        assert!(!label.is_empty());
    }
}

#[test]
fn the_world_being_played_is_the_newest_one_opened_since_the_game_started() {
    let mut worlds = vec![
        world("a", "A", None),
        world("b", "B", None),
        world("c", "C", None),
    ];
    assert_eq!(
        playing_world(&worlds, 1_000),
        None,
        "no lock, nothing opened"
    );
    worlds[0].lock_touched_ms = Some(500); // from an earlier session
    worlds[1].lock_touched_ms = Some(1_200);
    worlds[2].lock_touched_ms = Some(1_900);
    assert_eq!(playing_world(&worlds, 1_000), Some("c"));
    assert_eq!(playing_world(&worlds, 2_000), None);
    assert_eq!(playing_world(&worlds, 0), Some("c"));
}

#[test]
fn the_overview_shows_the_newest_three_of_each_kind_newest_first() {
    let session = |started| HistoryEvent::Session {
        started,
        seconds: 60,
        exit_code: None,
        outcome: SessionOutcome::Clean,
    };
    let change = |at: u64| HistoryEvent::Change {
        at,
        kind: ChangeKind::ContentAdded,
        subject: format!("m{at}"),
    };
    let read = HistoryRead {
        events: vec![
            session(1),
            change(2),
            session(3),
            session(4),
            change(5),
            session(6),
            change(7),
            change(8),
        ],
        skipped: 0,
    };
    let starts: Vec<_> = recent_sessions(&read, 3).iter().map(|s| s.0).collect();
    assert_eq!(starts, [6, 4, 3]);
    let changes: Vec<_> = recent_changes(&read, 3).iter().map(|c| c.0).collect();
    assert_eq!(changes, [8, 7, 5]);
    assert!(recent_sessions(&HistoryRead::default(), 3).is_empty());
}

#[test]
fn durations_and_sizes_read_naturally() {
    assert_eq!(duration_label(10), "不到 1 分钟");
    assert_eq!(duration_label(125), "2 分钟");
    assert_eq!(duration_label(3 * 3600 + 5 * 60), "3 小时 5 分钟");
    assert_eq!(size_label(0), "—");
    assert_eq!(size_label(900), "900 B");
    assert_eq!(size_label(2048), "2 KB");
    assert_eq!(size_label(5 * 1024 * 1024), "5.0 MB");
}

#[test]
fn every_outcome_and_change_has_copy_and_unknown_results_say_so() {
    assert!(outcome_label(SessionOutcome::Interrupted).contains("未知"));
    assert_eq!(change_label(ChangeKind::SnapshotRestored), "恢复了快照");
}

#[test]
fn problems_name_their_cause_without_raw_debug_text() {
    let text = |kind| {
        problem_text(&Problem {
            severity: Severity::Warning,
            kind,
        })
    };
    let (title, detail) = text(ProblemKind::NoJava { required: Some(21) });
    assert!(title.contains("Java") && detail.contains("21"));
    let (_, detail) = text(ProblemKind::DamagedFiles { count: 3 });
    assert!(detail.contains('3'));
    let (title, _) = text(ProblemKind::LastSessionFailed(SessionOutcome::Crashed));
    assert!(!title.contains("Crashed"));
}

#[test]
fn store_clears_pending_and_reports_presence() {
    let mut data = Data::default();
    data.pending.push(Section::Worlds);
    assert!(!data.has(Section::Worlds));
    data.store(Arrived::Worlds(Ok(Vec::new())));
    assert!(data.has(Section::Worlds) && data.pending.is_empty());
    data.store(Arrived::Content(ProjectKind::Shader, Err("x".into())));
    assert!(data.has(Section::Content(ProjectKind::Shader)));
    assert!(!data.has(Section::Content(ProjectKind::Mod)));
}
