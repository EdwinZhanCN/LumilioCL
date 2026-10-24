//! `launcher.db`: the one place its connections are made, its schema version
//! kept and its migrations run. The library, game settings and world map
//! each query their own tables through it.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// Stored as SQLite's `user_version`; bump with a migration when tables change.
pub(crate) const SCHEMA_VERSION: u32 = 6;

#[derive(Debug)]
pub(crate) enum DatabaseError {
    /// Written by a newer launcher; must not be changed.
    NewerSchema(u32),
    Sqlite(rusqlite::Error),
}

impl Display for DatabaseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewerSchema(version) => {
                write!(
                    f,
                    "database was written by a newer launcher (schema {version})"
                )
            }
            Self::Sqlite(error) => write!(f, "database failed: {error}"),
        }
    }
}

impl Error for DatabaseError {}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Opens `launcher.db`, creating every table the launcher keeps there and
/// bringing an older schema up to date. Before a change an old file cannot
/// read, a copy of it is kept beside it.
pub(crate) fn open(path: &Path) -> Result<Connection, DatabaseError> {
    let db = Connection::open(path)?;
    let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(DatabaseError::NewerSchema(version));
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
        CREATE TABLE IF NOT EXISTS game_settings_presets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1,
            settings TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS game_settings_bindings (
            instance TEXT PRIMARY KEY NOT NULL,
            preset INTEGER NOT NULL,
            state TEXT NOT NULL
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
        );
        CREATE TABLE IF NOT EXISTS world_map_annotations (
            id INTEGER PRIMARY KEY,
            instance TEXT NOT NULL,
            world TEXT NOT NULL,
            dimension TEXT NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('marker', 'route')),
            name TEXT NOT NULL,
            color INTEGER NOT NULL,
            points TEXT NOT NULL,
            linked_source TEXT,
            linked_raw_id TEXT
        );",
    )?;
    // Schema 5's initial preset payload contained only values. Schema 6
    // retains those values when a field is excluded from sharing.
    if version == 5 {
        db.execute_batch("UPDATE game_settings_presets SET settings = json_object('values', json(settings), 'excluded', json('[]'));")?;
    }
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

/// A connection for queries of one's own tables; the schema stays [`open`]'s.
pub(crate) fn connect(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open(path)
}
