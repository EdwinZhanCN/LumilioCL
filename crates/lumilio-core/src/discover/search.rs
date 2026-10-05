use super::kinds::{ProjectKind, project_page_url};

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
