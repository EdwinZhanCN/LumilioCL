//! The library of instances a user owns, persisted in `launcher.db` under the
//! launcher root.
//!
//! The store is small and synchronous; callers on an interactive thread should
//! open and mutate it from a background task.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::launch::LaunchDirectories;
use crate::layout::Layout;
use crate::persist::{self, PersistError, Versioned};
use crate::tuning::InstanceLaunch;

/// Stored as SQLite's `user_version`; bump with a migration when tables change.
const SCHEMA_VERSION: u32 = 4;

/// SQLite integers are signed; timestamps and counters never get near the limit.
fn int<T: TryInto<i64>>(value: T) -> i64 {
    value.try_into().unwrap_or(i64::MAX)
}

fn unsigned(value: i64) -> u64 {
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
struct Index {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    instances: Vec<InstanceRecord>,
    #[serde(default)]
    collections: Vec<Collection>,
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
    layout: Layout,
    db: Connection,
    index: Index,
    recovered: bool,
    preserved: Option<PathBuf>,
}

impl InstanceStore {
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

    fn connect(path: &Path) -> Result<Connection, StoreError> {
        let db = Connection::open(path)?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::NewerSchema(version));
        }
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS instances (
                position INTEGER NOT NULL,
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                game_version TEXT NOT NULL,
                loader TEXT NOT NULL,
                loader_version TEXT,
                favorite INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                last_played INTEGER,
                play_seconds INTEGER NOT NULL,
                installed INTEGER NOT NULL,
                java_path TEXT,
                max_memory_mb INTEGER,
                min_memory_mb INTEGER,
                jvm_arguments TEXT NOT NULL,
                tuning TEXT,
                source_project TEXT
            );
            CREATE TABLE IF NOT EXISTS collections (
                position INTEGER NOT NULL,
                name TEXT PRIMARY KEY NOT NULL
            );
            CREATE TABLE IF NOT EXISTS collection_members (
                collection TEXT NOT NULL,
                position INTEGER NOT NULL,
                instance TEXT NOT NULL,
                PRIMARY KEY (collection, instance)
            );
            CREATE TABLE IF NOT EXISTS world_map_seeds (
                instance TEXT NOT NULL,
                seed INTEGER NOT NULL,
                version TEXT NOT NULL,
                PRIMARY KEY (instance, seed, version)
            );
            CREATE TABLE IF NOT EXISTS world_map_links (
                instance TEXT NOT NULL,
                folder TEXT NOT NULL,
                xaero_dir TEXT NOT NULL,
                PRIMARY KEY (instance, folder)
            );",
        )?;
        // Schema 1 had no `tuning` column. Keep the old file beside the new
        // one before the first change, then add the column.
        let has_tuning = db
            .prepare("SELECT 1 FROM pragma_table_info('instances') WHERE name = 'tuning'")?
            .exists([])?;
        if !has_tuning {
            if version == 1 {
                let mut backup = path.as_os_str().to_owned();
                backup.push(".v1");
                let backup = PathBuf::from(backup);
                if !backup.exists() {
                    // Best effort: a failed copy must not block the upgrade of data
                    // that is otherwise intact.
                    let _ = fs::copy(path, backup);
                }
            }
            db.execute_batch("ALTER TABLE instances ADD COLUMN tuning TEXT;")?;
        }
        // Schema 3 added `source_project`. An older build would not know the
        // column, so the version moves with it.
        let has_source = db
            .prepare("SELECT 1 FROM pragma_table_info('instances') WHERE name = 'source_project'")?
            .exists([])?;
        if !has_source {
            if version == 2 {
                let mut backup = path.as_os_str().to_owned();
                backup.push(".v2");
                let backup = PathBuf::from(backup);
                if !backup.exists() {
                    let _ = fs::copy(path, backup);
                }
            }
            db.execute_batch("ALTER TABLE instances ADD COLUMN source_project TEXT;")?;
        }
        if version != 0 {
            db.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(db)
    }

    /// Reads the tables into memory; a fresh database first imports the
    /// legacy JSON library when one exists.
    fn load(&mut self) -> Result<(), StoreError> {
        let version: u32 = self
            .db
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == 0 {
            let legacy = self.layout.legacy_library();
            if legacy.exists() {
                let loaded = persist::load::<Index>(&legacy, 1)?;
                self.recovered |= loaded.recovered;
                if self.preserved.is_none() {
                    self.preserved = loaded.preserved;
                }
                self.index = loaded.value;
            }
            self.save(self.index.clone())?;
            if legacy.exists() && !self.recovered {
                let mut done = legacy.clone().into_os_string();
                done.push(".imported");
                fs::rename(&legacy, PathBuf::from(done))?;
            }
            return Ok(());
        }
        self.index = self.read()?;
        Ok(())
    }

    fn read(&self) -> Result<Index, StoreError> {
        let mut index = Index::default();
        let mut statement = self.db.prepare(
            "SELECT id, name, game_version, loader, loader_version, favorite, created_at,
                    last_played, play_seconds, installed, java_path, max_memory_mb,
                    min_memory_mb, jvm_arguments, tuning, source_project
             FROM instances ORDER BY position",
        )?;
        let rows = statement.query_map([], |row| {
            let loader: String = row.get(3)?;
            let java_path: Option<String> = row.get(10)?;
            let jvm: String = row.get(13)?;
            let tuning: Option<String> = row.get(14)?;
            Ok(InstanceRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                game_version: row.get(2)?,
                loader: serde_json::from_value(serde_json::Value::String(loader))
                    .unwrap_or_default(),
                loader_version: row.get(4)?,
                favorite: row.get(5)?,
                created_at: unsigned(row.get(6)?),
                last_played: row.get::<_, Option<i64>>(7)?.map(unsigned),
                play_seconds: unsigned(row.get(8)?),
                installed: row.get(9)?,
                settings: InstanceSettings {
                    java_path: java_path.map(PathBuf::from),
                    max_memory_mb: row.get(11)?,
                    min_memory_mb: row.get(12)?,
                    jvm_arguments: serde_json::from_str(&jvm).unwrap_or_default(),
                    launch: tuning
                        .and_then(|text| serde_json::from_str(&text).ok())
                        .unwrap_or_default(),
                },
                source_project: row.get(15)?,
            })
        })?;
        for row in rows {
            index.instances.push(row?);
        }
        let mut statement = self
            .db
            .prepare("SELECT name FROM collections ORDER BY position")?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for name in names {
            let mut statement = self.db.prepare(
                "SELECT instance FROM collection_members WHERE collection = ?1 ORDER BY position",
            )?;
            let members = statement
                .query_map([&name], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            index.collections.push(Collection { name, members });
        }
        Ok(index)
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
    /// library and with no profile folder already on disk.
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

    pub fn create_collection(&mut self, name: &str) -> Result<(), StoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::InvalidName);
        }
        if self.index.collections.iter().any(|c| c.name == name) {
            return Err(StoreError::DuplicateCollection(name.to_owned()));
        }
        let mut next = self.index.clone();
        next.collections.push(Collection {
            name: name.to_owned(),
            members: Vec::new(),
        });
        self.save(next)
    }

    pub fn delete_collection(&mut self, name: &str) -> Result<(), StoreError> {
        let mut next = self.index.clone();
        let before = next.collections.len();
        next.collections.retain(|c| c.name != name);
        if next.collections.len() == before {
            return Err(StoreError::UnknownCollection(name.to_owned()));
        }
        self.save(next)
    }

    /// Gives a collection another name; its games stay in it, in their place.
    pub fn rename_collection(&mut self, from: &str, to: &str) -> Result<(), StoreError> {
        let to = to.trim();
        if to.is_empty() {
            return Err(StoreError::InvalidName);
        }
        let mut next = self.index.clone();
        if from != to && next.collections.iter().any(|c| c.name == to) {
            return Err(StoreError::DuplicateCollection(to.to_owned()));
        }
        let target = next
            .collections
            .iter_mut()
            .find(|c| c.name == from)
            .ok_or_else(|| StoreError::UnknownCollection(from.to_owned()))?;
        target.name = to.to_owned();
        self.save(next)
    }

    /// Puts an instance in exactly these collections and in no other, in one
    /// commit. A name that is not a collection changes nothing.
    pub fn set_collections_of(
        &mut self,
        instance: &str,
        wanted: &[String],
    ) -> Result<(), StoreError> {
        if self.get(instance).is_none() {
            return Err(StoreError::UnknownInstance(instance.to_owned()));
        }
        if let Some(unknown) = wanted
            .iter()
            .find(|name| !self.index.collections.iter().any(|c| &c.name == *name))
        {
            return Err(StoreError::UnknownCollection(unknown.clone()));
        }
        let mut next = self.index.clone();
        for collection in &mut next.collections {
            let member = wanted.contains(&collection.name);
            let present = collection.members.iter().any(|id| id == instance);
            match (member, present) {
                (true, false) => collection.members.push(instance.to_owned()),
                (false, true) => collection.members.retain(|id| id != instance),
                _ => {}
            }
        }
        self.save(next)
    }

    /// Adds or removes an instance from a collection; adding twice is a no-op.
    pub fn set_membership(
        &mut self,
        collection: &str,
        instance: &str,
        member: bool,
    ) -> Result<(), StoreError> {
        if self.get(instance).is_none() {
            return Err(StoreError::UnknownInstance(instance.to_owned()));
        }
        let mut next = self.index.clone();
        let target = next
            .collections
            .iter_mut()
            .find(|c| c.name == collection)
            .ok_or_else(|| StoreError::UnknownCollection(collection.to_owned()))?;
        target.members.retain(|id| id != instance);
        if member {
            target.members.push(instance.to_owned());
        }
        self.save(next)
    }

    fn edit(
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

    fn unique_id(&self, name: &str) -> String {
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
        // is somebody's data: a new instance must not adopt it.
        while self.get(&candidate).is_some() || self.layout.profile(&candidate).exists() {
            candidate = format!("{base}-{counter}");
            counter += 1;
        }
        candidate
    }

    /// Replaces the tables in one transaction, so a crash leaves either the
    /// old or the new library, never a torn one.
    fn save(&mut self, next: Index) -> Result<(), StoreError> {
        let transaction = self.db.transaction()?;
        transaction.execute_batch(
            "DELETE FROM instances; DELETE FROM collections; DELETE FROM collection_members;",
        )?;
        for (position, record) in next.instances.iter().enumerate() {
            let loader = serde_json::to_value(record.loader)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            let jvm = serde_json::to_string(&record.settings.jvm_arguments)
                .map_err(|error| StoreError::Encode(error.to_string()))?;
            let tuning = if record.settings.launch.is_default() {
                None
            } else {
                Some(
                    serde_json::to_string(&record.settings.launch)
                        .map_err(|error| StoreError::Encode(error.to_string()))?,
                )
            };
            transaction.execute(
                "INSERT INTO instances VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
                params![
                    int(position),
                    record.id,
                    record.name,
                    record.game_version,
                    loader,
                    record.loader_version,
                    record.favorite,
                    int(record.created_at),
                    record.last_played.map(int),
                    int(record.play_seconds),
                    record.installed,
                    record
                        .settings
                        .java_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    record.settings.max_memory_mb,
                    record.settings.min_memory_mb,
                    jvm,
                    tuning,
                    record.source_project,
                ],
            )?;
        }
        for (position, collection) in next.collections.iter().enumerate() {
            transaction.execute(
                "INSERT INTO collections VALUES (?1, ?2)",
                params![int(position), collection.name],
            )?;
            for (at, member) in collection.members.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO collection_members VALUES (?1, ?2, ?3)",
                    params![collection.name, int(at), member],
                )?;
            }
        }
        transaction.execute(
            "DELETE FROM world_map_seeds WHERE instance NOT IN (SELECT id FROM instances)",
            [],
        )?;
        transaction.execute(
            "DELETE FROM world_map_links WHERE instance NOT IN (SELECT id FROM instances)",
            [],
        )?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
        self.index = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
