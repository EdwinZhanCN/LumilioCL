//! Modrinth's API protocol. All network I/O is supplied by the plugin host;
//! installation, hashing, mirrors and proxies remain the launcher's concern.

mod project;
mod protocol;
mod query;
mod search;
mod tags;
mod text;
mod versions;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use lumilio_plugin_api::content::*;
use lumilio_plugin_api::{
    API_VERSION, FetchRequest, HostContext, Manifest, Permission, Plugin, PluginError,
};
use query::Query;

pub const ID: &str = "lumilio.modrinth";
const API_BASE: &str = "https://api.modrinth.com";

pub struct Modrinth;

impl Plugin for Modrinth {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID.into(),
            name: text::name(),
            description: text::description(),
            version: env!("CARGO_PKG_VERSION").into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![Permission::Network {
                hosts: vec!["api.modrinth.com".into()],
            }],
            settings: Vec::new(),
        }
    }

    fn content_source(&self) -> Option<&dyn ContentSource> {
        Some(self)
    }
}

impl ContentSource for Modrinth {
    fn capabilities(&self) -> Capabilities {
        let picks = || {
            Some(PickSupport {
                max_included: u32::MAX,
                exclude: true,
                any: true,
                all: true,
            })
        };
        Capabilities {
            kinds: [
                ProjectKind::Mod,
                ProjectKind::Modpack,
                ProjectKind::ResourcePack,
                ProjectKind::Shader,
            ]
            .into_iter()
            .map(|kind| KindSupport {
                kind,
                filters: FilterSupport {
                    max_game_versions: u32::MAX,
                    categories: picks(),
                    loaders: (kind != ProjectKind::ResourcePack).then(picks).flatten(),
                    environment: matches!(kind, ProjectKind::Mod | ProjectKind::Modpack),
                    open_source: true,
                    hidden_projects: true,
                    excluded_disclosures: true,
                    excluded_types: true,
                },
                sorts: vec![
                    Sort::Relevance,
                    Sort::Downloads,
                    Sort::Follows,
                    Sort::Newest,
                    Sort::Updated,
                ],
            })
            .collect(),
            fingerprints: vec![FingerprintKind::Sha1],
        }
    }

    fn search(
        &self,
        ctx: &dyn HostContext,
        query: &SearchQuery,
    ) -> Result<SearchPage, PluginError> {
        query.validate(&self.capabilities())?;
        search::decode_search(&get(ctx, query.url())?)
    }

    fn project(&self, ctx: &dyn HostContext, id: &str) -> Result<Project, PluginError> {
        let id = segment(id)?;
        let mut project = project::decode_project(&get(ctx, endpoint(&format!("project/{id}")))?)?;
        // Missing authors are decoration, not a reason to hide a project.
        project.author = get(ctx, endpoint(&format!("project/{id}/members")))
            .and_then(|bytes| tags::decode_owner(&bytes))
            .ok()
            .flatten();
        Ok(project)
    }

    fn versions(
        &self,
        ctx: &dyn HostContext,
        project: &str,
        changelog: bool,
    ) -> Result<Vec<Version>, PluginError> {
        versions::decode_versions(&get(
            ctx,
            endpoint(&format!(
                "project/{}/version?include_changelog={changelog}",
                segment(project)?
            )),
        )?)
    }

    fn version_files(
        &self,
        ctx: &dyn HostContext,
        version: &VersionRef,
    ) -> Result<Vec<VersionFile>, PluginError> {
        Ok(read_version(ctx, version)?.files)
    }

    fn dependencies(
        &self,
        ctx: &dyn HostContext,
        version: &VersionRef,
    ) -> Result<Vec<Dependency>, PluginError> {
        Ok(read_version(ctx, version)?.dependencies)
    }

    fn filter_choices(&self, ctx: &dyn HostContext) -> Result<FilterChoices, PluginError> {
        Ok(FilterChoices {
            categories: tags::decode_categories(&get(ctx, endpoint("tag/category"))?)?,
            game_versions: tags::decode_game_versions(&get(ctx, endpoint("tag/game_version"))?)?,
            // The original loader list only sharpens the choices.
            loaders: get(ctx, endpoint("tag/loader"))
                .and_then(|bytes| tags::decode_loaders(&bytes))
                .unwrap_or_default(),
        })
    }

    fn identify(
        &self,
        ctx: &dyn HostContext,
        files: &[FileIdentity],
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        identify(ctx, files, None)
    }

    fn latest_for(
        &self,
        ctx: &dyn HostContext,
        files: &[FileIdentity],
        compatibility: &Compatibility,
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        identify(ctx, files, Some(compatibility))
    }

    fn project_summaries(
        &self,
        ctx: &dyn HostContext,
        projects: &[String],
    ) -> Result<Vec<ProjectSummary>, PluginError> {
        if projects.is_empty() {
            return Ok(Vec::new());
        }
        let bytes = get(
            ctx,
            endpoint(&format!("projects?ids={}", tags::encoded_list(projects))),
        )?;
        let raw: Vec<tags::RawSummary> = protocol::decode(&bytes)?;
        let mut summaries = tags::decode_project_summaries(&bytes)?;
        let teams_by_project: BTreeMap<_, _> = raw
            .into_iter()
            .filter_map(|raw| Some((raw.id?, raw.team?)))
            .collect();
        let mut teams: Vec<_> = teams_by_project.values().cloned().collect();
        teams.sort();
        teams.dedup();
        if !teams.is_empty() {
            let authors = get(
                ctx,
                endpoint(&format!("teams?ids={}", tags::encoded_list(&teams))),
            )
            .and_then(|bytes| tags::decode_team_authors(&bytes))
            .unwrap_or_default();
            for summary in &mut summaries {
                summary.author = teams_by_project
                    .get(&summary.id)
                    .and_then(|team| authors.get(team))
                    .cloned();
            }
        }
        Ok(summaries)
    }
}

fn endpoint(path: &str) -> String {
    format!("{API_BASE}/v2/{path}")
}

// Source IDs are path segments, never URL paths or query fragments.
fn segment(id: &str) -> Result<String, PluginError> {
    if id.is_empty() || id == "." || id == ".." {
        return Err(PluginError::InvalidInput("invalid Modrinth ID".into()));
    }
    Ok(id
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect())
}

fn get(ctx: &dyn HostContext, url: String) -> Result<Vec<u8>, PluginError> {
    response(ctx.request(FetchRequest::get(url))?)
}

fn response(response: lumilio_plugin_api::FetchResponse) -> Result<Vec<u8>, PluginError> {
    if !(200..300).contains(&response.status) {
        return Err(PluginError::Transient(format!(
            "Modrinth returned HTTP {}",
            response.status
        )));
    }
    Ok(response.body)
}

fn read_version(ctx: &dyn HostContext, reference: &VersionRef) -> Result<Version, PluginError> {
    let raw = protocol::decode(&get(
        ctx,
        endpoint(&format!("version/{}", segment(&reference.version_id)?)),
    )?)?;
    let version = versions::version_from_raw(raw).ok_or_else(|| {
        PluginError::Unavailable("Modrinth version has no installable files".into())
    })?;
    if version.id != reference.version_id
        || (!reference.project_id.is_empty() && version.project_id != reference.project_id)
    {
        return Err(PluginError::Unavailable(
            "Modrinth returned a different version or project".into(),
        ));
    }
    Ok(version)
}

fn identify(
    ctx: &dyn HostContext,
    files: &[FileIdentity],
    compatibility: Option<&Compatibility>,
) -> Result<BTreeMap<String, Version>, PluginError> {
    let mut by_hash: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in files {
        let hash = file
            .fingerprints
            .iter()
            .find_map(|fingerprint| match fingerprint {
                Fingerprint::Sha1(hash) => Some(hash.to_ascii_lowercase()),
                _ => None,
            })
            .ok_or_else(|| {
                PluginError::InvalidInput("Modrinth requires a SHA-1 fingerprint".into())
            })?;
        by_hash.entry(hash).or_default().push(file.key.clone());
    }
    if by_hash.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut body =
        serde_json::json!({"hashes": by_hash.keys().collect::<Vec<_>>(), "algorithm": "sha1"});
    let path = if let Some(compatibility) = compatibility {
        body["loaders"] = serde_json::json!(compatibility.loader.iter().collect::<Vec<_>>());
        body["game_versions"] = serde_json::json!([compatibility.game_version]);
        "version_files/update"
    } else {
        "version_files"
    };
    let bytes = response(ctx.request(FetchRequest::json(
        endpoint(path),
        body.to_string().into_bytes(),
    ))?)?;
    let versions = versions::decode_version_map(&bytes)?;
    let mut found = BTreeMap::new();
    for (hash, version) in versions {
        if let Some(keys) = by_hash.get(&hash.to_ascii_lowercase()) {
            for key in keys {
                found.insert(key.clone(), version.clone());
            }
        }
    }
    Ok(found)
}
