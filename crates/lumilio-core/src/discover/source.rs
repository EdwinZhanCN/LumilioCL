//! The launcher's way to ask the enabled content sources (plugins) for
//! projects, versions and file identities. Nothing here knows a protocol.

use std::collections::BTreeMap;

use lumilio_plugin_api::content::{
    Capabilities, Compatibility, FileIdentity, Fingerprint, FingerprintKind, KindSupport,
    PickSupport, Sort, Stance as ApiStance, VersionRef,
};
use lumilio_plugin_api::{ContentSource, HostContext, PluginError};

use super::convert::{
    catalog_from_api, category_from_api, format_from_api, game_version_from_api, kind_from_api,
    loader_from_api, page_from_api, project_from_api, query_to_api, resolved_from_api,
    summary_from_api, version_from_api,
};
use super::error::DiscoverError;
use super::kinds::{Catalog, ProjectKind, SortIndex, TextFormat, loader_tag};
use super::project::Project;
use super::query::SearchQuery;
use super::search::SearchPage;
use super::tags::{CategoryTag, GameVersionTag, LoaderTag, ProjectSummary};
use super::versions::{ResolvedFile, Version};
use crate::instance::Loader;
use crate::plugins::{PluginContentSource, PluginHost, PluginStatus};

/// What the Discover filters offer, as the source describes it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceFilters {
    pub categories: Vec<CategoryTag>,
    pub game_versions: Vec<GameVersionTag>,
    pub loaders: Vec<LoaderTag>,
}

/// What a source can filter and sort by for one kind of project; Discover
/// offers only these controls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KindAbilities {
    pub kind: ProjectKind,
    pub sorts: Vec<SortIndex>,
    pub game_versions: bool,
    pub categories: bool,
    pub loaders: bool,
    pub environment: bool,
    pub open_source: bool,
    /// Leaving out what the game already has.
    pub hide_installed: bool,
    /// Excluding disclosure kinds and other project types.
    pub advanced: bool,
    /// Leaving out a picked category or loader.
    pub exclusions: bool,
    /// The source's own page listing this kind.
    pub browse_url: Option<String>,
}

/// What the launcher knows about one installed file, for asking sources
/// which project it belongs to. The SHA-1 is also the answer's key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileHashes {
    pub sha1: String,
    /// CurseForge's fingerprint; computed only for sources that need it.
    pub murmur2: Option<u32>,
}

impl FileHashes {
    #[must_use]
    pub fn sha1(sha1: impl Into<String>) -> Self {
        Self {
            sha1: sha1.into(),
            murmur2: None,
        }
    }
}

fn sort_from_api(sort: Sort) -> Option<SortIndex> {
    match sort {
        Sort::Relevance => Some(SortIndex::Relevance),
        Sort::Downloads => Some(SortIndex::Downloads),
        Sort::Follows => Some(SortIndex::Follows),
        Sort::Newest => Some(SortIndex::Newest),
        Sort::Updated => Some(SortIndex::Updated),
        Sort::Name | Sort::Author => None,
    }
}

fn abilities_from(capabilities: &Capabilities) -> Vec<KindAbilities> {
    capabilities
        .kinds
        .iter()
        .map(|support| KindAbilities {
            kind: kind_from_api(support.kind),
            sorts: support
                .sorts
                .iter()
                .copied()
                .filter_map(sort_from_api)
                .collect(),
            game_versions: support.filters.max_game_versions > 0,
            categories: support.filters.categories.is_some(),
            loaders: support.filters.loaders.is_some(),
            environment: support.filters.environment,
            open_source: support.filters.open_source,
            hide_installed: support.filters.hidden_projects,
            advanced: support.filters.excluded_disclosures || support.filters.excluded_types,
            exclusions: [&support.filters.categories, &support.filters.loaders]
                .into_iter()
                .flatten()
                .any(|picks| picks.exclude),
            browse_url: support.browse_url.clone(),
        })
        .collect()
}

/// Narrows picks to what the source can do: no exclusions it cannot make,
/// "any of" turned into "all of" (or the reverse) when only one is offered,
/// and no more than it takes. The search may come back narrower than asked,
/// never silently wider.
fn fit_picks(
    picks: Vec<lumilio_plugin_api::content::Pick>,
    support: Option<&PickSupport>,
) -> Vec<lumilio_plugin_api::content::Pick> {
    let Some(support) = support else {
        return Vec::new();
    };
    let mut picks: Vec<_> = picks
        .into_iter()
        .filter(|pick| !pick.name.trim().is_empty())
        .filter(|pick| support.exclude || pick.stance == ApiStance::Include)
        .collect();
    let any_count = picks
        .iter()
        .filter(|pick| pick.stance == ApiStance::Include && pick.any)
        .count();
    if any_count > 1 && !support.any {
        for pick in &mut picks {
            pick.any = false;
        }
    }
    let included = picks
        .iter()
        .filter(|pick| pick.stance == ApiStance::Include)
        .count();
    let all_any = picks
        .iter()
        .filter(|pick| pick.stance == ApiStance::Include)
        .all(|pick| pick.any);
    if included > 1 && !support.all && !all_any {
        if support.any {
            for pick in &mut picks {
                pick.any = pick.stance == ApiStance::Include;
            }
        } else if let Some(first) = picks
            .iter()
            .position(|pick| pick.stance == ApiStance::Include)
        {
            // One at a time is all the source takes.
            let mut at = 0;
            picks.retain(|pick| {
                let keep = pick.stance != ApiStance::Include || at == first;
                at += 1;
                keep
            });
        }
    }
    let mut room = usize::try_from(support.max_included).unwrap_or(usize::MAX);
    picks.retain(|pick| {
        if pick.stance != ApiStance::Include {
            return true;
        }
        if room == 0 {
            return false;
        }
        room -= 1;
        true
    });
    picks
}

/// The query as the source can answer it; see [`fit_picks`].
pub(super) fn fit_query(
    mut query: lumilio_plugin_api::content::SearchQuery,
    support: &KindSupport,
) -> lumilio_plugin_api::content::SearchQuery {
    let filters = &support.filters;
    // A sort the source lacks falls back to its first; the plugin would
    // otherwise refuse the whole search.
    if !support.sorts.contains(&query.sort)
        && let Some(first) = support.sorts.first()
    {
        query.sort = *first;
    }
    query
        .game_versions
        .truncate(usize::try_from(filters.max_game_versions).unwrap_or(usize::MAX));
    query.categories = fit_picks(query.categories, filters.categories.as_ref());
    query.loaders = fit_picks(query.loaders, filters.loaders.as_ref());
    if !filters.environment {
        query.client = false;
        query.server = false;
    }
    if !filters.open_source {
        query.open_source = None;
    }
    if !filters.hidden_projects {
        query.hidden_projects.clear();
    }
    if !filters.excluded_disclosures {
        query.excluded_disclosures.clear();
    }
    if !filters.excluded_types {
        query.excluded_types.clear();
    }
    query
}

/// One enabled content source. Every call runs inside the plugin host's
/// worker, timeout and permission boundary.
pub struct ContentClient<'a> {
    host: &'a PluginHost,
    plugin: String,
    name: String,
    capabilities: Capabilities,
}

impl PluginHost {
    fn client(&self, source: PluginContentSource) -> ContentClient<'_> {
        ContentClient {
            host: self,
            plugin: source.plugin,
            name: source.name,
            capabilities: source.capabilities,
        }
    }

    /// Why there is no source to ask: one that stopped, or none at all.
    async fn missing_source(&self) -> DiscoverError {
        match self.stopped_content_source().await {
            Some((name, message)) => {
                DiscoverError::Unavailable(format!("{name} stopped: {message}"))
            }
            None => DiscoverError::NoSource,
        }
    }

    /// The first enabled content source, or [`DiscoverError::NoSource`].
    pub async fn content_client(&self) -> Result<ContentClient<'_>, DiscoverError> {
        match self.content_sources().await.into_iter().next() {
            Some(source) => Ok(self.client(source)),
            None => Err(self.missing_source().await),
        }
    }

    /// The enabled content source with this plugin ID.
    pub async fn content_client_for(
        &self,
        source: &str,
    ) -> Result<ContentClient<'_>, DiscoverError> {
        if let Some(found) = self
            .content_sources()
            .await
            .into_iter()
            .find(|candidate| candidate.plugin == source)
        {
            return Ok(self.client(found));
        }
        let stopped = self
            .list()
            .await
            .into_iter()
            .find_map(|info| match info.status {
                PluginStatus::Failed { message } if info.manifest.id == source => Some((
                    info.manifest.name.get(&self.locale_tag()).to_owned(),
                    message,
                )),
                _ => None,
            });
        Err(match stopped {
            Some((name, message)) => {
                DiscoverError::Unavailable(format!("{name} stopped: {message}"))
            }
            None => DiscoverError::NoSource,
        })
    }

    /// Every enabled content source, in the order the launcher lists them.
    pub async fn content_clients(&self) -> Vec<ContentClient<'_>> {
        self.content_sources()
            .await
            .into_iter()
            .map(|source| self.client(source))
            .collect()
    }

    /// The first enabled source that speaks `catalog`'s IDs.
    pub async fn catalog_client(&self, catalog: Catalog) -> Option<ContentClient<'_>> {
        self.content_clients()
            .await
            .into_iter()
            .find(|client| client.catalog() == Some(catalog))
    }
}

impl ContentClient<'_> {
    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn abilities(&self) -> Vec<KindAbilities> {
        abilities_from(&self.capabilities)
    }

    #[must_use]
    pub fn catalog(&self) -> Option<Catalog> {
        self.capabilities.catalog.map(catalog_from_api)
    }

    #[must_use]
    pub fn text_format(&self) -> TextFormat {
        format_from_api(self.capabilities.text_format)
    }

    /// Whether the source recognizes files by CurseForge's fingerprint, so
    /// the launcher has to compute it.
    #[must_use]
    pub fn wants_murmur2(&self) -> bool {
        self.fingerprints(FingerprintKind::Murmur2) && !self.fingerprints(FingerprintKind::Sha1)
    }

    async fn call<R, F>(&self, call: F) -> Result<R, DiscoverError>
    where
        R: Send + 'static,
        F: FnOnce(&dyn ContentSource, &dyn HostContext) -> Result<R, PluginError> + Send + 'static,
    {
        match self.host.call_content(&self.plugin, call).await {
            Some(value) => Ok(value),
            None => Err(self.unavailable().await),
        }
    }

    /// A missing answer means the plugin is off, was already stopped, or
    /// stopped just now; say which.
    async fn unavailable(&self) -> DiscoverError {
        let info = self
            .host
            .list()
            .await
            .into_iter()
            .find(|info| info.manifest.id == self.plugin);
        match info.map(|info| info.status) {
            Some(PluginStatus::Failed { message }) => {
                DiscoverError::Unavailable(format!("{} stopped: {message}", self.name))
            }
            Some(PluginStatus::Enabled) => {
                let why = self.host.last_transient_error(&self.plugin).await;
                DiscoverError::Unavailable(match why {
                    Some(why) => format!("{} did not answer: {why}", self.name),
                    None => format!("{} did not answer", self.name),
                })
            }
            _ => DiscoverError::NoSource,
        }
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchPage, DiscoverError> {
        let query = query_to_api(query);
        // An unsupported selection would make the plugin refuse the search,
        // and a refusal stops the plugin.
        let Some(support) = self
            .capabilities
            .kinds
            .iter()
            .find(|support| support.kind == query.kind)
        else {
            return Ok(SearchPage::default());
        };
        let query = fit_query(query, support);
        match self.host.search_content(&self.plugin, query).await {
            Some(page) => Ok(page_from_api(page)),
            None => Err(self.unavailable().await),
        }
    }

    /// The project, and its author when the source names one.
    pub async fn project(&self, id: &str) -> Result<(Project, Option<String>), DiscoverError> {
        let id = id.to_owned();
        let format = self.text_format();
        let project = self
            .call(move |source, ctx| source.project(ctx, &id))
            .await?;
        Ok(project_from_api(project, format))
    }

    pub async fn versions(&self, project: &str) -> Result<Vec<Version>, DiscoverError> {
        self.versions_in(project, false).await
    }

    /// Like [`Self::versions`], with each version's changelog where the
    /// source gives them all at once.
    pub async fn versions_with_changelog(
        &self,
        project: &str,
    ) -> Result<Vec<Version>, DiscoverError> {
        self.versions_in(project, true).await
    }

    async fn versions_in(
        &self,
        project: &str,
        changelog: bool,
    ) -> Result<Vec<Version>, DiscoverError> {
        let project = project.to_owned();
        let format = self.text_format();
        let versions = self
            .call(move |source, ctx| source.versions(ctx, &project, changelog))
            .await?;
        Ok(versions
            .into_iter()
            .map(|version| version_from_api(version, format))
            .collect())
    }

    /// One version's changelog, for sources that leave it out of `versions`.
    pub async fn changelog(&self, project: &str, version: &str) -> Result<String, DiscoverError> {
        let reference = VersionRef {
            project_id: project.to_owned(),
            version_id: version.to_owned(),
        };
        self.call(move |source, ctx| source.changelog(ctx, &reference))
            .await
    }

    pub async fn filters(&self) -> Result<SourceFilters, DiscoverError> {
        let choices = self.call(|source, ctx| source.filter_choices(ctx)).await?;
        Ok(SourceFilters {
            categories: choices
                .categories
                .into_iter()
                .map(category_from_api)
                .collect(),
            game_versions: choices
                .game_versions
                .into_iter()
                .map(game_version_from_api)
                .collect(),
            loaders: choices.loaders.into_iter().map(loader_from_api).collect(),
        })
    }

    /// Which version each file (by SHA-1) belongs to. Files the source does
    /// not know, or that lack the fingerprint it needs, are absent from the
    /// answer.
    pub async fn identify(
        &self,
        files: &[FileHashes],
    ) -> Result<BTreeMap<String, Version>, DiscoverError> {
        let identities = self.identities(files);
        if identities.is_empty() {
            return Ok(BTreeMap::new());
        }
        let format = self.text_format();
        let found = self
            .call(move |source, ctx| source.identify(ctx, &identities))
            .await?;
        Ok(versions_by_key(found, format))
    }

    /// The newest compatible version for each file (by SHA-1). Files with no
    /// compatible version, or unknown to the source, are absent.
    pub async fn latest_for(
        &self,
        files: &[FileHashes],
        loader: Loader,
        game_version: &str,
    ) -> Result<BTreeMap<String, Version>, DiscoverError> {
        let identities = self.identities(files);
        if identities.is_empty() {
            return Ok(BTreeMap::new());
        }
        let compatibility = Compatibility {
            game_version: game_version.to_owned(),
            loader: loader_tag(loader).map(str::to_owned),
        };
        let format = self.text_format();
        let found = self
            .call(move |source, ctx| source.latest_for(ctx, &identities, &compatibility))
            .await?;
        Ok(versions_by_key(found, format))
    }

    /// Several projects at once, for labelling installed files.
    pub async fn project_summaries(
        &self,
        ids: &[String],
    ) -> Result<Vec<ProjectSummary>, DiscoverError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids.to_vec();
        let summaries = self
            .call(move |source, ctx| source.project_summaries(ctx, &ids))
            .await?;
        Ok(summaries.into_iter().map(summary_from_api).collect())
    }

    /// The files a pack names by (project, version) in this source's catalog.
    /// References the source does not know are absent.
    pub async fn resolve_files(
        &self,
        references: &[(String, String)],
    ) -> Result<Vec<ResolvedFile>, DiscoverError> {
        if references.is_empty() {
            return Ok(Vec::new());
        }
        let references: Vec<VersionRef> = references
            .iter()
            .map(|(project, version)| VersionRef {
                project_id: project.clone(),
                version_id: version.clone(),
            })
            .collect();
        let resolved = self
            .call(move |source, ctx| source.resolve_files(ctx, &references))
            .await?;
        Ok(resolved.into_iter().map(resolved_from_api).collect())
    }

    fn fingerprints(&self, kind: FingerprintKind) -> bool {
        self.capabilities.fingerprints.contains(&kind)
    }

    /// Each file with the fingerprints this source reads; files without one
    /// it can use are left out. The SHA-1 is the correlation key: the source
    /// hands answers back under it and never sees a local path.
    fn identities(&self, files: &[FileHashes]) -> Vec<FileIdentity> {
        let (sha1, murmur2) = (
            self.fingerprints(FingerprintKind::Sha1),
            self.fingerprints(FingerprintKind::Murmur2),
        );
        files
            .iter()
            .filter_map(|file| {
                let mut fingerprints = Vec::new();
                if sha1 {
                    fingerprints.push(Fingerprint::Sha1(file.sha1.clone()));
                }
                if murmur2 && let Some(value) = file.murmur2 {
                    fingerprints.push(Fingerprint::Murmur2(value));
                }
                (!fingerprints.is_empty()).then(|| FileIdentity {
                    key: file.sha1.clone(),
                    fingerprints,
                })
            })
            .collect()
    }
}

fn versions_by_key(
    found: BTreeMap<String, lumilio_plugin_api::content::Version>,
    format: TextFormat,
) -> BTreeMap<String, Version> {
    found
        .into_iter()
        .map(|(key, version)| (key, version_from_api(version, format)))
        .collect()
}
