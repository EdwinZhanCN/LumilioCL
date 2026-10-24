use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::LauncherService;
use crate::activity::CancellationToken;
use crate::content::discover::{ProjectKind, SearchPage, SearchQuery, TextFormat, Version};
use crate::content::{
    AddedFile, ContentResult, ContentSourceInfo, DependencyNeed, DependencyReport, DiscoverFilters,
    InstalledProject, ProjectDetail,
};
use crate::content::{ContentError, ContentItem};
use crate::error::ServiceError;
use crate::instances::modpack::PendingDownload;
use crate::transfer::Transport;
use crate::{TranslatedDocument, TranslationError};

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn import_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        files: Vec<std::path::PathBuf>,
    ) -> Result<Vec<(String, Result<String, ContentError>)>, ServiceError> {
        self.content.import_content(instance_id, kind, files).await
    }

    pub async fn project_versions(
        &self,
        source: &str,
        project: &str,
    ) -> Result<Vec<Version>, ServiceError> {
        self.content.project_versions(source, project).await
    }

    #[must_use]
    pub fn game_dir(&self, id: &str) -> std::path::PathBuf {
        self.content.game_dir(id)
    }

    pub async fn content(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<Vec<ContentItem>, ServiceError> {
        self.content.content(id, kind).await
    }

    pub async fn content_details(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<crate::content::ContentList, ServiceError> {
        self.content.content_details(id, kind).await
    }

    pub async fn set_content_state(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
        enabled: bool,
    ) -> Result<Vec<ContentResult>, ServiceError> {
        self.content
            .set_content_state(id, kind, file_names, enabled)
            .await
    }

    pub async fn delete_content(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
    ) -> Result<Vec<ContentResult>, ServiceError> {
        self.content.delete_content(id, kind, file_names).await
    }

    pub async fn save_version_as(
        &self,
        source: &str,
        project: &str,
        version_id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.content
            .save_version_as(source, project, version_id, destination, cancel)
            .await
    }

    pub async fn installed_projects(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        source: &str,
    ) -> Result<BTreeMap<String, InstalledProject>, ServiceError> {
        self.content
            .installed_projects(instance_id, kind, source)
            .await
    }

    pub async fn missing_dependencies(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        source: &str,
        project: &str,
        version_id: Option<&str>,
    ) -> Result<Vec<DependencyNeed>, ServiceError> {
        self.content
            .missing_dependencies(instance_id, kind, source, project, version_id)
            .await
    }

    pub async fn dependency_report(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        source: &str,
        project: &str,
        version_id: Option<&str>,
    ) -> Result<DependencyReport, ServiceError> {
        self.content
            .dependency_report(instance_id, kind, source, project, version_id)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn switch_content_version(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        file_name: &str,
        source: &str,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.content
            .switch_content_version(
                instance_id,
                kind,
                file_name,
                source,
                project,
                version_id,
                cancel,
            )
            .await
    }

    pub async fn content_sources(&self) -> Vec<ContentSourceInfo> {
        self.content.content_sources().await
    }

    pub async fn search(
        &self,
        source: &str,
        query: &SearchQuery,
    ) -> Result<SearchPage, ServiceError> {
        self.content.search(source, query).await
    }

    pub async fn discover_filters(&self, source: &str) -> Result<DiscoverFilters, ServiceError> {
        self.content.discover_filters(source).await
    }

    pub async fn project_detail(
        &self,
        source: &str,
        project: &str,
    ) -> Result<ProjectDetail, ServiceError> {
        self.content.project_detail(source, project).await
    }

    pub async fn version_changelog(
        &self,
        source: &str,
        project: &str,
        version: &str,
    ) -> Result<String, ServiceError> {
        self.content
            .version_changelog(source, project, version)
            .await
    }

    pub async fn install_version(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        source: &str,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.content
            .install_version(instance_id, kind, source, project, version_id, cancel)
            .await
    }

    pub async fn install_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        source: &str,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.content
            .install_content(instance_id, kind, source, project, cancel)
            .await
    }

    pub async fn translate_body(
        &self,
        source: &str,
        project: &str,
        original: &str,
        format: TextFormat,
    ) -> Result<TranslatedDocument, TranslationError> {
        self.content
            .translate_body(source, project, original, format)
            .await
    }

    pub async fn translate_summaries(
        &self,
        source: &str,
        items: &[(String, String)],
    ) -> BTreeMap<String, String> {
        self.content.translate_summaries(source, items).await
    }

    pub async fn clear_translation_cache(&self) -> Result<(), TranslationError> {
        self.content.clear_translation_cache().await
    }

    pub async fn pending_downloads(&self, id: &str) -> Result<Vec<PendingDownload>, ServiceError> {
        self.content.pending_downloads(id).await
    }

    pub async fn add_pending_files(
        &self,
        id: &str,
        files: Vec<PathBuf>,
    ) -> Result<Vec<(String, AddedFile)>, ServiceError> {
        self.content.add_pending_files(id, files).await
    }
}
