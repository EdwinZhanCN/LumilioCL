use super::kinds::{ProjectKind, loader_tag};
use crate::instance::Loader;

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
