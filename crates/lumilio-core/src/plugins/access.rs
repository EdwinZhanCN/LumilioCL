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

/// Whether `name` matches a grant's pattern, in which one `*` is any text.
fn name_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => {
            name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail)
        }
        None => name == pattern,
    }
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
    below(game_dir, &clean)
}

/// `clean` below `game_dir`, refusing a symbolic link on the way. The path
/// need not exist: once a part is missing, the parts after it are appended
/// unchecked, so the result is always the whole path asked for.
fn below(game_dir: &Path, clean: &Path) -> Result<PathBuf, PluginError> {
    let mut full = game_dir.to_path_buf();
    let mut parts = clean.components();
    while let Some(part) = parts.next() {
        full.push(part);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(PluginError::PermissionDenied);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                full.extend(parts.by_ref());
                break;
            }
            Err(error) => return Err(crate::world_map::write::io_error("inspect", &full, &error)),
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

/// The absolute path of a file the plugin may write: below a `WriteGameFiles`
/// grant, with a name the grant allows, and no link on the way. Like
/// [`resolve`], the file itself need not exist yet.
pub(super) fn resolve_write(
    game_dir: &Path,
    manifest: &Manifest,
    relative: &str,
) -> Result<PathBuf, PluginError> {
    let clean = plain(relative)?;
    let name = clean
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(PluginError::PermissionDenied)?;
    let granted = manifest.permissions.iter().any(|permission| {
        matches!(permission, Permission::WriteGameFiles { under, names }
            if clean.starts_with(under) && clean != Path::new(under) && name_matches(names, name))
    });
    if !granted {
        return Err(PluginError::PermissionDenied);
    }
    below(game_dir, &clean)
}

/// A readable file's bytes with the [`lumilio_plugin_api::FileInfo`] a later
/// write must present.
pub(super) fn read_info(
    game_dir: &Path,
    manifest: &Manifest,
    relative: &str,
) -> Result<(Vec<u8>, lumilio_plugin_api::FileInfo), PluginError> {
    let full = resolve(game_dir, manifest, relative)?;
    let meta =
        std::fs::metadata(&full).map_err(|error| PluginError::Unavailable(error.to_string()))?;
    if !meta.is_file() {
        return Err(PluginError::InvalidInput("not a file".into()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(PluginError::Unavailable("file is too large".into()));
    }
    crate::world_map::write::describe(&full)
}

pub(super) fn stat(
    game_dir: &Path,
    manifest: &Manifest,
    relative: &str,
) -> Result<Option<lumilio_plugin_api::FileStat>, PluginError> {
    let full = resolve(game_dir, manifest, relative)?;
    match std::fs::symlink_metadata(&full) {
        Ok(meta) if meta.is_file() => Ok(Some(lumilio_plugin_api::FileStat {
            len: meta.len(),
            modified_ms: crate::world_map::write::millis_of(meta.modified().ok()),
        })),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PluginError::Unavailable(error.to_string())),
    }
}

pub(super) fn read_range(
    game_dir: &Path,
    manifest: &Manifest,
    relative: &str,
    offset: u64,
    len: usize,
) -> Result<Vec<u8>, PluginError> {
    use std::io::{Read, Seek, SeekFrom};
    if len > lumilio_plugin_api::MAX_RANGE {
        return Err(PluginError::InvalidInput("range is too large".into()));
    }
    let full = resolve(game_dir, manifest, relative)?;
    let mut file =
        std::fs::File::open(&full).map_err(|error| PluginError::Unavailable(error.to_string()))?;
    let meta = file
        .metadata()
        .map_err(|error| PluginError::Unavailable(error.to_string()))?;
    if !meta.is_file() {
        return Err(PluginError::InvalidInput("not a file".into()));
    }
    // Past the end is an empty range, however far past (a seek beyond
    // `i64::MAX` would be an error instead).
    if offset >= meta.len() {
        return Ok(Vec::new());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| PluginError::Unavailable(error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(len as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| PluginError::Unavailable(error.to_string()))?;
    Ok(bytes)
}

pub(super) fn list_dir(
    game_dir: &Path,
    manifest: &Manifest,
    dir: &str,
    after: Option<&str>,
    limit: usize,
) -> Result<lumilio_plugin_api::DirPage, PluginError> {
    use lumilio_plugin_api::{DirEntry, DirPage, MAX_PAGE};
    let limit = limit.clamp(1, MAX_PAGE);
    let full = resolve(game_dir, manifest, dir)?;
    let mut names: Vec<DirEntry> = match std::fs::read_dir(&full) {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| {
                let kind = entry.file_type().ok()?;
                // Links are never followed, so they are not listed either.
                if kind.is_symlink() {
                    return None;
                }
                Some(DirEntry {
                    name: entry.file_name().to_str()?.to_owned(),
                    is_dir: kind.is_dir(),
                })
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(PluginError::Unavailable(error.to_string())),
    };
    names.sort_by(|a, b| a.name.cmp(&b.name));
    let start = after.map_or(0, |after| {
        names.partition_point(|entry| entry.name.as_str() <= after)
    });
    let rest = &names[start.min(names.len())..];
    let page: Vec<DirEntry> = rest.iter().take(limit).cloned().collect();
    let next = (rest.len() > limit)
        .then(|| page.last().map(|entry| entry.name.clone()))
        .flatten();
    Ok(DirPage {
        entries: page,
        next,
    })
}
