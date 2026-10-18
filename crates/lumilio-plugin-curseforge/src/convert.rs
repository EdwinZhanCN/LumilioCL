//! CurseForge's answers in the plugin API's shapes.

use lumilio_plugin_api::content::{
    CategoryTag, Dependency, GalleryImage, GameVersionTag, Project, ProjectLinks, ProjectSummary,
    ResolvedFile, SearchHit, SideSupport, Version, VersionFile,
};

use crate::protocol::{
    CLASS_RESOURCE_PACKS, RawCategory, RawFile, RawMinecraftVersion, RawMod, channel,
    dependency_kind, kind_of, pack_file_kind,
};
use crate::text;

/// CurseForge has no slugs it can be asked by, so the ID doubles as one.
fn handle(id: u64) -> String {
    id.to_string()
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

/// Category names shown on a project, without the class itself.
fn category_names(raw: &RawMod, locale: &str) -> Vec<String> {
    raw.categories
        .iter()
        .filter(|category| !category.is_class.unwrap_or(false))
        .map(|category| text::category(category.id, &category.name, locale))
        .collect()
}

/// A project as a search result. Projects of other classes are dropped.
pub(crate) fn search_hit(raw: RawMod, locale: &str) -> Option<SearchHit> {
    let kind = kind_of(raw.class_id)?;
    let categories = category_names(&raw, locale);
    Some(SearchHit {
        project_id: raw.id.to_string(),
        slug: handle(raw.id),
        description: raw.summary.clone(),
        author: raw.author().unwrap_or_default(),
        kind,
        all_categories: categories.clone(),
        categories,
        loaders: raw.loaders(),
        downloads: raw.download_count.max(0.0) as u64,
        follows: None,
        published: raw.date_created.clone(),
        updated: if raw.date_released.is_empty() {
            raw.date_modified.clone()
        } else {
            raw.date_released.clone()
        },
        environment: None,
        icon_url: raw.icon(),
        page_url: raw.website().unwrap_or_default().to_owned(),
        manual_download: raw.allow_mod_distribution == Some(false),
        title: raw.name,
    })
}

pub(crate) fn project(raw: RawMod, body: String, locale: &str) -> Option<Project> {
    let kind = kind_of(raw.class_id)?;
    let gallery = raw
        .screenshots
        .iter()
        .enumerate()
        .filter_map(|(at, shot)| {
            let full = non_empty(shot.url.clone())?;
            Some(GalleryImage {
                url: non_empty(shot.thumbnail_url.clone()).unwrap_or_else(|| full.clone()),
                full_url: full,
                title: shot.title.clone(),
                description: shot.description.clone(),
                featured: false,
                ordering: i64::try_from(at).unwrap_or(i64::MAX),
                created: String::new(),
            })
        })
        .collect();
    Some(Project {
        id: raw.id.to_string(),
        slug: handle(raw.id),
        description: raw.summary.clone(),
        body,
        kind,
        categories: category_names(&raw, locale),
        loaders: raw.loaders(),
        downloads: raw.download_count.max(0.0) as u64,
        followers: None,
        published: raw.date_created.clone(),
        updated: if raw.date_released.is_empty() {
            raw.date_modified.clone()
        } else {
            raw.date_released.clone()
        },
        client_side: SideSupport::Unknown,
        server_side: SideSupport::Unknown,
        license: None,
        links: ProjectLinks {
            source: non_empty(raw.links.source_url.clone()),
            issues: non_empty(raw.links.issues_url.clone()),
            wiki: non_empty(raw.links.wiki_url.clone()),
            discord: None,
        },
        gallery,
        icon_url: raw.icon(),
        game_versions: raw.game_versions(),
        page_url: raw.website().unwrap_or_default().to_owned(),
        author: raw.author(),
        manual_download: raw.allow_mod_distribution == Some(false),
        title: raw.name,
    })
}

/// The page where a person downloads one file of a project.
pub(crate) fn file_page(website: Option<&str>, file: u64) -> Option<String> {
    website.map(|site| format!("{}/files/{file}", site.trim_end_matches('/')))
}

fn version_file(raw: &RawFile, website: Option<&str>) -> Option<VersionFile> {
    if raw.file_name.is_empty() {
        return None;
    }
    Some(VersionFile {
        url: raw
            .download_url
            .clone()
            .filter(|url| url.starts_with("https://"))
            .unwrap_or_default(),
        filename: raw.file_name.clone(),
        primary: true,
        size: raw.file_length,
        sha1: raw.sha1(),
        download_page: file_page(website, raw.id),
    })
}

/// One file as a version. `website` is the project's page, for files the
/// author lets people download only there.
pub(crate) fn version(raw: &RawFile, website: Option<&str>) -> Option<Version> {
    let file = version_file(raw, website)?;
    let name = if raw.display_name.trim().is_empty() {
        raw.file_name.clone()
    } else {
        raw.display_name.trim().to_owned()
    };
    Some(Version {
        id: raw.id.to_string(),
        project_id: raw.mod_id.to_string(),
        number: name.clone(),
        name,
        channel: channel(raw.release_type),
        game_versions: raw.minecraft_versions(),
        loaders: raw.loaders(),
        published: raw.file_date.clone(),
        files: vec![file],
        dependencies: raw
            .dependencies
            .iter()
            .map(|dependency| Dependency {
                project_id: Some(dependency.mod_id.to_string()),
                version_id: None,
                kind: dependency_kind(dependency.relation_type),
            })
            .collect(),
        downloads: raw.download_count.max(0.0) as u64,
        changelog: String::new(),
    })
}

pub(crate) fn summary(raw: &RawMod) -> ProjectSummary {
    ProjectSummary {
        id: raw.id.to_string(),
        slug: handle(raw.id),
        title: raw.name.clone(),
        kind: kind_of(raw.class_id),
        icon_url: raw.icon(),
        author: raw.author(),
        page_url: raw.website().unwrap_or_default().to_owned(),
    }
}

pub(crate) fn resolved(raw: &RawFile, project: Option<&RawMod>) -> Option<ResolvedFile> {
    let website = project.and_then(RawMod::website);
    Some(ResolvedFile {
        project_id: raw.mod_id.to_string(),
        version_id: raw.id.to_string(),
        title: project
            .map(|project| project.name.clone())
            .unwrap_or_default(),
        kind: pack_file_kind(project.and_then(|project| project.class_id)),
        file: version_file(raw, website)?,
    })
}

/// Categories of the four kinds, by ID; resolutions get their own group as
/// on Modrinth.
pub(crate) fn categories(raw: Vec<RawCategory>, locale: &str) -> Vec<CategoryTag> {
    raw.into_iter()
        .filter(|category| !category.is_class.unwrap_or(false))
        .filter_map(|category| {
            let kind = kind_of(category.class_id)?;
            // `16x`, `128x`, `512x and Higher`.
            let resolution = category.class_id == Some(CLASS_RESOURCE_PACKS)
                && category.name.split(' ').next().is_some_and(|word| {
                    word.strip_suffix('x')
                        .is_some_and(|size| size.parse::<u32>().is_ok())
                });
            Some(CategoryTag {
                id: category.id.to_string(),
                name: text::category(category.id, &category.name, locale),
                header: if resolution {
                    "resolutions"
                } else {
                    "categories"
                }
                .to_owned(),
                kind,
            })
        })
        .collect()
}

/// Minecraft versions, newest first as CurseForge lists them.
pub(crate) fn game_versions(raw: Vec<RawMinecraftVersion>) -> Vec<GameVersionTag> {
    raw.into_iter()
        .filter(|version| !version.version_string.trim().is_empty())
        .map(|version| {
            let snapshot = version
                .version_string
                .to_ascii_lowercase()
                .contains("snapshot");
            GameVersionTag {
                release: !snapshot,
                snapshot,
                version: version.version_string,
                published: String::new(),
            }
        })
        .collect()
}
