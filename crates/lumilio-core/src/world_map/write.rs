//! The host's half of writing a plugin's game file (plan W7): refuse when the
//! file is not what the plugin read, keep a copy of what is replaced outside
//! the game directory, and swap the new content in whole.
use lumilio_plugin_api::{FileInfo, PluginError};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Copies of one file kept in the backup directory.
pub const KEEP_BACKUPS: usize = 10;
const CONFLICT: &str = "map-edit-conflict";

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

/// A file's bytes and what it looked like when they were read.
pub fn describe(path: &Path) -> Result<(Vec<u8>, FileInfo), PluginError> {
    let bytes = fs::read(path).map_err(unavailable)?;
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_err(unavailable)?;
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
    let parent = path
        .parent()
        .ok_or_else(|| PluginError::InvalidInput("no parent directory".into()))?;
    match (expected, path.exists()) {
        (Some(expected), true) => {
            let (_, now) = describe(path)?;
            if &now != expected {
                return Err(unavailable(CONFLICT));
            }
            backup(path, label, backups)?;
        }
        (Some(_), false) => return Err(unavailable(CONFLICT)),
        (None, true) => return Err(unavailable(CONFLICT)),
        (None, false) => fs::create_dir_all(parent).map_err(unavailable)?,
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PluginError::InvalidInput("bad file name".into()))?;
    let temporary = parent.join(format!("{name}.lumilio-tmp"));
    let written = (|| {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(unavailable(error));
    }
    Ok(())
}

fn backup(path: &Path, label: &str, backups: &Path) -> Result<(), PluginError> {
    let folder = backups.join(label.replace(['/', '\\'], "__"));
    fs::create_dir_all(&folder).map_err(unavailable)?;
    let mut stamp = millis(SystemTime::now());
    // Two writes in one millisecond must not overwrite each other's copy.
    while folder.join(stamp.to_string()).exists() {
        stamp += 1;
    }
    fs::copy(path, folder.join(stamp.to_string())).map_err(unavailable)?;
    let mut copies: Vec<(i64, std::path::PathBuf)> = fs::read_dir(&folder)
        .map_err(unavailable)?
        .flatten()
        .filter_map(|entry| Some((entry.file_name().to_str()?.parse().ok()?, entry.path())))
        .collect();
    copies.sort();
    let extra = copies.len().saturating_sub(KEEP_BACKUPS);
    for (_, old) in copies.into_iter().take(extra) {
        let _ = fs::remove_file(old);
    }
    Ok(())
}
