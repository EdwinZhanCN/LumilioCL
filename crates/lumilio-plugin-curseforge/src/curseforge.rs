use std::collections::{BTreeMap, BTreeSet};

use lumilio_plugin_api::content::{
    Capabilities, Catalog, Compatibility, ContentSource, Dependency, FileIdentity, FilterChoices,
    FilterSupport, Fingerprint, FingerprintKind, KindSupport, PickSupport, Project, ProjectKind,
    ProjectSummary, ResolvedFile, SearchPage, SearchQuery, Sort, TextFormat, Version, VersionFile,
    VersionRef,
};
use lumilio_plugin_api::{
    API_VERSION, FetchRequest, FetchResponse, HostContext, Manifest, Permission, Plugin,
    PluginError,
};

use super::protocol::{Envelope, GAME, RawFile, RawMatches, RawMod, decode};
use super::{convert, protocol, query, text};

pub const ID: &str = "lumilio.curseforge";
pub(super) const HOST: &str = "api.curseforge.com";
pub(super) const API_BASE: &str = "https://api.curseforge.com";
/// Largest batch the plugin sends in one request.
pub(super) const BATCH: usize = 1000;
/// The file list is read in pages of this many; CurseForge's busiest
/// projects have a few thousand files.
pub(super) const FILE_PAGE: u64 = 1000;
/// Most file pages one call reads (10 000 files).
pub(super) const FILE_PAGES: u64 = 10;

pub struct CurseForge {
    pub(super) key: Option<String>,
}

impl Default for CurseForge {
    fn default() -> Self {
        Self::with_key(option_env!("CF_API_KEY"))
    }
}

impl CurseForge {
    /// The plugin with the given key; blank means none.
    #[must_use]
    pub fn with_key(key: Option<&str>) -> Self {
        Self {
            key: key
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(str::to_owned),
        }
    }

    /// Whether this build can talk to CurseForge's API.
    #[must_use]
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    pub(super) fn request(&self, request: FetchRequest) -> FetchRequest {
        let mut request = request;
        request
            .headers
            .push(("accept".into(), "application/json".into()));
        if let Some(key) = &self.key {
            request.headers.push(("x-api-key".into(), key.clone()));
        }
        request
    }

    pub(super) fn get(&self, ctx: &dyn HostContext, path: &str) -> Result<Vec<u8>, PluginError> {
        self.send(ctx, FetchRequest::get(format!("{API_BASE}{path}")))
    }

    pub(super) fn post(
        &self,
        ctx: &dyn HostContext,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<Vec<u8>, PluginError> {
        self.send(
            ctx,
            FetchRequest::json(format!("{API_BASE}{path}"), body.to_string().into_bytes()),
        )
    }

    pub(super) fn send(
        &self,
        ctx: &dyn HostContext,
        request: FetchRequest,
    ) -> Result<Vec<u8>, PluginError> {
        // A slow or absent network is the world's fault, not the plugin's.
        let answer = ctx
            .request(self.request(request))
            .map_err(|error| match error {
                PluginError::PermissionDenied => PluginError::PermissionDenied,
                other => PluginError::Transient(other.to_string()),
            })?;
        self.check(answer)
    }

    pub(super) fn check(&self, answer: FetchResponse) -> Result<Vec<u8>, PluginError> {
        match answer.status {
            200..=299 => Ok(answer.body),
            401 | 403 if self.key.is_none() => Err(PluginError::Transient(
                "CurseForge needs an API key, and this build of the launcher has none".into(),
            )),
            401 | 403 => Err(PluginError::Transient(format!(
                "CurseForge refused the launcher's API key (HTTP {})",
                answer.status
            ))),
            status => Err(PluginError::Transient(format!(
                "CurseForge returned HTTP {status}"
            ))),
        }
    }

    pub(super) fn read_mod(&self, ctx: &dyn HostContext, id: u64) -> Result<RawMod, PluginError> {
        let raw: Envelope<RawMod> = decode(&self.get(ctx, &format!("/v1/mods/{id}"))?)?;
        Ok(raw.data)
    }

    pub(super) fn read_mods(
        &self,
        ctx: &dyn HostContext,
        ids: &[u64],
    ) -> Result<Vec<RawMod>, PluginError> {
        let mut found = Vec::new();
        for batch in ids.chunks(BATCH) {
            let body = serde_json::json!({ "modIds": batch, "filterPcOnly": true });
            let raw: Envelope<Vec<RawMod>> = decode(&self.post(ctx, "/v1/mods", &body)?)?;
            found.extend(raw.data);
        }
        Ok(found)
    }

    pub(super) fn read_file(
        &self,
        ctx: &dyn HostContext,
        reference: &VersionRef,
    ) -> Result<RawFile, PluginError> {
        let (project, file) = (
            number(&reference.project_id)?,
            number(&reference.version_id)?,
        );
        let raw: Envelope<RawFile> =
            decode(&self.get(ctx, &format!("/v1/mods/{project}/files/{file}"))?)?;
        if raw.data.id != file || raw.data.mod_id != project {
            return Err(PluginError::Transient(
                "CurseForge returned a different file or project".into(),
            ));
        }
        Ok(raw.data)
    }

    pub(super) fn read_files(
        &self,
        ctx: &dyn HostContext,
        ids: &[u64],
    ) -> Result<Vec<RawFile>, PluginError> {
        let mut found = Vec::new();
        for batch in ids.chunks(BATCH) {
            let body = serde_json::json!({ "fileIds": batch });
            let raw: Envelope<Vec<RawFile>> = decode(&self.post(ctx, "/v1/mods/files", &body)?)?;
            found.extend(raw.data);
        }
        Ok(found)
    }

    /// Files whose fingerprints CurseForge knows, by fingerprint.
    pub(super) fn matches(
        &self,
        ctx: &dyn HostContext,
        fingerprints: &[u32],
    ) -> Result<BTreeMap<u32, RawFile>, PluginError> {
        let mut found = BTreeMap::new();
        for batch in fingerprints.chunks(BATCH) {
            let body = serde_json::json!({ "fingerprints": batch });
            let raw: Envelope<RawMatches> =
                decode(&self.post(ctx, &format!("/v1/fingerprints/{GAME}"), &body)?)?;
            for matched in raw.data.exact_matches {
                if let Ok(fingerprint) = u32::try_from(matched.file.file_fingerprint) {
                    found.insert(fingerprint, matched.file);
                }
            }
        }
        Ok(found)
    }

    /// The project's page for files that may only be downloaded there; one
    /// more request, only when some file needs it.
    pub(super) fn website_for(&self, ctx: &dyn HostContext, files: &[RawFile]) -> Option<String> {
        let project = files
            .iter()
            .find(|file| file.download_url.is_none())?
            .mod_id;
        self.read_mod(ctx, project)
            .ok()
            .and_then(|raw| raw.website().map(str::to_owned))
    }
}

impl Plugin for CurseForge {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID.into(),
            name: text::name(),
            description: text::description(self.has_key()),
            version: env!("CARGO_PKG_VERSION").into(),
            api: API_VERSION,
            // Without a key every request is refused; off until a build has one.
            default_enabled: self.has_key(),
            permissions: vec![Permission::Network {
                hosts: vec![HOST.into()],
            }],
            settings: Vec::new(),
        }
    }

    fn content_source(&self) -> Option<&dyn ContentSource> {
        Some(self)
    }
}

/// CurseForge IDs are numbers; anything else is not one of its projects.
/// Not a fault of the plugin, so the call fails without stopping it.
pub(super) fn number(id: &str) -> Result<u64, PluginError> {
    id.trim()
        .parse()
        .map_err(|_| PluginError::Transient(format!("{id:?} is not a CurseForge ID")))
}

pub(super) fn numbers(ids: &[String]) -> Vec<u64> {
    let mut numbers: Vec<u64> = ids.iter().filter_map(|id| id.trim().parse().ok()).collect();
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

impl ContentSource for CurseForge {
    fn capabilities(&self) -> Capabilities {
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
                    max_game_versions: query::MAX_GAME_VERSIONS,
                    // Several categories narrow the results: all of them.
                    categories: Some(PickSupport {
                        max_included: query::MAX_CATEGORIES,
                        exclude: false,
                        any: false,
                        all: true,
                    }),
                    // Several loaders widen them: any of them.
                    loaders: matches!(kind, ProjectKind::Mod | ProjectKind::Modpack).then_some(
                        PickSupport {
                            max_included: query::MAX_LOADERS,
                            exclude: false,
                            any: true,
                            all: false,
                        },
                    ),
                    environment: false,
                    open_source: false,
                    hidden_projects: false,
                    excluded_disclosures: false,
                    excluded_types: false,
                },
                sorts: vec![
                    Sort::Relevance,
                    Sort::Downloads,
                    Sort::Newest,
                    Sort::Updated,
                ],
                browse_url: Some(query::browse_url(kind)),
            })
            .collect(),
            fingerprints: vec![FingerprintKind::Murmur2],
            catalog: Some(Catalog::CurseForge),
            text_format: TextFormat::Html,
        }
    }

    fn search(
        &self,
        ctx: &dyn HostContext,
        query: &SearchQuery,
    ) -> Result<SearchPage, PluginError> {
        query.validate(&self.capabilities())?;
        let Some((start, wanted)) = query::window(query) else {
            // Past CurseForge's 10 000 results there is nothing more to show.
            return Ok(SearchPage {
                hits: Vec::new(),
                total_hits: query::RESULT_LIMIT,
            });
        };
        // CurseForge returns at most 50 at a time; a larger page is read in parts.
        let locale = ctx.locale().to_owned();
        let (mut hits, mut total, mut read) = (Vec::new(), 0, 0);
        while read < wanted {
            let size = (wanted - read).min(query::PAGE_LIMIT);
            let path = query::search_path(query, start + read, size);
            let raw: Envelope<Vec<RawMod>> = decode(&self.get(ctx, &path)?)?;
            total = raw
                .pagination
                .map_or(0, |pagination| pagination.total_count);
            let count = raw.data.len() as u64;
            hits.extend(
                raw.data
                    .into_iter()
                    .filter_map(|raw| convert::search_hit(raw, &locale)),
            );
            read += count;
            if count < size {
                break;
            }
        }
        Ok(SearchPage {
            hits,
            total_hits: total.min(query::RESULT_LIMIT),
        })
    }

    fn project(&self, ctx: &dyn HostContext, id: &str) -> Result<Project, PluginError> {
        let id = number(id)?;
        let raw = self.read_mod(ctx, id)?;
        // The long description is decoration: a project without one still shows.
        let body = self
            .get(ctx, &format!("/v1/mods/{id}/description"))
            .and_then(|bytes| decode::<Envelope<String>>(&bytes))
            .map(|raw| raw.data)
            .unwrap_or_default();
        convert::project(raw, body, ctx.locale()).ok_or_else(|| {
            PluginError::Transient("not a mod, modpack, resource pack or shader".into())
        })
    }

    fn versions(
        &self,
        ctx: &dyn HostContext,
        project: &str,
        _changelog: bool,
    ) -> Result<Vec<Version>, PluginError> {
        // Changelogs cost a request each; they are read one at a time with
        // `changelog`.
        let id = number(project)?;
        let website = self
            .read_mod(ctx, id)
            .ok()
            .and_then(|raw| raw.website().map(str::to_owned));
        let mut files: Vec<RawFile> = Vec::new();
        for page in 0..FILE_PAGES {
            let index = page * FILE_PAGE;
            let raw: Envelope<Vec<RawFile>> = decode(&self.get(
                ctx,
                &format!("/v1/mods/{id}/files?index={index}&pageSize={FILE_PAGE}"),
            )?)?;
            let total = raw
                .pagination
                .map_or(0, |pagination| pagination.total_count);
            let read = raw.data.len() as u64;
            files.extend(raw.data);
            if read < FILE_PAGE || index + read >= total {
                break;
            }
        }
        Ok(files
            .into_iter()
            .filter_map(|file| convert::version(&file, website.as_deref()))
            .collect())
    }

    fn version_files(
        &self,
        ctx: &dyn HostContext,
        version: &VersionRef,
    ) -> Result<Vec<VersionFile>, PluginError> {
        let file = self.read_file(ctx, version)?;
        let website = self.website_for(ctx, std::slice::from_ref(&file));
        Ok(convert::version(&file, website.as_deref())
            .map(|version| version.files)
            .unwrap_or_default())
    }

    fn dependencies(
        &self,
        ctx: &dyn HostContext,
        version: &VersionRef,
    ) -> Result<Vec<Dependency>, PluginError> {
        Ok(convert::version(&self.read_file(ctx, version)?, None)
            .map(|version| version.dependencies)
            .unwrap_or_default())
    }

    fn changelog(
        &self,
        ctx: &dyn HostContext,
        version: &VersionRef,
    ) -> Result<String, PluginError> {
        let (project, file) = (number(&version.project_id)?, number(&version.version_id)?);
        let raw: Envelope<String> =
            decode(&self.get(ctx, &format!("/v1/mods/{project}/files/{file}/changelog"))?)?;
        Ok(raw.data)
    }

    fn filter_choices(&self, ctx: &dyn HostContext) -> Result<FilterChoices, PluginError> {
        let categories: Envelope<Vec<protocol::RawCategory>> =
            decode(&self.get(ctx, &format!("/v1/categories?gameId={GAME}"))?)?;
        let versions: Envelope<Vec<protocol::RawMinecraftVersion>> =
            decode(&self.get(ctx, "/v1/minecraft/version")?)?;
        Ok(FilterChoices {
            categories: convert::categories(categories.data, ctx.locale()),
            game_versions: convert::game_versions(versions.data),
            loaders: query::loader_tags(),
        })
    }

    fn identify(
        &self,
        ctx: &dyn HostContext,
        files: &[FileIdentity],
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        let keys = keys_by_fingerprint(files)?;
        if keys.is_empty() {
            return Ok(BTreeMap::new());
        }
        let fingerprints: Vec<u32> = keys.keys().copied().collect();
        let found = self.matches(ctx, &fingerprints)?;
        let mut identified = BTreeMap::new();
        for (fingerprint, file) in found {
            let Some(version) = convert::version(&file, None) else {
                continue;
            };
            for key in keys.get(&fingerprint).into_iter().flatten() {
                identified.insert(key.clone(), version.clone());
            }
        }
        Ok(identified)
    }

    fn latest_for(
        &self,
        ctx: &dyn HostContext,
        files: &[FileIdentity],
        compatibility: &Compatibility,
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        let keys = keys_by_fingerprint(files)?;
        if keys.is_empty() {
            return Ok(BTreeMap::new());
        }
        let fingerprints: Vec<u32> = keys.keys().copied().collect();
        let installed = self.matches(ctx, &fingerprints)?;
        if installed.is_empty() {
            return Ok(BTreeMap::new());
        }
        let projects: Vec<u64> = installed
            .values()
            .map(|file| file.mod_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mods: BTreeMap<u64, RawMod> = self
            .read_mods(ctx, &projects)?
            .into_iter()
            .map(|raw| (raw.id, raw))
            .collect();
        // The newest compatible file of each project, if newer than the one
        // installed and no less stable.
        let mut newer: BTreeMap<u32, u64> = BTreeMap::new();
        for (fingerprint, file) in &installed {
            let Some(project) = mods.get(&file.mod_id) else {
                continue;
            };
            if let Some(chosen) = query::newest_compatible(project, file, compatibility) {
                newer.insert(*fingerprint, chosen);
            }
        }
        let wanted: Vec<u64> = newer
            .values()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if wanted.is_empty() {
            return Ok(BTreeMap::new());
        }
        let files: BTreeMap<u64, RawFile> = self
            .read_files(ctx, &wanted)?
            .into_iter()
            .map(|file| (file.id, file))
            .collect();
        let mut latest = BTreeMap::new();
        for (fingerprint, chosen) in newer {
            let Some(file) = files.get(&chosen) else {
                continue;
            };
            let website = mods.get(&file.mod_id).and_then(RawMod::website);
            let Some(version) = convert::version(file, website) else {
                continue;
            };
            for key in keys.get(&fingerprint).into_iter().flatten() {
                latest.insert(key.clone(), version.clone());
            }
        }
        Ok(latest)
    }

    fn project_summaries(
        &self,
        ctx: &dyn HostContext,
        projects: &[String],
    ) -> Result<Vec<ProjectSummary>, PluginError> {
        let ids = numbers(projects);
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .read_mods(ctx, &ids)?
            .iter()
            .map(convert::summary)
            .collect())
    }

    fn resolve_files(
        &self,
        ctx: &dyn HostContext,
        files: &[VersionRef],
    ) -> Result<Vec<ResolvedFile>, PluginError> {
        let wanted: BTreeMap<u64, u64> = files
            .iter()
            .filter_map(|reference| {
                Some((
                    reference.version_id.trim().parse().ok()?,
                    reference.project_id.trim().parse().ok()?,
                ))
            })
            .collect();
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<u64> = wanted.keys().copied().collect();
        let found: Vec<RawFile> = self
            .read_files(ctx, &ids)?
            .into_iter()
            .filter(|file| wanted.get(&file.id) == Some(&file.mod_id))
            .collect();
        let projects: Vec<u64> = found
            .iter()
            .map(|file| file.mod_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        // Names and types are needed to place files; without them the pack
        // cannot be installed correctly, so a failure here fails the call.
        let mods: BTreeMap<u64, RawMod> = self
            .read_mods(ctx, &projects)?
            .into_iter()
            .map(|raw| (raw.id, raw))
            .collect();
        Ok(found
            .iter()
            .filter_map(|file| convert::resolved(file, mods.get(&file.mod_id)))
            .collect())
    }
}

/// Correlation keys by the Murmur2 fingerprint the host computed.
pub(super) fn keys_by_fingerprint(
    files: &[FileIdentity],
) -> Result<BTreeMap<u32, Vec<String>>, PluginError> {
    let mut keys: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for file in files {
        let fingerprint = file
            .fingerprints
            .iter()
            .find_map(|fingerprint| match fingerprint {
                Fingerprint::Murmur2(value) => Some(*value),
                Fingerprint::Sha1(_) => None,
            })
            .ok_or_else(|| {
                PluginError::InvalidInput("CurseForge requires a Murmur2 fingerprint".into())
            })?;
        keys.entry(fingerprint).or_default().push(file.key.clone());
    }
    Ok(keys)
}
