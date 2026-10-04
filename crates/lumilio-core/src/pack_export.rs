//! Exporting an instance as a Modrinth modpack (`.mrpack`).
//!
//! The pack is a zip with `modrinth.index.json`, the files Modrinth can serve
//! listed there with their hashes and addresses, and every other chosen file
//! under `overrides/`. The planning is plain functions over file names and
//! hashes so it can be tested without a network; the service does the looking
//! up. The format is the public Modrinth pack format that `modpack` imports.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use serde_json::json;
use sha2::{Digest, Sha512};

use crate::discover::Version;
use crate::instance::Loader;
use crate::modpack::{is_safe_relative, is_trusted_source};

const INDEX: &str = "modrinth.index.json";
/// The folders whose files Modrinth may know, and so may be linked.
const LINKABLE_FOLDERS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];
/// The game's lock file; it means nothing outside a running game.
const LOCK_FILE: &str = "session.lock";

/// Which pack format to write.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PackFormat {
    /// Modrinth `.mrpack`: files Modrinth knows are listed by address.
    #[default]
    Modrinth,
    /// A MultiMC / Prism instance in a zip: every chosen file is carried.
    Prism,
}

impl PackFormat {
    /// The extension a saved pack of this format has.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Modrinth => "mrpack",
            Self::Prism => "zip",
        }
    }
}

/// What the person chose for the pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportSpec {
    pub format: PackFormat,
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    /// Files and folders of the game directory to include, `/`-separated.
    pub include: Vec<String>,
}

/// A file the pack lists by address instead of carrying.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedFile {
    pub path: String,
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
    pub url: String,
}

/// What an export came to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportReport {
    /// Files listed by address.
    pub linked: usize,
    /// Files carried inside the pack.
    pub bundled: usize,
    /// Modrinth could not be asked, so everything was carried.
    pub lookup_failed: bool,
    /// The pack's size in bytes.
    pub size: u64,
}

#[derive(Debug)]
pub enum ExportError {
    Io(io::Error),
    Zip(String),
    /// The pack needs a name and a version.
    MissingDetails,
    /// A chosen path leaves the game directory.
    UnsafePath(String),
    /// A pack with a mod loader must say which version of it.
    NoLoaderVersion,
    NothingChosen,
    Cancelled,
}

impl Display for ExportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "modpack export failed: {error}"),
            Self::Zip(message) => write!(f, "modpack archive error: {message}"),
            Self::MissingDetails => f.write_str("the pack needs a name and a version"),
            Self::UnsafePath(path) => write!(f, "{path:?} is not inside the game directory"),
            Self::NoLoaderVersion => {
                f.write_str("the game has no loader version to put in the pack")
            }
            Self::NothingChosen => f.write_str("nothing is chosen for the pack"),
            Self::Cancelled => f.write_str("the export was cancelled"),
        }
    }
}

impl Error for ExportError {}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for ExportError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error.to_string())
    }
}

/// Every file under the chosen paths, relative to `game_dir`, sorted and
/// without repeats. Links are skipped rather than followed.
pub fn collect_files(game_dir: &Path, include: &[String]) -> Result<Vec<String>, ExportError> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let path = format!("{prefix}/{name}");
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(&entry.path(), &path, out)?;
            } else if kind.is_file() && name != LOCK_FILE {
                out.insert(path);
            }
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    for chosen in include {
        let raw = chosen.as_str();
        let chosen = raw.trim_end_matches('/');
        if raw.starts_with('/') || !is_safe_relative(chosen) {
            return Err(ExportError::UnsafePath(chosen.to_owned()));
        }
        let path = game_dir.join(chosen);
        let Ok(metadata) = path.symlink_metadata() else {
            continue;
        };
        if metadata.is_dir() {
            walk(&path, chosen, &mut files)?;
        } else if metadata.is_file() && !chosen.ends_with(LOCK_FILE) {
            files.insert(chosen.to_owned());
        }
    }
    if files.is_empty() {
        return Err(ExportError::NothingChosen);
    }
    Ok(files.into_iter().collect())
}

/// The files Modrinth might know: archives directly inside `mods/`,
/// `resourcepacks/` or `shaderpacks/` that are switched on.
#[must_use]
pub fn linkable(files: &[String]) -> Vec<&String> {
    files
        .iter()
        .filter(|path| {
            let mut parts = path.split('/');
            let (Some(folder), Some(name), None) = (parts.next(), parts.next(), parts.next())
            else {
                return false;
            };
            LINKABLE_FOLDERS.contains(&folder) && (name.ends_with(".jar") || name.ends_with(".zip"))
        })
        .collect()
}

/// Matches files Modrinth identified (by SHA-1) with the address to fetch
/// them from. A file whose address is not a trusted source, or that differs
/// from what Modrinth lists, stays out and is carried instead.
#[must_use]
pub fn link_identified(
    hashed: &[(String, String, String, u64)],
    identified: &BTreeMap<String, Version>,
) -> Vec<LinkedFile> {
    hashed
        .iter()
        .filter_map(|(path, sha1, sha512, size)| {
            let version = identified.get(sha1)?;
            let file = version
                .files
                .iter()
                .find(|file| file.sha1.as_deref() == Some(sha1.as_str()))?;
            is_trusted_source(&file.url).then(|| LinkedFile {
                path: path.clone(),
                sha1: sha1.clone(),
                sha512: sha512.clone(),
                size: *size,
                url: file.url.clone(),
            })
        })
        .collect()
}

pub fn sha512_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha512::new();
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

/// `modrinth.index.json` for a pack of this game.
pub fn index_json(
    spec: &ExportSpec,
    minecraft: &str,
    loader: Loader,
    loader_version: Option<&str>,
    linked: &[LinkedFile],
) -> Result<String, ExportError> {
    if spec.name.trim().is_empty() || spec.version.trim().is_empty() {
        return Err(ExportError::MissingDetails);
    }
    let mut dependencies = BTreeMap::new();
    dependencies.insert("minecraft".to_owned(), minecraft.to_owned());
    let key = match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric-loader"),
        Loader::Quilt => Some("quilt-loader"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
    };
    if let Some(key) = key {
        let version = loader_version
            .filter(|version| !version.is_empty())
            .ok_or(ExportError::NoLoaderVersion)?;
        dependencies.insert(key.to_owned(), version.to_owned());
    }
    let files: Vec<_> = linked
        .iter()
        .map(|file| {
            json!({
                "path": file.path,
                "hashes": { "sha1": file.sha1, "sha512": file.sha512 },
                "env": { "client": "required", "server": "required" },
                "downloads": [file.url],
                "fileSize": file.size,
            })
        })
        .collect();
    let mut index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": spec.version.trim(),
        "name": spec.name.trim(),
        "files": files,
        "dependencies": dependencies,
    });
    if let Some(summary) = spec
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        index["summary"] = json!(summary);
    }
    serde_json::to_string_pretty(&index).map_err(|error| ExportError::Zip(error.to_string()))
}

/// The `mmc-pack.json` of a MultiMC / Prism instance.
pub fn prism_pack_json(
    minecraft: &str,
    loader: Loader,
    loader_version: Option<&str>,
) -> Result<String, ExportError> {
    let mut components =
        vec![json!({"uid": "net.minecraft", "version": minecraft, "important": true})];
    let uid = match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("net.fabricmc.fabric-loader"),
        Loader::Quilt => Some("org.quiltmc.quilt-loader"),
        Loader::Forge => Some("net.minecraftforge"),
        Loader::NeoForge => Some("net.neoforged"),
    };
    if let Some(uid) = uid {
        let version = loader_version
            .filter(|version| !version.is_empty())
            .ok_or(ExportError::NoLoaderVersion)?;
        components.push(json!({"uid": uid, "version": version}));
    }
    serde_json::to_string_pretty(&json!({"components": components, "formatVersion": 1}))
        .map_err(|error| ExportError::Zip(error.to_string()))
}

/// Writes a MultiMC / Prism instance zip: `instance.cfg`, `mmc-pack.json` and
/// every chosen file under `.minecraft/`, through a `.part` file.
pub fn write_prism(
    destination: &Path,
    game_dir: &Path,
    name: &str,
    pack_json: &str,
    files: &[String],
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, ExportError> {
    let part = destination.with_extension("zip.part");
    let written = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&part)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        writer.start_file("instance.cfg", options)?;
        io::Write::write_all(
            &mut writer,
            format!(
                "InstanceType=OneSix\nname={}\n",
                name.replace(['\n', '\r'], " ")
            )
            .as_bytes(),
        )?;
        writer.start_file("mmc-pack.json", options)?;
        io::Write::write_all(&mut writer, pack_json.as_bytes())?;
        for path in files {
            if cancelled() {
                return Err(ExportError::Cancelled);
            }
            writer.start_file(format!(".minecraft/{path}"), options)?;
            io::copy(&mut fs::File::open(game_dir.join(path))?, &mut writer)?;
        }
        writer.finish()?;
        fs::rename(&part, destination)?;
        Ok::<_, ExportError>(fs::metadata(destination)?.len())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&part);
    }
    written
}

/// Writes the pack: the index, then each carried file under `overrides/`,
/// through a `.part` file renamed at the end. `cancelled` is asked between
/// files; a cancelled export leaves nothing. Returns the pack's size.
pub fn write_pack(
    destination: &Path,
    game_dir: &Path,
    index: &str,
    carried: &[String],
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, ExportError> {
    let part = destination.with_extension("mrpack.part");
    let written = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&part)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        writer.start_file(INDEX, options)?;
        io::Write::write_all(&mut writer, index.as_bytes())?;
        for path in carried {
            if cancelled() {
                return Err(ExportError::Cancelled);
            }
            writer.start_file(format!("overrides/{path}"), options)?;
            io::copy(&mut fs::File::open(game_dir.join(path))?, &mut writer)?;
        }
        writer.finish()?;
        fs::rename(&part, destination)?;
        Ok::<_, ExportError>(fs::metadata(destination)?.len())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&part);
    }
    written
}

#[cfg(test)]
mod tests;
