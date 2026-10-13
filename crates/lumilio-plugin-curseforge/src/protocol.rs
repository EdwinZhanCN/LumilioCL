//! CurseForge's wire format (`https://docs.curseforge.com/rest-api/`) and the
//! numbers it uses for games, classes, loaders and sort orders.

use lumilio_plugin_api::PluginError;
use lumilio_plugin_api::content::{
    DependencyKind, PackFileKind, ProjectKind, ReleaseChannel, Sort,
};
use serde::Deserialize;

/// Minecraft's game ID.
pub(crate) const GAME: u32 = 432;

pub(crate) const CLASS_MODS: u32 = 6;
pub(crate) const CLASS_MODPACKS: u32 = 4471;
pub(crate) const CLASS_RESOURCE_PACKS: u32 = 12;
pub(crate) const CLASS_SHADERS: u32 = 6552;
pub(crate) const CLASS_DATA_PACKS: u32 = 6945;

pub(crate) fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, PluginError> {
    // A malformed answer (a mirror's error page, a changed field) fails the
    // call, not the source.
    serde_json::from_slice(bytes)
        .map_err(|error| PluginError::Transient(format!("unexpected CurseForge answer: {error}")))
}

#[derive(Deserialize)]
pub(crate) struct Envelope<T> {
    pub(crate) data: T,
    #[serde(default)]
    pub(crate) pagination: Option<Pagination>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Pagination {
    #[serde(default)]
    pub(crate) total_count: u64,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawLinks {
    pub(crate) website_url: Option<String>,
    pub(crate) wiki_url: Option<String>,
    pub(crate) issues_url: Option<String>,
    pub(crate) source_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawCategory {
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) name: String,
    pub(crate) class_id: Option<u32>,
    pub(crate) is_class: Option<bool>,
}

#[derive(Deserialize)]
pub(crate) struct RawAuthor {
    #[serde(default)]
    pub(crate) name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawAsset {
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) thumbnail_url: Option<String>,
    pub(crate) url: Option<String>,
}

/// The newest file of a project for one game version, loader and channel.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawFileIndex {
    #[serde(default)]
    pub(crate) game_version: String,
    pub(crate) file_id: u64,
    #[serde(default)]
    pub(crate) release_type: u8,
    pub(crate) mod_loader: Option<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawMod {
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) links: RawLinks,
    #[serde(default)]
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) download_count: f64,
    pub(crate) class_id: Option<u32>,
    #[serde(default)]
    pub(crate) categories: Vec<RawCategory>,
    #[serde(default)]
    pub(crate) authors: Vec<RawAuthor>,
    pub(crate) logo: Option<RawAsset>,
    #[serde(default)]
    pub(crate) screenshots: Vec<RawAsset>,
    #[serde(default)]
    pub(crate) latest_files_indexes: Vec<RawFileIndex>,
    pub(crate) allow_mod_distribution: Option<bool>,
    #[serde(default)]
    pub(crate) date_created: String,
    #[serde(default)]
    pub(crate) date_released: String,
    #[serde(default)]
    pub(crate) date_modified: String,
}

impl RawMod {
    pub(crate) fn author(&self) -> Option<String> {
        self.authors
            .iter()
            .map(|author| author.name.trim())
            .find(|name| !name.is_empty())
            .map(str::to_owned)
    }

    pub(crate) fn website(&self) -> Option<&str> {
        self.links
            .website_url
            .as_deref()
            .filter(|url| url.starts_with("https://"))
    }

    pub(crate) fn icon(&self) -> Option<String> {
        self.logo.as_ref().and_then(|logo| {
            logo.thumbnail_url
                .clone()
                .filter(|url| !url.is_empty())
                .or_else(|| logo.url.clone().filter(|url| !url.is_empty()))
        })
    }

    /// The loaders the project's newest files name, in the launcher's words.
    pub(crate) fn loaders(&self) -> Vec<String> {
        let mut loaders: Vec<String> = self
            .latest_files_indexes
            .iter()
            .filter_map(|index| loader_name(index.mod_loader?))
            .map(str::to_owned)
            .collect();
        loaders.sort();
        loaders.dedup();
        loaders
    }

    pub(crate) fn game_versions(&self) -> Vec<String> {
        let mut versions: Vec<String> = Vec::new();
        for index in &self.latest_files_indexes {
            if !index.game_version.is_empty() && !versions.contains(&index.game_version) {
                versions.push(index.game_version.clone());
            }
        }
        versions
    }
}

#[derive(Deserialize)]
pub(crate) struct RawHash {
    #[serde(default)]
    pub(crate) value: String,
    #[serde(default)]
    pub(crate) algo: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawSortableVersion {
    #[serde(default)]
    pub(crate) game_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawDependency {
    pub(crate) mod_id: u64,
    #[serde(default)]
    pub(crate) relation_type: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawFile {
    pub(crate) id: u64,
    pub(crate) mod_id: u64,
    #[serde(default)]
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) file_name: String,
    #[serde(default)]
    pub(crate) release_type: u8,
    #[serde(default)]
    pub(crate) hashes: Vec<RawHash>,
    #[serde(default)]
    pub(crate) file_date: String,
    #[serde(default)]
    pub(crate) file_length: u64,
    #[serde(default)]
    pub(crate) download_count: f64,
    pub(crate) download_url: Option<String>,
    /// Game versions, loaders and environments in one list.
    #[serde(default)]
    pub(crate) game_versions: Vec<String>,
    /// The same list; only game versions have `gameVersion` set.
    #[serde(default)]
    pub(crate) sortable_game_versions: Vec<RawSortableVersion>,
    #[serde(default)]
    pub(crate) dependencies: Vec<RawDependency>,
    #[serde(default)]
    pub(crate) file_fingerprint: u64,
}

impl RawFile {
    pub(crate) fn sha1(&self) -> Option<String> {
        self.hashes
            .iter()
            .find(|hash| hash.algo == 1 && hash.value.len() == 40)
            .map(|hash| hash.value.to_ascii_lowercase())
    }

    /// Minecraft versions only. Files without the sortable list fall back to
    /// the names that look like versions.
    pub(crate) fn minecraft_versions(&self) -> Vec<String> {
        let mut versions: Vec<String> = self
            .sortable_game_versions
            .iter()
            .map(|version| version.game_version.trim())
            .filter(|version| !version.is_empty())
            .map(str::to_owned)
            .collect();
        if versions.is_empty() {
            versions = self
                .game_versions
                .iter()
                .filter(|name| looks_like_game_version(name))
                .cloned()
                .collect();
        }
        versions.dedup();
        versions
    }

    /// The loaders the file names, in the launcher's words. A file from
    /// before 1.14 that names none ran on Forge, the only loader there was.
    pub(crate) fn loaders(&self) -> Vec<String> {
        let mut loaders: Vec<String> = self
            .game_versions
            .iter()
            .filter_map(|name| version_loader(name))
            .map(str::to_owned)
            .collect();
        loaders.sort();
        loaders.dedup();
        if loaders.is_empty() {
            let versions = self.minecraft_versions();
            if !versions.is_empty() && versions.iter().all(|version| before_fabric(version)) {
                loaders.push("forge".to_owned());
            }
        }
        loaders
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawMatch {
    pub(crate) file: RawFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawMatches {
    #[serde(default)]
    pub(crate) exact_matches: Vec<RawMatch>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawMinecraftVersion {
    #[serde(default)]
    pub(crate) version_string: String,
}

pub(crate) fn class_of(kind: ProjectKind) -> u32 {
    match kind {
        ProjectKind::Mod => CLASS_MODS,
        ProjectKind::Modpack => CLASS_MODPACKS,
        ProjectKind::ResourcePack => CLASS_RESOURCE_PACKS,
        ProjectKind::Shader => CLASS_SHADERS,
    }
}

pub(crate) fn kind_of(class: Option<u32>) -> Option<ProjectKind> {
    match class? {
        CLASS_MODS => Some(ProjectKind::Mod),
        CLASS_MODPACKS => Some(ProjectKind::Modpack),
        CLASS_RESOURCE_PACKS => Some(ProjectKind::ResourcePack),
        CLASS_SHADERS => Some(ProjectKind::Shader),
        _ => None,
    }
}

pub(crate) fn pack_file_kind(class: Option<u32>) -> PackFileKind {
    match class {
        Some(CLASS_MODS) => PackFileKind::Mod,
        Some(CLASS_RESOURCE_PACKS) => PackFileKind::ResourcePack,
        Some(CLASS_SHADERS) => PackFileKind::Shader,
        Some(CLASS_DATA_PACKS) => PackFileKind::DataPack,
        _ => PackFileKind::Other,
    }
}

/// `ModLoaderType` in the launcher's loader names.
pub(crate) fn loader_name(loader: u8) -> Option<&'static str> {
    Some(match loader {
        1 => "forge",
        3 => "liteloader",
        4 => "fabric",
        5 => "quilt",
        6 => "neoforge",
        _ => return None,
    })
}

pub(crate) fn loader_type(name: &str) -> Option<u8> {
    Some(match name.to_ascii_lowercase().as_str() {
        "forge" => 1,
        "liteloader" => 3,
        "fabric" => 4,
        "quilt" => 5,
        "neoforge" => 6,
        _ => return None,
    })
}

/// A loader or shader loader named in a file's version list.
fn version_loader(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "forge" => "forge",
        "fabric" => "fabric",
        "quilt" => "quilt",
        "neoforge" => "neoforge",
        "liteloader" => "liteloader",
        "iris" => "iris",
        "optifine" => "optifine",
        "canvas" => "canvas",
        _ => return None,
    })
}

fn looks_like_game_version(name: &str) -> bool {
    let mut parts = name.split('.');
    matches!(parts.next(), Some(major) if major.parse::<u32>().is_ok())
        && matches!(parts.next(), Some(minor) if minor.split('-').next().is_some_and(|m| m.parse::<u32>().is_ok()))
}

/// `1.x` with x below 14, when Forge was the only loader.
pub(crate) fn before_fabric(version: &str) -> bool {
    let mut parts = version.split(['.', '-']);
    parts.next() == Some("1")
        && parts
            .next()
            .and_then(|minor| minor.parse::<u32>().ok())
            .is_some_and(|minor| minor < 14)
}

pub(crate) fn channel(release_type: u8) -> ReleaseChannel {
    match release_type {
        2 => ReleaseChannel::Beta,
        3 => ReleaseChannel::Alpha,
        _ => ReleaseChannel::Release,
    }
}

pub(crate) fn dependency_kind(relation: u8) -> DependencyKind {
    match relation {
        1 => DependencyKind::Embedded,
        2 => DependencyKind::Optional,
        3 => DependencyKind::Required,
        5 => DependencyKind::Incompatible,
        _ => DependencyKind::Other,
    }
}

/// `ModsSearchSortField`. With no words, "featured" is not a ranking, so
/// relevance falls back to popularity.
pub(crate) fn sort_field(sort: Sort, has_text: bool) -> u8 {
    match sort {
        Sort::Relevance if has_text => 1,
        Sort::Relevance => 2,
        Sort::Updated => 3,
        Sort::Name => 4,
        Sort::Author => 5,
        Sort::Downloads => 6,
        Sort::Newest => 11,
        Sort::Follows => 2,
    }
}
