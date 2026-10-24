//! What the launcher found and did when it opened its data root.
//!
//! Opening never hides damage: unreadable originals are kept, and everything
//! the launcher repaired or could not repair is reported as a [`RecoveryNote`]
//! for Diagnostics to show.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::activity::ActivityLog;
use crate::instances::InstanceStore;
use crate::settings::SettingsStore;
use crate::storage::Layout;

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
/// settled first, games never started get their folder, then damaged files
/// are reported.
pub(crate) fn startup_notes(
    layout: &Layout,
    store: &InstanceStore,
    settings: &SettingsStore,
    log: &ActivityLog,
) -> Vec<RecoveryNote> {
    let mut notes = crate::instances::deletion::recover(layout, store);
    notes.extend(
        crate::instances::snapshot_files::recover_interrupted(layout)
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
    notes.extend(crate::instances::staged::recover(layout, store));
    give_unstarted_games_folders(layout, store);
    notes.extend(crate::launching::settle_sessions(layout, store));
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

/// Games made before creating one made their folder, and never started since,
/// have none: they get an empty one, so that from here on a missing folder
/// means it was moved or deleted (plan game-folder-missing, D2). A game that
/// was played is left for diagnostics to report.
fn give_unstarted_games_folders(layout: &Layout, store: &InstanceStore) {
    for record in store.instances() {
        let game = layout.game(&record.id);
        if record.last_played.is_none() && !game.exists() {
            let _ = std::fs::create_dir_all(game);
        }
    }
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
