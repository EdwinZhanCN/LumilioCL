//! What the Activity page shows: running tasks and a durable history.
//!
//! Behavior notes: `docs/behavior/activity-log.md`. The [`TaskBoard`] holds
//! running tasks in memory; finishing one appends it to the [`ActivityLog`]
//! (JSON lines), so history survives restarts.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskCategory {
    Download,
    Install,
    Update,
    Repair,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", content = "message", rename_all = "snake_case")]
pub enum TaskOutcome {
    Succeeded,
    Failed(String),
    Cancelled,
}

/// What it takes to run a task again with the same input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RetryAction {
    InstallInstance {
        instance: String,
    },
    RepairInstance {
        instance: String,
    },
    ChangeRuntime {
        instance: String,
        game_version: String,
        loader: crate::instance::Loader,
        loader_version: Option<String>,
    },
    InstallContent {
        instance: String,
        kind: crate::discover::ProjectKind,
        project: String,
        version: Option<String>,
    },
    SwitchContent {
        instance: String,
        kind: crate::discover::ProjectKind,
        file_name: String,
        project: String,
        version_id: String,
    },
    CopyInstance {
        source: String,
        name: String,
        include_worlds: bool,
    },
    InstallModpack {
        project: String,
    },
    BackupInstance {
        instance: String,
        path: String,
    },
    RestoreBackup {
        path: String,
    },
    InstallJava {
        major: Option<u32>,
    },
    SaveVersion {
        project: String,
        version_id: String,
        path: String,
    },
    ImportPack {
        path: String,
    },
}

/// What a task's progress numbers count.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressUnit {
    /// Files (or other whole steps).
    #[default]
    Items,
    Bytes,
}

/// A finished task, as kept in the log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FinishedTask {
    pub category: TaskCategory,
    pub label: String,
    pub instance_id: Option<String>,
    pub started: u64,
    pub finished: u64,
    pub outcome: TaskOutcome,
    /// How to run it again; older log lines have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveTask {
    pub id: u64,
    pub category: TaskCategory,
    pub label: String,
    pub instance_id: Option<String>,
    pub started: u64,
    /// Done and total in `unit`, if known.
    pub progress: Option<(u64, u64)>,
    pub unit: ProgressUnit,
    pub retry: Option<RetryAction>,
}

/// Running tasks. Not thread-safe by itself; share it behind a lock.
#[derive(Debug, Default)]
pub struct TaskBoard {
    next_id: u64,
    active: Vec<ActiveTask>,
    unrecorded: Vec<FinishedTask>,
}

impl TaskBoard {
    /// Registers a running task and returns its id.
    pub fn start(
        &mut self,
        category: TaskCategory,
        label: impl Into<String>,
        instance_id: Option<String>,
        now: u64,
    ) -> u64 {
        self.next_id += 1;
        self.active.push(ActiveTask {
            id: self.next_id,
            category,
            label: label.into(),
            instance_id,
            started: now,
            progress: None,
            unit: ProgressUnit::Items,
            retry: None,
        });
        self.next_id
    }

    /// Remembers how to run a task again if it fails.
    pub fn set_retry(&mut self, id: u64, retry: RetryAction) {
        if let Some(task) = self.active.iter_mut().find(|task| task.id == id) {
            task.retry = Some(retry);
        }
    }

    /// Names the game a task is about, once that is known.
    pub fn set_instance(&mut self, id: u64, instance: &str) {
        if let Some(task) = self.active.iter_mut().find(|task| task.id == id) {
            task.instance_id = Some(instance.to_owned());
        }
    }

    /// Like [`Self::progress`], counting bytes.
    pub fn progress_bytes(&mut self, id: u64, done: u64, total: u64) {
        if let Some(task) = self.active.iter_mut().find(|task| task.id == id) {
            task.unit = ProgressUnit::Bytes;
        }
        self.progress(id, done, total);
    }

    /// Updates progress. Done never decreases and never exceeds a known total;
    /// unknown ids are ignored.
    pub fn progress(&mut self, id: u64, done: u64, total: u64) {
        if let Some(task) = self.active.iter_mut().find(|task| task.id == id) {
            let previous = task.progress.map_or(0, |(done, _)| done);
            let done = done
                .max(previous)
                .min(if total > 0 { total } else { u64::MAX });
            task.progress = Some((done, total));
        }
    }

    /// Ends a task, appending it to `log`. `None` if the id is not running
    /// (finishing twice records once).
    pub fn finish(
        &mut self,
        id: u64,
        outcome: TaskOutcome,
        now: u64,
        log: &ActivityLog,
    ) -> io::Result<Option<FinishedTask>> {
        let Some(position) = self.active.iter().position(|task| task.id == id) else {
            return Ok(None);
        };
        let task = self.active.remove(position);
        let finished = FinishedTask {
            category: task.category,
            label: task.label,
            instance_id: task.instance_id,
            started: task.started,
            finished: now.max(task.started),
            outcome,
            retry: task.retry,
        };
        if let Err(error) = log.append(&finished) {
            self.unrecorded.push(finished);
            return Err(error);
        }
        Ok(Some(finished))
    }

    /// Terminal results whose log write failed. They are not resumable tasks.
    #[must_use]
    pub fn unrecorded(&self) -> &[FinishedTask] {
        &self.unrecorded
    }

    #[must_use]
    pub fn active(&self) -> &[ActiveTask] {
        &self.active
    }

    /// Forgets results whose log write failed, so a clear leaves none behind.
    pub fn clear_unrecorded(&mut self) {
        self.unrecorded.clear();
    }

    /// Running tasks of one category, oldest first.
    pub fn active_in(&self, category: TaskCategory) -> impl Iterator<Item = &ActiveTask> {
        self.active
            .iter()
            .filter(move |task| task.category == category)
    }

    /// The count the navigation badge shows.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

pub struct ActivityLog {
    path: PathBuf,
}

impl ActivityLog {
    #[must_use]
    pub fn open(root: &Path) -> Self {
        Self {
            path: root.join("activity.jsonl"),
        }
    }

    pub fn append(&self, task: &FinishedTask) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut line = serde_json::to_string(task).map_err(io::Error::other)?;
        line.push('\n');
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?
            .write_all(line.as_bytes())
    }

    /// Every understandable entry, oldest first, and how many lines were
    /// skipped (torn or from a newer launcher).
    pub fn read(&self) -> io::Result<(Vec<FinishedTask>, usize)> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(error) => return Err(error),
        };
        let mut tasks = Vec::new();
        let mut skipped = 0;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str(line) {
                Ok(task) => tasks.push(task),
                Err(_) => skipped += 1,
            }
        }
        Ok((tasks, skipped))
    }

    /// Newest first, optionally one category, at most `limit`.
    pub fn recent(
        &self,
        category: Option<TaskCategory>,
        limit: usize,
    ) -> io::Result<Vec<FinishedTask>> {
        let (mut tasks, _) = self.read()?;
        tasks.reverse();
        Ok(tasks
            .into_iter()
            .filter(|task| category.is_none_or(|wanted| task.category == wanted))
            .take(limit)
            .collect())
    }

    /// Keeps only the newest `keep` entries, atomically.
    pub fn compact(&self, keep: usize) -> io::Result<()> {
        let (tasks, _) = self.read()?;
        let start = tasks.len().saturating_sub(keep);
        let mut text = String::new();
        for task in &tasks[start..] {
            text.push_str(&serde_json::to_string(task).map_err(io::Error::other)?);
            text.push('\n');
        }
        let temporary = self.path.with_extension("jsonl.tmp");
        fs::write(&temporary, text)?;
        fs::rename(temporary, &self.path)
    }
}

#[cfg(test)]
mod tests {
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
}
