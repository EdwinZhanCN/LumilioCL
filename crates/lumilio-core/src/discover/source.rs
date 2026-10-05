//! The launcher's way to ask the enabled content source (a plugin) for
//! projects, versions and file identities. Nothing here knows a protocol.

use std::collections::BTreeMap;

use lumilio_plugin_api::content::{
    Capabilities, Compatibility, FileIdentity, Fingerprint, FingerprintKind, Sort,
};
use lumilio_plugin_api::{ContentSource, HostContext, PluginError};

use super::convert::{
    category_from_api, game_version_from_api, kind_from_api, loader_from_api, page_from_api,
    project_from_api, query_to_api, summary_from_api, version_from_api,
};
use super::error::DiscoverError;
use super::kinds::{ProjectKind, SortIndex, loader_tag};
use super::project::Project;
use super::query::SearchQuery;
use super::search::SearchPage;
use super::tags::{CategoryTag, GameVersionTag, LoaderTag, ProjectSummary};
use super::versions::Version;
use crate::instance::Loader;
use crate::plugins::{PluginHost, PluginStatus};

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
                .filter_map(|sort| match sort {
                    Sort::Relevance => Some(SortIndex::Relevance),
                    Sort::Downloads => Some(SortIndex::Downloads),
                    Sort::Follows => Some(SortIndex::Follows),
                    Sort::Newest => Some(SortIndex::Newest),
                    Sort::Updated => Some(SortIndex::Updated),
                    Sort::Name | Sort::Author => None,
                })
                .collect(),
            game_versions: support.filters.max_game_versions > 0,
            categories: support.filters.categories.is_some(),
            loaders: support.filters.loaders.is_some(),
            environment: support.filters.environment,
            open_source: support.filters.open_source,
            hide_installed: support.filters.hidden_projects,
            advanced: support.filters.excluded_disclosures || support.filters.excluded_types,
        })
        .collect()
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
    /// The first enabled content source, or [`DiscoverError::NoSource`].
    pub async fn content_client(&self) -> Result<ContentClient<'_>, DiscoverError> {
        let Some(source) = self.content_sources().await.into_iter().next() else {
            return Err(match self.stopped_content_source().await {
                Some((name, message)) => {
                    DiscoverError::Unavailable(format!("{name}已停止：{message}"))
                }
                None => DiscoverError::NoSource,
            });
        };
        Ok(ContentClient {
            host: self,
            plugin: source.plugin,
            name: source.name,
            capabilities: source.capabilities,
        })
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
                DiscoverError::Unavailable(format!("{}已停止：{message}", self.name))
            }
            Some(PluginStatus::Enabled) => {
                let why = self.host.last_transient_error(&self.plugin).await;
                DiscoverError::Unavailable(match why {
                    Some(why) => format!("{}没有回答：{why}", self.name),
                    None => format!("{}没有回答", self.name),
                })
            }
            _ => DiscoverError::NoSource,
        }
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchPage, DiscoverError> {
        let mut query = query_to_api(query);
        // A sort the source lacks falls back to its first; the plugin would
        // otherwise refuse the whole search, and a refusal stops the plugin.
        if let Some(support) = self
            .capabilities
            .kinds
            .iter()
            .find(|s| s.kind == query.kind)
            && !support.sorts.contains(&query.sort)
            && let Some(first) = support.sorts.first()
        {
            query.sort = *first;
        }
        match self.host.search_content(&self.plugin, query).await {
            Some(page) => Ok(page_from_api(page)),
            None => Err(self.unavailable().await),
        }
    }

    /// The project, and its author when the source names one.
    pub async fn project(&self, id: &str) -> Result<(Project, Option<String>), DiscoverError> {
        let id = id.to_owned();
        let project = self
            .call(move |source, ctx| source.project(ctx, &id))
            .await?;
        Ok(project_from_api(project))
    }

    pub async fn versions(&self, project: &str) -> Result<Vec<Version>, DiscoverError> {
        self.versions_in(project, false).await
    }

    /// Like [`Self::versions`], with each version's changelog.
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
        let versions = self
            .call(move |source, ctx| source.versions(ctx, &project, changelog))
            .await?;
        Ok(versions.into_iter().map(version_from_api).collect())
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
    /// not know are absent from the answer.
    pub async fn identify(
        &self,
        sha1s: &[String],
    ) -> Result<BTreeMap<String, Version>, DiscoverError> {
        if sha1s.is_empty() || !self.fingerprints(FingerprintKind::Sha1) {
            return Ok(BTreeMap::new());
        }
        let files = identities(sha1s);
        let found = self
            .call(move |source, ctx| source.identify(ctx, &files))
            .await?;
        Ok(versions_by_key(found))
    }

    /// The newest compatible version for each file (by SHA-1). Files with no
    /// compatible version, or unknown to the source, are absent.
    pub async fn latest_for(
        &self,
        sha1s: &[String],
        loader: Loader,
        game_version: &str,
    ) -> Result<BTreeMap<String, Version>, DiscoverError> {
        if sha1s.is_empty() || !self.fingerprints(FingerprintKind::Sha1) {
            return Ok(BTreeMap::new());
        }
        let files = identities(sha1s);
        let compatibility = Compatibility {
            game_version: game_version.to_owned(),
            loader: loader_tag(loader).map(str::to_owned),
        };
        let found = self
            .call(move |source, ctx| source.latest_for(ctx, &files, &compatibility))
            .await?;
        Ok(versions_by_key(found))
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

    fn fingerprints(&self, kind: FingerprintKind) -> bool {
        self.capabilities.fingerprints.contains(&kind)
    }
}

/// The hash doubles as the correlation key: the source hands answers back
/// under it and never sees a local path.
fn identities(sha1s: &[String]) -> Vec<FileIdentity> {
    sha1s
        .iter()
        .map(|hash| FileIdentity {
            key: hash.clone(),
            fingerprints: vec![Fingerprint::Sha1(hash.clone())],
        })
        .collect()
}

fn versions_by_key(
    found: BTreeMap<String, lumilio_plugin_api::content::Version>,
) -> BTreeMap<String, Version> {
    found
        .into_iter()
        .map(|(key, version)| (key, version_from_api(version)))
        .collect()
}
