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
//!   logs/                the launcher's own warnings and errors
//! ```
//!
//! Game files (client jars, loader profiles, libraries, assets) sit in `meta/`
//! so two instances of one version never install it twice; everything an
//! instance can change lives in its own profile.

use std::path::{Path, PathBuf};

use crate::instance::InstanceRecord;
use lumilio_minecraft::LaunchDirectories;

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

    /// The launcher's own warnings and errors (the app writes them).
    #[must_use]
    pub fn launcher_log(&self) -> PathBuf {
        self.root.join("logs/launcher.log")
    }

    /// The launcher log before the current one filled up.
    #[must_use]
    pub fn earlier_launcher_log(&self) -> PathBuf {
        self.root.join("logs/launcher.1.log")
    }

    pub fn map_cache(&self) -> PathBuf {
        self.root.join("cache/world-map/seed")
    }

    /// Notes on downloads stopped part-way, whose bytes wait beside their
    /// destination for the next attempt (plan download-engine T2). Outside
    /// `cache/` so clearing the cache cannot orphan those bytes.
    #[must_use]
    pub fn partial_downloads(&self) -> PathBuf {
        self.root.join("partial-downloads")
    }

    /// Baked block assets: local bakes by jar SHA-1 and the downloaded
    /// bundle (plan schematic-studio, D2/D3). Safe to delete; rebuilt on use.
    #[must_use]
    pub fn model_cache(&self) -> PathBuf {
        self.root.join("cache/block-assets")
    }

    /// Map tiles drawn from one instance's saves; rebuilt when deleted.
    #[must_use]
    pub fn map_tiles(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("map-cache")
    }

    /// The pre-SQLite library file, imported once and then set aside.
    #[must_use]
    pub fn legacy_library(&self) -> PathBuf {
        self.root.join("library.json")
    }

    /// Where local theme files and the home wallpaper are kept.
    #[must_use]
    pub fn themes(&self) -> PathBuf {
        self.root.join("themes")
    }

    /// The schematic studio's saved replacement tables and recent targets,
    /// shared by every instance (plan schematic-studio T24).
    #[must_use]
    pub fn replace_schemes(&self) -> PathBuf {
        self.root.join("schematic-replace.json")
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

    /// One instance's schematic operation log and the backups it names, in
    /// launcher data beside the game directory, never inside it
    /// (plan schematic-studio, D8).
    #[must_use]
    pub fn schematic_ops(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("schematic-ops")
    }

    /// The resource packs added to one instance's schematic studio from
    /// elsewhere: their paths only, the files are never copied
    /// (plan schematic-studio, T17).
    #[must_use]
    pub fn schematic_packs(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("schematic-packs.json")
    }

    /// The order one instance's schematic studio layers its mods and
    /// resource packs in, lowest priority first (plan schematic-studio, T36).
    #[must_use]
    pub fn schematic_order(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("schematic-order.json")
    }

    /// Which of one instance's mods and resource packs each schematic studio
    /// folder draws with: all are off until switched on (plan
    /// schematic-studio, T21, D23).
    #[must_use]
    pub fn schematic_sources(&self, instance_id: &str) -> PathBuf {
        self.profile(instance_id).join("schematic-sources.json")
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
mod tests;
