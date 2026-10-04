//! Where everything lives under the launcher root.
//!
//! ```text
//! <root>/
//!   launcher.db          instances, collections (SQLite)
//!   settings.toml …      small launcher-wide files
//!   meta/                shared, installed once per version
//!     libraries/ versions/ assets/ natives/<release>/
//!   profiles/<id>/       one isolated folder per instance
//!     game/ history.jsonl snapshots/
//!   runtimes/            Java runtimes the launcher owns
//!   state/operations/    recoverable operation journals (deletion)
//! ```
//!
//! Game files (client jars, loader profiles, libraries, assets) sit in `meta/`
//! so two instances of one version never install it twice; everything an
//! instance can change lives in its own profile.

use std::path::{Path, PathBuf};

use crate::instance::InstanceRecord;
use crate::launch::LaunchDirectories;

#[derive(Clone, Debug)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn database(&self) -> PathBuf {
        self.root.join("launcher.db")
    }

    /// The pre-SQLite library file, imported once and then set aside.
    #[must_use]
    pub fn legacy_library(&self) -> PathBuf {
        self.root.join("library.json")
    }

    #[must_use]
    pub fn meta(&self) -> PathBuf {
        self.root.join("meta")
    }

    #[must_use]
    pub fn libraries(&self) -> PathBuf {
        self.meta().join("libraries")
    }

    #[must_use]
    pub fn versions(&self) -> PathBuf {
        self.meta().join("versions")
    }

    #[must_use]
    pub fn assets(&self) -> PathBuf {
        self.meta().join("assets")
    }

    /// Native libraries are extracted per release, not per instance.
    #[must_use]
    pub fn natives(&self, release_id: &str) -> PathBuf {
        self.meta().join("natives").join(release_id)
    }

    #[must_use]
    pub fn profiles(&self) -> PathBuf {
        self.root.join("profiles")
    }

    #[must_use]
    pub fn profile(&self, instance_id: &str) -> PathBuf {
        self.profiles().join(instance_id)
    }

    /// The game directory: mods, saves, options, resource packs.
    #[must_use]
    pub fn game(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("game")
    }

    /// Where the authlib-injector jar is kept.
    #[must_use]
    pub fn injector(&self) -> PathBuf {
        self.root.join("injector")
    }

    /// Small copies of the instance's screenshots, made on demand and safe to
    /// delete: they are rebuilt when missing.
    #[must_use]
    pub fn thumbnails(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("thumbnails")
    }

    #[must_use]
    pub fn history(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("history.jsonl")
    }

    #[must_use]
    pub fn snapshots(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("snapshots")
    }

    /// The directories one instance launches with: shared game files from
    /// `meta/` (natives per release), the game directory from its profile.
    #[must_use]
    pub fn launch_directories(&self, record: &InstanceRecord) -> LaunchDirectories {
        let release = record
            .release_id()
            .unwrap_or_else(|| record.game_version.clone());
        LaunchDirectories::new(
            self.libraries(),
            self.versions(),
            self.assets(),
            self.natives(&release),
            self.game(&record.id),
        )
    }

    /// Journals and quarantined files of operations that can be recovered.
    #[must_use]
    pub fn operations(&self) -> PathBuf {
        self.root.join("state").join("operations")
    }

    /// One small file per instance whose launch is in progress.
    #[must_use]
    pub fn sessions(&self) -> PathBuf {
        self.root.join("state").join("sessions")
    }

    #[must_use]
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_files_live_in_meta_and_instance_files_in_profiles() {
        let layout = Layout::new("/r");
        assert_eq!(layout.versions(), Path::new("/r/meta/versions"));
        assert_eq!(layout.libraries(), Path::new("/r/meta/libraries"));
        assert_eq!(layout.assets(), Path::new("/r/meta/assets"));
        assert_eq!(layout.natives("1.21"), Path::new("/r/meta/natives/1.21"));
        assert_eq!(layout.game("a"), Path::new("/r/profiles/a/game"));
        assert_eq!(
            layout.history("a"),
            Path::new("/r/profiles/a/history.jsonl")
        );
        assert_eq!(layout.snapshots("a"), Path::new("/r/profiles/a/snapshots"));
        assert_eq!(layout.database(), Path::new("/r/launcher.db"));
    }

    #[test]
    fn two_instances_never_share_a_profile() {
        let layout = Layout::new("/r");
        assert_ne!(layout.game("a"), layout.game("b"));
        assert_ne!(layout.profile("a"), layout.profile("b"));
    }
}
