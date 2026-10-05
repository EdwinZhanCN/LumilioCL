use super::LauncherService;
use super::error::ServiceError;
use super::types::{DiscoverFilters, ProjectDetail};
use crate::activity::CancellationToken;
use crate::discover::{ContentClient, ProjectKind, SearchPage, SearchQuery};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// The enabled content source, asked afresh every time so that turning
    /// the plugin off takes effect at once, whatever was cached.
    pub(super) async fn content_client(&self) -> Result<ContentClient<'_>, ServiceError> {
        Ok(self.plugins.content_client().await?)
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchPage, ServiceError> {
        Ok(self.content_client().await?.search(query).await?)
    }

    /// Categories, game versions and what the source can filter by, for the
    /// Discover filters. Fetched once per source and kept for the life of the
    /// service; a failure is not remembered, and a disabled source gives
    /// nothing even when its answer is cached.
    pub async fn discover_filters(&self) -> Result<DiscoverFilters, ServiceError> {
        let client = self.content_client().await?;
        let mut cached = self.filters.lock().await;
        if let Some((plugin, filters)) = cached.as_ref()
            && plugin == client.plugin_id()
        {
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
        *cached = Some((client.plugin_id().to_owned(), filters.clone()));
        Ok(filters)
    }

    /// Everything the detail window shows about a project.
    pub async fn project_detail(&self, project: &str) -> Result<ProjectDetail, ServiceError> {
        let client = self.content_client().await?;
        let (details, versions) = tokio::join!(client.project(project), client.versions(project));
        let (details, owner) = details?;
        let mut versions = versions?;
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(ProjectDetail {
            project: details,
            versions,
            // The owner is decoration; not knowing it must not hide the page.
            owner,
        })
    }

    /// Installs one specific version of a mod, resource pack or shader into
    /// an instance. Refused when it does not run on the instance.
    pub async fn install_version(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.install_from(instance_id, kind, project, Some(version_id), cancel)
            .await
    }

    /// Installs the newest compatible file of a mod, resource pack or shader
    /// into an instance and returns its file name.
    pub async fn install_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.install_from(instance_id, kind, project, None, cancel)
            .await
    }
}
