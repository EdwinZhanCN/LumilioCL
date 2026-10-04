use super::error::{DiscoverError, decode};
use super::kinds::{ProjectKind, project_page_url};
use super::search::SideSupport;
use serde::Deserialize;

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
pub(super) struct RawProject {
    pub(super) id: Option<String>,
    pub(super) slug: Option<String>,
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) description: String,
    #[serde(default)]
    pub(super) body: String,
    pub(super) project_type: Option<String>,
    #[serde(default)]
    pub(super) categories: Vec<String>,
    #[serde(default)]
    pub(super) loaders: Vec<String>,
    #[serde(default)]
    pub(super) downloads: u64,
    #[serde(default)]
    pub(super) followers: u64,
    #[serde(default)]
    pub(super) published: String,
    #[serde(default)]
    pub(super) updated: String,
    pub(super) client_side: Option<String>,
    pub(super) server_side: Option<String>,
    pub(super) license: Option<RawLicense>,
    pub(super) source_url: Option<String>,
    pub(super) issues_url: Option<String>,
    pub(super) wiki_url: Option<String>,
    pub(super) discord_url: Option<String>,
    #[serde(default)]
    pub(super) gallery: Vec<RawGallery>,
    pub(super) icon_url: Option<String>,
    #[serde(default)]
    pub(super) game_versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawLicense {
    pub(super) id: Option<String>,
    pub(super) name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawGallery {
    pub(super) url: Option<String>,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) featured: bool,
    #[serde(default)]
    pub(super) ordering: i64,
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

pub(super) fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

/// Images without an address are dropped; the featured one leads, the rest
/// keep the author's order.
pub(super) fn gallery_from_raw(raw: Vec<RawGallery>) -> Vec<GalleryImage> {
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
