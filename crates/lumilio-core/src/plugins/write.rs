//! The host's half of writing a game file for a plugin (plan W7) or for the
//! schematic studio (plan schematic-studio, D8): refuse when the file is not
//! what was read, keep a copy of what is replaced outside the game directory,
//! and swap the new content in whole.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lumilio_plugin_api::{FileInfo, PluginError};
use sha2::{Digest, Sha256};

/// Copies of one file kept in the backup directory.
pub const KEEP_BACKUPS: usize = 10;
/// The error text of a write refused because the file changed since it was read.
pub const CONFLICT: &str = "map-edit-conflict";

/// A modification time as milliseconds since the epoch, 0 when unknown.
pub fn millis_of(time: Option<SystemTime>) -> i64 {
    time.map_or(0, millis)
}

fn millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
    })
}

fn unavailable(error: impl ToString) -> PluginError {
    PluginError::Unavailable(error.to_string())
}

/// An I/O failure that names what was being done and to which path. A bare
/// `Not a directory (os error 20)` does not say which folder on the way is
/// really a file.
pub fn io_error(action: &str, path: &Path, error: &std::io::Error) -> PluginError {
    PluginError::Unavailable(format!("{action} {}: {error}", path.display()))
}

/// A file's bytes and what it looked like when they were read.
pub fn describe(path: &Path) -> Result<(Vec<u8>, FileInfo), PluginError> {
    let bytes = fs::read(path).map_err(|error| io_error("read", path, &error))?;
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_err(|error| io_error("stat", path, &error))?;
    let info = FileInfo {
        len: bytes.len() as u64,
        modified_ms: millis(modified),
        sha256: Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    };
    Ok((bytes, info))
}

/// Replaces `path` with `bytes`.
///
/// `expected` is what the plugin saw when it read the file: any difference in
/// length, modification time or hash means someone else changed it, and nothing
/// is written. `None` means a new file, which must not exist. The old content
/// is copied into `backups/<name>/<time>` first, where `name` is `label` (the
/// game-relative path with its separators flattened) so two worlds' files of
/// the same name never share a folder, and the new content is written beside
/// the file and renamed over it.
pub fn write_checked(
    path: &Path,
    label: &str,
    bytes: &[u8],
    expected: Option<&FileInfo>,
    backups: &Path,
) -> Result<(), PluginError> {
    write_checked_keeping(path, label, bytes, expected, backups, Some(KEEP_BACKUPS)).map(drop)
}

/// [`write_checked`] that keeps at most `keep` copies of the file (`None`
/// keeps them all) and returns the copy it made, if the file existed.
pub fn write_checked_keeping(
    path: &Path,
    label: &str,
    bytes: &[u8],
    expected: Option<&FileInfo>,
    backups: &Path,
    keep: Option<usize>,
) -> Result<Option<PathBuf>, PluginError> {
    let parent = path
        .parent()
        .ok_or_else(|| PluginError::InvalidInput("no parent directory".into()))?;
    let copy = match (expected, path.exists()) {
        (Some(expected), true) => {
            let (_, now) = describe(path)?;
            if &now != expected {
                return Err(unavailable(CONFLICT));
            }
            Some(backup(path, label, backups, keep)?)
        }
        (Some(_), false) => return Err(unavailable(CONFLICT)),
        (None, true) => return Err(unavailable(CONFLICT)),
        (None, false) => {
            fs::create_dir_all(parent)
                .map_err(|error| io_error("create folder", parent, &error))?;
            None
        }
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PluginError::InvalidInput("bad file name".into()))?;
    let temporary = parent.join(format!("{name}.lumilio-tmp"));
    let written = (|| {
        let mut file = fs::File::create(&temporary)
            .map_err(|error| io_error("create file", &temporary, &error))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| io_error("write file", &temporary, &error))?;
        fs::rename(&temporary, path).map_err(|error| io_error("replace file", path, &error))
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(copy)
}

/// Moves `path` into the backup directory instead of deleting it: refused
/// unless the file still matches `expected`, copied like a replaced file
/// (kept, never rotated) and only then removed. Returns the copy.
pub fn remove_checked_keeping(
    path: &Path,
    label: &str,
    expected: &FileInfo,
    backups: &Path,
) -> Result<PathBuf, PluginError> {
    if !path.exists() {
        return Err(unavailable(CONFLICT));
    }
    let (_, now) = describe(path)?;
    if &now != expected {
        return Err(unavailable(CONFLICT));
    }
    let copy = backup(path, label, backups, None)?;
    fs::remove_file(path).map_err(|error| io_error("remove file", path, &error))?;
    Ok(copy)
}

fn backup(
    path: &Path,
    label: &str,
    backups: &Path,
    keep: Option<usize>,
) -> Result<PathBuf, PluginError> {
    let folder = backups.join(label.replace(['/', '\\'], "__"));
    fs::create_dir_all(&folder)
        .map_err(|error| io_error("create backup folder", &folder, &error))?;
    let mut copies: Vec<(i64, std::path::PathBuf)> = fs::read_dir(&folder)
        .map_err(|error| io_error("read backup folder", &folder, &error))?
        .flatten()
        .filter_map(|entry| Some((entry.file_name().to_str()?.parse().ok()?, entry.path())))
        .collect();
    copies.sort();
    let mut stamp = millis(SystemTime::now());
    // Collision stamps can run ahead of the clock. The next copy must stay
    // newer than every retained copy, even after older names have been freed.
    if let Some((latest, _)) = copies.last() {
        stamp = stamp.max(
            latest
                .checked_add(1)
                .ok_or_else(|| unavailable("backup timestamp overflow"))?,
        );
    }
    let destination = folder.join(stamp.to_string());
    fs::copy(path, &destination).map_err(|error| io_error("back up to", &destination, &error))?;
    copies.push((stamp, destination.clone()));
    let extra = keep.map_or(0, |keep| copies.len().saturating_sub(keep));
    for (_, old) in copies.into_iter().take(extra) {
        let _ = fs::remove_file(old);
    }
    Ok(destination)
}
