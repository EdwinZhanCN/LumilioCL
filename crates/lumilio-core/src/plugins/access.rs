//! Read-only game file access for plugins. Every path is relative to the game
//! directory and must lie below a `ReadGameFiles` grant; `..`, absolute paths
//! and symbolic links are refused.

use std::path::{Component, Path, PathBuf};

use lumilio_plugin_api::{Manifest, Permission, PluginError};

/// Largest file a plugin may read.
pub(super) const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_LISTED: usize = 5000;
const MAX_DEPTH: usize = 8;

fn grants(manifest: &Manifest) -> Vec<PathBuf> {
    manifest
        .permissions
        .iter()
        .filter_map(|permission| match permission {
            Permission::ReadGameFiles { under } => Some(PathBuf::from(under)),
            _ => None,
        })
        .collect()
}

/// Splits a plugin-supplied path into plain components, or refuses it.
fn plain(path: &str) -> Result<PathBuf, PluginError> {
    let mut clean = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err(PluginError::PermissionDenied),
        }
    }
    Ok(clean)
}

/// The absolute path of `relative`, if a grant covers it and no part of it is
/// a symbolic link. The path itself need not exist yet.
pub(super) fn resolve(
    game_dir: &Path,
    manifest: &Manifest,
    relative: &str,
) -> Result<PathBuf, PluginError> {
    let clean = plain(relative)?;
    if !grants(manifest)
        .iter()
        .any(|under| clean.starts_with(under) || under.as_os_str().is_empty())
    {
        return Err(PluginError::PermissionDenied);
    }
    let mut full = game_dir.to_path_buf();
    for part in clean.components() {
        full.push(part);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(PluginError::PermissionDenied);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(PluginError::Unavailable(error.to_string())),
        }
    }
    Ok(full)
}

pub(super) fn read(
    game_dir: &Path,
    manifest: &Manifest,
    relative: &str,
) -> Result<Vec<u8>, PluginError> {
    let full = resolve(game_dir, manifest, relative)?;
    let meta =
        std::fs::metadata(&full).map_err(|error| PluginError::Unavailable(error.to_string()))?;
    if !meta.is_file() {
        return Err(PluginError::InvalidInput("not a file".into()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(PluginError::Unavailable("file is too large".into()));
    }
    std::fs::read(&full).map_err(|error| PluginError::Unavailable(error.to_string()))
}

pub(super) fn list(
    game_dir: &Path,
    manifest: &Manifest,
    dir: &str,
) -> Result<Vec<String>, PluginError> {
    let root = resolve(game_dir, manifest, dir)?;
    let mut found = Vec::new();
    match std::fs::symlink_metadata(&root) {
        Ok(meta) if meta.is_dir() => walk(game_dir, &root, 0, &mut found),
        Ok(_) | Err(_) => {}
    }
    found.sort();
    Ok(found)
}

fn walk(game_dir: &Path, dir: &Path, depth: usize, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if found.len() >= MAX_LISTED {
            return;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // Symbolic links are never followed.
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                walk(game_dir, &path, depth + 1, found);
            }
        } else if let Ok(relative) = path.strip_prefix(game_dir) {
            found.push(
                relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
}
