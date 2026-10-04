use super::API_BASE;
use super::error::{DiscoverError, decode};
use super::kinds::{ProjectKind, SortIndex, project_page_url};
use serde::Deserialize;
use url::Url;

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
    pub(super) fn facets(&self) -> Vec<Vec<String>> {
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
    pub(super) fn from_protocol(name: Option<&str>) -> Self {
        match name {
            Some("required") => Self::Required,
            Some("optional") => Self::Optional,
            Some("unsupported") => Self::Unsupported,
            _ => Self::Unknown,
        }
    }

    pub(super) const fn runs(self) -> bool {
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
pub(super) const MOD_LOADERS: [&str; 8] = [
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
pub(super) struct RawSearch {
    #[serde(default)]
    pub(super) hits: Vec<RawHit>,
    #[serde(default)]
    pub(super) total_hits: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawHit {
    pub(super) project_id: Option<String>,
    pub(super) slug: Option<String>,
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) description: String,
    #[serde(default)]
    pub(super) author: String,
    pub(super) project_type: Option<String>,
    #[serde(default)]
    pub(super) categories: Vec<String>,
    #[serde(default)]
    pub(super) display_categories: Vec<String>,
    #[serde(default)]
    pub(super) downloads: u64,
    #[serde(default)]
    pub(super) follows: u64,
    #[serde(default)]
    pub(super) date_modified: String,
    pub(super) client_side: Option<String>,
    pub(super) server_side: Option<String>,
    pub(super) icon_url: Option<String>,
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
