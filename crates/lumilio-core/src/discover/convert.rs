//! Conversions between the launcher's content types and the plugin API's.

use super::kinds::{ProjectKind, SortIndex};
use super::project::{GalleryImage, Project, ProjectLinks};
use super::query::{Pick, SearchQuery, Stance};
use super::search::{Environment, SearchHit, SearchPage, SideSupport};
use super::tags::{CategoryTag, GameVersionTag, LoaderTag, ProjectSummary};
use super::versions::{Dependency, DependencyKind, ReleaseChannel, Version, VersionFile};
use lumilio_plugin_api::content as api;

pub(super) fn kind_to_api(kind: ProjectKind) -> api::ProjectKind {
    match kind {
        ProjectKind::Mod => api::ProjectKind::Mod,
        ProjectKind::Modpack => api::ProjectKind::Modpack,
        ProjectKind::ResourcePack => api::ProjectKind::ResourcePack,
        ProjectKind::Shader => api::ProjectKind::Shader,
    }
}

pub(super) fn kind_from_api(kind: api::ProjectKind) -> ProjectKind {
    match kind {
        api::ProjectKind::Mod => ProjectKind::Mod,
        api::ProjectKind::Modpack => ProjectKind::Modpack,
        api::ProjectKind::ResourcePack => ProjectKind::ResourcePack,
        api::ProjectKind::Shader => ProjectKind::Shader,
    }
}

fn sort_to_api(sort: SortIndex) -> api::Sort {
    match sort {
        SortIndex::Relevance => api::Sort::Relevance,
        SortIndex::Downloads => api::Sort::Downloads,
        SortIndex::Follows => api::Sort::Follows,
        SortIndex::Newest => api::Sort::Newest,
        SortIndex::Updated => api::Sort::Updated,
    }
}

fn stance_to_api(stance: Stance) -> api::Stance {
    match stance {
        Stance::Include => api::Stance::Include,
        Stance::Exclude => api::Stance::Exclude,
    }
}

fn pick_to_api(pick: &Pick, any: bool) -> api::Pick {
    api::Pick {
        name: pick.name.clone(),
        stance: stance_to_api(pick.stance),
        any: any && pick.stance == Stance::Include || pick.any,
    }
}

/// Loaders and environment only count for the kinds that have them, as before
/// the source was a plugin; included loaders match as "any of" (ADR 0023).
pub(super) fn query_to_api(query: &SearchQuery) -> api::SearchQuery {
    let environment = query.has_environment();
    api::SearchQuery {
        text: query.text.clone(),
        kind: kind_to_api(query.kind),
        sort: sort_to_api(query.sort),
        page: query.page,
        page_size: query.page_size.clamp(1, 100),
        game_versions: query.game_versions.clone(),
        loaders: if query.has_loaders() {
            query
                .loaders
                .iter()
                .filter(|pick| !pick.name.trim().is_empty())
                .map(|pick| pick_to_api(pick, true))
                .collect()
        } else {
            Vec::new()
        },
        categories: query
            .categories
            .iter()
            .filter(|pick| !pick.name.trim().is_empty())
            .map(|pick| pick_to_api(pick, false))
            .collect(),
        client: environment && query.client,
        server: environment && query.server,
        open_source: query.open_source.map(stance_to_api),
        hidden_projects: query.hidden_projects.clone(),
        excluded_disclosures: query.excluded_disclosures.clone(),
        excluded_types: query.excluded_types.clone(),
    }
}

fn side_from_api(side: api::SideSupport) -> SideSupport {
    match side {
        api::SideSupport::Required => SideSupport::Required,
        api::SideSupport::Optional => SideSupport::Optional,
        api::SideSupport::Unsupported => SideSupport::Unsupported,
        api::SideSupport::Unknown => SideSupport::Unknown,
    }
}

fn environment_from_api(environment: api::Environment) -> Environment {
    match environment {
        api::Environment::ClientOrServer => Environment::ClientOrServer,
        api::Environment::ClientAndServer => Environment::ClientAndServer,
        api::Environment::ClientOnly => Environment::ClientOnly,
        api::Environment::ServerOnly => Environment::ServerOnly,
        api::Environment::SingleplayerOnly => Environment::SingleplayerOnly,
        api::Environment::DedicatedServerOnly => Environment::DedicatedServerOnly,
    }
}

pub(super) fn page_from_api(page: api::SearchPage) -> SearchPage {
    SearchPage {
        total_hits: page.total_hits,
        hits: page
            .hits
            .into_iter()
            .map(|hit| SearchHit {
                project_id: hit.project_id,
                slug: hit.slug,
                title: hit.title,
                description: hit.description,
                author: hit.author,
                kind: kind_from_api(hit.kind),
                categories: hit.categories,
                all_categories: hit.all_categories,
                loaders: hit.loaders,
                downloads: hit.downloads,
                follows: hit.follows,
                published: hit.published,
                updated: hit.updated,
                environment: hit.environment.map(environment_from_api),
                icon_url: hit.icon_url,
            })
            .collect(),
    }
}

/// The project and the author the source named for it, if any.
pub(super) fn project_from_api(project: api::Project) -> (Project, Option<String>) {
    let author = project.author;
    (
        Project {
            id: project.id,
            slug: project.slug,
            title: project.title,
            description: project.description,
            body: project.body,
            kind: kind_from_api(project.kind),
            categories: project.categories,
            loaders: project.loaders,
            downloads: project.downloads,
            followers: project.followers,
            published: project.published,
            updated: project.updated,
            client_side: side_from_api(project.client_side),
            server_side: side_from_api(project.server_side),
            license: project.license,
            links: ProjectLinks {
                source: project.links.source,
                issues: project.links.issues,
                wiki: project.links.wiki,
                discord: project.links.discord,
            },
            gallery: project
                .gallery
                .into_iter()
                .map(|image| GalleryImage {
                    url: image.url,
                    full_url: image.full_url,
                    title: image.title,
                    description: image.description,
                    featured: image.featured,
                    ordering: image.ordering,
                    created: image.created,
                })
                .collect(),
            icon_url: project.icon_url,
            game_versions: project.game_versions,
        },
        author,
    )
}

pub(super) fn version_from_api(version: api::Version) -> Version {
    Version {
        id: version.id,
        project_id: version.project_id,
        name: version.name,
        number: version.number,
        channel: match version.channel {
            api::ReleaseChannel::Alpha => ReleaseChannel::Alpha,
            api::ReleaseChannel::Beta => ReleaseChannel::Beta,
            api::ReleaseChannel::Release => ReleaseChannel::Release,
        },
        game_versions: version.game_versions,
        loaders: version.loaders,
        published: version.published,
        files: version
            .files
            .into_iter()
            .map(|file| VersionFile {
                url: file.url,
                filename: file.filename,
                primary: file.primary,
                size: file.size,
                sha1: file.sha1,
            })
            .collect(),
        dependencies: version
            .dependencies
            .into_iter()
            .map(|dependency| Dependency {
                project_id: dependency.project_id,
                version_id: dependency.version_id,
                kind: match dependency.kind {
                    api::DependencyKind::Required => DependencyKind::Required,
                    api::DependencyKind::Optional => DependencyKind::Optional,
                    api::DependencyKind::Embedded => DependencyKind::Embedded,
                    api::DependencyKind::Incompatible => DependencyKind::Incompatible,
                    api::DependencyKind::Other => DependencyKind::Other,
                },
            })
            .collect(),
        downloads: version.downloads,
        changelog: version.changelog,
    }
}

pub(super) fn category_from_api(tag: api::CategoryTag) -> CategoryTag {
    CategoryTag {
        name: tag.name,
        header: tag.header,
        kind: kind_from_api(tag.kind),
    }
}

pub(super) fn loader_from_api(tag: api::LoaderTag) -> LoaderTag {
    LoaderTag {
        name: tag.name,
        project_types: tag
            .kinds
            .into_iter()
            .map(|kind| kind_from_api(kind).protocol_name().to_owned())
            .chain(tag.other_kinds)
            .collect(),
    }
}

pub(super) fn game_version_from_api(tag: api::GameVersionTag) -> GameVersionTag {
    GameVersionTag {
        version: tag.version,
        release: tag.release,
        snapshot: tag.snapshot,
        published: tag.published,
    }
}

pub(super) fn summary_from_api(summary: api::ProjectSummary) -> ProjectSummary {
    ProjectSummary {
        id: summary.id,
        slug: summary.slug,
        title: summary.title,
        kind: summary.kind.map(kind_from_api),
        icon_url: summary.icon_url,
        author: summary.author,
    }
}
