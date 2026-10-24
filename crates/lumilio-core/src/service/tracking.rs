use tokio::sync::mpsc;

use super::LauncherService;
use crate::activity::{CancellationToken, Meter, Tasks};
use crate::error::ServiceError;
use crate::launching::{
    LaunchRequest, LaunchSignal, LaunchSteps, LaunchUpdate, Launcher, serving_java,
};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// [`install_tracked`] through a new launcher that may download the Java a
    /// Forge or NeoForge installer runs on.
    pub(super) async fn install_serving_java(
        &self,
        task: u64,
        chain: crate::transfer::SourceChain,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let (watcher, events) = tokio::sync::broadcast::channel(1024);
        let (launcher, java_requests) = LaunchSteps::launcher(self, chain).await;
        let launcher = launcher.watched_by(watcher);
        let watch = cancel.clone();
        let install = serving_java(
            self,
            java_requests,
            &watch,
            install_tracked(&self.tasks, task, &launcher, request, updates, cancel),
        );
        self.tasks
            .watching(task, events, Meter::Files, install)
            .await
    }

    /// Holds the downloads of a running task until [`Self::resume_task`]:
    /// they let go of their connections and keep their bytes. `false` when
    /// the task is unknown, finished or cannot be stopped.
    pub fn pause_task(&self, id: u64) -> bool {
        self.tasks.pause(id)
    }

    /// Lets a paused task's downloads continue from the bytes they kept.
    pub fn resume_task(&self, id: u64) -> bool {
        self.tasks.resume(id)
    }
}

/// Installs through `launcher`, passing its updates on while showing their
/// file counts as the task's progress.
async fn install_tracked<T: Transport + Clone>(
    tasks: &Tasks,
    task: u64,
    launcher: &Launcher<T>,
    request: LaunchRequest,
    updates: mpsc::UnboundedSender<LaunchUpdate>,
    cancel: CancellationToken,
) -> Result<(), ServiceError> {
    let (inner, mut seen) = mpsc::unbounded_channel();
    let forward = async {
        while let Some(update) = seen.recv().await {
            if let LaunchUpdate::Signal(LaunchSignal::Progress { done, total }) = &update {
                tasks.progress(task, *done, *total);
            }
            let _ = updates.send(update);
        }
    };
    let (installed, ()) = tokio::join!(launcher.install(request, inner, cancel), forward);
    installed.map_err(ServiceError::from)
}
