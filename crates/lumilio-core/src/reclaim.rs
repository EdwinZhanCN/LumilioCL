//! Finding, and removing, shared game files that no game uses (`ADR 0017`).
//!
//! Shared files live under `meta/`: version folders (a release's or loader's
//! json and jar), natives extracted per version, libraries, and assets. A
//! file is *unused* when no game's version needs it. The scan is read-only and
//! conservative: when anything cannot be classified, nothing it touches is
//! offered. Behavior notes: `docs/behavior/reclaim.md`.

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

/// What the scan found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Reclaimable {
    /// Files (or version/natives folders, counted as one) nobody uses, with
    /// their sizes. Everything is below `meta/`.
    pub unused: Vec<(PathBuf, u64)>,
    /// Parts that were left alone on purpose, and why.
    pub kept: Vec<String>,
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
        report.kept.push(
            "有 Forge / NeoForge 游戏：它们安装时生成的库文件没有清单，库文件夹先不清理".to_owned(),
        );
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
        report
            .kept
            .push("有游戏的资源索引读不到，资源文件先不清理".to_owned());
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
mod tests {
    use super::*;
    use crate::instance::InstanceSettings;

    fn record(
        id: &str,
        game: &str,
        loader: Loader,
        loader_version: Option<&str>,
    ) -> InstanceRecord {
        InstanceRecord {
            id: id.into(),
            name: id.into(),
            game_version: game.into(),
            loader,
            loader_version: loader_version.map(str::to_owned),
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: true,
            settings: InstanceSettings::default(),
        }
    }

    fn put(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    /// A small shared tree: 1.21.1 and 1.20.1 (vanilla), plus a Fabric profile
    /// for 1.21.1, each with libraries, assets and natives.
    fn tree() -> (tempfile::TempDir, Layout) {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let meta = layout.meta();
        let version = |id: &str, library: &str, assets: &str| {
            format!(
                r#"{{"id":"{id}","assets":"{assets}","assetIndex":{{"id":"{assets}"}},
                    "logging":{{"client":{{"file":{{"id":"log-{assets}.xml"}}}}}},
                    "libraries":[{{"name":"x:{library}:1","downloads":{{"artifact":{{"path":"x/{library}/1/{library}-1.jar"}}}}}},
                                 {{"name":"net.fabricmc:fabric-loader:0.16.0"}}]}}"#
            )
        };
        put(
            &meta.join("versions/1.21.1/1.21.1.json"),
            &version("1.21.1", "lwjgl", "17"),
        );
        put(&meta.join("versions/1.21.1/1.21.1.jar"), "client");
        put(
            &meta.join("versions/fabric-loader-0.16.0-1.21.1/fabric-loader-0.16.0-1.21.1.json"),
            &version("fabric-loader-0.16.0-1.21.1", "lwjgl", "17"),
        );
        put(
            &meta.join("versions/1.20.1/1.20.1.json"),
            &version("1.20.1", "old", "12"),
        );
        put(&meta.join("natives/1.21.1/n.dylib"), "n");
        put(&meta.join("natives/1.20.1/n.dylib"), "n");
        for path in [
            "x/lwjgl/1/lwjgl-1.jar",
            "x/old/1/old-1.jar",
            "x/stray/1/stray-1.jar",
            "net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar",
        ] {
            put(&meta.join("libraries").join(path), "lib");
        }
        put(
            &meta.join("assets/indexes/17.json"),
            r#"{"objects":{"a":{"hash":"aa11"},"b":{"hash":"bb22"}}}"#,
        );
        put(
            &meta.join("assets/indexes/12.json"),
            r#"{"objects":{"c":{"hash":"cc33"}}}"#,
        );
        for hash in ["aa11", "bb22", "cc33", "dd44"] {
            put(
                &meta.join("assets/objects").join(&hash[..2]).join(hash),
                hash,
            );
        }
        put(&meta.join("assets/log_configs/log-17.xml"), "x");
        put(&meta.join("assets/log_configs/log-12.xml"), "x");
        (dir, layout)
    }

    fn names(report: &Reclaimable, layout: &Layout) -> Vec<String> {
        let meta = layout.meta();
        report
            .unused
            .iter()
            .map(|(path, _)| {
                path.strip_prefix(&meta)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn what_no_game_needs_is_found_and_what_one_needs_is_kept() {
        let (_dir, layout) = tree();
        let games = [record("a", "1.21.1", Loader::Fabric, Some("0.16.0"))];
        let report = scan(&layout, &games).unwrap();
        let found = names(&report, &layout);
        for gone in [
            "versions/1.20.1",
            "natives/1.20.1",
            "libraries/x/old/1/old-1.jar",
            "libraries/x/stray/1/stray-1.jar",
            "assets/objects/cc/cc33",
            "assets/objects/dd/dd44",
            "assets/indexes/12.json",
            "assets/log_configs/log-12.xml",
        ] {
            assert!(
                found.iter().any(|f| f == gone),
                "{gone} should be unused: {found:?}"
            );
        }
        for kept in [
            "versions/1.21.1",
            "versions/fabric-loader-0.16.0-1.21.1",
            "natives/1.21.1",
            "libraries/x/lwjgl/1/lwjgl-1.jar",
            "libraries/net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar",
            "assets/objects/aa/aa11",
            "assets/indexes/17.json",
            "assets/log_configs/log-17.xml",
        ] {
            assert!(!found.iter().any(|f| f == kept), "{kept} is in use");
        }
        assert!(report.bytes() > 0);
    }

    #[test]
    fn with_no_games_everything_shared_is_unused_and_removing_frees_it_and_nothing_else() {
        let (dir, layout) = tree();
        let before = crate::storage::directory_size(&layout.meta());
        put(&dir.path().join("profiles/keep/game/options.txt"), "mine");
        let report = scan(&layout, &[]).unwrap();
        let freed = remove(&layout, &report).unwrap();
        assert_eq!(freed, report.bytes());
        assert!(freed > 0 && freed <= before);
        assert_eq!(crate::storage::directory_size(&layout.meta()), 0);
        assert!(dir.path().join("profiles/keep/game/options.txt").is_file());
        // Scanning again finds nothing more.
        assert!(scan(&layout, &[]).unwrap().unused.is_empty());
    }

    #[test]
    fn a_game_whose_version_cannot_be_read_blocks_the_whole_removal() {
        let (_dir, layout) = tree();
        fs::write(layout.versions().join("1.21.1/1.21.1.json"), "garbled").unwrap();
        let games = [record("a", "1.21.1", Loader::Vanilla, None)];
        assert!(matches!(
            scan(&layout, &games),
            Err(ReclaimError::Unreadable(name)) if name == "1.21.1"
        ));
    }

    #[test]
    fn installer_made_loaders_keep_the_libraries_and_an_unreadable_asset_index_keeps_the_assets() {
        let (_dir, layout) = tree();
        let games = [record("a", "1.21.1", Loader::NeoForge, Some("21.1.50"))];
        let report = scan(&layout, &games).unwrap();
        assert!(
            names(&report, &layout)
                .iter()
                .all(|n| !n.starts_with("libraries/"))
        );
        assert!(report.kept.iter().any(|note| note.contains("Forge")));

        fs::remove_file(layout.assets().join("indexes/17.json")).unwrap();
        let vanilla = [record("a", "1.21.1", Loader::Vanilla, None)];
        let report = scan(&layout, &vanilla).unwrap();
        assert!(
            names(&report, &layout)
                .iter()
                .all(|n| !n.starts_with("assets/objects/"))
        );
        assert!(report.kept.iter().any(|note| note.contains("资源")));
    }

    #[test]
    fn removal_never_follows_a_link_or_leaves_meta() {
        let (dir, layout) = tree();
        let outside = dir.path().join("outside.txt");
        fs::write(&outside, "keep me").unwrap();
        let link = layout.libraries().join("x/stray/link.jar");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let found = Reclaimable {
            unused: vec![
                (link.clone(), 1),
                (outside.clone(), 7),
                (layout.meta().join("../outside.txt"), 7),
            ],
            kept: Vec::new(),
        };
        assert_eq!(remove(&layout, &found).unwrap(), 0);
        assert!(outside.is_file() && link.symlink_metadata().is_ok());
    }
}
