use super::error::ServiceError;
use super::support::{TaskLease, meter_bytes};
use super::types::now;
use super::{LauncherService, TRANSFER_CONCURRENCY};
use crate::activity::CancellationToken;
use crate::activity_log::{TaskBoard, TaskCategory, TaskOutcome};
use crate::launcher::{LaunchRequest, LaunchServiceError, LaunchUpdate, Launcher};
use crate::transfer::Transport;
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc;

impl<T: Transport + Clone> LauncherService<T> {
    /// Files downloaded at once for content and installs.
    pub(super) async fn transfer_concurrency(&self) -> usize {
        self.settings
            .lock()
            .await
            .get()
            .download_concurrency
            .map_or(TRANSFER_CONCURRENCY, |count| count as usize)
    }

    /// Installs through `launcher`, passing its updates on while showing their
    /// file counts as the task's progress.
    pub(super) async fn install_tracked(
        board: &StdMutex<TaskBoard>,
        task: u64,
        launcher: &Launcher<T>,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let (inner, mut seen) = mpsc::unbounded_channel();
        let forward = async {
            while let Some(update) = seen.recv().await {
                if let LaunchUpdate::Signal(crate::launch_session::LaunchSignal::Progress {
                    done,
                    total,
                }) = &update
                {
                    board
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .progress(task, *done, *total);
                }
                let _ = updates.send(update);
            }
        };
        let (installed, ()) = tokio::join!(launcher.install(request, inner, cancel), forward);
        installed.map_err(ServiceError::from)
    }

    /// Runs `work` while the byte progress of the engine behind `events` is
    /// shown as the progress of task `task`.
    pub(super) async fn metered<R>(
        &self,
        task: u64,
        events: tokio::sync::broadcast::Receiver<crate::transfer::TransferEvent>,
        work: impl std::future::Future<Output = R>,
    ) -> R {
        let meter = meter_bytes(&self.board, task, events);
        tokio::pin!(work);
        tokio::pin!(meter);
        tokio::select! {
            result = &mut work => result,
            () = &mut meter => work.await,
        }
    }

    pub(super) fn begin(
        &self,
        category: TaskCategory,
        label: String,
        instance: Option<String>,
        cancel: Option<&CancellationToken>,
    ) -> TaskLease<'_> {
        let id = self
            .board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .start(category, label, instance, now());
        if let Some(cancel) = cancel {
            self.cancels
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(id, cancel.clone());
        }
        TaskLease {
            board: &self.board,
            log: &self.log,
            cancels: &self.cancels,
            id,
            ended: false,
        }
    }

    pub(super) fn end<V>(&self, task: TaskLease<'_>, result: &Result<V, ServiceError>) {
        let outcome = match result {
            Ok(_) => TaskOutcome::Succeeded,
            Err(ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled)) => {
                TaskOutcome::Cancelled
            }
            Err(error) => TaskOutcome::Failed(error.to_string()),
        };
        task.finish(outcome);
    }
}
