use crate::protocol::decode;
use crate::protocol::{Kind, environment};
use lumilio_plugin_api::PluginError;
use lumilio_plugin_api::content::ProjectKind;
use lumilio_plugin_api::content::{SearchHit, SearchPage};
use serde::Deserialize;

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
pub fn decode_search(bytes: &[u8]) -> Result<SearchPage, PluginError> {
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
            let slug = hit
                .slug
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| project_id.clone());
            Some(SearchHit {
                page_url: format!("https://modrinth.com/{}/{slug}", kind.protocol_name()),
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
                    .and_then(|name| environment(name)),
                icon_url: hit.icon_url.filter(|url| !url.is_empty()),
            })
        })
        .collect();
    Ok(SearchPage {
        hits,
        total_hits: raw.total_hits,
    })
}
