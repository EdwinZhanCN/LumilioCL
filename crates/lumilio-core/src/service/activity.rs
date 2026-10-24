use std::path::Path;

use tokio::sync::mpsc;

use super::LauncherService;
use crate::activity::RetryAction;
use crate::activity::{ActivityView, CancellationToken, Traffic};
use crate::error::ServiceError;
use crate::launching::LaunchUpdate;
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// Asks a running task to stop. `false` when the id is unknown, already
    /// finished, or not cancellable; the task's own outcome is still decided by
    /// the work itself.
    pub fn cancel_task(&self, id: u64) -> bool {
        self.tasks.cancel(id)
    }

    /// Runs a task again from the input it was started with (its
    /// [`FinishedTask::retry`](crate::FinishedTask::retry)). It is a new task
    /// of its own: the old entry stays in the history.
    pub async fn retry_task(
        &self,
        action: RetryAction,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        match action {
            RetryAction::InstallInstance { instance } => {
                self.install_instance(&instance, updates, cancel).await
            }
            RetryAction::RepairInstance { instance } => {
                self.repair_instance(&instance, updates, cancel).await
            }
            RetryAction::ChangeRuntime {
                instance,
                game_version,
                loader,
                loader_version,
            } => self
                .change_runtime(
                    &instance,
                    &game_version,
                    loader,
                    loader_version.as_deref(),
                    updates,
                    cancel,
                )
                .await
                .map(|_| ()),
            RetryAction::InstallContent {
                instance,
                kind,
                source,
                project,
                version,
            } => match version {
                Some(version) => self
                    .install_version(&instance, kind, &source, &project, &version, cancel)
                    .await
                    .map(|_| ()),
                None => self
                    .install_content(&instance, kind, &source, &project, cancel)
                    .await
                    .map(|_| ()),
            },
            RetryAction::SwitchContent {
                instance,
                kind,
                file_name,
                source,
                project,
                version_id,
            } => self
                .switch_content_version(
                    &instance,
                    kind,
                    &file_name,
                    &source,
                    &project,
                    &version_id,
                    cancel,
                )
                .await
                .map(|_| ()),
            RetryAction::CopyInstance {
                source,
                name,
                include_worlds,
            } => self
                .copy_instance(&source, &name, include_worlds, cancel)
                .await
                .map(|_| ()),
            RetryAction::BackupInstance { instance, path } => self
                .backup_instance(&instance, Path::new(&path), cancel)
                .await
                .map(|_| ()),
            RetryAction::RestoreBackup { path } => self
                .restore_backup(Path::new(&path), cancel)
                .await
                .map(|_| ()),
            RetryAction::InstallJava {
                major,
                source,
                build,
            } => self
                .install_java_from(source, major, build, cancel)
                .await
                .map(|_| ()),
            RetryAction::SaveVersion {
                source,
                project,
                version_id,
                path,
            } => self
                .save_version_as(&source, &project, &version_id, Path::new(&path), cancel)
                .await
                .map(|_| ()),
            RetryAction::InstallModpack {
                source,
                project,
                version,
            } => self
                .install_modpack(&source, &project, version.as_deref(), cancel)
                .await
                .map(|_| ()),
            RetryAction::ImportPack { path } => self
                .import_modpack_file(Path::new(&path), cancel)
                .await
                .map(|_| ()),
        }
    }

    /// Forgets every finished task; running ones are untouched.
    pub fn clear_finished(&self) -> std::io::Result<()> {
        self.tasks.clear_finished()
    }

    /// Running tasks and the most recent finished ones.
    pub fn activity(&self, finished_limit: usize) -> ActivityView {
        let connections = self.network.budget();
        self.tasks.view(
            finished_limit,
            Traffic {
                open: connections.open(),
                limit: connections.size(),
                received: connections.received(),
            },
        )
    }
}
