use std::path::Path;

use super::LauncherService;
use crate::diagnostics::{GameLogSource, GameLogs, Problem};
use crate::error::ServiceError;
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn history(&self, id: &str) -> Result<crate::history::HistoryRead, ServiceError> {
        self.diagnostics.history(id).await
    }

    pub async fn logs(&self, id: &str) -> Result<GameLogs, ServiceError> {
        self.diagnostics.logs(id).await
    }

    pub async fn crash_report(
        &self,
        id: &str,
        file_name: &str,
    ) -> Result<(String, Vec<crate::plugins::PluginFinding>), ServiceError> {
        let (_, runtimes) = self.java.environment().await;
        self.diagnostics.crash_report(id, file_name, runtimes).await
    }

    pub async fn game_log(&self, id: &str, source: GameLogSource) -> Result<String, ServiceError> {
        self.diagnostics.game_log(id, source).await
    }

    pub async fn analyze_game_log(
        &self,
        id: &str,
        text: String,
        crash: bool,
    ) -> Result<(String, Vec<crate::plugins::PluginFinding>), ServiceError> {
        let (_, runtimes) = self.java.environment().await;
        self.diagnostics
            .analyze_game_log(id, text, crash, runtimes)
            .await
    }

    pub async fn export_game_log(
        &self,
        id: &str,
        source: GameLogSource,
        live: String,
        destination: &Path,
    ) -> Result<(), ServiceError> {
        self.diagnostics
            .export_game_log(id, source, live, destination)
            .await
    }

    pub async fn instance_size(&self, id: &str) -> Result<u64, ServiceError> {
        self.diagnostics.instance_size(id).await
    }

    pub async fn list_files(
        &self,
        id: &str,
        relative: &str,
    ) -> Result<Vec<crate::diagnostics::FileEntry>, ServiceError> {
        self.diagnostics.list_files(id, relative).await
    }

    pub async fn problems(&self, id: &str) -> Result<Vec<Problem>, ServiceError> {
        let (_, runtimes) = self.java.environment().await;
        self.diagnostics.problems(id, runtimes).await
    }

    pub async fn export_log(
        &self,
        id: &str,
        crash_report: Option<&str>,
        destination: &Path,
    ) -> Result<(), ServiceError> {
        self.diagnostics
            .export_log(id, crash_report, destination)
            .await
    }

    pub async fn export_diagnostics(&self, destination: &Path) -> Result<(), ServiceError> {
        let installations = self.java.java_installations().await;
        let activity = self.activity(50);
        self.diagnostics
            .export_diagnostics(destination, installations, activity)
            .await
    }

    pub async fn reclaimable(&self) -> Result<crate::reclaim::Reclaimable, ServiceError> {
        self.diagnostics.reclaimable().await
    }

    pub async fn reclaim(&self) -> Result<u64, ServiceError> {
        self.diagnostics.reclaim().await
    }

    pub async fn storage_usage(&self) -> crate::storage::StorageUsage {
        self.diagnostics.storage_usage().await
    }

    pub async fn clear_cache(&self) -> Result<u64, ServiceError> {
        self.diagnostics.clear_cache().await
    }
}
