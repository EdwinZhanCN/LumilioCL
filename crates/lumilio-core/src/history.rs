//! Per-instance history: play sessions and changes.
//!
//! Behavior notes: `docs/behavior/history.md`. The log is append-only JSON
//! lines, so a crash can at worst tear the final line, which reading skips.

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
mod tests {
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
}
