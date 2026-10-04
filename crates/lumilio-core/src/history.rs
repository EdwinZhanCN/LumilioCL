//! Per-instance history: play sessions and changes.
//!
//! The log is append-only JSON lines, so a crash can at worst tear the final
//! line, which reading skips.

use crate::layout::Layout;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::instance::{InstanceStore, StoreError};
use crate::process::GameExit;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    /// The game ran and exited with code 0.
    Clean,
    /// The game ran and exited with a failure code (or was signalled).
    Crashed,
    /// The process ended before the game finished starting.
    FailedToStart,
    /// The user stopped it.
    Stopped,
    /// Preparation failed before any game process existed.
    FailedToPrepare,
    /// The user cancelled while the launch was still preparing.
    Cancelled,
    /// The launcher itself ended while this launch was in progress, so how the
    /// game finished (or whether it still runs) is unknown.
    Interrupted,
}

impl SessionOutcome {
    #[must_use]
    pub const fn of(exit: &GameExit) -> Self {
        if exit.killed {
            Self::Stopped
        } else if !exit.was_running {
            Self::FailedToStart
        } else if matches!(exit.code, Some(0)) {
            Self::Clean
        } else {
            Self::Crashed
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    ContentAdded,
    ContentRemoved,
    ContentEnabled,
    ContentDisabled,
    ContentUpdated,
    SettingsChanged,
    GameVersionChanged,
    Repaired,
    WorldCopied,
    WorldDeleted,
    WorldImported,
    SnapshotCreated,
    SnapshotRestored,
    SnapshotDeleted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HistoryEvent {
    Session {
        started: u64,
        seconds: u64,
        exit_code: Option<i32>,
        outcome: SessionOutcome,
    },
    Change {
        at: u64,
        kind: ChangeKind,
        /// What changed: a file name, a setting, a version.
        subject: String,
    },
}

impl HistoryEvent {
    /// When the event happened.
    #[must_use]
    pub const fn at(&self) -> u64 {
        match self {
            Self::Session { started, .. } => *started,
            Self::Change { at, .. } => *at,
        }
    }
}

/// What reading a log found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HistoryRead {
    /// Events oldest first.
    pub events: Vec<HistoryEvent>,
    /// Lines that could not be understood and were skipped.
    pub skipped: usize,
}

pub struct HistoryLog {
    path: PathBuf,
}

impl HistoryLog {
    #[must_use]
    pub fn for_instance(root: &Path, instance_id: &str) -> Self {
        Self {
            path: Layout::new(root).history(instance_id),
        }
    }

    pub fn append(&self, event: &HistoryEvent) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut line = serde_json::to_string(event).map_err(io::Error::other)?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())
    }

    /// Reads every understandable event, oldest first. A missing log is empty.
    pub fn read(&self) -> io::Result<HistoryRead> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(HistoryRead::default());
            }
            Err(error) => return Err(error),
        };
        let mut read = HistoryRead::default();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str(line) {
                Ok(event) => read.events.push(event),
                Err(_) => read.skipped += 1,
            }
        }
        Ok(read)
    }

    /// Sessions, newest first.
    pub fn sessions(&self) -> io::Result<Vec<HistoryEvent>> {
        self.newest_first(|event| matches!(event, HistoryEvent::Session { .. }))
    }

    /// Changes, newest first.
    pub fn changes(&self) -> io::Result<Vec<HistoryEvent>> {
        self.newest_first(|event| matches!(event, HistoryEvent::Change { .. }))
    }

    fn newest_first(&self, keep: impl Fn(&HistoryEvent) -> bool) -> io::Result<Vec<HistoryEvent>> {
        let mut events: Vec<_> = self
            .read()?
            .events
            .into_iter()
            .filter(|e| keep(e))
            .collect();
        events.reverse();
        Ok(events)
    }

    /// Keeps only the newest `keep` events, rewriting the file atomically.
    pub fn compact(&self, keep: usize) -> io::Result<()> {
        let read = self.read()?;
        let start = read.events.len().saturating_sub(keep);
        let mut text = String::new();
        for event in &read.events[start..] {
            text.push_str(&serde_json::to_string(event).map_err(io::Error::other)?);
            text.push('\n');
        }
        let temporary = self.path.with_extension("jsonl.tmp");
        fs::write(&temporary, text)?;
        fs::rename(temporary, &self.path)
    }
}

/// Records a launch that never produced a process (failed or cancelled while
/// preparing). Play time and last-played are not touched: nothing was played.
pub fn record_attempt(
    root: &Path,
    instance_id: &str,
    started: u64,
    outcome: SessionOutcome,
) -> io::Result<()> {
    HistoryLog::for_instance(root, instance_id).append(&HistoryEvent::Session {
        started,
        seconds: 0,
        exit_code: None,
        outcome,
    })
}

/// Records a finished session in the log and in the instance's totals.
pub fn finish_session(
    store: &mut InstanceStore,
    instance_id: &str,
    started: u64,
    exit: &GameExit,
) -> Result<(), StoreError> {
    let seconds = exit.ran_for.as_secs();
    HistoryLog::for_instance(store.root(), instance_id)
        .append(&HistoryEvent::Session {
            started,
            seconds,
            exit_code: exit.code,
            outcome: SessionOutcome::of(exit),
        })
        .map_err(StoreError::Io)?;
    store.record_session(instance_id, started, seconds)
}

#[cfg(test)]
mod tests;
