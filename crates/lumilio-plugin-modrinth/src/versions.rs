use crate::protocol::decode;
use lumilio_plugin_api::PluginError;
use lumilio_plugin_api::content::{
    Dependency, DependencyKind, ReleaseChannel, Version, VersionFile,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct RawVersion {
    pub(super) id: Option<String>,
    pub(super) project_id: Option<String>,
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) version_number: String,
    #[serde(default)]
    pub(super) version_type: String,
    #[serde(default)]
    pub(super) game_versions: Vec<String>,
    #[serde(default)]
    pub(super) loaders: Vec<String>,
    #[serde(default)]
    pub(super) date_published: String,
    #[serde(default)]
    pub(super) downloads: u64,
    #[serde(default)]
    pub(super) files: Vec<RawFile>,
    #[serde(default)]
    pub(super) dependencies: Vec<RawDependency>,
    pub(super) changelog: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawFile {
    pub(super) url: Option<String>,
    pub(super) filename: Option<String>,
    #[serde(default)]
    pub(super) primary: bool,
    #[serde(default)]
    pub(super) size: u64,
    #[serde(default)]
    pub(super) hashes: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawDependency {
    pub(super) project_id: Option<String>,
    pub(super) version_id: Option<String>,
    #[serde(default)]
    pub(super) dependency_type: String,
}

/// Decodes a version list. Versions without files or an id are dropped:
/// there is nothing to install from them.
pub fn decode_versions(bytes: &[u8]) -> Result<Vec<Version>, PluginError> {
    let raw: Vec<RawVersion> = decode(bytes)?;
    Ok(raw.into_iter().filter_map(version_from_raw).collect())
}

/// Decodes an answer shaped `{ "<sha1>": <version>, … }`.
pub(super) fn decode_version_map(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, Version>, PluginError> {
    let raw: std::collections::BTreeMap<String, RawVersion> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|(hash, version)| Some((hash, version_from_raw(version)?)))
        .collect())
}

pub(super) fn version_from_raw(raw: RawVersion) -> Option<Version> {
    let id = raw.id.filter(|id| !id.is_empty())?;
    let files: Vec<VersionFile> = raw
        .files
        .into_iter()
        .filter_map(|file| {
            Some(VersionFile {
                url: file.url.filter(|url| !url.is_empty())?,
                filename: file.filename.filter(|name| !name.is_empty())?,
                primary: file.primary,
                size: file.size,
                sha1: file.hashes.get("sha1").cloned(),
            })
        })
        .collect();
    if files.is_empty() {
        return None;
    }
    let channel = match raw.version_type.as_str() {
        "beta" => ReleaseChannel::Beta,
        "alpha" => ReleaseChannel::Alpha,
        _ => ReleaseChannel::Release,
    };
    Some(Version {
        project_id: raw.project_id.unwrap_or_default(),
        id,
        name: raw.name,
        number: raw.version_number,
        channel,
        game_versions: raw.game_versions,
        loaders: raw.loaders,
        published: raw.date_published,
        downloads: raw.downloads,
        changelog: raw.changelog.unwrap_or_default(),
        files,
        dependencies: raw
            .dependencies
            .into_iter()
            .map(|dependency| Dependency {
                kind: match dependency.dependency_type.as_str() {
                    "required" => DependencyKind::Required,
                    "optional" => DependencyKind::Optional,
                    "embedded" => DependencyKind::Embedded,
                    "incompatible" => DependencyKind::Incompatible,
                    _ => DependencyKind::Other,
                },
                project_id: dependency.project_id,
                version_id: dependency.version_id,
            })
            .collect(),
    })
}
