//! CurseForge modpacks: a zip with `manifest.json`, which names its files by
//! project and file ID (a CurseForge-catalog content source resolves them),
//! and an overrides folder copied over the game directory.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::{ClientSupport, ModpackError, PackFile, PackIndex, PendingDownload, is_safe_relative};
use crate::discover::{ResolvedFile, is_safe_file_name};
use crate::instances::Loader;

pub const MANIFEST: &str = "manifest.json";
const MANIFEST_LIMIT: u64 = 8 * 1024 * 1024;

/// Hosts a CurseForge pack's files may come from: CurseForge's CDN, and
/// Modrinth's for files whose authors published them there too.
const TRUSTED_HOSTS: [&str; 3] = [
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
    "cdn.modrinth.com",
];

/// Whether a CurseForge pack file may be downloaded from `url`.
#[must_use]
pub fn is_trusted_source(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| {
        parsed.scheme() == "https"
            && parsed
                .host_str()
                .is_some_and(|host| TRUSTED_HOSTS.contains(&host))
    })
}

/// A project's page on CurseForge, by ID alone.
#[must_use]
pub fn project_page(project: &str) -> String {
    format!("https://www.curseforge.com/projects/{project}")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    manifest_type: Option<String>,
    minecraft: Option<RawMinecraft>,
    name: Option<String>,
    version: Option<String>,
    author: Option<String>,
    #[serde(default)]
    files: Vec<RawFile>,
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMinecraft {
    version: Option<String>,
    #[serde(default)]
    mod_loaders: Vec<RawLoader>,
}

#[derive(Deserialize)]
struct RawLoader {
    id: Option<String>,
    #[serde(default)]
    primary: bool,
}

const fn required_by_default() -> bool {
    true
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "projectID")]
    project_id: Option<u64>,
    #[serde(rename = "fileID")]
    file_id: Option<u64>,
    #[serde(default = "required_by_default")]
    required: bool,
}

/// One file the manifest names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestFile {
    pub project: String,
    pub file: String,
    /// The pack's author had it switched off; it is installed switched off.
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub minecraft: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// The folder in the zip copied over the game directory.
    pub overrides: String,
    pub files: Vec<ManifestFile>,
}

/// A loader as the manifest writes it (`forge-47.2.0`, `neoforge-21.1.251`,
/// `fabric-0.15.11`), with a version that may repeat the game's
/// (`neoforge-1.20.1-47.1.106`).
fn loader(id: &str, minecraft: &str) -> Option<(Loader, String)> {
    let (name, version) = id.split_once('-')?;
    let loader = match name.to_ascii_lowercase().as_str() {
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        _ => return None,
    };
    let version = version
        .strip_prefix(&format!("{minecraft}-"))
        .unwrap_or(version)
        .trim();
    (!version.is_empty()).then(|| (loader, version.to_owned()))
}

/// Parses `manifest.json`: a Minecraft modpack with a game version, at most
/// one primary loader the launcher knows, a safe overrides folder and
/// numeric project and file IDs.
pub fn parse_manifest(json: &str) -> Result<Manifest, ModpackError> {
    let invalid = |reason: &str| ModpackError::Invalid(reason.to_owned());
    let raw: RawManifest =
        serde_json::from_str(json).map_err(|error| ModpackError::Invalid(error.to_string()))?;
    if raw
        .manifest_type
        .as_deref()
        .is_some_and(|kind| kind != "minecraftModpack")
    {
        return Err(invalid("not a Minecraft modpack"));
    }
    let game = raw
        .minecraft
        .ok_or_else(|| invalid("no minecraft section"))?;
    let minecraft = game
        .version
        .filter(|version| !version.trim().is_empty())
        .ok_or_else(|| invalid("no minecraft version"))?;
    let chosen = game
        .mod_loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| game.mod_loaders.first());
    let (loader, loader_version) = match chosen.and_then(|chosen| chosen.id.as_deref()) {
        None => (Loader::Vanilla, None),
        Some(id) => {
            let (loader, version) = loader(id, &minecraft)
                .ok_or_else(|| ModpackError::Invalid(format!("unknown mod loader {id:?}")))?;
            (loader, Some(version))
        }
    };
    let overrides = raw
        .overrides
        .map(|folder| folder.trim_matches('/').to_owned())
        .filter(|folder| !folder.is_empty())
        .unwrap_or_else(|| "overrides".to_owned());
    if !is_safe_relative(&overrides) {
        return Err(ModpackError::UnsafePath(overrides));
    }
    let mut files = Vec::new();
    for file in raw.files {
        let (Some(project), Some(id)) = (file.project_id, file.file_id) else {
            return Err(invalid("a file has no project or file ID"));
        };
        files.push(ManifestFile {
            project: project.to_string(),
            file: id.to_string(),
            required: file.required,
        });
    }
    Ok(Manifest {
        name: raw
            .name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| invalid("the pack has no name"))?,
        version: raw.version.unwrap_or_default(),
        author: raw.author.filter(|author| !author.trim().is_empty()),
        minecraft,
        loader,
        loader_version,
        overrides,
        files,
    })
}

/// Reads the manifest out of a CurseForge pack zip.
pub fn read_manifest(pack: &Path) -> Result<Manifest, ModpackError> {
    let mut archive = zip::ZipArchive::new(fs::File::open(pack)?)?;
    let entry = archive
        .by_name(MANIFEST)
        .map_err(|_| ModpackError::Invalid("no manifest.json".to_owned()))?;
    let mut text = String::new();
    entry
        .take(MANIFEST_LIMIT)
        .read_to_string(&mut text)
        .map_err(|_| ModpackError::Invalid("manifest is unreadable".to_owned()))?;
    parse_manifest(&text)
}

/// The pack as the launcher stages it: each resolved file in its kind's
/// folder (switched off when the author had it off), downloaded from
/// CurseForge or, for files the author lets people download only on
/// CurseForge's site, from `elsewhere` (addresses by SHA-1). What remains —
/// such files nobody else has, files of types without a folder, and files
/// CurseForge no longer knows — is listed for the person to add by hand.
pub fn index(
    manifest: &Manifest,
    resolved: &[ResolvedFile],
    elsewhere: &BTreeMap<String, String>,
) -> Result<PackIndex, ModpackError> {
    let by_reference: BTreeMap<(&str, &str), &ResolvedFile> = resolved
        .iter()
        .map(|file| ((file.project_id.as_str(), file.version_id.as_str()), file))
        .collect();
    let mut files = Vec::new();
    let mut manual = Vec::new();
    let mut seen = BTreeSet::new();
    for wanted in &manifest.files {
        let Some(found) = by_reference.get(&(wanted.project.as_str(), wanted.file.as_str())) else {
            manual.push(PendingDownload {
                title: String::new(),
                file_name: format!("{}/{}", wanted.project, wanted.file),
                path: String::new(),
                sha1: None,
                size: 0,
                page: Some(project_page(&wanted.project)),
            });
            continue;
        };
        let name = &found.file.filename;
        if !is_safe_file_name(name) {
            return Err(ModpackError::UnsafePath(name.clone()));
        }
        let sha1 = found.file.sha1.clone();
        let page = found
            .file
            .download_page
            .clone()
            .or_else(|| Some(project_page(&found.project_id)));
        let title = if found.title.is_empty() {
            name.clone()
        } else {
            found.title.clone()
        };
        let Some(folder) = found.kind.folder() else {
            manual.push(PendingDownload {
                title,
                file_name: name.clone(),
                path: String::new(),
                sha1,
                size: found.file.size,
                page,
            });
            continue;
        };
        let suffix = if wanted.required {
            ""
        } else {
            crate::content::DISABLED_SUFFIX
        };
        let path = format!("{folder}/{name}{suffix}");
        if !seen.insert(path.clone()) {
            continue;
        }
        let source = if found.file.downloadable() {
            Some(found.file.url.clone())
        } else {
            sha1.as_ref().and_then(|sha1| elsewhere.get(sha1)).cloned()
        };
        match source {
            Some(url) => files.push(PackFile {
                path,
                sha1,
                size: found.file.size,
                downloads: vec![url],
                client: ClientSupport::Required,
            }),
            None => manual.push(PendingDownload {
                title,
                file_name: name.clone(),
                path,
                sha1,
                size: found.file.size,
                page,
            }),
        }
    }
    Ok(PackIndex {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        summary: manifest.author.clone(),
        minecraft: manifest.minecraft.clone(),
        loader: manifest.loader,
        loader_version: manifest.loader_version.clone(),
        files,
        overrides: vec![format!("{}/", manifest.overrides)],
        manual,
    })
}

#[cfg(test)]
mod tests;
