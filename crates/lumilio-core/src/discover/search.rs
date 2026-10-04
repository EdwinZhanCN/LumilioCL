use super::error::{DiscoverError, decode};
use super::kinds::{ProjectKind, project_page_url};
use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    /// The organization when the project belongs to one, else its author.
    pub author: String,
    pub kind: ProjectKind,
    /// Display categories, without the loaders.
    pub categories: Vec<String>,
    /// Every category the project has, without the loaders; what a card folds
    /// into its overflow tag.
    pub all_categories: Vec<String>,
    /// Loaders the project supports (`fabric`, `forge`, …); a modpack's are
    /// those of the pack, not the pack format.
    pub loaders: Vec<String>,
    pub downloads: u64,
    pub follows: u64,
    /// RFC 3339 time the project was published.
    pub published: String,
    /// RFC 3339 time of the last change.
    pub updated: String,
    pub environment: Option<Environment>,
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
}

/// Where a project can be used, in the words Modrinth's cards use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Environment {
    ClientOrServer,
    ClientAndServer,
    ClientOnly,
    ServerOnly,
    SingleplayerOnly,
    DedicatedServerOnly,
}

impl Environment {
    /// Reads the environment value of a v3 project (`client_only`, …).
    fn from_protocol(name: &str) -> Option<Self> {
        Some(match name {
            "client_or_server" | "client_or_server_prefers_both" => Self::ClientOrServer,
            "client_and_server" => Self::ClientAndServer,
            "client_only" | "client_only_server_optional" => Self::ClientOnly,
            "server_only" | "server_only_client_optional" => Self::ServerOnly,
            "singleplayer_only" => Self::SingleplayerOnly,
            "dedicated_server_only" => Self::DedicatedServerOnly,
            _ => return None,
        })
    }
}

/// Summarizes the two side flags (a v2 project). Unknown on either side says
/// nothing.
#[must_use]
pub fn environment(client: SideSupport, server: SideSupport) -> Option<Environment> {
    use SideSupport::{Optional, Required, Unsupported};
    match (client, server) {
        (Optional, Optional) => Some(Environment::ClientOrServer),
        (Required, Required) => Some(Environment::ClientAndServer),
        (Optional | Required, Optional | Unsupported) => Some(Environment::ClientOnly),
        (Optional | Unsupported, Optional | Required) => Some(Environment::ServerOnly),
        _ => None,
    }
}

/// Names Modrinth lists next to categories that are loaders, platforms or
/// shader loaders.
pub(super) const LOADER_NAMES: [&str; 28] = [
    "fabric",
    "forge",
    "neoforge",
    "quilt",
    "liteloader",
    "rift",
    "modloader",
    "babric",
    "bta-babric",
    "legacy-fabric",
    "nilloader",
    "ornithe",
    "java-agent",
    "paper",
    "purpur",
    "spigot",
    "bukkit",
    "folia",
    "sponge",
    "bungeecord",
    "velocity",
    "waterfall",
    "geyser",
    "datapack",
    "iris",
    "optifine",
    "canvas",
    "vanilla",
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

#[derive(Debug, Default, Deserialize)]
pub(super) struct RawFields {
    #[serde(default)]
    environment: Vec<String>,
    #[serde(default)]
    mrpack_loaders: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawHit {
    pub(super) project_id: Option<String>,
    pub(super) slug: Option<String>,
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) summary: String,
    #[serde(default)]
    pub(super) author: String,
    pub(super) organization: Option<String>,
    #[serde(default)]
    pub(super) project_types: Vec<String>,
    #[serde(default)]
    pub(super) categories: Vec<String>,
    #[serde(default)]
    pub(super) display_categories: Vec<String>,
    #[serde(default)]
    pub(super) loaders: Vec<String>,
    #[serde(default)]
    pub(super) downloads: u64,
    #[serde(default)]
    pub(super) follows: u64,
    #[serde(default)]
    pub(super) date_created: String,
    #[serde(default)]
    pub(super) date_modified: String,
    pub(super) icon_url: Option<String>,
    pub(super) project_loader_fields: Option<RawFields>,
}

fn is_loader(name: &str) -> bool {
    LOADER_NAMES.contains(&name)
}

/// Decodes a search answer. Hits of an unknown project type or without an id
/// or title are dropped, not fatal.
pub fn decode_search(bytes: &[u8]) -> Result<SearchPage, DiscoverError> {
    let raw: RawSearch = decode(bytes)?;
    let hits = raw
        .hits
        .into_iter()
        .filter_map(|hit| {
            let kind = hit
                .project_types
                .iter()
                .find_map(|name| ProjectKind::from_protocol(name))?;
            let project_id = hit.project_id.filter(|id| !id.is_empty())?;
            let title = hit.name.filter(|title| !title.is_empty())?;
            let fields = hit.project_loader_fields.unwrap_or_default();
            let mut loaders: Vec<String> = Vec::new();
            for name in &hit.loaders {
                let named = if name == "mrpack" {
                    fields.mrpack_loaders.iter()
                } else {
                    std::slice::from_ref(name).iter()
                };
                for loader in named {
                    if !loaders.contains(loader) {
                        loaders.push(loader.clone());
                    }
                }
            }
            let shown = if hit.display_categories.is_empty() {
                &hit.categories
            } else {
                &hit.display_categories
            };
            let without_loaders = |names: &[String]| -> Vec<String> {
                names
                    .iter()
                    .filter(|name| !is_loader(name))
                    .cloned()
                    .collect()
            };
            Some(SearchHit {
                slug: hit
                    .slug
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| project_id.clone()),
                project_id,
                title,
                description: hit.summary,
                author: hit
                    .organization
                    .filter(|name| !name.is_empty())
                    .unwrap_or(hit.author),
                kind,
                categories: without_loaders(shown),
                all_categories: without_loaders(&hit.categories),
                loaders,
                downloads: hit.downloads,
                follows: hit.follows,
                published: hit.date_created,
                updated: hit.date_modified,
                environment: fields
                    .environment
                    .first()
                    .and_then(|name| Environment::from_protocol(name)),
                icon_url: hit.icon_url.filter(|url| !url.is_empty()),
            })
        })
        .collect();
    Ok(SearchPage {
        hits,
        total_hits: raw.total_hits,
    })
}
