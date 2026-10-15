//! Schematic edits written to a game's files (plan schematic-studio, D8).
//!
//! Every write keeps the replaced file in the instance's launcher data, writes
//! only when the file still looks as it did when it was read, and appends one
//! line to the instance's operation log. The log and its backups are what the
//! studio's history lists and restores from; the game directory holds neither.
//!
//! These functions do blocking file I/O; the service runs them off the async
//! threads and holds the instance so that a running game keeps them out.

use crate::plugins::write::{describe, io_error, remove_checked_keeping, write_checked_keeping};
use lumilio_plugin_api::{FileInfo, PluginError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

pub mod schemes;
#[cfg(test)]
mod tests;

/// The operation log inside an instance's schematic folder: one JSON object per line.
pub const LOG_FILE: &str = "log.jsonl";
/// Backups inside an instance's schematic folder.
pub const BACKUP_DIR: &str = "backups";
/// The `operation` of a restore's own log line.
pub const RESTORE: &str = "restore";

/// What a caller is doing to a schematic, as the log records it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicOperation {
    /// A short, stable name: `replace`, `convert`, `metadata`.
    pub kind: String,
    /// The parameters in a line, for the history to show.
    pub summary: String,
}

/// One line of an instance's schematic operation log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SchematicLogEntry {
    /// The line's number in the log, from 1. The log is only appended to, so
    /// a number keeps naming the same operation.
    #[serde(skip)]
    pub index: usize,
    /// Seconds since the Unix epoch.
    pub time: u64,
    pub operation: String,
    /// The game-relative path of the file, with `/` separators.
    pub file: String,
    /// The copy of the file as it was before, relative to the schematic
    /// folder; `None` when there was no file to keep.
    pub backup: Option<String>,
    pub summary: String,
    /// For a restore, the line whose backup was put back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restores: Option<usize>,
}

/// Replaces `game_file` (named `file` relative to the game directory) with
/// `bytes` when it still matches `expected`: back up, write, log. Without
/// `expected` it writes a new file, and refuses when one is there (a
/// conversion saved beside its original, plan schematic-studio T23).
pub fn apply(
    folder: &Path,
    game_file: &Path,
    file: &str,
    bytes: &[u8],
    expected: Option<&FileInfo>,
    operation: &SchematicOperation,
    time: u64,
) -> Result<SchematicLogEntry, PluginError> {
    let copy = write_checked_keeping(
        game_file,
        file,
        bytes,
        expected,
        &folder.join(BACKUP_DIR),
        None,
    )?;
    let entry = SchematicLogEntry {
        index: 0,
        time,
        operation: operation.kind.clone(),
        file: file.to_owned(),
        backup: copy.map(|copy| relative(folder, &copy)).transpose()?,
        summary: operation.summary.clone(),
        restores: None,
    };
    append(folder, entry)
}

/// Takes `game_file` out of the game directory into the backups when it still
/// matches `expected`, and logs it. Nothing is ever deleted: restoring the
/// line puts the file back (plan schematic-studio, D9).
pub fn remove(
    folder: &Path,
    game_file: &Path,
    file: &str,
    expected: &FileInfo,
    operation: &SchematicOperation,
    time: u64,
) -> Result<SchematicLogEntry, PluginError> {
    let copy = remove_checked_keeping(game_file, file, expected, &folder.join(BACKUP_DIR))?;
    let entry = SchematicLogEntry {
        index: 0,
        time,
        operation: operation.kind.clone(),
        file: file.to_owned(),
        backup: Some(relative(folder, &copy)?),
        summary: operation.summary.clone(),
        restores: None,
    };
    append(folder, entry)
}

/// One copy kept in the backups folder (plan schematic-studio, T40), whether
/// or not a log line names it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeptBackup {
    /// Relative to the schematic folder, as a log line names it
    /// (`backups/schematics__house.litematic/1700000000000`).
    pub path: String,
    /// The game-relative file it is a copy of, read back from its folder's
    /// name (`/` is kept as `__`).
    pub file: String,
    pub size: u64,
    /// Seconds since the Unix epoch, from the copy's name.
    pub time: u64,
}

/// Every copy in the backups folder, newest first. Nothing kept is empty.
pub fn backups(folder: &Path) -> Result<Vec<KeptBackup>, PluginError> {
    let root = folder.join(BACKUP_DIR);
    let listing = match fs::read_dir(&root) {
        Ok(listing) => listing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("read", &root, &error)),
    };
    let mut kept = Vec::new();
    for label in listing.flatten() {
        let Some(name) = label.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(copies) = fs::read_dir(label.path()) else {
            continue;
        };
        for copy in copies.flatten() {
            let Some(stamp) = copy.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let (Ok(millis), Ok(meta)) = (stamp.parse::<u64>(), copy.metadata()) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            kept.push(KeptBackup {
                path: format!("{BACKUP_DIR}/{name}/{stamp}"),
                file: name.replace("__", "/"),
                size: meta.len(),
                time: millis / 1000,
            });
        }
    }
    kept.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| b.path.cmp(&a.path)));
    Ok(kept)
}

/// Puts the backup of `entry` back at `game_file`. What is there now is backed
/// up first, so a restore can itself be undone, and the restore is logged.
/// A line that made a new file (no backup) is restored by taking the file
/// into the backups, as a deletion is (T23).
pub fn restore(
    folder: &Path,
    game_file: &Path,
    entry: &SchematicLogEntry,
    time: u64,
) -> Result<SchematicLogEntry, PluginError> {
    let Some(backup) = entry.backup.as_deref() else {
        let (_, current) = describe(game_file)?;
        let copy =
            remove_checked_keeping(game_file, &entry.file, &current, &folder.join(BACKUP_DIR))?;
        let restored = SchematicLogEntry {
            index: 0,
            time,
            operation: RESTORE.into(),
            file: entry.file.clone(),
            backup: Some(relative(folder, &copy)?),
            summary: String::new(),
            restores: Some(entry.index),
        };
        return append(folder, restored);
    };
    put_back(
        folder,
        game_file,
        &entry.file,
        backup,
        Some(entry.index),
        time,
    )
}

/// Puts the kept copy `backup` (a [`KeptBackup::path`]) back at `game_file`,
/// the game file `file`, as [`restore`] does for a log line: for a copy no
/// line names (T40). What is there now is backed up first; the restore is
/// logged with the copy in its summary.
pub fn restore_backup(
    folder: &Path,
    game_file: &Path,
    file: &str,
    backup: &str,
    time: u64,
) -> Result<SchematicLogEntry, PluginError> {
    put_back(folder, game_file, file, backup, None, time)
}

fn put_back(
    folder: &Path,
    game_file: &Path,
    file: &str,
    backup: &str,
    restores: Option<usize>,
    time: u64,
) -> Result<SchematicLogEntry, PluginError> {
    let source = backup_path(folder, backup)?;
    let bytes = fs::read(&source).map_err(|error| io_error("read", &source, &error))?;
    let current = match fs::symlink_metadata(game_file) {
        Ok(_) => Some(describe(game_file)?.1),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(io_error("inspect", game_file, &error)),
    };
    let copy = write_checked_keeping(
        game_file,
        file,
        &bytes,
        current.as_ref(),
        &folder.join(BACKUP_DIR),
        None,
    )?;
    let restored = SchematicLogEntry {
        index: 0,
        time,
        operation: RESTORE.into(),
        file: file.to_owned(),
        backup: copy.map(|copy| relative(folder, &copy)).transpose()?,
        // A line's restore names the line; a loose copy's names the copy.
        summary: if restores.is_some() {
            String::new()
        } else {
            backup.to_owned()
        },
        restores,
    };
    append(folder, restored)
}

/// The log's entries, newest first. A line that cannot be read is skipped
/// but still counts, so numbers stay stable. No log means no operations.
pub fn history(folder: &Path) -> Result<Vec<SchematicLogEntry>, PluginError> {
    let path = folder.join(LOG_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("read", &path, &error)),
    };
    let mut entries: Vec<_> = text
        .lines()
        .enumerate()
        .filter_map(|(at, line)| {
            let mut entry: SchematicLogEntry = serde_json::from_str(line).ok()?;
            entry.index = at + 1;
            Some(entry)
        })
        .collect();
    entries.reverse();
    Ok(entries)
}

/// One entry by its line number.
pub fn entry(folder: &Path, index: usize) -> Result<SchematicLogEntry, PluginError> {
    history(folder)?
        .into_iter()
        .find(|entry| entry.index == index)
        .ok_or_else(|| PluginError::InvalidInput("no such operation".into()))
}

fn append(folder: &Path, mut entry: SchematicLogEntry) -> Result<SchematicLogEntry, PluginError> {
    // A first line may come before any backup made the folder (a new file).
    fs::create_dir_all(folder).map_err(|error| io_error("create folder", folder, &error))?;
    let path = folder.join(LOG_FILE);
    let mut line = serde_json::to_string(&entry)
        .map_err(|error| PluginError::Unavailable(error.to_string()))?;
    line.push('\n');
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(io_error("read", &path, &error)),
    };
    // A line cut short by a crash is closed, not continued.
    if !text.is_empty() && !text.ends_with('\n') {
        line.insert(0, '\n');
    }
    let lines = text.lines().count();
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| io_error("open", &path, &error))?;
    log.write_all(line.as_bytes())
        .and_then(|()| log.sync_all())
        .map_err(|error| io_error("append to", &path, &error))?;
    entry.index = lines + 1;
    Ok(entry)
}

/// `copy` relative to `folder`, with `/` separators.
fn relative(folder: &Path, copy: &Path) -> Result<String, PluginError> {
    let inside = copy
        .strip_prefix(folder)
        .map_err(|_| PluginError::Unavailable("backup outside its folder".into()))?;
    Ok(inside
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

/// A logged backup's path, which must stay inside the backup folder: the log
/// is a file on disk and is not trusted to point elsewhere.
fn backup_path(folder: &Path, backup: &str) -> Result<PathBuf, PluginError> {
    let path = Path::new(backup);
    let plain = path
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    if !plain || !path.starts_with(BACKUP_DIR) || path == Path::new(BACKUP_DIR) {
        return Err(PluginError::InvalidInput("bad backup path".into()));
    }
    Ok(folder.join(path))
}
