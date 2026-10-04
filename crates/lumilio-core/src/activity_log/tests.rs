use super::*;

fn log() -> (tempfile::TempDir, ActivityLog) {
    let dir = tempfile::tempdir().unwrap();
    let log = ActivityLog::open(dir.path());
    (dir, log)
}

#[test]
fn failed_log_write_keeps_one_terminal_fact_and_never_revives_it() {
    let (dir, log) = log();
    fs::create_dir(dir.path().join("activity.jsonl")).unwrap();
    let mut board = TaskBoard::default();
    let id = board.start(TaskCategory::Install, "x", None, 10);
    assert!(board.finish(id, TaskOutcome::Succeeded, 5, &log).is_err());
    assert_eq!(board.active_count(), 0);
    assert_eq!(board.unrecorded().len(), 1);
    assert_eq!(board.unrecorded()[0].finished, 10);
    board.progress(id, 1, 1);
    assert!(
        board
            .finish(id, TaskOutcome::Cancelled, 20, &log)
            .unwrap()
            .is_none()
    );
    assert_eq!(board.active_count(), 0);
    assert_eq!(board.unrecorded().len(), 1);
    assert_eq!(board.unrecorded()[0].outcome, TaskOutcome::Succeeded);
}

#[test]
fn a_finished_task_leaves_the_board_and_enters_the_log() {
    let (dir, log) = log();
    let mut board = TaskBoard::default();
    let id = board.start(TaskCategory::Install, "1.21.1", Some("a".into()), 10);
    assert_eq!(board.active_count(), 1);
    let done = board
        .finish(id, TaskOutcome::Succeeded, 25, &log)
        .unwrap()
        .unwrap();
    assert_eq!((done.started, done.finished), (10, 25));
    assert_eq!(board.active_count(), 0);
    // It survives a "restart": a fresh log handle reads it back.
    let recent = ActivityLog::open(dir.path()).recent(None, 10).unwrap();
    assert_eq!(recent, [done]);
}

#[test]
fn finishing_twice_or_an_unknown_id_records_nothing() {
    let (_dir, log) = log();
    let mut board = TaskBoard::default();
    let id = board.start(TaskCategory::Download, "x", None, 1);
    assert!(
        board
            .finish(id, TaskOutcome::Cancelled, 2, &log)
            .unwrap()
            .is_some()
    );
    assert!(
        board
            .finish(id, TaskOutcome::Succeeded, 3, &log)
            .unwrap()
            .is_none()
    );
    assert!(
        board
            .finish(999, TaskOutcome::Succeeded, 3, &log)
            .unwrap()
            .is_none()
    );
    assert_eq!(log.read().unwrap().0.len(), 1);
}

#[test]
fn a_log_line_from_before_retry_existed_still_reads_and_a_retry_survives_the_log() {
    let (_dir, log) = log();
    std::fs::write(
        log.path.clone(),
        "{\"category\":\"install\",\"label\":\"old\",\"instance_id\":null,\"started\":1,\"finished\":2,\"outcome\":{\"result\":\"succeeded\"}}\n",
    )
    .unwrap();
    let mut board = TaskBoard::default();
    let id = board.start(TaskCategory::Download, "new", Some("a".into()), 3);
    board.set_retry(
        id,
        RetryAction::InstallModpack {
            project: "pack".into(),
        },
    );
    board
        .finish(id, TaskOutcome::Failed("no route".into()), 4, &log)
        .unwrap();
    let (tasks, skipped) = log.read().unwrap();
    assert_eq!(skipped, 0);
    assert_eq!(tasks[0].retry, None);
    assert_eq!(
        tasks[1].retry,
        Some(RetryAction::InstallModpack {
            project: "pack".into()
        })
    );
}

#[test]
fn progress_is_monotonic_and_bounded() {
    let mut board = TaskBoard::default();
    let id = board.start(TaskCategory::Download, "x", None, 1);
    board.progress(id, 5, 10);
    board.progress(id, 3, 10);
    assert_eq!(board.active()[0].progress, Some((5, 10)));
    board.progress(id, 50, 10);
    assert_eq!(board.active()[0].progress, Some((10, 10)));
    board.progress(12345, 1, 1);
    assert_eq!(board.active().len(), 1);
}

#[test]
fn a_clock_that_goes_backwards_never_makes_a_negative_duration() {
    let (_dir, log) = log();
    let mut board = TaskBoard::default();
    let id = board.start(TaskCategory::Repair, "x", None, 100);
    let done = board
        .finish(id, TaskOutcome::Succeeded, 50, &log)
        .unwrap()
        .unwrap();
    assert_eq!(done.finished, 100);
}

#[test]
fn views_filter_by_category_newest_first_and_ids_are_distinct() {
    let (_dir, log) = log();
    let mut board = TaskBoard::default();
    let a = board.start(TaskCategory::Install, "a", None, 1);
    let b = board.start(TaskCategory::Update, "b", None, 2);
    let c = board.start(TaskCategory::Install, "c", None, 3);
    assert!(a != b && b != c);
    assert_eq!(board.active_in(TaskCategory::Install).count(), 2);
    for (id, at) in [(a, 5), (b, 6), (c, 7)] {
        board
            .finish(id, TaskOutcome::Failed("boom".into()), at, &log)
            .unwrap();
    }
    let installs = log.recent(Some(TaskCategory::Install), 10).unwrap();
    let labels: Vec<_> = installs.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(labels, ["c", "a"]);
    assert_eq!(installs[0].outcome, TaskOutcome::Failed("boom".into()));
    assert_eq!(log.recent(None, 2).unwrap().len(), 2);
}

#[test]
fn torn_lines_are_skipped_and_compact_keeps_the_newest() {
    let (dir, log) = log();
    let mut board = TaskBoard::default();
    for n in 1..=4 {
        let id = board.start(TaskCategory::Download, format!("t{n}"), None, n);
        board.finish(id, TaskOutcome::Succeeded, n, &log).unwrap();
    }
    let path = dir.path().join("activity.jsonl");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("{\"category\":\"down\n");
    fs::write(&path, text).unwrap();
    let (tasks, skipped) = log.read().unwrap();
    assert_eq!((tasks.len(), skipped), (4, 1));
    log.compact(2).unwrap();
    let labels: Vec<_> = log.read().unwrap().0.into_iter().map(|t| t.label).collect();
    assert_eq!(labels, ["t3", "t4"]);
}
