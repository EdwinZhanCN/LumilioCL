//! What the Activity page shows: running tasks and a durable history.
//!
//! The [`TaskBoard`] holds running tasks in memory; finishing one appends it to
//! the [`ActivityLog`] (JSON lines), so history survives restarts.

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

/// What a task is doing, as a type rather than a sentence: the interface
/// turns it into words in the person's language.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum TaskLabel {
    Typed {
        action: TaskAction,
        subject: String,
    },
    /// A line written before labels were typed: shown as it was.
    Text(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskAction {
    InstallGame,
    RepairGame,
    ChangeVersion,
    ExportWorld,
    DownloadContent,
    InstallContent,
    SwitchContentVersion,
    ImportGame,
    BackupGame,
    RestoreBackup,
    InstallJava,
    CopyGame,
    InstallModpack,
    ImportModpack,
}

impl From<&str> for TaskLabel {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for TaskLabel {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl TaskLabel {
    /// The label's own text: the subject of a typed label, or the whole text
    /// of a written one. The action itself needs the catalog; this is what a
    /// log line or a caller with no words can show.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Typed { subject, .. } => subject,
            Self::Text(text) => text,
        }
    }
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
    pub label: TaskLabel,
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
    pub label: TaskLabel,
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
        label: impl Into<TaskLabel>,
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
mod tests;
