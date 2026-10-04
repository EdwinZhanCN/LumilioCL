use super::error::{DiscoverError, decode};
use super::kinds::ProjectKind;
use serde::Deserialize;

/// A category a project can be filed under, for one project type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryTag {
    pub name: String,
    /// Modrinth's grouping, e.g. `categories`, `features`, `resolutions`.
    pub header: String,
    pub kind: ProjectKind,
}

/// A loader (or shader loader, or platform) and the project types it serves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoaderTag {
    pub name: String,
    /// Modrinth's own names (`mod`, `plugin`, `datapack`, …), not only ours.
    pub project_types: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameVersionTag {
    pub version: String,
    /// A full release, as opposed to a snapshot or pre-release.
    pub release: bool,
    /// A snapshot; what is neither this nor a release is a legacy version
    /// (alpha, beta, classic).
    pub snapshot: bool,
    pub published: String,
}

#[derive(Deserialize)]
pub(super) struct RawCategory {
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) header: String,
    pub(super) project_type: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawLoader {
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) supported_project_types: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct RawGameVersion {
    pub(super) version: Option<String>,
    #[serde(default)]
    pub(super) version_type: String,
    #[serde(default)]
    pub(super) date: String,
}

#[derive(Deserialize)]
pub(super) struct RawMember {
    #[serde(default)]
    pub(super) role: String,
    pub(super) user: Option<RawUser>,
    pub(super) team_id: Option<String>,
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
pub(super) struct RawSummary {
    pub(super) id: Option<String>,
    pub(super) slug: Option<String>,
    pub(super) title: Option<String>,
    pub(super) project_type: Option<String>,
    pub(super) icon_url: Option<String>,
    pub(super) team: Option<String>,
}

/// A JSON list of ids, percent-encoded for a query string.
pub(super) fn encoded_list(ids: &[String]) -> String {
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
pub(super) struct RawUser {
    pub(super) username: Option<String>,
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

pub fn decode_loaders(bytes: &[u8]) -> Result<Vec<LoaderTag>, DiscoverError> {
    let raw: Vec<RawLoader> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|tag| {
            Some(LoaderTag {
                name: tag.name.filter(|name| !name.is_empty())?,
                project_types: tag.supported_project_types,
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
                snapshot: tag.version_type == "snapshot",
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
