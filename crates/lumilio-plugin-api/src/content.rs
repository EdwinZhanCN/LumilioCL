//! Source-independent content contracts. IDs belong to their source; the
//! host keeps the plugin ID alongside them and owns installation and hashing.
//!
//! Search filter semantics retain the attribution from the core query model:
//! adapted from Modrinth App's `packages/ui/src/utils/search.ts`
//! (`newFilters` / `getEnvironmentFilterGroups`), GPL-3.0-only.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{HostContext, PluginError};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ProjectKind {
    Mod,
    Modpack,
    ResourcePack,
    Shader,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Sort {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
    Name,
    Author,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Stance {
    #[default]
    Include,
    Exclude,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pick {
    /// Opaque source option ID, not necessarily a displayed category name.
    pub name: String,
    pub stance: Stance,
    pub any: bool,
}

/// Limits apply per filter, per project kind. A source can support one
/// category without supporting exclusions or multiple combined categories.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PickSupport {
    pub max_included: u32,
    pub exclude: bool,
    pub any: bool,
    pub all: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FilterSupport {
    /// Zero means unsupported; a single-version API reports 1.
    pub max_game_versions: u32,
    pub categories: Option<PickSupport>,
    pub loaders: Option<PickSupport>,
    pub environment: bool,
    pub open_source: bool,
    pub hidden_projects: bool,
    pub excluded_disclosures: bool,
    pub excluded_types: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KindSupport {
    pub kind: ProjectKind,
    pub filters: FilterSupport,
    pub sorts: Vec<Sort>,
    /// The source's own web page listing this kind, if it has one.
    #[serde(default)]
    pub browse_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FingerprintKind {
    Sha1,
    /// CurseForge's: MurmurHash2 (seed 1) of the file without its
    /// whitespace bytes (tab, line feed, carriage return, space).
    Murmur2,
}

/// Whose project and version IDs a source speaks. Pack formats that name
/// files by ID, and exports that link to one site, look for a source by this.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Catalog {
    Modrinth,
    CurseForge,
}

/// How a source writes project bodies and changelogs.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum TextFormat {
    #[default]
    Markdown,
    Html,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Capabilities {
    pub kinds: Vec<KindSupport>,
    pub fingerprints: Vec<FingerprintKind>,
    #[serde(default)]
    pub catalog: Option<Catalog>,
    #[serde(default)]
    pub text_format: TextFormat,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchQuery {
    pub text: String,
    pub kind: ProjectKind,
    pub sort: Sort,
    pub page: u32,
    pub page_size: u32,
    pub game_versions: Vec<String>,
    pub loaders: Vec<Pick>,
    pub categories: Vec<Pick>,
    pub client: bool,
    pub server: bool,
    pub open_source: Option<Stance>,
    pub hidden_projects: Vec<String>,
    pub excluded_disclosures: Vec<String>,
    pub excluded_types: Vec<String>,
}

impl SearchQuery {
    /// Checks the advertised semantics so callers can report an unsupported
    /// selection instead of silently widening the search.
    pub fn validate(&self, capabilities: &Capabilities) -> Result<(), PluginError> {
        let invalid = || PluginError::InvalidInput("unsupported content search selection".into());
        let kind = capabilities
            .kinds
            .iter()
            .find(|kind| kind.kind == self.kind)
            .ok_or_else(invalid)?;
        let filters = &kind.filters;
        if !kind.sorts.contains(&self.sort)
            || self.game_versions.len() as u64 > u64::from(filters.max_game_versions)
            || !picks_supported(&self.categories, filters.categories.as_ref())
            || !picks_supported(&self.loaders, filters.loaders.as_ref())
            || ((self.client || self.server) && !filters.environment)
            || (self.open_source.is_some() && !filters.open_source)
            || (!self.hidden_projects.is_empty() && !filters.hidden_projects)
            || (!self.excluded_disclosures.is_empty() && !filters.excluded_disclosures)
            || (!self.excluded_types.is_empty() && !filters.excluded_types)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

fn picks_supported(picks: &[Pick], support: Option<&PickSupport>) -> bool {
    let picks: Vec<_> = picks
        .iter()
        .filter(|pick| !pick.name.trim().is_empty())
        .collect();
    if picks.is_empty() {
        return true;
    }
    let Some(support) = support else {
        return false;
    };
    let included: Vec<_> = picks
        .iter()
        .filter(|pick| pick.stance == Stance::Include)
        .collect();
    let any_count = included.iter().filter(|pick| pick.any).count();
    included.len() as u64 <= u64::from(support.max_included)
        && (support.exclude || picks.iter().all(|pick| pick.stance == Stance::Include))
        && (support.any || any_count <= 1)
        && (support.all || included.len() <= 1 || any_count == included.len())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Environment {
    ClientOrServer,
    ClientAndServer,
    ClientOnly,
    ServerOnly,
    SingleplayerOnly,
    DedicatedServerOnly,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SideSupport {
    Required,
    Optional,
    Unsupported,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub kind: ProjectKind,
    pub categories: Vec<String>,
    pub all_categories: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: u64,
    /// `None` when the source does not count followers.
    pub follows: Option<u64>,
    /// RFC 3339, empty when unavailable.
    pub published: String,
    pub updated: String,
    pub environment: Option<Environment>,
    pub icon_url: Option<String>,
    pub page_url: String,
    /// The author lets people download the files only on the source's own
    /// site; a launcher may not download them.
    #[serde(default)]
    pub manual_download: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub total_hits: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GalleryImage {
    pub url: String,
    pub full_url: String,
    pub title: String,
    pub description: String,
    pub featured: bool,
    pub ordering: i64,
    pub created: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectLinks {
    pub source: Option<String>,
    pub issues: Option<String>,
    pub wiki: Option<String>,
    pub discord: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub kind: ProjectKind,
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: u64,
    /// `None` when the source does not count followers.
    pub followers: Option<u64>,
    pub published: String,
    pub updated: String,
    pub client_side: SideSupport,
    pub server_side: SideSupport,
    pub license: Option<String>,
    pub links: ProjectLinks,
    pub gallery: Vec<GalleryImage>,
    pub icon_url: Option<String>,
    pub game_versions: Vec<String>,
    pub page_url: String,
    pub author: Option<String>,
    /// See [`SearchHit::manual_download`].
    #[serde(default)]
    pub manual_download: bool,
}

/// Both IDs are needed by APIs whose version endpoint is below its project.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionRef {
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ReleaseChannel {
    Alpha,
    Beta,
    Release,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DependencyKind {
    Required,
    Optional,
    Embedded,
    Incompatible,
    Other,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub kind: DependencyKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionFile {
    /// Empty when the author lets people download the file only on the
    /// source's site, at `download_page`.
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub sha1: Option<String>,
    /// The file's page on the source's site, where a person can download it.
    #[serde(default)]
    pub download_page: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub number: String,
    pub channel: ReleaseChannel,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub published: String,
    pub files: Vec<VersionFile>,
    pub dependencies: Vec<Dependency>,
    pub downloads: u64,
    pub changelog: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Fingerprint {
    Sha1(String),
    Murmur2(u32),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileIdentity {
    /// A host-chosen correlation key. No local path is needed by the source.
    pub key: String,
    pub fingerprints: Vec<Fingerprint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Compatibility {
    pub game_version: String,
    pub loader: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CategoryTag {
    pub id: String,
    pub name: String,
    pub header: String,
    pub kind: ProjectKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LoaderTag {
    pub name: String,
    pub kinds: Vec<ProjectKind>,
    /// Source-defined project types outside `ProjectKind` (`plugin`,
    /// `datapack`); they tell the host which loaders are not for game mods.
    #[serde(default)]
    pub other_kinds: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameVersionTag {
    pub version: String,
    pub release: bool,
    pub snapshot: bool,
    pub published: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FilterChoices {
    pub categories: Vec<CategoryTag>,
    pub loaders: Vec<LoaderTag>,
    pub game_versions: Vec<GameVersionTag>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub kind: Option<ProjectKind>,
    pub icon_url: Option<String>,
    pub author: Option<String>,
    /// The project's page on the source's site; empty when unknown.
    #[serde(default)]
    pub page_url: String,
}

/// What a file a pack names is, which decides the folder it goes into.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PackFileKind {
    Mod,
    ResourcePack,
    Shader,
    DataPack,
    /// A type the host has no folder for (a world, configuration, …).
    Other,
}

/// One file a pack names by project and version ID, as its source knows it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedFile {
    pub project_id: String,
    pub version_id: String,
    /// The project's name; empty when the source did not say.
    pub title: String,
    pub kind: PackFileKind,
    pub file: VersionFile,
}

/// All operations run inside the host's worker/timeout/panic boundary.
/// Implementations must reject unsupported queries rather than drop filters.
/// Missing recognition/update entries mean unknown/no compatible update;
/// errors mean unavailable and must not masquerade as an empty successful map.
pub trait ContentSource: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    fn search(&self, ctx: &dyn HostContext, query: &SearchQuery)
    -> Result<SearchPage, PluginError>;
    fn project(&self, ctx: &dyn HostContext, id: &str) -> Result<Project, PluginError>;
    fn versions(
        &self,
        ctx: &dyn HostContext,
        project: &str,
        changelog: bool,
    ) -> Result<Vec<Version>, PluginError>;
    fn version_files(
        &self,
        ctx: &dyn HostContext,
        version: &VersionRef,
    ) -> Result<Vec<VersionFile>, PluginError>;
    fn dependencies(
        &self,
        ctx: &dyn HostContext,
        version: &VersionRef,
    ) -> Result<Vec<Dependency>, PluginError>;
    fn filter_choices(&self, ctx: &dyn HostContext) -> Result<FilterChoices, PluginError>;
    fn identify(
        &self,
        ctx: &dyn HostContext,
        files: &[FileIdentity],
    ) -> Result<BTreeMap<String, Version>, PluginError>;
    fn latest_for(
        &self,
        ctx: &dyn HostContext,
        files: &[FileIdentity],
        compatibility: &Compatibility,
    ) -> Result<BTreeMap<String, Version>, PluginError>;
    fn project_summaries(
        &self,
        ctx: &dyn HostContext,
        projects: &[String],
    ) -> Result<Vec<ProjectSummary>, PluginError>;
    /// One version's changelog, in the source's [`TextFormat`]. For sources
    /// that leave changelogs out of `versions` because each costs a request.
    fn changelog(
        &self,
        _ctx: &dyn HostContext,
        _version: &VersionRef,
    ) -> Result<String, PluginError> {
        Ok(String::new())
    }
    /// The files a pack in this source's [`Catalog`] names by reference.
    /// References the source does not know are absent from the answer.
    fn resolve_files(
        &self,
        _ctx: &dyn HostContext,
        _files: &[VersionRef],
    ) -> Result<Vec<ResolvedFile>, PluginError> {
        Ok(Vec::new())
    }
}
