use super::kinds::{ProjectKind, TextFormat, loader_tag};
use crate::instances::Loader;

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
    /// Empty when the author lets people download the file only on the
    /// source's site (see `download_page`).
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub sha1: Option<String>,
    /// The file's page on the source's site.
    pub download_page: Option<String>,
}

impl VersionFile {
    /// Whether a launcher may download the file itself.
    #[must_use]
    pub fn downloadable(&self) -> bool {
        !self.url.is_empty()
    }
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
    /// Empty unless asked for (`versions_with_changelog`), and empty from
    /// sources that give changelogs one at a time (`ContentClient::changelog`).
    pub changelog: String,
    pub changelog_format: TextFormat,
}

/// What a file a pack names is, which decides the folder it goes into.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackFileKind {
    Mod,
    ResourcePack,
    Shader,
    DataPack,
    /// A type the launcher has no folder for (a world, configuration, …).
    Other,
}

impl PackFileKind {
    /// The game-directory folder files of this kind go into.
    #[must_use]
    pub const fn folder(self) -> Option<&'static str> {
        match self {
            Self::Mod => Some("mods"),
            Self::ResourcePack => Some("resourcepacks"),
            Self::Shader => Some("shaderpacks"),
            Self::DataPack => Some("datapacks"),
            Self::Other => None,
        }
    }
}

/// One file a pack names by project and version ID, as its source knows it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFile {
    pub project_id: String,
    pub version_id: String,
    /// The project's name; empty when the source did not say.
    pub title: String,
    pub kind: PackFileKind,
    pub file: VersionFile,
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
