//! Finding, and removing, shared game files that no game uses (`ADR 0017`).
//!
//! Shared files live under `meta/`: version folders (a release's or loader's
//! json and jar), natives extracted per version, libraries, and assets. A
//! file is *unused* when no game's version needs it. The scan is read-only and
//! conservative: when anything cannot be classified, nothing it touches is
//! offered.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::instance::{InstanceRecord, Loader};
use crate::layout::Layout;

#[derive(Debug)]
pub enum ReclaimError {
    Io(io::Error),
    /// A game's version metadata could not be read, so what it needs is not
    /// known and nothing may be removed.
    Unreadable(String),
}

impl Display for ReclaimError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not look through the shared files: {error}"),
            Self::Unreadable(what) => write!(
                f,
                "the version information of {what:?} cannot be read, so nothing is offered for removal"
            ),
        }
    }
}

impl Error for ReclaimError {}

impl From<io::Error> for ReclaimError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Why a part of the shared files was left alone on purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeptReason {
    /// A Forge / NeoForge game is installed: the libraries its installer made
    /// have no manifest, so the library folder is left as it is.
    InstallerLibraries,
    /// An asset index of a needed version cannot be read, so the asset
    /// objects are left as they are.
    UnreadableAssetIndex,
}

/// What the scan found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Reclaimable {
    /// Files (or version/natives folders, counted as one) nobody uses, with
    /// their sizes. Everything is below `meta/`.
    pub unused: Vec<(PathBuf, u64)>,
    /// Parts that were left alone on purpose, and why.
    pub kept: Vec<KeptReason>,
}

impl Reclaimable {
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.unused.iter().map(|(_, size)| size).sum()
    }
}

/// Whether a version folder's name is the one a game's loader makes for it.
fn is_profile_of(name: &str, record: &InstanceRecord) -> bool {
    let (Some(loader_version), game) = (record.loader_version.as_deref(), &record.game_version)
    else {
        return false;
    };
    match record.loader {
        Loader::Vanilla => false,
        Loader::Fabric => name == format!("fabric-loader-{loader_version}-{game}"),
        Loader::Quilt => name == format!("quilt-loader-{loader_version}-{game}"),
        // Installer-made profiles are named in several styles; any that holds
        // the loader's version and its name belongs to the game.
        Loader::Forge => {
            name.contains("forge") && !name.contains("neoforge") && name.contains(loader_version)
        }
        Loader::NeoForge => name.contains("neoforge") && name.contains(loader_version),
    }
}

/// Path of a library below `libraries/`, from its maven coordinates
/// `group:artifact:version[:classifier][@ext]`.
fn coordinate_path(name: &str) -> Option<String> {
    let (coords, extension) = name.split_once('@').map_or((name, "jar"), |(c, e)| (c, e));
    let mut parts = coords.split(':');
    let (group, artifact, version) = (parts.next()?, parts.next()?, parts.next()?);
    let classifier = parts.next();
    let file = match classifier {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{extension}"),
        None => format!("{artifact}-{version}.{extension}"),
    };
    Some(format!(
        "{}/{artifact}/{version}/{file}",
        group.replace('.', "/")
    ))
}

/// Everything below `value` named `path` inside a `downloads` object.
fn collect_paths(value: &Value, in_downloads: bool, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if in_downloads
                    && key == "path"
                    && let Some(path) = inner.as_str()
                {
                    out.insert(path.to_owned());
                }
                collect_paths(inner, in_downloads || key == "downloads", out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_paths(item, in_downloads, out);
            }
        }
        _ => {}
    }
}

fn size_of(path: &Path) -> u64 {
    crate::storage::directory_size(path)
}

/// Files below `dir` (not folders).
fn files_below(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => files_below(&path, out),
            Ok(_) => out.push(path),
            Err(_) => {}
        }
    }
}

/// Works out which shared files `instances` do not need. Read-only.
pub fn scan(layout: &Layout, instances: &[InstanceRecord]) -> Result<Reclaimable, ReclaimError> {
    let mut report = Reclaimable::default();
    let versions = layout.versions();
    let mut folders: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&versions) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir())
                && let Ok(name) = entry.file_name().into_string()
            {
                folders.push(name);
            }
        }
    }
    folders.sort();

    // The version folders the games need.
    let needed: Vec<&String> = folders
        .iter()
        .filter(|name| {
            instances
                .iter()
                .any(|record| **name == record.game_version || is_profile_of(name, record))
        })
        .collect();

    // What those versions need in turn.
    let mut library_paths = BTreeSet::new();
    let mut asset_indexes = BTreeSet::new();
    let mut log_configs = BTreeSet::new();
    for name in &needed {
        let json = versions.join(name.as_str()).join(format!("{name}.json"));
        let text =
            fs::read_to_string(&json).map_err(|_| ReclaimError::Unreadable((*name).clone()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|_| ReclaimError::Unreadable((*name).clone()))?;
        if let Some(libraries) = value.get("libraries").and_then(Value::as_array) {
            for library in libraries {
                if let Some(path) = library
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(coordinate_path)
                {
                    library_paths.insert(path);
                }
                collect_paths(library, false, &mut library_paths);
            }
        }
        if let Some(id) = value.pointer("/assetIndex/id").and_then(Value::as_str) {
            asset_indexes.insert(id.to_owned());
        } else if let Some(id) = value.get("assets").and_then(Value::as_str) {
            asset_indexes.insert(id.to_owned());
        }
        if let Some(id) = value
            .pointer("/logging/client/file/id")
            .and_then(Value::as_str)
        {
            log_configs.insert(id.to_owned());
        }
    }

    // Version folders and natives nobody needs.
    for name in &folders {
        if !needed.contains(&name) {
            let path = versions.join(name);
            report.unused.push((path.clone(), size_of(&path)));
        }
    }
    let natives = layout.meta().join("natives");
    if let Ok(entries) = fs::read_dir(&natives) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !needed.iter().any(|needed| **needed == name) {
                let path = entry.path();
                report.unused.push((path.clone(), size_of(&path)));
            }
        }
    }

    // Libraries: installer-made loaders (Forge, NeoForge) leave files no
    // metadata lists, so with one in the library nothing here is judged.
    let installer_games = instances
        .iter()
        .any(|record| matches!(record.loader, Loader::Forge | Loader::NeoForge));
    if installer_games {
        report.kept.push(KeptReason::InstallerLibraries);
    } else {
        let libraries = layout.libraries();
        let mut files = Vec::new();
        files_below(&libraries, &mut files);
        for file in files {
            let relative = file
                .strip_prefix(&libraries)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if !library_paths.contains(&relative) {
                let size = size_of(&file);
                report.unused.push((file, size));
            }
        }
    }

    // Assets: objects by hash from each needed index. If an index cannot be
    // read, the objects are left alone.
    let assets = layout.assets();
    let mut object_hashes = BTreeSet::new();
    let mut indexes_readable = true;
    for id in &asset_indexes {
        let text = fs::read_to_string(assets.join("indexes").join(format!("{id}.json")));
        match text
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        {
            Some(index) => {
                if let Some(objects) = index.get("objects").and_then(Value::as_object) {
                    for object in objects.values() {
                        if let Some(hash) = object.get("hash").and_then(Value::as_str) {
                            object_hashes.insert(hash.to_owned());
                        }
                    }
                }
            }
            None => indexes_readable = false,
        }
    }
    if indexes_readable {
        let mut files = Vec::new();
        files_below(&assets.join("objects"), &mut files);
        for file in files {
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !object_hashes.contains(&name) {
                let size = size_of(&file);
                report.unused.push((file, size));
            }
        }
        if let Ok(entries) = fs::read_dir(assets.join("indexes")) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let id = name.strip_suffix(".json").unwrap_or(&name);
                if !asset_indexes.contains(id) {
                    let path = entry.path();
                    report.unused.push((path.clone(), size_of(&path)));
                }
            }
        }
    } else {
        report.kept.push(KeptReason::UnreadableAssetIndex);
    }
    if let Ok(entries) = fs::read_dir(assets.join("log_configs")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !log_configs.contains(&name) {
                let path = entry.path();
                report.unused.push((path.clone(), size_of(&path)));
            }
        }
    }
    report.unused.sort();
    Ok(report)
}

/// Removes what a scan offered. Every path is checked to be a real file or
/// folder below `meta/` (never a link, never outside) before it is touched,
/// and folders left empty are tidied away. Returns the bytes freed.
pub fn remove(layout: &Layout, found: &Reclaimable) -> Result<u64, ReclaimError> {
    let meta = layout.meta();
    let real_meta = fs::canonicalize(&meta).unwrap_or_else(|_| meta.clone());
    let mut freed = 0;
    for (path, size) in &found.unused {
        // Lexically below meta/, no `..`, and really below it once the folders
        // on the way are resolved: a link in the middle cannot lead out.
        if !path.starts_with(&meta)
            || path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
            || !path
                .parent()
                .and_then(|parent| fs::canonicalize(parent).ok())
                .is_some_and(|parent| parent.starts_with(&real_meta))
        {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
        freed += size;
        // Tidy folders this left empty, up to (not including) `meta/`.
        let mut parent = path.parent();
        while let Some(dir) = parent {
            if dir == meta || !dir.starts_with(&meta) || fs::remove_dir(dir).is_err() {
                break;
            }
            parent = dir.parent();
        }
    }
    Ok(freed)
}

#[cfg(test)]
mod tests;
