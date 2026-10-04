use super::*;
use crate::instance::{Loader, NewInstance};
use std::time::Duration;

fn exit(code: Option<i32>, was_running: bool, killed: bool) -> GameExit {
    GameExit {
        code,
        ran_for: Duration::from_secs(90),
        killed,
        was_running,
    }
}

fn change(at: u64) -> HistoryEvent {
    HistoryEvent::Change {
        at,
        kind: ChangeKind::ContentAdded,
        subject: format!("mod-{at}.jar"),
    }
}

#[test]
fn outcomes_follow_how_the_process_ended() {
    assert_eq!(
        SessionOutcome::of(&exit(Some(0), true, false)),
        SessionOutcome::Clean
    );
    assert_eq!(
        SessionOutcome::of(&exit(Some(1), true, false)),
        SessionOutcome::Crashed
    );
    assert_eq!(
        SessionOutcome::of(&exit(None, true, false)),
        SessionOutcome::Crashed
    );
    assert_eq!(
        SessionOutcome::of(&exit(Some(1), false, false)),
        SessionOutcome::FailedToStart
    );
    assert_eq!(
        SessionOutcome::of(&exit(None, true, true)),
        SessionOutcome::Stopped
    );
}

#[test]
fn appends_and_reads_back_in_order_with_newest_first_views() {
    let dir = tempfile::tempdir().unwrap();
    let log = HistoryLog::for_instance(dir.path(), "a");
    assert!(log.read().unwrap().events.is_empty());
    log.append(&change(1)).unwrap();
    log.append(&HistoryEvent::Session {
        started: 2,
        seconds: 5,
        exit_code: Some(0),
        outcome: SessionOutcome::Clean,
    })
    .unwrap();
    log.append(&change(3)).unwrap();
    assert_eq!(log.read().unwrap().events.len(), 3);
    let changes: Vec<_> = log
        .changes()
        .unwrap()
        .iter()
        .map(HistoryEvent::at)
        .collect();
    assert_eq!(changes, [3, 1]);
    assert_eq!(log.sessions().unwrap().len(), 1);
}

#[test]
fn a_torn_or_foreign_line_is_skipped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let log = HistoryLog::for_instance(dir.path(), "a");
    log.append(&change(1)).unwrap();
    let path = dir.path().join("profiles/a/history.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"event\":\"session\",\"start\n{\"event\":\"future_thing\"}\n\n");
    std::fs::write(&path, text).unwrap();
    log.append(&change(2)).unwrap();
    let read = log.read().unwrap();
    assert_eq!(read.events.len(), 2);
    assert_eq!(read.skipped, 2);
}

#[test]
fn compact_keeps_the_newest_events() {
    let dir = tempfile::tempdir().unwrap();
    let log = HistoryLog::for_instance(dir.path(), "a");
    for at in 1..=5 {
        log.append(&change(at)).unwrap();
    }
    log.compact(2).unwrap();
    let ats: Vec<_> = log
        .read()
        .unwrap()
        .events
        .iter()
        .map(HistoryEvent::at)
        .collect();
    assert_eq!(ats, [4, 5]);
    log.compact(100).unwrap();
    assert_eq!(log.read().unwrap().events.len(), 2);
}

#[test]
fn finishing_a_session_updates_the_log_and_the_instance_totals() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    store
        .create(
            NewInstance {
                name: "a".into(),
                game_version: "1.21.1".into(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            1,
        )
        .unwrap();
    finish_session(&mut store, "a", 1000, &exit(Some(0), true, false)).unwrap();
    finish_session(&mut store, "a", 2000, &exit(Some(1), true, false)).unwrap();
    let record = store.get("a").unwrap();
    assert_eq!(record.play_seconds, 180);
    assert_eq!(record.last_played, Some(2000));
    let sessions = HistoryLog::for_instance(dir.path(), "a")
        .sessions()
        .unwrap();
    assert!(matches!(
        sessions[0],
        HistoryEvent::Session {
            outcome: SessionOutcome::Crashed,
            started: 2000,
            ..
        }
    ));
}
