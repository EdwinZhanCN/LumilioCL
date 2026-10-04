use super::error::{DiscoverError, decode};
use super::kinds::{ProjectKind, loader_tag};
use crate::instance::Loader;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReleaseChannel {
    /// Alpha < Beta < Release, so `max` prefers the most stable.
    Alpha,
    Beta,
    Release,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyKind {
    Required,
    Optional,
    Embedded,
    Incompatible,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub kind: DependencyKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub sha1: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub number: String,
    pub channel: ReleaseChannel,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// RFC 3339, sorts chronologically.
    pub published: String,
    pub files: Vec<VersionFile>,
    pub dependencies: Vec<Dependency>,
    pub downloads: u64,
    /// Markdown; empty unless asked for (`versions_with_changelog`).
    pub changelog: String,
}

impl Version {
    /// The file to install: the one Modrinth flags as primary, else the first.
    #[must_use]
    pub fn install_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|file| file.primary)
            .or_else(|| self.files.first())
    }

    pub fn required_dependencies(&self) -> impl Iterator<Item = &Dependency> {
        self.dependencies
            .iter()
            .filter(|dependency| dependency.kind == DependencyKind::Required)
    }
}

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
pub fn decode_versions(bytes: &[u8]) -> Result<Vec<Version>, DiscoverError> {
    let raw: Vec<RawVersion> = decode(bytes)?;
    Ok(raw.into_iter().filter_map(version_from_raw).collect())
}

/// Decodes an answer shaped `{ "<sha1>": <version>, … }`.
pub(super) fn decode_version_map(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, Version>, DiscoverError> {
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

/// The newest version that runs on `game_version` (and, for mods, `loader`),
/// preferring stable releases over betas over alphas.
///
/// Resource packs and shaders declare loaders such as `minecraft` or `iris`,
/// which say nothing about the instance, so the loader only filters mods.
#[must_use]
pub fn pick_version<'a>(
    versions: &'a [Version],
    kind: ProjectKind,
    game_version: &str,
    loader: Loader,
) -> Option<&'a Version> {
    versions
        .iter()
        .filter(|version| fits(version, kind, game_version, loader))
        .max_by(|a, b| {
            a.channel
                .cmp(&b.channel)
                .then_with(|| a.published.cmp(&b.published))
        })
}

/// Whether `version` runs on an instance of `game_version` and `loader`
/// (the loader only counts for mods).
#[must_use]
pub fn fits(version: &Version, kind: ProjectKind, game_version: &str, loader: Loader) -> bool {
    let tag = if kind == ProjectKind::Mod {
        loader_tag(loader)
    } else {
        None
    };
    version.game_versions.iter().any(|v| v == game_version)
        && tag.is_none_or(|tag| version.loaders.iter().any(|l| l.eq_ignore_ascii_case(tag)))
}
