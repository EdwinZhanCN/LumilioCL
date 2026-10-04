//! Staged publication of a new instance's game folder.
//!
//! Behavior notes: `docs/behavior/staged-instances.md`. Importing a pack or
//! copying an instance builds the whole game folder under
//! `state/operations/<kind>-<id>/game/` first. Only when it is complete is the
//! library record created and the folder renamed to `profiles/<id>/game`, so a
//! failure, a cancel or a crash never leaves a half-made instance behind.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::instance::InstanceStore;
use crate::layout::Layout;
use crate::recovery::RecoveryNote;

const GAME: &str = "game";
const READY: &str = "staged.ok";
/// Operation folders this module owns.
const KINDS: [&str; 2] = ["import", "copy"];

/// A game folder being built for the instance `id`.
pub(crate) struct Staged {
    dir: PathBuf,
    id: String,
}

impl Staged {
    /// Opens a fresh staging folder. An existing one (an unsettled earlier
    /// attempt for the same id) is never reused or deleted here.
    pub(crate) fn begin(layout: &Layout, kind: &str, id: &str) -> io::Result<Self> {
        if !KINDS.contains(&kind) || !is_plain(id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "staging needs a known kind and a plain instance id",
            ));
        }
        let dir = layout.operations().join(format!("{kind}-{id}"));
        fs::create_dir_all(layout.operations())?;
        fs::create_dir(&dir)?;
        fs::create_dir(dir.join(GAME))?;
        Ok(Self {
            dir,
            id: id.to_owned(),
        })
    }

    /// Where the game files are built.
    pub(crate) fn game_dir(&self) -> PathBuf {
        self.dir.join(GAME)
    }

    /// Declares the folder complete and verified. Recovery only trusts a
    /// folder that carries this mark.
    pub(crate) fn mark_ready(&self) -> io::Result<()> {
        fs::write(self.dir.join(READY), b"")
    }

    /// Gives the folder up; nothing was published.
    pub(crate) fn discard(self) {
        let _ = fs::remove_dir_all(&self.dir);
    }

    /// Moves the finished folder to `profiles/<id>/game` and removes the
    /// staging folder. The library record must already exist. On failure the
    /// staged folder is untouched, so the caller can still [`Self::discard`] it.
    pub(crate) fn publish(&self, layout: &Layout) -> io::Result<()> {
        let target = layout.game(&self.id);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(self.dir.join(GAME), &target)?;
        let _ = fs::remove_dir_all(&self.dir);
        Ok(())
    }
}

fn is_plain(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

/// Settles staging folders an earlier run left behind:
/// - not marked ready: nothing was published, remove it;
/// - ready, the record exists but its game folder does not: finish the rename;
/// - ready, record and game folder both exist: only garbage is left, remove it;
/// - ready but no record: the staged files are discarded and reported.
pub(crate) fn recover(layout: &Layout, store: &InstanceStore) -> Vec<RecoveryNote> {
    let Ok(entries) = fs::read_dir(layout.operations()) else {
        return Vec::new();
    };
    let mut dirs: Vec<(PathBuf, String)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = KINDS
                .iter()
                .find_map(|kind| name.strip_prefix(&format!("{kind}-")))?
                .to_owned();
            Some((entry.path(), id))
        })
        .collect();
    dirs.sort();
    dirs.into_iter()
        .filter_map(|(dir, id)| settle(layout, store, &dir, id))
        .collect()
}

fn settle(layout: &Layout, store: &InstanceStore, dir: &Path, id: String) -> Option<RecoveryNote> {
    if !is_plain(&id) || !dir.join(READY).exists() {
        let _ = fs::remove_dir_all(dir);
        return None;
    }
    let published = layout.game(&id).exists();
    match (store.get(&id).is_some(), published) {
        (true, false) => {
            let staged = Staged {
                dir: dir.to_path_buf(),
                id: id.clone(),
            };
            Some(match staged.publish(layout) {
                Ok(()) => RecoveryNote::PublishCompleted { instance_id: id },
                Err(error) => RecoveryNote::PublishStuck {
                    instance_id: id,
                    path: dir.to_path_buf(),
                    reason: error.to_string(),
                },
            })
        }
        (true, true) => {
            let _ = fs::remove_dir_all(dir);
            None
        }
        (false, _) => {
            let _ = fs::remove_dir_all(dir);
            Some(RecoveryNote::PublishDiscarded { instance_id: id })
        }
    }
}

#[cfg(test)]
mod tests;
