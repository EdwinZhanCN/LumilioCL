use super::LauncherService;
use super::error::ServiceError;
use super::types::{DiscoverFilters, ProjectDetail};
use crate::activity::CancellationToken;
use crate::discover::{ModrinthClient, ProjectKind, SearchPage, SearchQuery};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub(super) async fn modrinth(&self) -> Result<ModrinthClient<T>, ServiceError> {
        Ok(ModrinthClient::new(self.transport.clone()).with_sources(self.chain().await?))
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchPage, ServiceError> {
        self.modrinth()
            .await?
            .search(query)
            .await
            .map_err(|error| ServiceError::Remote(error.to_string()))
    }

    /// Categories and game versions for the Discover filters. Fetched once and
    /// kept for the life of the service; a failure is not remembered.
    pub async fn discover_filters(&self) -> Result<DiscoverFilters, ServiceError> {
        let mut cached = self.filters.lock().await;
        if let Some(filters) = cached.as_ref() {
            return Ok(filters.clone());
        }
        let client = self.modrinth().await?;
        let (categories, game_versions) = tokio::join!(client.categories(), client.game_versions());
        let filters = DiscoverFilters {
            categories: categories.map_err(|error| ServiceError::Remote(error.to_string()))?,
            game_versions: game_versions
                .map_err(|error| ServiceError::Remote(error.to_string()))?,
        };
        *cached = Some(filters.clone());
        Ok(filters)
    }

    /// Everything the detail window shows about a project.
    pub async fn project_detail(&self, project: &str) -> Result<ProjectDetail, ServiceError> {
        let client = self.modrinth().await?;
        let (details, versions, owner) = tokio::join!(
            client.project(project),
            client.versions(project),
            client.owner(project)
        );
        let remote =
            |error: crate::discover::DiscoverError| ServiceError::Remote(error.to_string());
        let mut versions = versions.map_err(remote)?;
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(ProjectDetail {
            project: details.map_err(remote)?,
            versions,
            // The owner is decoration; not knowing it must not hide the page.
            owner: owner.ok().flatten(),
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
