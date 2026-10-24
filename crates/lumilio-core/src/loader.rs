//! Mod loaders: which exist, how to find their versions, and their profiles.
//!
//! Fabric and Quilt publish a "profile" per (game version, loader version): a
//! release manifest that inherits from the vanilla one and adds libraries and a
//! main class.

use serde::Deserialize;
use url::Url;

use crate::fetch::{FetchError, fetch_decoded};
use crate::instance::Loader;
use crate::transfer::{TransferEngine, Transport};

/// Loaders the launcher can install and start.
pub const LAUNCHABLE_LOADERS: [Loader; 5] = [
    Loader::Vanilla,
    Loader::Fabric,
    Loader::Quilt,
    Loader::Forge,
    Loader::NeoForge,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoaderVersion {
    pub version: String,
    /// Quilt does not say; versions with a pre-release tag are unstable.
    pub stable: bool,
}

#[derive(Debug)]
pub enum LoaderError {
    /// Forge and NeoForge are not installable yet.
    Unsupported(Loader),
    Decode(String),
    Fetch(FetchError),
}

impl std::fmt::Display for LoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(loader) => write!(f, "{loader:?} cannot be installed yet"),
            Self::Decode(message) => write!(f, "unexpected loader answer: {message}"),
            Self::Fetch(error) => write!(f, "loader metadata unavailable: {error}"),
        }
    }
}

impl std::error::Error for LoaderError {}

impl From<FetchError> for LoaderError {
    fn from(error: FetchError) -> Self {
        Self::Fetch(error)
    }
}

const FABRIC_META: &str = "https://meta.fabricmc.net/v2/versions/loader";
const QUILT_META: &str = "https://meta.quiltmc.org/v3/versions/loader";

fn base(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Fabric => Some(FABRIC_META),
        Loader::Quilt => Some(QUILT_META),
        _ => None,
    }
}

fn join(base: &str, segments: &[&str]) -> String {
    let mut url = Url::parse(base).expect("constant address is valid");
    url.path_segments_mut()
        .expect("http addresses can have segments")
        .extend(segments);
    url.into()
}

/// Address listing the loader versions available for a game version.
#[must_use]
pub fn versions_url(loader: Loader, game_version: &str) -> Option<String> {
    Some(join(base(loader)?, &[game_version]))
}

/// Address of the profile for a (game version, loader version) pair.
#[must_use]
pub fn profile_url(loader: Loader, game_version: &str, loader_version: &str) -> Option<String> {
    Some(join(
        base(loader)?,
        &[game_version, loader_version, "profile", "json"],
    ))
}

#[derive(Deserialize)]
struct RawEntry {
    loader: Option<RawLoader>,
}

#[derive(Deserialize)]
struct RawLoader {
    version: Option<String>,
    stable: Option<bool>,
}

/// Decodes a loader version list, newest first as published. Entries without a
/// version are dropped.
pub fn decode_versions(bytes: &[u8]) -> Result<Vec<LoaderVersion>, LoaderError> {
    let entries: Vec<RawEntry> =
        serde_json::from_slice(bytes).map_err(|error| LoaderError::Decode(error.to_string()))?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let loader = entry.loader?;
            let version = loader.version.filter(|v| !v.is_empty())?;
            let stable = loader
                .stable
                .unwrap_or_else(|| !version.contains(['-', '+']) || version.contains("+build"));
            Some(LoaderVersion { version, stable })
        })
        .collect())
}

/// The loader versions available for `game_version`.
pub async fn fetch_versions<T: Transport>(
    engine: &TransferEngine<T>,
    sources: &[String],
) -> Result<Vec<LoaderVersion>, LoaderError> {
    Ok(fetch_decoded(engine, sources, |bytes| decode_versions(&bytes)).await?)
}

/// The newest stable loader version, else the newest of any kind.
#[must_use]
pub fn recommended(versions: &[LoaderVersion]) -> Option<&LoaderVersion> {
    versions
        .iter()
        .find(|version| version.stable)
        .or_else(|| versions.first())
}

/// Rewrites a downloaded profile so it has the id LumilioCL expects and
/// inherits from `game_version`, whatever the publisher called them. The rest
/// is untouched.
pub fn normalize_profile(
    json: &[u8],
    release_id: &str,
    game_version: &str,
) -> Result<String, LoaderError> {
    let mut value: serde_json::Value =
        serde_json::from_slice(json).map_err(|error| LoaderError::Decode(error.to_string()))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| LoaderError::Decode("profile is not an object".to_owned()))?;
    object.insert("id".to_owned(), release_id.into());
    object.insert("inheritsFrom".to_owned(), game_version.into());
    Ok(value.to_string())
}

#[cfg(test)]
mod tests;
