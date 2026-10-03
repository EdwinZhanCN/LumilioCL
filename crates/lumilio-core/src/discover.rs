//! Discover: finding mods, modpacks, resource packs and shaders on Modrinth.
//!
//! Behavior notes: `docs/behavior/discover.md`. Search, project, and version
//! shapes follow Modrinth's public API documentation.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::fetch::{FetchError, fetch_document, post_document};
use crate::instance::Loader;
use crate::transfer::{SourceChain, TransferError, TransferRequest, Transport};

pub const API_BASE: &str = "https://api.modrinth.com";
pub const SITE_BASE: &str = "https://modrinth.com";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Modpack,
    Mod,
    ResourcePack,
    Shader,
}

impl ProjectKind {
    /// The name Modrinth uses for this kind in queries and page addresses.
    #[must_use]
    pub const fn protocol_name(self) -> &'static str {
        match self {
            Self::Modpack => "modpack",
            Self::Mod => "mod",
            Self::ResourcePack => "resourcepack",
            Self::Shader => "shader",
        }
    }

    #[must_use]
    pub fn from_protocol(name: &str) -> Option<Self> {
        match name {
            "modpack" => Some(Self::Modpack),
            "mod" => Some(Self::Mod),
            "resourcepack" => Some(Self::ResourcePack),
            "shader" => Some(Self::Shader),
            _ => None,
        }
    }

    /// The folder inside an instance's game directory that holds this kind of
    /// file; modpacks are not dropped into a folder.
    #[must_use]
    pub const fn install_folder(self) -> Option<&'static str> {
        match self {
            Self::Mod => Some("mods"),
            Self::ResourcePack => Some("resourcepacks"),
            Self::Shader => Some("shaderpacks"),
            Self::Modpack => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortIndex {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl SortIndex {
    const fn protocol_name(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Follows => "follows",
            Self::Newest => "newest",
            Self::Updated => "updated",
        }
    }
}

/// The public web page of a project — what the webview shows.
#[must_use]
pub fn project_page_url(kind: ProjectKind, slug_or_id: &str) -> String {
    format!("{SITE_BASE}/{}/{slug_or_id}", kind.protocol_name())
}

/// The public listing of every project of a kind, e.g. `…/mods`.
#[must_use]
pub fn browse_page_url(kind: ProjectKind) -> String {
    format!("{SITE_BASE}/{}s", kind.protocol_name())
}

fn loader_tag(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
        Loader::Quilt => Some("quilt"),
    }
}

#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub text: String,
    pub kind: ProjectKind,
    pub game_version: Option<String>,
    /// Loader names (`fabric`, `forge`, …); any one may match. Only mods and
    /// modpacks have loaders worth filtering by; other kinds ignore it.
    pub loaders: Vec<String>,
    /// Category names; a project must have all of them.
    pub categories: Vec<String>,
    pub sort: SortIndex,
    /// Zero-based.
    pub page: u32,
    pub page_size: u32,
}

impl SearchQuery {
    #[must_use]
    pub fn new(kind: ProjectKind) -> Self {
        Self {
            text: String::new(),
            kind,
            game_version: None,
            loaders: Vec::new(),
            categories: Vec::new(),
            sort: SortIndex::default(),
            page: 0,
            page_size: 20,
        }
    }

    /// Facet groups; groups are ANDed, each group here holds one clause.
    fn facets(&self) -> Vec<Vec<String>> {
        let mut facets = vec![vec![format!("project_type:{}", self.kind.protocol_name())]];
        if let Some(version) = self
            .game_version
            .as_deref()
            .filter(|v| !v.trim().is_empty())
        {
            facets.push(vec![format!("versions:{version}")]);
        }
        let loaders: Vec<String> = self
            .loaders
            .iter()
            .map(|loader| loader.trim())
            .filter(|loader| !loader.is_empty())
            .map(|loader| format!("categories:{loader}"))
            .collect();
        if matches!(self.kind, ProjectKind::Mod | ProjectKind::Modpack) && !loaders.is_empty() {
            facets.push(loaders);
        }
        for category in self
            .categories
            .iter()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
        {
            facets.push(vec![format!("categories:{category}")]);
        }
        facets
    }

    #[must_use]
    pub fn url(&self) -> String {
        let mut url = Url::parse(API_BASE).expect("constant address is valid");
        url.set_path("/v2/search");
        let facets = serde_json::to_string(&self.facets()).expect("strings always encode");
        let page_size = self.page_size.clamp(1, 100);
        url.query_pairs_mut()
            .append_pair("query", self.text.trim())
            .append_pair("facets", &facets)
            .append_pair("offset", &(self.page.saturating_mul(page_size)).to_string())
            .append_pair("limit", &page_size.to_string())
            .append_pair("index", self.sort.protocol_name());
        url.into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub kind: ProjectKind,
    /// Display categories, without the loaders.
    pub categories: Vec<String>,
    /// Mod loaders the project supports (`fabric`, `forge`, …).
    pub loaders: Vec<String>,
    pub downloads: u64,
    pub follows: u64,
    /// RFC 3339 time of the last change.
    pub updated: String,
    pub client_side: SideSupport,
    pub server_side: SideSupport,
    pub icon_url: Option<String>,
}

/// Whether a project runs on one side of the connection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SideSupport {
    Required,
    Optional,
    Unsupported,
    #[default]
    Unknown,
}

impl SideSupport {
    fn from_protocol(name: Option<&str>) -> Self {
        match name {
            Some("required") => Self::Required,
            Some("optional") => Self::Optional,
            Some("unsupported") => Self::Unsupported,
            _ => Self::Unknown,
        }
    }

    const fn runs(self) -> bool {
        matches!(self, Self::Required | Self::Optional)
    }
}

/// Where a project can be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Environment {
    ClientAndServer,
    ClientOnly,
    ServerOnly,
}

/// Summarizes the two side flags. Unknown on both sides says nothing.
#[must_use]
pub fn environment(client: SideSupport, server: SideSupport) -> Option<Environment> {
    match (client.runs(), server.runs()) {
        (true, true) => Some(Environment::ClientAndServer),
        (true, false) if server == SideSupport::Unsupported => Some(Environment::ClientOnly),
        (false, true) if client == SideSupport::Unsupported => Some(Environment::ServerOnly),
        _ => None,
    }
}

/// Loader names Modrinth lists next to categories in search results.
const MOD_LOADERS: [&str; 8] = [
    "fabric",
    "forge",
    "neoforge",
    "quilt",
    "liteloader",
    "rift",
    "modloader",
    "babric",
];

impl SearchHit {
    #[must_use]
    pub fn page_url(&self) -> String {
        project_page_url(self.kind, &self.slug)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub total_hits: u64,
}

impl SearchPage {
    /// How many pages the whole result set spans.
    #[must_use]
    pub fn page_count(&self, page_size: u32) -> u32 {
        let size = u64::from(page_size.max(1));
        u32::try_from(self.total_hits.div_ceil(size)).unwrap_or(u32::MAX)
    }
}

#[derive(Debug, Deserialize)]
struct RawSearch {
    #[serde(default)]
    hits: Vec<RawHit>,
    #[serde(default)]
    total_hits: u64,
}

#[derive(Debug, Deserialize)]
struct RawHit {
    project_id: Option<String>,
    slug: Option<String>,
    title: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    project_type: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    display_categories: Vec<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    follows: u64,
    #[serde(default)]
    date_modified: String,
    client_side: Option<String>,
    server_side: Option<String>,
    icon_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GalleryImage {
    pub url: String,
    pub title: String,
    pub description: String,
    pub featured: bool,
    pub ordering: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectLinks {
    pub source: Option<String>,
    pub issues: Option<String>,
    pub wiki: Option<String>,
    pub discord: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    /// Long markdown description.
    pub body: String,
    pub kind: ProjectKind,
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: u64,
    pub followers: u64,
    pub published: String,
    pub updated: String,
    pub client_side: SideSupport,
    pub server_side: SideSupport,
    pub license: Option<String>,
    pub links: ProjectLinks,
    /// Featured image first, then by the author's order.
    pub gallery: Vec<GalleryImage>,
    pub icon_url: Option<String>,
    pub game_versions: Vec<String>,
}

impl Project {
    #[must_use]
    pub fn page_url(&self) -> String {
        project_page_url(self.kind, &self.slug)
    }
}

#[derive(Debug, Deserialize)]
struct RawProject {
    id: Option<String>,
    slug: Option<String>,
    title: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
    project_type: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    followers: u64,
    #[serde(default)]
    published: String,
    #[serde(default)]
    updated: String,
    client_side: Option<String>,
    server_side: Option<String>,
    license: Option<RawLicense>,
    source_url: Option<String>,
    issues_url: Option<String>,
    wiki_url: Option<String>,
    discord_url: Option<String>,
    #[serde(default)]
    gallery: Vec<RawGallery>,
    icon_url: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawLicense {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawGallery {
    url: Option<String>,
    title: Option<String>,
    description: Option<String>,
    #[serde(default)]
    featured: bool,
    #[serde(default)]
    ordering: i64,
}

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
struct RawVersion {
    id: Option<String>,
    project_id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    dependencies: Vec<RawDependency>,
    changelog: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    url: Option<String>,
    filename: Option<String>,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    hashes: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct RawDependency {
    project_id: Option<String>,
    version_id: Option<String>,
    #[serde(default)]
    dependency_type: String,
}

#[derive(Debug)]
pub enum DiscoverError {
    Decode(String),
    Fetch(FetchError),
}

impl Display for DiscoverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(message) => write!(f, "unexpected Modrinth answer: {message}"),
            Self::Fetch(error) => write!(f, "Modrinth unavailable: {error}"),
        }
    }
}

impl Error for DiscoverError {}

impl From<FetchError> for DiscoverError {
    fn from(error: FetchError) -> Self {
        Self::Fetch(error)
    }
}

fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, DiscoverError> {
    serde_json::from_slice(bytes).map_err(|error| DiscoverError::Decode(error.to_string()))
}

/// Decodes a search answer. Hits of an unknown project type or without an id
/// or title are dropped, not fatal.
pub fn decode_search(bytes: &[u8]) -> Result<SearchPage, DiscoverError> {
    let raw: RawSearch = decode(bytes)?;
    let hits = raw
        .hits
        .into_iter()
        .filter_map(|hit| {
            let kind = ProjectKind::from_protocol(hit.project_type.as_deref()?)?;
            let project_id = hit.project_id.filter(|id| !id.is_empty())?;
            let title = hit.title.filter(|title| !title.is_empty())?;
            let shown = if hit.display_categories.is_empty() {
                hit.categories
            } else {
                hit.display_categories
            };
            let (loaders, categories): (Vec<String>, Vec<String>) = shown
                .into_iter()
                .partition(|name| MOD_LOADERS.contains(&name.as_str()));
            Some(SearchHit {
                slug: hit
                    .slug
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| project_id.clone()),
                project_id,
                title,
                description: hit.description,
                author: hit.author,
                kind,
                categories,
                loaders,
                downloads: hit.downloads,
                follows: hit.follows,
                updated: hit.date_modified,
                client_side: SideSupport::from_protocol(hit.client_side.as_deref()),
                server_side: SideSupport::from_protocol(hit.server_side.as_deref()),
                icon_url: hit.icon_url.filter(|url| !url.is_empty()),
            })
        })
        .collect();
    Ok(SearchPage {
        hits,
        total_hits: raw.total_hits,
    })
}

pub fn decode_project(bytes: &[u8]) -> Result<Project, DiscoverError> {
    let raw: RawProject = decode(bytes)?;
    let missing = |field: &str| DiscoverError::Decode(format!("project has no {field}"));
    let id = raw
        .id
        .filter(|v| !v.is_empty())
        .ok_or_else(|| missing("id"))?;
    let kind = raw
        .project_type
        .as_deref()
        .and_then(ProjectKind::from_protocol)
        .ok_or_else(|| missing("supported type"))?;
    Ok(Project {
        slug: raw
            .slug
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| id.clone()),
        title: raw.title.unwrap_or_default(),
        id,
        description: raw.description,
        body: raw.body,
        kind,
        categories: raw.categories,
        loaders: raw.loaders,
        downloads: raw.downloads,
        followers: raw.followers,
        published: raw.published,
        updated: raw.updated,
        client_side: SideSupport::from_protocol(raw.client_side.as_deref()),
        server_side: SideSupport::from_protocol(raw.server_side.as_deref()),
        license: raw
            .license
            .and_then(|license| license.name.filter(|n| !n.is_empty()).or(license.id)),
        links: ProjectLinks {
            source: non_empty(raw.source_url),
            issues: non_empty(raw.issues_url),
            wiki: non_empty(raw.wiki_url),
            discord: non_empty(raw.discord_url),
        },
        gallery: gallery_from_raw(raw.gallery),
        icon_url: raw.icon_url.filter(|url| !url.is_empty()),
        game_versions: raw.game_versions,
    })
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

/// Images without an address are dropped; the featured one leads, the rest
/// keep the author's order.
fn gallery_from_raw(raw: Vec<RawGallery>) -> Vec<GalleryImage> {
    let mut images: Vec<GalleryImage> = raw
        .into_iter()
        .filter_map(|image| {
            Some(GalleryImage {
                url: non_empty(image.url)?,
                title: image.title.unwrap_or_default(),
                description: image.description.unwrap_or_default(),
                featured: image.featured,
                ordering: image.ordering,
            })
        })
        .collect();
    images.sort_by_key(|image| (!image.featured, image.ordering));
    images
}

/// Decodes a version list. Versions without files or an id are dropped:
/// there is nothing to install from them.
pub fn decode_versions(bytes: &[u8]) -> Result<Vec<Version>, DiscoverError> {
    let raw: Vec<RawVersion> = decode(bytes)?;
    Ok(raw.into_iter().filter_map(version_from_raw).collect())
}

/// Decodes an answer shaped `{ "<sha1>": <version>, … }`.
fn decode_version_map(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, Version>, DiscoverError> {
    let raw: std::collections::BTreeMap<String, RawVersion> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|(hash, version)| Some((hash, version_from_raw(version)?)))
        .collect())
}

fn version_from_raw(raw: RawVersion) -> Option<Version> {
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

#[derive(Debug, Eq, PartialEq)]
pub enum IntentError {
    /// Modpacks are installed as new instances, not copied into a folder.
    NotAFileKind,
    NoFile,
    /// The file name would escape the destination folder or is not portable.
    UnsafeFileName(String),
    Transfer(String),
}

impl Display for IntentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFileKind => f.write_str("modpacks are installed as instances"),
            Self::NoFile => f.write_str("version has no downloadable file"),
            Self::UnsafeFileName(name) => write!(f, "unsafe file name {name:?}"),
            Self::Transfer(message) => write!(f, "{message}"),
        }
    }
}

impl Error for IntentError {}

impl From<TransferError> for IntentError {
    fn from(error: TransferError) -> Self {
        Self::Transfer(error.to_string())
    }
}

pub(crate) fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':'])
        && !name.chars().any(char::is_control)
        && name.trim() == name
}

/// The transfer that puts `version`'s file into `game_dir`, verified by the
/// published size and SHA-1. `sources` should already include any mirrors.
pub fn install_request(
    kind: ProjectKind,
    version: &Version,
    game_dir: &Path,
    sources: Vec<String>,
) -> Result<TransferRequest, IntentError> {
    let folder = kind.install_folder().ok_or(IntentError::NotAFileKind)?;
    let file = version.install_file().ok_or(IntentError::NoFile)?;
    if !is_safe_file_name(&file.filename) {
        return Err(IntentError::UnsafeFileName(file.filename.clone()));
    }
    let destination: PathBuf = game_dir.join(folder).join(&file.filename);
    let mut request = TransferRequest::new(
        format!("content:{}:{}", version.project_id, version.id),
        sources,
        destination,
    )?;
    if file.size > 0 {
        request = request.expect_size(file.size);
    }
    if let Some(sha1) = &file.sha1 {
        request = request.expect_sha1(sha1)?;
    }
    Ok(request)
}

/// A thin client over the public API.
pub struct ModrinthClient<T> {
    transport: T,
    base: String,
    /// Mirrors tried for read requests, as configured in settings.
    sources: Option<SourceChain>,
}

impl<T: Transport> ModrinthClient<T> {
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            base: API_BASE.to_owned(),
            sources: None,
        }
    }

    /// Sends read requests through these mirrors before the official address.
    #[must_use]
    pub fn with_sources(mut self, sources: SourceChain) -> Self {
        self.sources = Some(sources);
        self
    }

    fn candidates(&self, url: String) -> Vec<String> {
        match &self.sources {
            Some(chain) => chain.candidates(&url),
            None => vec![url],
        }
    }

    /// Points the client at another API root (tests, mirrors).
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into().trim_end_matches('/').to_owned();
        self
    }

    #[cfg(test)]
    pub(crate) fn transport_for_tests(&self) -> &T {
        &self.transport
    }

    fn address(&self, url: String) -> String {
        url.replacen(API_BASE, &self.base, 1)
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchPage, DiscoverError> {
        let bytes =
            fetch_document(&self.transport, &self.candidates(self.address(query.url()))).await?;
        decode_search(&bytes)
    }

    pub async fn project(&self, id_or_slug: &str) -> Result<Project, DiscoverError> {
        let url = format!("{}/v2/project/{id_or_slug}", self.base);
        decode_project(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Which Modrinth version each file (by SHA-1) belongs to. Files Modrinth
    /// does not know are absent from the answer.
    pub async fn identify(
        &self,
        sha1s: &[String],
    ) -> Result<std::collections::BTreeMap<String, Version>, DiscoverError> {
        if sha1s.is_empty() {
            return Ok(Default::default());
        }
        let body = serde_json::json!({ "hashes": sha1s, "algorithm": "sha1" });
        let url = format!("{}/v2/version_files", self.base);
        decode_version_map(
            &post_document(&self.transport, &url, body.to_string().into_bytes()).await?,
        )
    }

    /// The newest compatible version for each file (by SHA-1). Files with no
    /// compatible version, or unknown to Modrinth, are absent.
    pub async fn latest_for(
        &self,
        sha1s: &[String],
        loader: Loader,
        game_version: &str,
    ) -> Result<std::collections::BTreeMap<String, Version>, DiscoverError> {
        if sha1s.is_empty() {
            return Ok(Default::default());
        }
        let loaders: Vec<&str> = loader_tag(loader).into_iter().collect();
        let body = serde_json::json!({
            "hashes": sha1s,
            "algorithm": "sha1",
            "loaders": loaders,
            "game_versions": [game_version],
        });
        let url = format!("{}/v2/version_files/update", self.base);
        decode_version_map(
            &post_document(&self.transport, &url, body.to_string().into_bytes()).await?,
        )
    }

    pub async fn versions(&self, id_or_slug: &str) -> Result<Vec<Version>, DiscoverError> {
        let url = format!(
            "{}/v2/project/{id_or_slug}/version?include_changelog=false",
            self.base
        );
        decode_versions(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Like [`Self::versions`], with each version's changelog.
    pub async fn versions_with_changelog(
        &self,
        id_or_slug: &str,
    ) -> Result<Vec<Version>, DiscoverError> {
        let url = format!(
            "{}/v2/project/{id_or_slug}/version?include_changelog=true",
            self.base
        );
        decode_versions(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Every category tag, for all project types.
    pub async fn categories(&self) -> Result<Vec<CategoryTag>, DiscoverError> {
        let url = format!("{}/v2/tag/category", self.base);
        decode_categories(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Every game version Modrinth knows, newest first as published.
    pub async fn game_versions(&self) -> Result<Vec<GameVersionTag>, DiscoverError> {
        let url = format!("{}/v2/tag/game_version", self.base);
        decode_game_versions(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// The project's owner, if the team lists one.
    pub async fn owner(&self, id_or_slug: &str) -> Result<Option<String>, DiscoverError> {
        let url = format!("{}/v2/project/{id_or_slug}/members", self.base);
        decode_owner(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Several projects at once, for labelling installed files.
    pub async fn project_summaries(
        &self,
        ids: &[String],
    ) -> Result<Vec<ProjectSummary>, DiscoverError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/v2/projects?ids={}", self.base, encoded_list(ids));
        decode_project_summaries(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Who each team's project belongs to, by team id.
    pub async fn team_authors(
        &self,
        team_ids: &[String],
    ) -> Result<std::collections::BTreeMap<String, String>, DiscoverError> {
        if team_ids.is_empty() {
            return Ok(Default::default());
        }
        let url = format!("{}/v2/teams?ids={}", self.base, encoded_list(team_ids));
        decode_team_authors(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }
}

/// A category a project can be filed under, for one project type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryTag {
    pub name: String,
    /// Modrinth's grouping, e.g. `categories`, `features`, `resolutions`.
    pub header: String,
    pub kind: ProjectKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameVersionTag {
    pub version: String,
    /// A full release, as opposed to a snapshot or pre-release.
    pub release: bool,
    pub published: String,
}

#[derive(Deserialize)]
struct RawCategory {
    name: Option<String>,
    #[serde(default)]
    header: String,
    project_type: Option<String>,
}

#[derive(Deserialize)]
struct RawGameVersion {
    version: Option<String>,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    date: String,
}

#[derive(Deserialize)]
struct RawMember {
    #[serde(default)]
    role: String,
    user: Option<RawUser>,
    team_id: Option<String>,
}

/// What an installed file's label needs to know about its project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub kind: Option<ProjectKind>,
    pub icon_url: Option<String>,
    pub team: Option<String>,
}

#[derive(Deserialize)]
struct RawSummary {
    id: Option<String>,
    slug: Option<String>,
    title: Option<String>,
    project_type: Option<String>,
    icon_url: Option<String>,
    team: Option<String>,
}

/// A JSON list of ids, percent-encoded for a query string.
fn encoded_list(ids: &[String]) -> String {
    let json = serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_owned());
    url::form_urlencoded::byte_serialize(json.as_bytes()).collect()
}

/// Projects without an id are dropped.
pub fn decode_project_summaries(bytes: &[u8]) -> Result<Vec<ProjectSummary>, DiscoverError> {
    let raw: Vec<RawSummary> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|raw| {
            let id = raw.id.filter(|id| !id.is_empty())?;
            Some(ProjectSummary {
                slug: raw
                    .slug
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| id.clone()),
                title: raw.title.unwrap_or_default(),
                kind: raw
                    .project_type
                    .as_deref()
                    .and_then(ProjectKind::from_protocol),
                icon_url: raw.icon_url.filter(|url| !url.is_empty()),
                team: raw.team.filter(|team| !team.is_empty()),
                id,
            })
        })
        .collect())
}

/// One author per team: the member whose role says owner, else the first
/// listed. Teams without a named member are left out.
pub fn decode_team_authors(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, String>, DiscoverError> {
    let teams: Vec<Vec<RawMember>> = decode(bytes)?;
    let mut authors = std::collections::BTreeMap::new();
    for members in teams {
        let team = members.iter().find_map(|member| member.team_id.clone());
        let chosen = members
            .iter()
            .find(|member| member.role.eq_ignore_ascii_case("owner"))
            .or_else(|| members.first());
        if let (Some(team), Some(name)) = (
            team,
            chosen.and_then(|member| member.user.as_ref()?.username.clone()),
        ) {
            authors.insert(team, name);
        }
    }
    Ok(authors)
}

#[derive(Deserialize)]
struct RawUser {
    username: Option<String>,
}

/// Categories of unsupported project types are dropped.
pub fn decode_categories(bytes: &[u8]) -> Result<Vec<CategoryTag>, DiscoverError> {
    let raw: Vec<RawCategory> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|tag| {
            Some(CategoryTag {
                name: tag.name.filter(|name| !name.is_empty())?,
                header: tag.header,
                kind: ProjectKind::from_protocol(tag.project_type.as_deref()?)?,
            })
        })
        .collect())
}

pub fn decode_game_versions(bytes: &[u8]) -> Result<Vec<GameVersionTag>, DiscoverError> {
    let raw: Vec<RawGameVersion> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|tag| {
            Some(GameVersionTag {
                version: tag.version.filter(|version| !version.is_empty())?,
                release: tag.version_type == "release",
                published: tag.date,
            })
        })
        .collect())
}

pub fn decode_owner(bytes: &[u8]) -> Result<Option<String>, DiscoverError> {
    let raw: Vec<RawMember> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .find(|member| member.role.eq_ignore_ascii_case("owner"))
        .and_then(|member| member.user?.username))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{TransportFuture, TransportResponse};
    use std::collections::BTreeMap;

    const SHA1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn version(
        id: &str,
        channel: &str,
        published: &str,
        games: &[&str],
        loaders: &[&str],
    ) -> String {
        format!(
            r#"{{"id":"{id}","project_id":"P","name":"n","version_number":"{id}",
                "version_type":"{channel}","game_versions":{games:?},"loaders":{loaders:?},
                "date_published":"{published}",
                "files":[{{"url":"https://cdn/x/{id}.jar","filename":"{id}.jar","primary":true,
                           "size":10,"hashes":{{"sha1":"{SHA1}"}}}}],
                "dependencies":[]}}"#
        )
    }

    fn versions() -> Vec<Version> {
        let body = format!(
            "[{}]",
            [
                version(
                    "old-release",
                    "release",
                    "2024-01-01T00:00:00Z",
                    &["1.21.1"],
                    &["fabric"]
                ),
                version(
                    "new-release",
                    "release",
                    "2024-06-01T00:00:00Z",
                    &["1.21.1"],
                    &["fabric"]
                ),
                version(
                    "newest-beta",
                    "beta",
                    "2024-09-01T00:00:00Z",
                    &["1.21.1"],
                    &["fabric"]
                ),
                version(
                    "forge-only",
                    "release",
                    "2024-12-01T00:00:00Z",
                    &["1.21.1"],
                    &["forge"]
                ),
                version(
                    "other-game",
                    "release",
                    "2025-01-01T00:00:00Z",
                    &["1.20.4"],
                    &["fabric"]
                ),
            ]
            .join(",")
        );
        decode_versions(body.as_bytes()).unwrap()
    }

    #[test]
    fn summaries_and_team_authors_decode_with_fallbacks() {
        let summaries = decode_project_summaries(
            br#"[{"id":"AANobbMI","slug":"sodium","title":"Sodium","project_type":"mod",
                  "icon_url":"https://x/i.webp","team":"4reLOAKe"},
                 {"id":"","title":"dropped"},
                 {"id":"B","project_type":"future","icon_url":""}]"#,
        )
        .unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].kind, Some(ProjectKind::Mod));
        assert_eq!(
            summaries[1].slug, "B",
            "the id stands in for a missing slug"
        );
        assert_eq!(summaries[1].icon_url, None);
        let authors = decode_team_authors(
            br#"[[{"role":"Original Author","team_id":"t1","user":{"username":"jelly"}},
                  {"role":"Owner","team_id":"t1","user":{"username":"lead"}}],
                 [{"role":"Developer","team_id":"t2","user":{"username":"solo"}}],
                 [{"role":"Owner","team_id":"t3"}]]"#,
        )
        .unwrap();
        assert_eq!(authors["t1"], "lead", "the owner wins");
        assert_eq!(authors["t2"], "solo", "else the first member");
        assert!(!authors.contains_key("t3"), "no name, no author");
        assert_eq!(encoded_list(&["a b".to_owned()]), "%5B%22a+b%22%5D");
    }

    #[test]
    fn browse_pages_are_the_plural_listing_of_each_kind() {
        assert_eq!(
            browse_page_url(ProjectKind::Mod),
            format!("{SITE_BASE}/mods")
        );
        assert_eq!(
            browse_page_url(ProjectKind::Modpack),
            format!("{SITE_BASE}/modpacks")
        );
        assert_eq!(
            browse_page_url(ProjectKind::ResourcePack),
            format!("{SITE_BASE}/resourcepacks")
        );
        assert_eq!(
            browse_page_url(ProjectKind::Shader),
            format!("{SITE_BASE}/shaders")
        );
    }

    #[test]
    fn search_url_carries_facets_paging_and_sort() {
        let mut query = SearchQuery::new(ProjectKind::Mod);
        query.text = " sodium ".to_owned();
        query.game_version = Some("1.21.1".to_owned());
        query.loaders = vec!["fabric".to_owned(), " quilt ".to_owned(), String::new()];
        query.categories = vec!["optimization".to_owned(), "lightweight".to_owned()];
        query.sort = SortIndex::Downloads;
        query.page = 2;
        query.page_size = 10;
        let url = Url::parse(&query.url()).unwrap();
        assert_eq!(url.path(), "/v2/search");
        let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs["query"], "sodium");
        assert_eq!(pairs["offset"], "20");
        assert_eq!(pairs["limit"], "10");
        assert_eq!(pairs["index"], "downloads");
        assert_eq!(
            pairs["facets"],
            r#"[["project_type:mod"],["versions:1.21.1"],["categories:fabric","categories:quilt"],["categories:optimization"],["categories:lightweight"]]"#
        );
    }

    #[test]
    fn loaders_only_filter_mods_and_modpacks_and_page_size_is_bounded() {
        let mut query = SearchQuery::new(ProjectKind::Shader);
        query.loaders = vec!["fabric".to_owned()];
        query.page_size = 5000;
        let url = Url::parse(&query.url()).unwrap();
        let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs["facets"], r#"[["project_type:shader"]]"#);
        assert_eq!(pairs["limit"], "100");
    }

    #[test]
    fn decodes_hits_and_drops_unusable_ones() {
        let page = decode_search(
            br#"{"total_hits": 45, "hits": [
                {"project_id":"A","slug":"sodium","title":"Sodium","author":"jelly",
                 "project_type":"mod","categories":["fabric","optimization"],
                 "display_categories":["optimization"],"downloads":9,"icon_url":""},
                {"project_id":"B","title":"Weird","project_type":"plugin"},
                {"project_type":"mod","title":"No id"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(page.hits.len(), 1);
        let hit = &page.hits[0];
        assert_eq!(hit.categories, ["optimization"]);
        assert_eq!(hit.icon_url, None);
        assert_eq!(hit.page_url(), "https://modrinth.com/mod/sodium");
        assert_eq!(page.page_count(20), 3);
        assert_eq!(SearchPage::default().page_count(20), 0);
    }

    #[test]
    fn decodes_projects_and_rejects_unsupported_ones() {
        let project = decode_project(
            br##"{"id":"A","slug":"s","title":"T","project_type":"resourcepack","body":"# hi"}"##,
        )
        .unwrap();
        assert_eq!(project.kind, ProjectKind::ResourcePack);
        assert_eq!(project.page_url(), "https://modrinth.com/resourcepack/s");
        assert!(decode_project(br#"{"id":"A","project_type":"plugin"}"#).is_err());
        assert!(decode_project(b"nope").is_err());
    }

    #[test]
    fn versions_without_files_are_dropped_and_unknown_dependency_kinds_survive() {
        let versions = decode_versions(
            br#"[
                {"id":"empty","files":[]},
                {"id":"v","project_id":"P","version_type":"strange",
                 "files":[{"url":"u","filename":"f.jar","hashes":{}}],
                 "dependencies":[{"project_id":"D","dependency_type":"required"},
                                 {"project_id":"E","dependency_type":"future"}]}
            ]"#,
        )
        .unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].channel, ReleaseChannel::Release);
        assert_eq!(versions[0].dependencies[1].kind, DependencyKind::Other);
        let required: Vec<_> = versions[0].required_dependencies().collect();
        assert_eq!(required.len(), 1);
    }

    #[test]
    fn the_primary_file_wins_over_the_first() {
        let version = decode_versions(
            br#"[{"id":"v","files":[
                {"url":"u1","filename":"sources.jar","primary":false,"hashes":{}},
                {"url":"u2","filename":"mod.jar","primary":true,"hashes":{}}]}]"#,
        )
        .unwrap()
        .remove(0);
        assert_eq!(version.install_file().unwrap().filename, "mod.jar");
    }

    #[test]
    fn picks_the_newest_stable_compatible_version() {
        let all = versions();
        let picked = pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Fabric).unwrap();
        // The newer beta and the forge/other-game builds do not qualify.
        assert_eq!(picked.id, "new-release");
        let forge = pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Forge).unwrap();
        assert_eq!(forge.id, "forge-only");
        assert!(pick_version(&all, ProjectKind::Mod, "1.19", Loader::Fabric).is_none());
        // Without a stable build the beta is used.
        let betas: Vec<_> = all
            .iter()
            .filter(|v| v.id == "newest-beta")
            .cloned()
            .collect();
        assert_eq!(
            pick_version(&betas, ProjectKind::Mod, "1.21.1", Loader::Fabric)
                .unwrap()
                .id,
            "newest-beta"
        );
    }

    #[test]
    fn loaders_do_not_filter_resource_packs() {
        let list = format!(
            "[{}]",
            version(
                "rp",
                "release",
                "2024-01-01T00:00:00Z",
                &["1.21.1"],
                &["minecraft"]
            )
        );
        let all = decode_versions(list.as_bytes()).unwrap();
        assert!(pick_version(&all, ProjectKind::ResourcePack, "1.21.1", Loader::Fabric).is_some());
        assert!(pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Fabric).is_none());
    }

    #[test]
    fn install_requests_target_the_right_folder_and_verify_the_file() {
        let all = versions();
        let version = pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Fabric).unwrap();
        let game = Path::new("/lib/instances/x/game");
        let request = install_request(
            ProjectKind::Mod,
            version,
            game,
            vec!["https://cdn/a".into()],
        )
        .unwrap();
        assert_eq!(request.destination(), game.join("mods/new-release.jar"));
        assert_eq!(request.expected_sha1(), Some(SHA1));
        assert_eq!(request.id(), "content:P:new-release");
        let shader = install_request(
            ProjectKind::Shader,
            version,
            game,
            vec!["https://cdn/a".into()],
        )
        .unwrap();
        assert_eq!(
            shader.destination(),
            game.join("shaderpacks/new-release.jar")
        );
        assert_eq!(
            install_request(ProjectKind::Modpack, version, game, vec!["u".into()]).unwrap_err(),
            IntentError::NotAFileKind
        );
    }

    #[test]
    fn unsafe_file_names_are_refused() {
        for name in [
            "../evil.jar",
            "a/b.jar",
            "a\\b.jar",
            "..",
            "c:evil.jar",
            " pad.jar",
        ] {
            let mut version = versions().remove(0);
            version.files[0].filename = name.to_owned();
            assert_eq!(
                install_request(
                    ProjectKind::Mod,
                    &version,
                    Path::new("/g"),
                    vec!["u".into()]
                )
                .unwrap_err(),
                IntentError::UnsafeFileName(name.to_owned()),
                "{name}"
            );
        }
    }

    struct Routes(BTreeMap<String, Vec<u8>>);

    impl Transport for Routes {
        fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
            Box::pin(async move {
                match self.0.get(source) {
                    Some(body) => Ok(TransportResponse::from_bytes(200, body.clone())),
                    None => Ok(TransportResponse::from_bytes(404, Vec::new())),
                }
            })
        }
    }

    #[tokio::test]
    async fn the_client_hits_the_documented_endpoints() {
        let mut query = SearchQuery::new(ProjectKind::Mod);
        query.text = "x".to_owned();
        let mut routes = BTreeMap::new();
        routes.insert(
            query.url().replacen(API_BASE, "http://mock", 1),
            br#"{"total_hits":1,"hits":[{"project_id":"A","title":"T","project_type":"mod"}]}"#
                .to_vec(),
        );
        routes.insert(
            "http://mock/v2/project/A".to_owned(),
            br#"{"id":"A","project_type":"mod","title":"T"}"#.to_vec(),
        );
        routes.insert(
            "http://mock/v2/project/A/version?include_changelog=false".to_owned(),
            format!(
                "[{}]",
                version(
                    "v1",
                    "release",
                    "2024-01-01T00:00:00Z",
                    &["1.21.1"],
                    &["fabric"]
                )
            )
            .into_bytes(),
        );
        let client = ModrinthClient::new(Routes(routes)).with_base("http://mock/");
        assert_eq!(client.search(&query).await.unwrap().hits.len(), 1);
        assert_eq!(client.project("A").await.unwrap().title, "T");
        assert_eq!(client.versions("A").await.unwrap().len(), 1);
        assert!(matches!(
            client.project("missing").await,
            Err(DiscoverError::Fetch(_))
        ));
    }

    #[test]
    fn modpacks_filter_by_loader_too() {
        let mut query = SearchQuery::new(ProjectKind::Modpack);
        query.loaders = vec!["neoforge".to_owned()];
        let url = Url::parse(&query.url()).unwrap();
        let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            pairs["facets"],
            r#"[["project_type:modpack"],["categories:neoforge"]]"#
        );
    }

    #[test]
    fn hits_carry_stats_environment_and_split_loaders_from_categories() {
        let page = decode_search(
            br#"{"total_hits": 1, "hits": [
                {"project_id":"A","slug":"fo","title":"FO","author":"me","project_type":"modpack",
                 "display_categories":["lightweight","fabric","multiplayer"],
                 "downloads":17731400,"follows":4886,"date_modified":"2026-09-27T10:00:00Z",
                 "client_side":"required","server_side":"optional"}]}"#,
        )
        .unwrap();
        let hit = &page.hits[0];
        assert_eq!(hit.categories, ["lightweight", "multiplayer"]);
        assert_eq!(hit.loaders, ["fabric"]);
        assert_eq!((hit.downloads, hit.follows), (17_731_400, 4886));
        assert_eq!(hit.updated, "2026-09-27T10:00:00Z");
        assert_eq!(
            environment(hit.client_side, hit.server_side),
            Some(Environment::ClientAndServer)
        );
    }

    #[test]
    fn environment_reads_the_two_side_flags() {
        use SideSupport::*;
        assert_eq!(
            environment(Required, Unsupported),
            Some(Environment::ClientOnly)
        );
        assert_eq!(
            environment(Unsupported, Required),
            Some(Environment::ServerOnly)
        );
        assert_eq!(
            environment(Optional, Optional),
            Some(Environment::ClientAndServer)
        );
        // Not knowing is not the same as not running there.
        assert_eq!(environment(Unknown, Unknown), None);
        assert_eq!(environment(Required, Unknown), None);
        assert_eq!(environment(Unsupported, Unsupported), None);
    }

    #[test]
    fn a_project_decodes_gallery_links_license_and_dates() {
        let project = decode_project(
            br##"{"id":"P","slug":"sodium","title":"Sodium","description":"fast","body":"# hi",
                "project_type":"mod","categories":["optimization"],"loaders":["fabric"],
                "downloads":5,"followers":9,"published":"2020-01-01T00:00:00Z",
                "updated":"2026-01-01T00:00:00Z","client_side":"required","server_side":"unsupported",
                "license":{"id":"LGPL-3.0","name":"GNU LGPL v3"},
                "source_url":"https://github.com/x/y","issues_url":"","wiki_url":null,
                "gallery":[
                  {"url":"https://cdn/b.png","title":"B","featured":false,"ordering":1},
                  {"url":"","title":"no address"},
                  {"url":"https://cdn/a.png","title":"A","description":"first","featured":true,"ordering":5},
                  {"url":"https://cdn/c.png","featured":false,"ordering":0}]}"##,
        )
        .unwrap();
        assert_eq!(project.followers, 9);
        assert_eq!(project.license.as_deref(), Some("GNU LGPL v3"));
        assert_eq!(
            project.links.source.as_deref(),
            Some("https://github.com/x/y")
        );
        assert_eq!(project.links.issues, None, "an empty address is no link");
        assert_eq!(project.links.wiki, None);
        let order: Vec<_> = project.gallery.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(
            order,
            [
                "https://cdn/a.png",
                "https://cdn/c.png",
                "https://cdn/b.png"
            ],
            "featured first, then the author's order; unaddressed images dropped"
        );
        assert_eq!(project.gallery[0].description, "first");
    }

    #[test]
    fn tags_decode_and_unsupported_types_are_dropped() {
        let categories = decode_categories(
            br#"[{"icon":"<svg/>","name":"adventure","project_type":"modpack","header":"categories"},
                 {"name":"plugin-thing","project_type":"plugin","header":"categories"},
                 {"name":"","project_type":"mod","header":"categories"},
                 {"name":"lightweight","project_type":"mod","header":"categories"}]"#,
        )
        .unwrap();
        assert_eq!(categories.len(), 2);
        assert_eq!(categories[0].kind, ProjectKind::Modpack);
        assert_eq!(categories[1].name, "lightweight");

        let versions = decode_game_versions(
            br#"[{"version":"26.3","version_type":"release","date":"2026-09-01T00:00:00Z","major":true},
                 {"version":"26.4-pre1","version_type":"snapshot","date":"2026-09-20T00:00:00Z"},
                 {"version":"","version_type":"release"}]"#,
        )
        .unwrap();
        assert_eq!(versions.len(), 2);
        assert!(versions[0].release);
        assert!(!versions[1].release);
    }

    #[test]
    fn the_owner_is_the_member_with_the_owner_role() {
        let owner = decode_owner(
            br#"[{"role":"Member","user":{"username":"helper"}},
                 {"role":"Owner","user":{"username":"jelly"}}]"#,
        )
        .unwrap();
        assert_eq!(owner.as_deref(), Some("jelly"));
        assert_eq!(decode_owner(b"[]").unwrap(), None);
    }
}
