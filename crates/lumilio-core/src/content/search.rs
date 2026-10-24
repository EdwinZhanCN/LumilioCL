use super::Content;
use super::{ContentSourceInfo, DiscoverFilters, ProjectDetail};
use crate::activity::CancellationToken;
use crate::discover::{ContentClient, ProjectKind, SearchPage, SearchQuery};
use crate::error::ServiceError;
use crate::transfer::Transport;

impl<T: Transport + Clone> Content<T> {
    /// The enabled content source with this plugin ID, asked afresh every
    /// time so that turning the plugin off takes effect at once, whatever
    /// was cached.
    pub(crate) async fn content_client(
        &self,
        source: &str,
    ) -> Result<ContentClient<'_>, ServiceError> {
        Ok(self.plugins.content_client_for(source).await?)
    }

    /// The content sources that are on, in the order the launcher lists them.
    pub(crate) async fn content_sources(&self) -> Vec<ContentSourceInfo> {
        self.plugins
            .content_clients()
            .await
            .into_iter()
            .map(|client| ContentSourceInfo {
                id: client.plugin_id().to_owned(),
                name: client.name().to_owned(),
                catalog: client.catalog(),
            })
            .collect()
    }

    pub(crate) async fn search(
        &self,
        source: &str,
        query: &SearchQuery,
    ) -> Result<SearchPage, ServiceError> {
        Ok(self.content_client(source).await?.search(query).await?)
    }

    /// Categories, game versions and what the source can filter by, for the
    /// Discover filters. Fetched once per source and kept for the life of the
    /// service; a failure is not remembered, and a disabled source gives
    /// nothing even when its answer is cached.
    pub(crate) async fn discover_filters(
        &self,
        source: &str,
    ) -> Result<DiscoverFilters, ServiceError> {
        let client = self.content_client(source).await?;
        if let Some(filters) = self.filters.lock().await.get(client.plugin_id()) {
            return Ok(filters.clone());
        }
        let found = client.filters().await?;
        let filters = DiscoverFilters {
            source: client.name().to_owned(),
            abilities: client.abilities(),
            categories: found.categories,
            game_versions: found.game_versions,
            loaders: found.loaders,
        };
        self.filters
            .lock()
            .await
            .insert(client.plugin_id().to_owned(), filters.clone());
        Ok(filters)
    }

    /// Everything the detail window shows about a project.
    pub(crate) async fn project_detail(
        &self,
        source: &str,
        project: &str,
    ) -> Result<ProjectDetail, ServiceError> {
        let client = self.content_client(source).await?;
        let (details, versions) = tokio::join!(client.project(project), client.versions(project));
        let (details, owner) = details?;
        let mut versions = versions?;
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(ProjectDetail {
            project: details,
            versions,
            // The owner is decoration; not knowing it must not hide the page.
            owner,
            source: client.plugin_id().to_owned(),
            source_name: client.name().to_owned(),
        })
    }

    /// One version's changelog, for sources that give them one at a time.
    pub(crate) async fn version_changelog(
        &self,
        source: &str,
        project: &str,
        version: &str,
    ) -> Result<String, ServiceError> {
        Ok(self
            .content_client(source)
            .await?
            .changelog(project, version)
            .await?)
    }

    /// Installs one specific version of a mod, resource pack or shader into
    /// an instance. Refused when it does not run on the instance.
    pub(crate) async fn install_version(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        source: &str,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.install_from(instance_id, kind, source, project, Some(version_id), cancel)
            .await
    }

    /// Installs the newest compatible file of a mod, resource pack or shader
    /// into an instance and returns its file name.
    pub(crate) async fn install_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        source: &str,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.install_from(instance_id, kind, source, project, None, cancel)
            .await
    }
}
