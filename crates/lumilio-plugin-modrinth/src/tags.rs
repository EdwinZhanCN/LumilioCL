use crate::protocol::Kind;
use crate::protocol::decode;
use lumilio_plugin_api::PluginError;
use lumilio_plugin_api::content::ProjectKind;
use lumilio_plugin_api::content::{CategoryTag, GameVersionTag, LoaderTag, ProjectSummary};
use serde::Deserialize;

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
pub fn decode_project_summaries(bytes: &[u8]) -> Result<Vec<ProjectSummary>, PluginError> {
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
                author: None,
                id,
            })
        })
        .collect())
}

/// One author per team: the member whose role says owner, else the first
/// listed. Teams without a named member are left out.
pub fn decode_team_authors(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, String>, PluginError> {
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
pub fn decode_categories(bytes: &[u8]) -> Result<Vec<CategoryTag>, PluginError> {
    let raw: Vec<RawCategory> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|tag| {
            Some(CategoryTag {
                id: tag.name.clone().filter(|name| !name.is_empty())?,
                name: tag.name.filter(|name| !name.is_empty())?,
                header: tag.header,
                kind: ProjectKind::from_protocol(tag.project_type.as_deref()?)?,
            })
        })
        .collect())
}

pub fn decode_loaders(bytes: &[u8]) -> Result<Vec<LoaderTag>, PluginError> {
    let raw: Vec<RawLoader> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .filter_map(|tag| {
            Some(LoaderTag {
                name: tag.name.filter(|name| !name.is_empty())?,
                kinds: tag
                    .supported_project_types
                    .iter()
                    .filter_map(|name| ProjectKind::from_protocol(name))
                    .collect(),
            })
        })
        .collect())
}

pub fn decode_game_versions(bytes: &[u8]) -> Result<Vec<GameVersionTag>, PluginError> {
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

pub fn decode_owner(bytes: &[u8]) -> Result<Option<String>, PluginError> {
    let raw: Vec<RawMember> = decode(bytes)?;
    Ok(raw
        .into_iter()
        .find(|member| member.role.eq_ignore_ascii_case("owner"))
        .and_then(|member| member.user?.username))
}
