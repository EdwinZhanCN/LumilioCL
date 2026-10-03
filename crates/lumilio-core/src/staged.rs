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
mod tests {
    use super::*;
    use crate::instance::{InstanceSettings, Loader, NewInstance};

    fn request(name: &str) -> NewInstance {
        NewInstance {
            name: name.to_owned(),
            game_version: "1.21.1".to_owned(),
            loader: Loader::Vanilla,
            loader_version: None,
        }
    }

    struct Setup {
        _dir: tempfile::TempDir,
        layout: Layout,
        store: InstanceStore,
    }

    fn setup() -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let store = InstanceStore::open(dir.path()).unwrap();
        Setup {
            _dir: dir,
            layout,
            store,
        }
    }

    fn stage(setup: &Setup, id: &str, ready: bool) -> Staged {
        let staged = Staged::begin(&setup.layout, "copy", id).unwrap();
        fs::create_dir_all(staged.game_dir().join("saves/W")).unwrap();
        fs::write(staged.game_dir().join("saves/W/level.dat"), b"world").unwrap();
        if ready {
            staged.mark_ready().unwrap();
        }
        staged
    }

    #[test]
    fn publishing_moves_the_whole_folder_and_leaves_no_operation_behind() {
        let mut s = setup();
        let staged = stage(&s, "fresh", true);
        s.store
            .create_as(
                "fresh",
                request("Fresh"),
                InstanceSettings::default(),
                false,
                1,
            )
            .unwrap();
        staged.publish(&s.layout).unwrap();
        assert_eq!(
            fs::read(s.layout.game("fresh").join("saves/W/level.dat")).unwrap(),
            b"world"
        );
        assert!(!s.layout.operations().join("copy-fresh").exists());
    }

    #[test]
    fn staging_refuses_unknown_kinds_odd_ids_and_an_unsettled_earlier_attempt() {
        let s = setup();
        assert!(Staged::begin(&s.layout, "weird", "a").is_err());
        assert!(Staged::begin(&s.layout, "copy", "../x").is_err());
        let first = Staged::begin(&s.layout, "copy", "a").unwrap();
        assert!(Staged::begin(&s.layout, "copy", "a").is_err());
        first.discard();
        assert!(!s.layout.operations().join("copy-a").exists());
    }

    #[test]
    fn recovery_settles_every_state_a_crash_can_leave() {
        let mut s = setup();
        // Never marked ready: nothing was published, silently removed.
        stage(&s, "raw", false);
        // Ready but the record was never created: discarded and reported.
        stage(&s, "norecord", true);
        // Ready and the record exists but the rename never happened: finished.
        stage(&s, "unfinished", true);
        s.store
            .create_as(
                "unfinished",
                request("Unfinished"),
                InstanceSettings::default(),
                false,
                1,
            )
            .unwrap();
        // Ready, record and folder both exist: only garbage remains.
        stage(&s, "done", true);
        s.store
            .create_as(
                "done",
                request("Done"),
                InstanceSettings::default(),
                false,
                1,
            )
            .unwrap();
        fs::create_dir_all(s.layout.game("done")).unwrap();

        let notes = recover(&s.layout, &s.store);
        assert_eq!(
            notes,
            [
                RecoveryNote::PublishDiscarded {
                    instance_id: "norecord".into()
                },
                RecoveryNote::PublishCompleted {
                    instance_id: "unfinished".into()
                },
            ]
        );
        assert_eq!(
            fs::read(s.layout.game("unfinished").join("saves/W/level.dat")).unwrap(),
            b"world"
        );
        assert!(
            fs::read_dir(s.layout.operations())
                .unwrap()
                .next()
                .is_none()
        );
        assert!(recover(&s.layout, &s.store).is_empty());
    }
}
