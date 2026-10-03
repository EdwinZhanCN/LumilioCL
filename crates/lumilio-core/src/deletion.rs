//! Recoverable instance deletion and its startup recovery.
//!
//! Behavior notes: `docs/behavior/deletion.md`. Deleting an instance is a
//! short journaled operation under `state/operations/<id>/`: the record is
//! written to `delete.json`, the profile folder is moved (a same-volume rename)
//! into the operation folder, the library commits, and only then are the moved
//! files removed. A crash or failure at any point leaves enough on disk for
//! [`recover`] to compare journal, library and folders, then either put the
//! profile back or finish the cleanup. Nothing is ever replayed blindly.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::instance::{InstanceRecord, InstanceStore};
use crate::layout::Layout;
use crate::recovery::RecoveryNote;

const SCHEMA: u32 = 1;
const JOURNAL: &str = "delete.json";
const MOVED: &str = "profile";

#[derive(Debug, Deserialize, Serialize)]
struct Journal {
    schema: u32,
    record: InstanceRecord,
    created: u64,
}

/// One deletion in progress.
pub(crate) struct Deletion {
    dir: PathBuf,
    profile: PathBuf,
    moved: bool,
}

impl Deletion {
    /// Journals the deletion. Nothing has moved yet, so failure has no effect.
    pub(crate) fn begin(layout: &Layout, record: &InstanceRecord, now: u64) -> io::Result<Self> {
        let dir = layout
            .operations()
            .join(format!("delete-{}-{now}", record.id));
        fs::create_dir_all(layout.operations())?;
        fs::create_dir(&dir)?;
        let journal = Journal {
            schema: SCHEMA,
            record: record.clone(),
            created: now,
        };
        let written = serde_json::to_vec_pretty(&journal)
            .map_err(io::Error::other)
            .and_then(|bytes| write_atomic(&dir.join(JOURNAL), &bytes));
        if let Err(error) = written {
            let _ = fs::remove_dir_all(&dir);
            return Err(error);
        }
        Ok(Self {
            dir,
            profile: layout.profile(&record.id),
            moved: false,
        })
    }

    /// Moves the profile folder into the operation folder. A missing profile
    /// is fine (nothing to protect). On failure nothing has moved.
    pub(crate) fn quarantine(&mut self) -> io::Result<()> {
        match fs::rename(&self.profile, self.dir.join(MOVED)) {
            Ok(()) => {
                self.moved = true;
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Forgets an operation that never moved anything.
    pub(crate) fn discard(self) {
        let _ = fs::remove_dir_all(&self.dir);
    }

    /// Undoes the quarantine after the library refused the removal. If the
    /// profile cannot be put back the journal stays so recovery can retry.
    pub(crate) fn rollback(self) -> io::Result<()> {
        if self.moved {
            fs::rename(self.dir.join(MOVED), &self.profile)?;
        }
        fs::remove_dir_all(&self.dir)
    }

    /// Removes the quarantined files and the journal after the library committed.
    pub(crate) fn finish(self) -> io::Result<()> {
        remove_operation(&self.dir)
    }
}

/// Removes the moved files before the journal, so a partial removal still
/// leaves the journal that explains the rest.
fn remove_operation(dir: &Path) -> io::Result<()> {
    match fs::remove_dir_all(dir.join(MOVED)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::remove_dir_all(dir)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
}

/// Settles every deletion left behind by an earlier run. Safe to call again:
/// finished operations leave no folder, stuck ones are reported every time.
pub(crate) fn recover(layout: &Layout, store: &InstanceStore) -> Vec<RecoveryNote> {
    let Ok(entries) = fs::read_dir(layout.operations()) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("delete-"))
        })
        .collect();
    dirs.sort();
    dirs.iter()
        .filter_map(|dir| settle(layout, store, dir))
        .collect()
}

fn settle(layout: &Layout, store: &InstanceStore, dir: &Path) -> Option<RecoveryNote> {
    let moved = dir.join(MOVED);
    let journal_path = dir.join(JOURNAL);
    let journal = match fs::read(&journal_path) {
        Ok(bytes) => match serde_json::from_slice::<Journal>(&bytes) {
            Ok(journal) if journal.schema <= SCHEMA => journal,
            Ok(journal) => {
                return Some(unusable(dir, format!("schema {} is newer", journal.schema)));
            }
            Err(error) => return Some(unusable(dir, error.to_string())),
        },
        // Crash while writing the journal: nothing can have moved yet.
        Err(error) if error.kind() == io::ErrorKind::NotFound && !moved.exists() => {
            let _ = fs::remove_dir_all(dir);
            return None;
        }
        Err(error) => return Some(unusable(dir, error.to_string())),
    };
    let id = journal.record.id.clone();
    let profile = layout.profile(&id);
    let stuck = |error: io::Error| RecoveryNote::DeleteStuck {
        instance_id: id.clone(),
        path: dir.to_path_buf(),
        reason: error.to_string(),
    };
    if store.get(&id).is_some() {
        match (profile.exists(), moved.exists()) {
            (false, true) => Some(
                fs::rename(&moved, &profile)
                    .and_then(|()| fs::remove_dir_all(dir))
                    .map_or_else(stuck, |()| RecoveryNote::DeleteRolledBack {
                        instance_id: id.clone(),
                    }),
            ),
            (true, true) => Some(RecoveryNote::DeleteConflict {
                instance_id: id,
                path: moved,
            }),
            (true, false) => {
                let _ = fs::remove_dir_all(dir);
                None
            }
            (false, false) => {
                let _ = fs::remove_dir_all(dir);
                Some(RecoveryNote::ProfileMissing { instance_id: id })
            }
        }
    } else {
        Some(
            fs::remove_dir_all(dir).map_or_else(stuck, |()| RecoveryNote::DeleteCompleted {
                instance_id: id.clone(),
            }),
        )
    }
}

fn unusable(dir: &Path, reason: String) -> RecoveryNote {
    RecoveryNote::JournalUnusable {
        path: dir.to_path_buf(),
        reason,
    }
}
