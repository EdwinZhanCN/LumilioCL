use super::error::ServiceError;
use super::support::{Meter, TaskLease, watch_transfers};
use super::types::now;
use super::{LauncherService, TRANSFER_CONCURRENCY};
use crate::activity::CancellationToken;
use crate::activity_log::{TaskBoard, TaskCategory, TaskLabel, TaskOutcome};
use crate::launcher::{LaunchRequest, LaunchServiceError, LaunchUpdate, Launcher};
use crate::transfer::{ConnectionBudget, TransferEngine, Transport};
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc;

impl<T: Transport + Clone> LauncherService<T> {
    /// The connections every download shares, sized by the current setting.
    pub(super) async fn connections(&self) -> ConnectionBudget {
        let size = self
            .settings
            .lock()
            .await
            .get()
            .download_concurrency
            .map_or(TRANSFER_CONCURRENCY, |count| count as usize);
        // The setting is validated as positive; a zero keeps the old size.
        let _ = self.connections.resize(size);
        self.connections.clone()
    }

    /// An engine for one task's downloads, on the shared connections.
    pub(super) async fn transfer_engine(&self) -> TransferEngine<T> {
        TransferEngine::with_budget(self.transport.clone(), self.connections().await)
            .keeping_partials(self.layout.partial_downloads())
    }

    /// An engine for documents read into memory, on the shared connections
    /// as they are sized now. It takes no lock, so callers holding the
    /// settings may use it.
    pub(super) fn fetcher(&self) -> TransferEngine<T> {
        TransferEngine::with_budget(self.transport.clone(), self.connections.clone())
    }

    /// [`Self::install_tracked`] through a new launcher that may download the
    /// Java a Forge or NeoForge installer runs on.
    pub(super) async fn install_serving_java(
        &self,
        task: u64,
        chain: crate::transfer::SourceChain,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let (watcher, events) = tokio::sync::broadcast::channel(1024);
        let (launcher, java_requests) = self
            .with_java_installer(
                Launcher::new(self.transport.clone(), chain)
                    .with_budget(self.connections().await)
                    .keeping_partials(self.layout.partial_downloads())
                    .watched_by(watcher),
            )
            .await;
        let watch = cancel.clone();
        let install = self.serving_java(
            java_requests,
            &watch,
            Self::install_tracked(&self.board, task, &launcher, request, updates, cancel),
        );
        self.watching(task, events, Meter::Files, install).await
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
    /// shown as the progress of task `task`, with the files it downloads.
    pub(super) async fn metered<R>(
        &self,
        task: u64,
        events: tokio::sync::broadcast::Receiver<crate::transfer::TransferEvent>,
        work: impl std::future::Future<Output = R>,
    ) -> R {
        self.watching(task, events, Meter::Bytes, work).await
    }

    /// Runs `work` while the transfers behind `events` are recorded as task
    /// `task`'s downloads.
    async fn watching<R>(
        &self,
        task: u64,
        events: tokio::sync::broadcast::Receiver<crate::transfer::TransferEvent>,
        meter: Meter,
        work: impl std::future::Future<Output = R>,
    ) -> R {
        let watch = watch_transfers(&self.board, task, events, meter);
        tokio::pin!(work);
        tokio::pin!(watch);
        tokio::select! {
            result = &mut work => result,
            () = &mut watch => work.await,
        }
    }

    /// Holds the downloads of a running task until [`Self::resume_task`]:
    /// they let go of their connections and keep their bytes. `false` when
    /// the task is unknown, finished or cannot be stopped.
    pub fn pause_task(&self, id: u64) -> bool {
        self.task_control(id).map(|token| token.pause()).is_some()
    }

    /// Lets a paused task's downloads continue from the bytes they kept.
    pub fn resume_task(&self, id: u64) -> bool {
        self.task_control(id).map(|token| token.resume()).is_some()
    }

    pub(super) fn task_control(&self, id: u64) -> Option<CancellationToken> {
        self.cancels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .cloned()
    }

    pub(super) fn begin(
        &self,
        category: TaskCategory,
        label: TaskLabel,
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
