use tokio::sync::broadcast;

use super::{Meter, Tasks, Traffic};
use crate::activity::{
    ActiveTask, ActivityLog, FinishedTask, TaskCategory, TaskLabel, TaskOutcome,
};
use crate::error::ServiceError;
use crate::transfer::{TransferEvent, TransferOutcome};

fn tasks() -> (Tasks, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    (Tasks::new(ActivityLog::open(dir.path())), dir)
}

fn label(text: &str) -> TaskLabel {
    TaskLabel::Text(text.to_owned())
}

fn shown(tasks: &Tasks, id: u64) -> ActiveTask {
    tasks
        .view(10, Traffic::default())
        .active
        .into_iter()
        .find(|task| task.id == id)
        .unwrap()
}

#[tokio::test]
async fn clearing_finished_work_empties_the_history_and_keeps_running_tasks() {
    let (tasks, _dir) = tasks();
    let finished = |text: &str| FinishedTask {
        category: TaskCategory::Download,
        label: label(text),
        instance_id: None,
        started: 1,
        finished: 2,
        outcome: TaskOutcome::Succeeded,
        retry: None,
    };
    tasks.log.append(&finished("one")).unwrap();
    tasks.log.append(&finished("two")).unwrap();
    let running = tasks.begin(TaskCategory::Install, label("running"), None, None);
    assert_eq!(tasks.view(10, Traffic::default()).finished.len(), 2);

    tasks.clear_finished().unwrap();

    let view = tasks.view(10, Traffic::default());
    assert!(view.finished.is_empty());
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].id, running.id);
}

#[tokio::test]
async fn the_bytes_of_all_transfers_add_up_to_one_task_and_an_unknown_length_hides_the_total() {
    let (tasks, _dir) = tasks();
    let task = tasks.begin(TaskCategory::Download, label("x"), None, None);
    let (events, receiver) = broadcast::channel(16);
    let progress = |id: &str, completed, total| TransferEvent::Progress {
        id: id.to_owned(),
        completed,
        total,
    };
    events.send(progress("a", 10, Some(100))).unwrap();
    events.send(progress("b", 5, Some(50))).unwrap();
    events.send(progress("a", 40, Some(100))).unwrap();
    drop(events);
    tasks.watch_transfers(task.id, receiver, Meter::Bytes).await;
    let first = shown(&tasks, task.id);
    assert_eq!(first.unit, crate::ProgressUnit::Bytes);
    assert_eq!(first.progress, Some((45, 150)));

    let task = tasks.begin(TaskCategory::Download, label("y"), None, None);
    let (events, receiver) = broadcast::channel(16);
    events.send(progress("c", 7, None)).unwrap();
    events.send(progress("d", 3, Some(30))).unwrap();
    drop(events);
    tasks.watch_transfers(task.id, receiver, Meter::Bytes).await;
    assert_eq!(
        shown(&tasks, task.id).progress,
        Some((10, 0)),
        "total 0 means unknown"
    );
}

#[tokio::test]
async fn a_task_shows_its_files_their_sources_and_the_sources_given_up() {
    let (tasks, _dir) = tasks();
    let task = tasks.begin(TaskCategory::Install, label("x"), None, None);
    let (events, receiver) = broadcast::channel(16);
    let id = || "lib".to_owned();
    for event in [
        TransferEvent::Started {
            id: id(),
            destination: "libraries/a/lib.jar".into(),
        },
        TransferEvent::Started {
            id: "done".to_owned(),
            destination: "done.jar".into(),
        },
        TransferEvent::Downloading {
            id: id(),
            source: "https://mirror.test/lib.jar".to_owned(),
            attempt: 1,
        },
        TransferEvent::SourceFailed {
            id: id(),
            source: "https://mirror.test/lib.jar".to_owned(),
            reason: "HTTP status 404".to_owned(),
            next: Some("https://official.test/lib.jar".to_owned()),
        },
        TransferEvent::Downloading {
            id: id(),
            source: "https://official.test/lib.jar".to_owned(),
            attempt: 1,
        },
        TransferEvent::Connections { id: id(), count: 3 },
        TransferEvent::Progress {
            id: id(),
            completed: 40,
            total: Some(100),
        },
        TransferEvent::Completed {
            id: "done".to_owned(),
            outcome: TransferOutcome::ReusedExisting,
        },
    ] {
        events.send(event).unwrap();
    }
    drop(events);
    tasks.watch_transfers(task.id, receiver, Meter::Files).await;

    let shown = shown(&tasks, task.id);
    assert_eq!(shown.progress, None, "files are counted by the launcher");
    let downloads = shown.downloads.unwrap();
    assert_eq!(downloads.files.len(), 1);
    let file = &downloads.files[0];
    assert_eq!(file.name, "lib.jar");
    assert_eq!(
        file.source.as_deref(),
        Some("https://official.test/lib.jar")
    );
    assert_eq!(
        (file.connections, file.done, file.total),
        (3, 40, Some(100))
    );
    assert_eq!(downloads.switches.len(), 1);
    assert_eq!(downloads.switches[0].file, "lib.jar");
    assert_eq!(
        downloads.switches[0].to.as_deref(),
        Some("https://official.test/lib.jar")
    );
}

#[tokio::test]
async fn log_failure_keeps_terminal_results_visible_without_resurrecting_tasks() {
    let (tasks, dir) = tasks();
    let log_path = dir.path().join("activity.jsonl");
    std::fs::create_dir(&log_path).unwrap();
    let result: Result<(), ServiceError> = Ok(());
    for _ in 0..2 {
        let task = tasks.begin(TaskCategory::Install, label("same operation"), None, None);
        tasks.end(task, &result);
    }
    let activity = tasks.view(10, Traffic::default());
    assert!(activity.active.is_empty());
    assert_eq!(activity.finished.len(), 2);
    assert!(
        activity
            .finished
            .iter()
            .all(|task| task.outcome == TaskOutcome::Succeeded)
    );
    assert!(tasks.view(0, Traffic::default()).finished.is_empty());
    std::fs::remove_dir(log_path).unwrap();
    assert_eq!(tasks.view(10, Traffic::default()).finished.len(), 2);
    assert!(tasks.view(10, Traffic::default()).active.is_empty());
}
