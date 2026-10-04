//! Importing Modrinth modpacks (`.mrpack`).
//!
//! The format is the public Modrinth pack format: a zip with
//! `modrinth.index.json`, plus `overrides/` and `client-overrides/` folders
//! copied into the game directory.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use serde::Deserialize;

use crate::activity::CancellationToken;
use crate::instance::{
    InstanceRecord, InstanceSettings, InstanceStore, Loader, NewInstance, StoreError,
};
use crate::layout::Layout;
use crate::staged::Staged;
use crate::transfer::{SourceChain, TransferEngine, TransferError, TransferRequest, Transport};

const INDEX: &str = "modrinth.index.json";
const INDEX_LIMIT: u64 = 8 * 1024 * 1024;
/// Largest total size of override files extracted.
pub const OVERRIDES_LIMIT: u64 = 16 * 1024 * 1024 * 1024;
const OVERRIDE_FOLDERS: [&str; 2] = ["overrides/", "client-overrides/"];

/// Hosts a pack may download from; the pack format restricts sources so a
/// pack cannot point the launcher at arbitrary servers.
const TRUSTED_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

/// Whether a pack file may be downloaded from `url`: https and a trusted host.
#[must_use]
pub fn is_trusted_source(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| {
        parsed.scheme() == "https"
            && parsed
                .host_str()
                .is_some_and(|host| TRUSTED_HOSTS.contains(&host))
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientSupport {
    Required,
    Optional,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackFile {
    /// Relative to the game directory, `/`-separated, validated.
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub downloads: Vec<String>,
    pub client: ClientSupport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackIndex {
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    pub minecraft: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub files: Vec<PackFile>,
}

#[derive(Debug)]
pub enum ModpackError {
    Io(io::Error),
    Zip(String),
    /// Not a Modrinth pack, or its index is unusable; carries the reason.
    Invalid(String),
    UnsafePath(String),
    /// A file has no download from a trusted host.
    UntrustedSource(String),
    Store(StoreError),
    Transfer(TransferError),
    Cancelled,
}

impl Display for ModpackError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "modpack storage failed: {error}"),
            Self::Zip(message) => write!(f, "modpack archive error: {message}"),
            Self::Invalid(reason) => write!(f, "invalid modpack: {reason}"),
            Self::UnsafePath(path) => write!(f, "modpack contains an unsafe path {path:?}"),
            Self::UntrustedSource(path) => {
                write!(f, "no trusted download source for {path:?}")
            }
            Self::Store(error) => write!(f, "{error}"),
            Self::Transfer(error) => write!(f, "{error}"),
            Self::Cancelled => f.write_str("import cancelled"),
        }
    }
}

impl Error for ModpackError {}

impl From<io::Error> for ModpackError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<zip::result::ZipError> for ModpackError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error.to_string())
    }
}
impl From<StoreError> for ModpackError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

#[derive(Deserialize)]
struct RawIndex {
    #[serde(rename = "formatVersion")]
    format_version: Option<u32>,
    game: Option<String>,
    #[serde(rename = "versionId", default)]
    version_id: String,
    name: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RawFile {
    path: Option<String>,
    #[serde(default)]
    hashes: BTreeMap<String, String>,
    env: Option<RawEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    file_size: u64,
}

#[derive(Deserialize)]
struct RawEnv {
    client: Option<String>,
}

/// A path a pack may write to: relative, `/`-separated plain names.
pub(crate) fn is_safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Parses `modrinth.index.json`.
///
/// Requires format version 1, game `minecraft`, a `minecraft` dependency, at
/// most one loader dependency, and a safe relative path plus a SHA-1 for every
/// file.
pub fn parse_index(json: &str) -> Result<PackIndex, ModpackError> {
    let raw: RawIndex =
        serde_json::from_str(json).map_err(|error| ModpackError::Invalid(error.to_string()))?;
    let invalid = |reason: &str| ModpackError::Invalid(reason.to_owned());
    if raw.format_version != Some(1) {
        return Err(invalid("unsupported format version"));
    }
    if raw.game.as_deref() != Some("minecraft") {
        return Err(invalid("not a Minecraft pack"));
    }
    let minecraft = raw
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| invalid("no minecraft version"))?;
    let mut loaders = raw.dependencies.iter().filter_map(|(key, version)| {
        let loader = match key.as_str() {
            "fabric-loader" => Loader::Fabric,
            "quilt-loader" => Loader::Quilt,
            "forge" => Loader::Forge,
            "neoforge" => Loader::NeoForge,
            _ => return None,
        };
        Some((loader, version.clone()))
    });
    let (loader, loader_version) = match (loaders.next(), loaders.next()) {
        (None, _) => (Loader::Vanilla, None),
        (Some((loader, version)), None) => (loader, Some(version)),
        (Some(_), Some(_)) => return Err(invalid("more than one mod loader")),
    };

    let mut files = Vec::new();
    for file in raw.files {
        let path = file.path.ok_or_else(|| invalid("a file has no path"))?;
        if !is_safe_relative(&path) {
            return Err(ModpackError::UnsafePath(path));
        }
        let sha1 = file
            .hashes
            .get("sha1")
            .cloned()
            .ok_or_else(|| invalid("a file has no sha1"))?;
        let client = match file.env.and_then(|env| env.client).as_deref() {
            Some("unsupported") => ClientSupport::Unsupported,
            Some("optional") => ClientSupport::Optional,
            _ => ClientSupport::Required,
        };
        files.push(PackFile {
            path,
            sha1,
            size: file.file_size,
            downloads: file.downloads,
            client,
        });
    }
    Ok(PackIndex {
        name: raw
            .name
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| invalid("the pack has no name"))?,
        version: raw.version_id,
        summary: raw.summary,
        minecraft,
        loader,
        loader_version,
        files,
    })
}

/// Reads the index out of an `.mrpack` archive.
pub fn read_index(pack: &Path) -> Result<PackIndex, ModpackError> {
    let mut archive = zip::ZipArchive::new(fs::File::open(pack)?)?;
    let entry = archive
        .by_name(INDEX)
        .map_err(|_| ModpackError::Invalid("no modrinth.index.json".to_owned()))?;
    let mut text = String::new();
    entry
        .take(INDEX_LIMIT)
        .read_to_string(&mut text)
        .map_err(|_| ModpackError::Invalid("index is unreadable".to_owned()))?;
    parse_index(&text)
}

/// Downloads to perform for a pack: every file the client needs (optional files
/// are included), verified by SHA-1 and size, from the mirror-expanded sources
/// that `allow` accepts. Files marked unsupported on the client are skipped.
pub fn plan(
    index: &PackIndex,
    game_dir: &Path,
    chain: &SourceChain,
    allow: fn(&str) -> bool,
) -> Result<Vec<TransferRequest>, ModpackError> {
    let mut requests = Vec::new();
    for file in &index.files {
        if file.client == ClientSupport::Unsupported {
            continue;
        }
        let trusted: Vec<&String> = file.downloads.iter().filter(|url| allow(url)).collect();
        if trusted.is_empty() {
            return Err(ModpackError::UntrustedSource(file.path.clone()));
        }
        let sources: Vec<String> = trusted
            .into_iter()
            .flat_map(|url| chain.candidates(url))
            .collect();
        let mut request = TransferRequest::new(
            format!("pack:{}", file.path),
            sources,
            game_dir.join(&file.path),
        )
        .map_err(ModpackError::Transfer)?;
        if file.size > 0 {
            request = request.expect_size(file.size);
        }
        request = request
            .expect_sha1(&file.sha1)
            .map_err(ModpackError::Transfer)?;
        requests.push(request);
    }
    Ok(requests)
}

/// Copies `overrides/` then `client-overrides/` into the game directory (the
/// latter wins). Entries that would escape it are skipped. Returns how many
/// files were written.
pub fn extract_overrides(pack: &Path, game_dir: &Path) -> Result<usize, ModpackError> {
    let mut archive = zip::ZipArchive::new(fs::File::open(pack)?)?;
    let mut total = 0_u64;
    for index in 0..archive.len() {
        total = total.saturating_add(archive.by_index(index)?.size());
    }
    if total > OVERRIDES_LIMIT {
        return Err(ModpackError::Invalid("overrides are too large".to_owned()));
    }
    let mut written = 0;
    for folder in OVERRIDE_FOLDERS {
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let Some(name) = entry.enclosed_name() else {
                continue;
            };
            let Some(relative) = name
                .to_str()
                .and_then(|text| text.strip_prefix(folder))
                .filter(|relative| is_safe_relative(relative))
            else {
                continue;
            };
            if entry.is_dir() {
                continue;
            }
            let target = game_dir.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            io::copy(&mut entry, &mut fs::File::create(target)?)?;
            written += 1;
        }
    }
    Ok(written)
}

/// Builds a pack's game folder in `game_dir`: overrides first, then every
/// download. Nothing here touches the library.
pub async fn stage_pack<T: Transport>(
    index: &PackIndex,
    pack: &Path,
    game_dir: &Path,
    engine: &TransferEngine<T>,
    chain: &SourceChain,
    allow: fn(&str) -> bool,
    cancel: CancellationToken,
) -> Result<(), ModpackError> {
    let requests = plan(index, game_dir, chain, allow)?;
    fs::create_dir_all(game_dir)?;
    extract_overrides(pack, game_dir)?;
    if cancel.is_cancelled() {
        return Err(ModpackError::Cancelled);
    }
    let report = engine.transfer_batch(requests, cancel).await;
    if report.failed() > 0 {
        let cancelled = report
            .results()
            .values()
            .any(|result| matches!(result, Err(TransferError::Cancelled)));
        return Err(if cancelled {
            ModpackError::Cancelled
        } else {
            let first = report
                .results()
                .values()
                .find_map(|result| result.as_ref().err().cloned())
                .expect("a failed batch has an error");
            ModpackError::Transfer(first)
        });
    }
    Ok(())
}

/// Imports a pack as a new instance.
///
/// The game folder is built and verified in a staging folder first; only then
/// is the record created and the folder moved into place. The library is locked
/// just for those two short steps, never while downloading. A failure or cancel
/// at any point removes the staging folder and leaves the library and every
/// existing instance exactly as they were.
pub async fn import<T: Transport>(
    store: &tokio::sync::Mutex<InstanceStore>,
    engine: &TransferEngine<T>,
    chain: &SourceChain,
    pack: &Path,
    now: u64,
    allow: fn(&str) -> bool,
    cancel: CancellationToken,
) -> Result<InstanceRecord, ModpackError> {
    if cancel.is_cancelled() {
        return Err(ModpackError::Cancelled);
    }
    let index = read_index(pack)?;
    let (id, layout) = {
        let store = store.lock().await;
        (store.suggest_id(&index.name)?, Layout::new(store.root()))
    };
    let staged = Staged::begin(&layout, "import", &id)?;
    let built = stage_pack(
        &index,
        pack,
        &staged.game_dir(),
        engine,
        chain,
        allow,
        cancel,
    )
    .await;
    if let Err(error) = built {
        staged.discard();
        return Err(error);
    }
    if let Err(error) = staged.mark_ready() {
        staged.discard();
        return Err(error.into());
    }
    let created = store
        .lock()
        .await
        .create_as(
            &id,
            NewInstance {
                name: index.name.clone(),
                game_version: index.minecraft.clone(),
                loader: index.loader,
                loader_version: index.loader_version.clone(),
            },
            InstanceSettings::default(),
            false,
            now,
        )
        .cloned();
    let record = match created {
        Ok(record) => record,
        Err(error) => {
            staged.discard();
            return Err(error.into());
        }
    };
    if let Err(error) = staged.publish(&layout) {
        let _ = store.lock().await.remove(&record.id);
        staged.discard();
        return Err(error.into());
    }
    Ok(record)
}

#[cfg(test)]
mod tests;
