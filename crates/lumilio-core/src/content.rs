//! What is installed in an instance: mods, resource packs, shaders.
//!
//! Behavior notes: `docs/behavior/content.md`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use sha1::{Digest, Sha1};

use crate::discover::{ProjectKind, is_safe_file_name};

/// Suffix that switches a file off without deleting it.
pub(crate) const DISABLED_SUFFIX: &str = ".disabled";

/// Largest metadata entry read out of a mod archive.
const METADATA_LIMIT: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentItem {
    /// The name on disk, including a `.disabled` suffix when switched off.
    pub file_name: String,
    /// The name without the `.disabled` suffix.
    pub display_name: String,
    pub enabled: bool,
    pub size: u64,
    /// Seconds since the Unix epoch; 0 when unknown.
    pub modified: u64,
    /// A folder-style pack rather than an archive.
    pub is_directory: bool,
}

#[derive(Debug)]
pub enum ContentError {
    Io(io::Error),
    /// Modpacks are not a folder of files.
    NotAFileKind,
    UnsafeFileName(String),
    NotFound(String),
    /// The other state of this file already exists.
    Conflict(String),
    /// The kind's folder is a link, so its files may live outside the instance.
    EscapesInstance(String),
    /// A file of another type than this kind holds (a `.zip` offered as a mod).
    WrongType(String),
}

impl Display for ContentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "content storage failed: {error}"),
            Self::NotAFileKind => f.write_str("modpacks are installed as instances"),
            Self::UnsafeFileName(name) => write!(f, "unsafe file name {name:?}"),
            Self::NotFound(name) => write!(f, "{name:?} is not installed"),
            Self::Conflict(name) => write!(f, "{name:?} already exists"),
            Self::EscapesInstance(name) => {
                write!(f, "{name:?} is a link; it could lead outside the instance")
            }
            Self::WrongType(name) => write!(f, "{name:?} is not this kind of content"),
        }
    }
}

impl Error for ContentError {}

impl From<io::Error> for ContentError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// The folder of one kind inside the game directory. A folder that is itself a
/// link is refused: every operation would act on whatever it points at.
pub(crate) fn folder(game_dir: &Path, kind: ProjectKind) -> Result<PathBuf, ContentError> {
    let name = kind.install_folder().ok_or(ContentError::NotAFileKind)?;
    let path = game_dir.join(name);
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(ContentError::EscapesInstance(name.to_owned()))
        }
        _ => Ok(path),
    }
}

fn base_name(file_name: &str) -> (&str, bool) {
    match file_name.strip_suffix(DISABLED_SUFFIX) {
        Some(base) => (base, false),
        None => (file_name, true),
    }
}

fn is_content_file(kind: ProjectKind, base: &str) -> bool {
    let lower = base.to_ascii_lowercase();
    match kind {
        ProjectKind::Mod => lower.ends_with(".jar"),
        _ => lower.ends_with(".zip"),
    }
}

/// Lists the installed items of one kind, sorted by name (ignoring case).
///
/// Mods are `.jar` files; resource packs and shaders are `.zip` files or
/// folders. Anything else, and hidden files, is ignored. A missing folder is
/// an empty list.
pub fn scan(game_dir: &Path, kind: ProjectKind) -> Result<Vec<ContentItem>, ContentError> {
    let folder = folder(game_dir, kind)?;
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut items = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        if file_name.starts_with('.') {
            continue;
        }
        let metadata = entry.metadata()?;
        let (base, enabled) = base_name(&file_name);
        let is_directory = metadata.is_dir();
        let wanted = if is_directory {
            kind != ProjectKind::Mod
        } else {
            metadata.is_file() && is_content_file(kind, base)
        };
        if !wanted {
            continue;
        }
        items.push(ContentItem {
            display_name: base.to_owned(),
            enabled,
            size: if is_directory { 0 } else { metadata.len() },
            modified: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |elapsed| elapsed.as_secs()),
            is_directory,
            file_name,
        });
    }
    items.sort_by_key(|item| item.display_name.to_lowercase());
    Ok(items)
}

/// Copies a local file into the kind's folder (IA P-ADD-FILES). The file must
/// be of the kind's type; a file already there with the same content is
/// simply kept, and one with other content — in either state — is never
/// replaced (`Conflict`). The copy lands under a temporary name first, so a
/// half-written file is never visible. Returns the installed file name.
pub fn import(game_dir: &Path, kind: ProjectKind, source: &Path) -> Result<String, ContentError> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ContentError::UnsafeFileName(source.display().to_string()))?
        .to_owned();
    if !is_safe_file_name(&name) {
        return Err(ContentError::UnsafeFileName(name));
    }
    if !source.is_file() || !is_content_file(kind, &name) {
        return Err(ContentError::WrongType(name));
    }
    let folder = folder(game_dir, kind)?;
    fs::create_dir_all(&folder)?;
    let target = folder.join(&name);
    let disabled = folder.join(format!("{name}{DISABLED_SUFFIX}"));
    if target.symlink_metadata().is_ok() {
        return if sha1_hex(&target)? == sha1_hex(source)? {
            Ok(name)
        } else {
            Err(ContentError::Conflict(name))
        };
    }
    if disabled.symlink_metadata().is_ok() {
        return Err(ContentError::Conflict(name));
    }
    let partial = folder.join(format!(".{name}.part"));
    fs::copy(source, &partial)?;
    if let Err(error) = fs::rename(&partial, &target) {
        let _ = fs::remove_file(&partial);
        return Err(error.into());
    }
    Ok(name)
}

fn existing(game_dir: &Path, kind: ProjectKind, file_name: &str) -> Result<PathBuf, ContentError> {
    if !is_safe_file_name(file_name) {
        return Err(ContentError::UnsafeFileName(file_name.to_owned()));
    }
    let path = folder(game_dir, kind)?.join(file_name);
    if path.symlink_metadata().is_err() {
        return Err(ContentError::NotFound(file_name.to_owned()));
    }
    Ok(path)
}

/// Switches an item on or off by adding or removing the `.disabled` suffix.
/// Switching to the state it is already in changes nothing. Returns the file
/// name after the change.
pub fn set_enabled(
    game_dir: &Path,
    kind: ProjectKind,
    file_name: &str,
    enabled: bool,
) -> Result<String, ContentError> {
    let path = existing(game_dir, kind, file_name)?;
    let (base, currently_enabled) = base_name(file_name);
    if currently_enabled == enabled {
        return Ok(file_name.to_owned());
    }
    let target_name = if enabled {
        base.to_owned()
    } else {
        format!("{file_name}{DISABLED_SUFFIX}")
    };
    let target = path.with_file_name(&target_name);
    if target.symlink_metadata().is_ok() {
        return Err(ContentError::Conflict(target_name));
    }
    fs::rename(path, target)?;
    Ok(target_name)
}

/// Deletes an item (a folder pack is removed recursively).
pub fn remove(game_dir: &Path, kind: ProjectKind, file_name: &str) -> Result<(), ContentError> {
    let path = existing(game_dir, kind, file_name)?;
    let metadata = path.symlink_metadata()?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// SHA-1 of a file, as lowercase hex — the key Modrinth identifies files by.
pub fn sha1_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// What a mod archive says about itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModMetadata {
    pub id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    /// `fabric`, `quilt`, `forge` or `neoforge`.
    pub loader: &'static str,
}

fn read_entry(archive: &mut zip::ZipArchive<fs::File>, name: &str) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    if entry.size() > METADATA_LIMIT {
        return None;
    }
    let mut text = String::new();
    entry.take(METADATA_LIMIT).read_to_string(&mut text).ok()?;
    Some(text)
}

#[derive(Deserialize)]
struct FabricManifest {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct QuiltManifest {
    quilt_loader: Option<QuiltLoader>,
}

#[derive(Deserialize)]
struct QuiltLoader {
    id: Option<String>,
    version: Option<String>,
    metadata: Option<QuiltMeta>,
}

#[derive(Deserialize)]
struct QuiltMeta {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ForgeManifest {
    #[serde(default)]
    mods: Vec<ForgeMod>,
}

#[derive(Deserialize)]
struct ForgeMod {
    #[serde(rename = "modId")]
    mod_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    version: Option<String>,
}

/// A version still holding a build-time placeholder (`${file.jarVersion}`) is
/// not a version.
fn real_version(version: Option<String>) -> Option<String> {
    version.filter(|v| !v.trim().is_empty() && !v.contains("${"))
}

/// Reads a mod's identity from its archive: `fabric.mod.json`,
/// `quilt.mod.json`, `META-INF/neoforge.mods.toml` or `META-INF/mods.toml`,
/// in that order. `None` when the archive is unreadable or declares nothing.
#[must_use]
pub fn read_mod_metadata(jar: &Path) -> Option<ModMetadata> {
    let mut archive = zip::ZipArchive::new(fs::File::open(jar).ok()?).ok()?;

    if let Some(text) = read_entry(&mut archive, "fabric.mod.json")
        && let Ok(manifest) = serde_json::from_str::<FabricManifest>(&text)
        && let Some(id) = manifest.id
    {
        return Some(ModMetadata {
            id,
            name: manifest.name,
            version: real_version(manifest.version),
            loader: "fabric",
        });
    }
    if let Some(text) = read_entry(&mut archive, "quilt.mod.json")
        && let Ok(manifest) = serde_json::from_str::<QuiltManifest>(&text)
        && let Some(loader) = manifest.quilt_loader
        && let Some(id) = loader.id
    {
        return Some(ModMetadata {
            id,
            name: loader.metadata.and_then(|meta| meta.name),
            version: real_version(loader.version),
            loader: "quilt",
        });
    }
    for (entry, loader) in [
        ("META-INF/neoforge.mods.toml", "neoforge"),
        ("META-INF/mods.toml", "forge"),
    ] {
        if let Some(text) = read_entry(&mut archive, entry)
            && let Ok(manifest) = toml::from_str::<ForgeManifest>(&text)
            && let Some(first) = manifest.mods.into_iter().next()
            && let Some(id) = first.mod_id
        {
            return Some(ModMetadata {
                id,
                name: first.display_name,
                version: real_version(first.version),
                loader,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests;
