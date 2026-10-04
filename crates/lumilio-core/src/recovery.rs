//! What the launcher found and did when it opened its data root.
//!
//! Opening never hides damage: unreadable originals are kept, and everything
//! the launcher repaired or could not repair is reported as a [`RecoveryNote`]
//! for Diagnostics to show.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::activity_log::ActivityLog;
use crate::history::{SessionOutcome, record_attempt};
use crate::instance::InstanceStore;
use crate::layout::Layout;
use crate::settings::SettingsStore;

/// What start-up recovery found or did. Plain facts: the interface decides
/// the wording.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryNote {
    /// The library still has the instance: its profile was moved back.
    DeleteRolledBack { instance_id: String },
    /// The library no longer has the instance: the leftover files are gone.
    DeleteCompleted { instance_id: String },
    /// The library has the instance but its profile folder is gone and was not
    /// in quarantine either; the record stays for Diagnostics to explain.
    ProfileMissing { instance_id: String },
    /// Both the profile and a quarantined copy exist; neither was touched.
    DeleteConflict { instance_id: String, path: PathBuf },
    /// The step failed again; the journal stays for the next start.
    DeleteStuck {
        instance_id: String,
        path: PathBuf,
        reason: String,
    },
    /// A journal this build cannot use; kept untouched.
    JournalUnusable { path: PathBuf, reason: String },
    /// The library database was unreadable. The original is kept at
    /// `preserved` and the library starts empty; `candidates` are profile
    /// folders found on disk that the new library does not know. They are
    /// leads for the user, not a restored library.
    LibraryRecovered {
        preserved: PathBuf,
        candidates: Vec<String>,
    },
    /// The settings file was unreadable; the original is kept at `preserved`
    /// and defaults are in use.
    SettingsRecovered { preserved: PathBuf },
    /// This many activity-log lines were torn or unknown and skipped.
    ActivityLogSkipped { count: usize },
    /// A new instance's staged game folder was complete and its record existed:
    /// the interrupted publication was finished.
    PublishCompleted { instance_id: String },
    /// A staged game folder had no library record and was discarded; the import
    /// or copy can be started again.
    PublishDiscarded { instance_id: String },
    /// Finishing an interrupted publication failed; the folder is kept.
    PublishStuck {
        instance_id: String,
        path: PathBuf,
        reason: String,
    },
    /// A snapshot restore was interrupted part-way; every replaced unit was put
    /// back as it was. The snapshot is still there to try again.
    RestoreRolledBack {
        instance_id: String,
        snapshot: String,
        units: Vec<PathBuf>,
    },
    /// An interrupted restore could not be undone; its folder is kept.
    RestoreStuck { path: PathBuf, reason: String },
    /// The launcher ended while this instance was launching or running. The
    /// game's fate is unknown: nothing was re-attached, killed or restarted.
    SessionInterrupted { instance_id: String, started: u64 },
}

/// Gathers the notes for a freshly opened root: interrupted deletions are
/// settled first, then damaged files are reported.
pub(crate) fn startup_notes(
    layout: &Layout,
    store: &InstanceStore,
    settings: &SettingsStore,
    log: &ActivityLog,
) -> Vec<RecoveryNote> {
    let mut notes = crate::deletion::recover(layout, store);
    notes.extend(
        crate::snapshots::recover_interrupted(layout)
            .into_iter()
            .map(|restore| match restore.outcome {
                Ok(units) => RecoveryNote::RestoreRolledBack {
                    instance_id: restore.instance_id,
                    snapshot: restore.snapshot,
                    units,
                },
                Err(reason) => RecoveryNote::RestoreStuck {
                    path: restore.operation,
                    reason,
                },
            }),
    );
    notes.extend(crate::staged::recover(layout, store));
    notes.extend(settle_sessions(layout, store));
    if let Some(preserved) = store.preserved_original() {
        notes.push(RecoveryNote::LibraryRecovered {
            preserved: preserved.to_path_buf(),
            candidates: profile_candidates(layout, store),
        });
    }
    if let Some(preserved) = settings.preserved_original() {
        notes.push(RecoveryNote::SettingsRecovered {
            preserved: preserved.to_path_buf(),
        });
    }
    if let Ok((_, skipped)) = log.read()
        && skipped > 0
    {
        notes.push(RecoveryNote::ActivityLogSkipped { count: skipped });
    }
    notes
}

/// Profile folders on disk that the library has no record of, sorted.
fn profile_candidates(layout: &Layout, store: &InstanceStore) -> Vec<String> {
    let known: BTreeSet<&str> = store
        .instances()
        .iter()
        .map(|record| record.id.as_str())
        .collect();
    let mut found: Vec<String> = std::fs::read_dir(layout.profiles())
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| !name.starts_with('.') && !known.contains(name.as_str()))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

const MARKER_SCHEMA: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct MarkerFile {
    schema: u32,
    instance_id: String,
    started: u64,
}

/// Proof, on disk, that a launch was in progress. It exists from the start of
/// preparation until the launch returns or its future is dropped, so a marker
/// found at start-up means the launcher itself died in between.
pub(crate) struct SessionMarker {
    path: PathBuf,
}

impl SessionMarker {
    pub(crate) fn place(layout: &Layout, instance_id: &str, started: u64) -> io::Result<Self> {
        // Ids come from the library and are plain; refuse anything that could
        // name a file outside the sessions folder.
        if instance_id.is_empty()
            || !instance_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "instance id is not a plain name",
            ));
        }
        fs::create_dir_all(layout.sessions())?;
        let path = layout.sessions().join(format!("{instance_id}.json"));
        let bytes = serde_json::to_vec(&MarkerFile {
            schema: MARKER_SCHEMA,
            instance_id: instance_id.to_owned(),
            started,
        })
        .map_err(io::Error::other)?;
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, &path)?;
        Ok(Self { path })
    }
}

impl Drop for SessionMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Turns leftover markers into a history record and a note. The game is never
/// signalled or re-attached: a stored process id cannot prove it is still the
/// same process.
fn settle_sessions(layout: &Layout, store: &InstanceStore) -> Vec<RecoveryNote> {
    let Ok(entries) = fs::read_dir(layout.sessions()) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|path| settle_session(layout, store, path))
        .collect()
}

fn settle_session(layout: &Layout, store: &InstanceStore, path: &Path) -> Option<RecoveryNote> {
    let marker = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<MarkerFile>(&bytes).ok());
    let Some(marker) = marker.filter(|marker| marker.schema <= MARKER_SCHEMA) else {
        // Unreadable or from a newer launcher: leave it, say so.
        return Some(RecoveryNote::JournalUnusable {
            path: path.to_path_buf(),
            reason: "session marker could not be read".to_owned(),
        });
    };
    if store.get(&marker.instance_id).is_some()
        && record_attempt(
            layout.root(),
            &marker.instance_id,
            marker.started,
            SessionOutcome::Interrupted,
        )
        .is_err()
    {
        // Not recorded yet: keep the marker so the next start tries again.
        return Some(RecoveryNote::SessionInterrupted {
            instance_id: marker.instance_id,
            started: marker.started,
        });
    }
    let _ = fs::remove_file(path);
    Some(RecoveryNote::SessionInterrupted {
        instance_id: marker.instance_id,
        started: marker.started,
    })
}
