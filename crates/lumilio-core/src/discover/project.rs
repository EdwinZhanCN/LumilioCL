use super::kinds::{ProjectKind, project_page_url};
use super::search::SideSupport;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GalleryImage {
    /// A small preview (about 350 px wide).
    pub url: String,
    /// The original file; the preview when Modrinth gives none.
    pub full_url: String,
    pub title: String,
    pub description: String,
    pub featured: bool,
    pub ordering: i64,
    /// RFC 3339, empty when unknown.
    pub created: String,
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
