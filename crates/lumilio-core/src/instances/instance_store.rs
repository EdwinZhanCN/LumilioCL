use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::persist;
use crate::persist::{PersistError, Versioned};
use crate::tuning::InstanceLaunch;
use lumilio_minecraft::LaunchDirectories;

/// Stored as SQLite's `user_version`; bump with a migration when tables change.
pub(super) const SCHEMA_VERSION: u32 = 6;

/// SQLite integers are signed; timestamps and counters never get near the limit.
pub(super) fn int<T: TryInto<i64>>(value: T) -> i64 {
    value.try_into().unwrap_or(i64::MAX)
}

pub(super) fn unsigned(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Loader {
    #[default]
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
}

/// Per-instance overrides; `None` means "use the launcher default".
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct InstanceSettings {
    pub java_path: Option<PathBuf>,
    pub max_memory_mb: Option<u32>,
    pub min_memory_mb: Option<u32>,
    pub jvm_arguments: Vec<String>,
    /// Window, arguments, environment and commands this instance sets itself.
    pub launch: InstanceLaunch,
}

impl InstanceSettings {
    /// Validates both explicit overrides and their combination with launcher defaults.
    pub fn validate_memory(
        &self,
        default_min: Option<u32>,
        default_max: Option<u32>,
    ) -> Result<(), StoreError> {
        crate::settings::validate_memory(self.min_memory_mb, self.max_memory_mb)
            .and_then(|()| {
                crate::settings::validate_memory(
                    self.min_memory_mb.or(default_min),
                    self.max_memory_mb.or(default_max),
                )
            })
            .map_err(|_| StoreError::InvalidMemory)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InstanceRecord {
    pub id: String,
    pub name: String,
    pub game_version: String,
    #[serde(default)]
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    /// Seconds since the Unix epoch.
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(default)]
    pub play_seconds: u64,
    /// Set once the game files for this version were installed successfully.
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub settings: InstanceSettings,
    /// The Modrinth project this game was installed from, when it came from a
    /// modpack there; Discover uses it to hide packs already in the library.
    #[serde(default)]
    pub source_project: Option<String>,
}

impl InstanceRecord {
    /// The id of the release manifest this instance launches: the game version
    /// for vanilla, `<loader>-loader-<loader version>-<game version>` for a
    /// loader instance. `None` for a loader instance with no loader version.
    #[must_use]
    pub fn release_id(&self) -> Option<String> {
        let prefix = match self.loader {
            Loader::Vanilla => return Some(self.game_version.clone()),
            Loader::Fabric => "fabric",
            Loader::Quilt => "quilt",
            Loader::Forge => "forge",
            Loader::NeoForge => "neoforge",
        };
        let loader_version = self.loader_version.as_deref().filter(|v| !v.is_empty())?;
        Some(format!(
            "{prefix}-loader-{loader_version}-{}",
            self.game_version
        ))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Collection {
    pub name: String,
    pub members: Vec<String>,
}

impl Versioned for Index {
    fn schema(&self) -> u32 {
        self.schema
    }
    fn set_schema(&mut self, schema: u32) {
        self.schema = schema;
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct Index {
    #[serde(default)]
    pub(super) schema: u32,
    #[serde(default)]
    pub(super) instances: Vec<InstanceRecord>,
    #[serde(default)]
    pub(super) collections: Vec<Collection>,
}

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Database(String),
    Encode(String),
    UnknownInstance(String),
    UnknownCollection(String),
    InvalidName,
    InvalidMemory,
    /// A launch override (window, arguments, command, quick-play target) is not usable.
    InvalidLaunch(String),
    DuplicateCollection(String),
    /// The id was taken, or a profile folder already uses it, since it was suggested.
    IdTaken(String),
    /// The index was written by a newer launcher and must not be overwritten.
    NewerSchema(u32),
}

impl Display for StoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "library storage failed: {error}"),
            Self::Encode(message) => write!(f, "library could not be encoded: {message}"),
            Self::Database(message) => write!(f, "library database failed: {message}"),
            Self::UnknownInstance(id) => write!(f, "no instance {id:?}"),
            Self::UnknownCollection(name) => write!(f, "no collection {name:?}"),
            Self::InvalidName => f.write_str("name is empty"),
            Self::InvalidMemory => f.write_str("instance memory values are out of range"),
            Self::InvalidLaunch(reason) => {
                write!(f, "instance launch settings are not valid: {reason}")
            }
            Self::DuplicateCollection(name) => write!(f, "collection {name:?} already exists"),
            Self::IdTaken(id) => write!(f, "instance id {id:?} is already in use"),
            Self::NewerSchema(version) => {
                write!(
                    f,
                    "library was written by a newer launcher (schema {version})"
                )
            }
        }
    }
}

impl Error for StoreError {}

impl From<PersistError> for StoreError {
    fn from(error: PersistError) -> Self {
        match error {
            PersistError::Io(error) => Self::Io(error),
            PersistError::Encode(message) => Self::Encode(message),
            PersistError::NewerSchema(version) => Self::NewerSchema(version),
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// What the user asks for when creating an instance.
#[derive(Clone, Debug)]
pub struct NewInstance {
    pub name: String,
    pub game_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
}

pub struct InstanceStore {
    pub(super) layout: Layout,
    pub(super) db: Connection,
    pub(super) index: Index,
    pub(super) recovered: bool,
    pub(super) preserved: Option<PathBuf>,
    /// Where saves are published (plan global-context).
    pub(super) hub: Option<crate::context::ContextHub>,
}

impl InstanceStore {
    pub(crate) fn publish_to(&mut self, hub: crate::context::ContextHub) {
        self.hub = Some(hub);
    }

    /// Opens the library under `root`, creating `launcher.db` when missing.
    ///
    /// A database that is not readable is preserved as `launcher.db.broken`
    /// and the library starts empty, so one bad write never locks the user
    /// out; [`Self::recovered_from_damage`] reports it. A database from a
    /// newer launcher is refused instead, because writing would destroy its
    /// data. A `library.json` from before the database existed is imported once.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let layout = Layout::new(root);
        fs::create_dir_all(layout.root())?;
        let path = layout.database();
        let (db, recovered, preserved) = match Self::connect(&path) {
            Ok(db) => (db, false, None),
            Err(StoreError::NewerSchema(version)) => return Err(StoreError::NewerSchema(version)),
            Err(_) if path.exists() => {
                let preserved = persist::set_aside(&path)?;
                (Self::connect(&path)?, true, Some(preserved))
            }
            Err(error) => return Err(error),
        };
        let mut store = Self {
            layout,
            db,
            index: Index::default(),
            recovered,
            preserved,
            hub: None,
        };
        store.load()?;
        Ok(store)
    }

    /// Makes every later write fail (or work again), to rehearse failures.
    #[cfg(test)]
    pub(crate) fn set_read_only(&mut self, read_only: bool) {
        self.db
            .pragma_update(None, "query_only", read_only)
            .unwrap();
    }

    #[must_use]
    pub fn preserved_original(&self) -> Option<&Path> {
        self.preserved.as_deref()
    }

    #[must_use]
    pub const fn recovered_from_damage(&self) -> bool {
        self.recovered
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.layout.root()
    }

    pub(crate) fn settings_database(&self) -> &Connection {
        &self.db
    }

    #[must_use]
    pub const fn layout(&self) -> &Layout {
        &self.layout
    }

    #[must_use]
    pub fn instances(&self) -> &[InstanceRecord] {
        &self.index.instances
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&InstanceRecord> {
        self.index.instances.iter().find(|record| record.id == id)
    }

    #[must_use]
    pub fn favorites(&self) -> Vec<&InstanceRecord> {
        self.index
            .instances
            .iter()
            .filter(|record| record.favorite)
            .collect()
    }

    /// Instances by most recent play, never-played ones last (newest first).
    #[must_use]
    pub fn recent(&self) -> Vec<&InstanceRecord> {
        let mut records: Vec<_> = self.index.instances.iter().collect();
        records.sort_by(|a, b| {
            b.last_played
                .cmp(&a.last_played)
                .then(b.created_at.cmp(&a.created_at))
        });
        records
    }

    #[must_use]
    pub fn collections(&self) -> &[Collection] {
        &self.index.collections
    }

    /// Game files are shared through `meta/` (per release); the game
    /// directory is the instance's own profile.
    #[must_use]
    pub fn directories(&self, record: &InstanceRecord) -> LaunchDirectories {
        self.layout.launch_directories(record)
    }

    pub fn create(
        &mut self,
        request: NewInstance,
        now: u64,
    ) -> Result<&InstanceRecord, StoreError> {
        let id = self.suggest_id(&request.name)?;
        self.create_as(&id, request, InstanceSettings::default(), false, now)
    }

    /// The id a new instance of this name would get right now: unused in the
    /// library, with no profile folder already on disk and no game being
    /// staged for it.
    pub fn suggest_id(&self, name: &str) -> Result<String, StoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::InvalidName);
        }
        Ok(self.unique_id(name))
    }

    /// Creates an instance under an id from [`Self::suggest_id`], with the given
    /// settings and installed flag. Refuses an id that has been taken since
    /// (another creation, or a profile folder that appeared): publishing work
    /// staged for that id must not land on someone else's data.
    pub fn create_as(
        &mut self,
        id: &str,
        request: NewInstance,
        settings: InstanceSettings,
        installed: bool,
        now: u64,
    ) -> Result<&InstanceRecord, StoreError> {
        let name = request.name.trim();
        if name.is_empty() {
            return Err(StoreError::InvalidName);
        }
        crate::settings::validate_memory(settings.min_memory_mb, settings.max_memory_mb)
            .map_err(|_| StoreError::InvalidMemory)?;
        if self.get(id).is_some() || self.layout.profile(id).exists() {
            return Err(StoreError::IdTaken(id.to_owned()));
        }
        let mut next = self.index.clone();
        next.instances.push(InstanceRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            game_version: request.game_version,
            loader: request.loader,
            loader_version: request.loader_version,
            favorite: false,
            created_at: now,
            last_played: None,
            play_seconds: 0,
            installed,
            settings,
            source_project: None,
        });
        self.save(next)?;
        Ok(self
            .index
            .instances
            .last()
            .expect("an instance was just pushed"))
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), StoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::InvalidName);
        }
        self.edit(id, |record| record.name = name.to_owned())
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<(), StoreError> {
        self.edit(id, |record| record.favorite = favorite)
    }

    pub fn set_source_project(&mut self, id: &str, project: &str) -> Result<(), StoreError> {
        self.edit(id, |record| {
            record.source_project = Some(project.to_owned())
        })
    }

    pub fn mark_installed(&mut self, id: &str, installed: bool) -> Result<(), StoreError> {
        self.edit(id, |record| record.installed = installed)
    }

    pub fn update_settings(
        &mut self,
        id: &str,
        settings: InstanceSettings,
    ) -> Result<(), StoreError> {
        crate::settings::validate_memory(settings.min_memory_mb, settings.max_memory_mb)
            .map_err(|_| StoreError::InvalidMemory)?;
        settings
            .launch
            .validate()
            .map_err(|error| StoreError::InvalidLaunch(error.to_string()))?;
        let mut settings = settings;
        settings.launch = settings.launch.normalized();
        self.edit(id, |record| record.settings = settings)
    }

    /// Changes the game version and loader of an instance. The files of the
    /// new combination must already be prepared: only the record changes, in
    /// one library transaction, and the instance counts as installed.
    pub fn set_runtime(
        &mut self,
        id: &str,
        game_version: &str,
        loader: Loader,
        loader_version: Option<&str>,
    ) -> Result<(), StoreError> {
        let game_version = game_version.trim();
        if game_version.is_empty() {
            return Err(StoreError::InvalidName);
        }
        self.edit(id, |record| {
            record.game_version = game_version.to_owned();
            record.loader = loader;
            record.loader_version = loader_version.map(str::to_owned);
            record.installed = true;
        })
    }

    /// Records a finished play session.
    pub fn record_session(
        &mut self,
        id: &str,
        started: u64,
        seconds: u64,
    ) -> Result<(), StoreError> {
        self.edit(id, |record| {
            record.last_played = Some(started);
            record.play_seconds = record.play_seconds.saturating_add(seconds);
        })
    }

    /// Removes the record and its collection memberships. Files on disk are
    /// left for the caller to delete. A failed database commit preserves the record;
    /// filesystem deletion needs its own recovery policy.
    pub fn remove(&mut self, id: &str) -> Result<InstanceRecord, StoreError> {
        let position = self
            .index
            .instances
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| StoreError::UnknownInstance(id.to_owned()))?;
        let mut next = self.index.clone();
        let record = next.instances.remove(position);
        for collection in &mut next.collections {
            collection.members.retain(|member| member != id);
        }
        self.save(next)?;
        Ok(record)
    }

    pub(super) fn edit(
        &mut self,
        id: &str,
        change: impl FnOnce(&mut InstanceRecord),
    ) -> Result<(), StoreError> {
        let mut next = self.index.clone();
        let record = next
            .instances
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| StoreError::UnknownInstance(id.to_owned()))?;
        change(record);
        self.save(next)
    }

    pub(super) fn unique_id(&self, name: &str) -> String {
        // ASCII on purpose: the id names the game's folder, and other
        // characters in a game path can break launching (HMCL warns before
        // installing a pack under such a name). The display name keeps them.
        let slug: String = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let mut base = slug
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        if base.is_empty() {
            base = "instance".to_owned();
        }
        let mut candidate = base.clone();
        let mut counter = 2;
        // A profile folder already on disk (an orphan, or one being deleted)
        // is somebody's data: a new instance must not adopt it. A staged
        // game (an import or copy still running) will be published as its id.
        while self.get(&candidate).is_some()
            || self.layout.profile(&candidate).exists()
            || crate::staged::holds(&self.layout, &candidate)
        {
            candidate = format!("{base}-{counter}");
            counter += 1;
        }
        candidate
    }
}
