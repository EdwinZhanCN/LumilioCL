//! Reading the asset files a bake uses out of a jar, a zip or a directory.

use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use zip::ZipArchive;

use crate::BakeError;
use crate::pack::{SourceKind, SourceStamp};

/// A jar or zip with more entries than this is refused.
const MAX_ENTRIES: usize = 200_000;
/// Largest single asset file read.
const MAX_FILE: u64 = 16 * 1024 * 1024;
/// Total bytes of asset files read from one source.
const MAX_TOTAL: u64 = 512 * 1024 * 1024;

/// Where a bake reads assets. Later sources override earlier ones file by
/// file, as the game layers resource packs.
#[derive(Clone, Debug)]
pub enum AssetSource {
    /// A client jar, a mod jar or a resource-pack zip.
    Archive(PathBuf),
    /// An archive already in memory.
    ArchiveBytes(Vec<u8>),
    /// An unpacked resource pack (the folder holding `assets/`).
    Directory(PathBuf),
}

impl AssetSource {
    /// An archive or a directory, by what is at `path`.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if path.is_dir() {
            Self::Directory(path)
        } else {
            Self::Archive(path)
        }
    }
}

/// What a bake reads from a source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Category {
    Blockstate,
    Model,
    Texture,
    TextureMeta,
    /// `data/<ns>/painting_variant/<name>.json`: a painting's size and
    /// picture, in the game data from 1.21 and in mods (plan
    /// schematic-studio T35).
    PaintingVariant,
}

/// One asset file: `namespace`, path inside its category folder without the
/// extension (`block/stone`), and its bytes.
pub(crate) struct Entry {
    pub category: Category,
    pub namespace: String,
    pub path: String,
    pub bytes: Vec<u8>,
}

pub(crate) struct ReadSource {
    pub stamp: SourceStamp,
    /// `version.json` at the root of a client jar.
    pub version_json: Option<Vec<u8>>,
    pub entries: Vec<Entry>,
}

/// `assets/<ns>/<category>/<path>.<ext>` → its category, namespace and path;
/// also `data/<ns>/painting_variant/<name>.json`.
pub(crate) fn classify(name: &str) -> Option<(Category, &str, &str)> {
    if let Some(rest) = name.strip_prefix("data/") {
        let (namespace, rest) = rest.split_once('/')?;
        let file = rest.strip_prefix("painting_variant/")?;
        if namespace.is_empty() || file.contains('/') || name.contains("..") || name.contains('\\')
        {
            return None;
        }
        return Some((
            Category::PaintingVariant,
            namespace,
            file.strip_suffix(".json")?,
        ));
    }
    let rest = name.strip_prefix("assets/")?;
    let (namespace, rest) = rest.split_once('/')?;
    let (folder, file) = rest.split_once('/')?;
    if namespace.is_empty() || file.is_empty() || name.contains("..") || name.contains('\\') {
        return None;
    }
    let (category, extension) = match folder {
        "blockstates" => (Category::Blockstate, ".json"),
        "models" => (Category::Model, ".json"),
        "textures" if file.ends_with(".png.mcmeta") => (Category::TextureMeta, ".png.mcmeta"),
        "textures" => (Category::Texture, ".png"),
        _ => return None,
    };
    Some((category, namespace, file.strip_suffix(extension)?))
}

pub(crate) fn read(source: &AssetSource) -> Result<ReadSource, BakeError> {
    match source {
        AssetSource::Archive(path) => {
            let bytes = fs::read(path).map_err(|error| BakeError::io(path, error))?;
            read_archive(&bytes).map_err(|error| error.at(path))
        }
        AssetSource::ArchiveBytes(bytes) => read_archive(bytes),
        AssetSource::Directory(root) => read_directory(root),
    }
}

fn read_archive(bytes: &[u8]) -> Result<ReadSource, BakeError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| BakeError::Archive(error.to_string()))?;
    if archive.len() > MAX_ENTRIES {
        return Err(BakeError::Archive(format!(
            "{} entries is more than any resource pack holds",
            archive.len()
        )));
    }
    let mut source = ReadSource {
        stamp: SourceStamp {
            kind: SourceKind::Archive,
            sha1: Some(hex(&Sha1::digest(bytes))),
            ..SourceStamp::default()
        },
        version_json: None,
        entries: Vec::new(),
    };
    let mut total = 0u64;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| BakeError::Archive(error.to_string()))?;
        if !file.is_file() {
            continue;
        }
        let name = file
            .name()
            .map_err(|error| BakeError::Archive(error.to_string()))?
            .into_owned();
        let wanted = name == "version.json" || classify(&name).is_some();
        if !wanted {
            continue;
        }
        let bytes = read_capped(file, &name, &mut total)?;
        if name == "version.json" {
            source.version_json = Some(bytes);
        } else if let Some((category, namespace, path)) = classify(&name) {
            source.entries.push(Entry {
                category,
                namespace: namespace.to_owned(),
                path: path.to_owned(),
                bytes,
            });
        }
    }
    Ok(source)
}

fn read_directory(root: &Path) -> Result<ReadSource, BakeError> {
    let mut source = ReadSource {
        stamp: SourceStamp {
            kind: SourceKind::Directory,
            ..SourceStamp::default()
        },
        version_json: None,
        entries: Vec::new(),
    };
    let mut total = 0u64;
    let mut stack = vec![root.join("assets")];
    while let Some(dir) = stack.pop() {
        let listing = match fs::read_dir(&dir) {
            Ok(listing) => listing,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(BakeError::io(&dir, error)),
        };
        for item in listing {
            let item = item.map_err(|error| BakeError::io(&dir, error))?;
            let path = item.path();
            let kind = item
                .file_type()
                .map_err(|error| BakeError::io(&path, error))?;
            if kind.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path
                .strip_prefix(root)
                .ok()
                .and_then(Path::to_str)
                .map(|name| name.replace(std::path::MAIN_SEPARATOR, "/"))
            else {
                continue;
            };
            let Some((category, namespace, asset)) = classify(&name) else {
                continue;
            };
            let file = File::open(&path).map_err(|error| BakeError::io(&path, error))?;
            source.entries.push(Entry {
                category,
                namespace: namespace.to_owned(),
                path: asset.to_owned(),
                bytes: read_capped(file, &name, &mut total)?,
            });
        }
    }
    // Directory listing order is the file system's; keep bakes reproducible.
    source
        .entries
        .sort_by(|a, b| (&a.namespace, &a.path).cmp(&(&b.namespace, &b.path)));
    Ok(source)
}

fn read_capped(reader: impl Read, name: &str, total: &mut u64) -> Result<Vec<u8>, BakeError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| BakeError::Archive(format!("{name}: {error}")))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(BakeError::Archive(format!(
            "{name} is larger than any asset file"
        )));
    }
    *total += bytes.len() as u64;
    if *total > MAX_TOTAL {
        return Err(BakeError::Archive(
            "the assets are larger than any resource pack".to_owned(),
        ));
    }
    Ok(bytes)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
