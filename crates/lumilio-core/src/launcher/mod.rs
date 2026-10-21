//! The launch service: from an instance record to a running game.
//!
//! It composes the catalog, release, install, repair-scan, Java, and process
//! pieces and reports everything as [`LaunchUpdate`]s, so a UI only has to
//! render a [`crate::LaunchSession`].

mod phases;
mod progress;
mod types;

#[cfg(all(test, unix))]
mod tests;

pub use self::progress::runtimes_root;
pub use self::types::{JavaRequest, LaunchRequest, LaunchServiceError, LaunchUpdate};

use crate::activity::CancellationToken;
use crate::java::JavaRuntime;
use crate::launch_session::LaunchSignal;
use crate::process::GameExit;
use crate::transfer::{SourceChain, Transport};
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc;

/// Shown in the game's debug screen as the launcher brand.
/// Where the game may record how a direct start went, inside the game folder.
const QUICK_PLAY_LOG: &str = "quickPlay/lumilio.json";

const LAUNCHER_NAME: &str = "LumilioCL";

pub struct Launcher<T> {
    transport: T,
    sources: SourceChain,
    concurrency: usize,
    spawned: Option<mpsc::UnboundedSender<()>>,
    before_start: Option<mpsc::UnboundedSender<tokio::sync::oneshot::Sender<Result<(), String>>>>,
    /// Where to ask for a Java when none installed fits; `None` fails instead.
    java_installer: Option<mpsc::UnboundedSender<JavaRequest>>,
    /// Java downloaded during this launch, beside the request's runtimes.
    installed_java: StdMutex<Vec<JavaRuntime>>,
}

impl<T> Launcher<T>
where
    T: Transport + Clone,
{
    #[must_use]
    pub fn new(transport: T, sources: SourceChain) -> Self {
        let concurrency = sources.preferred_concurrency();
        Self {
            transport,
            sources,
            concurrency,
            spawned: None,
            before_start: None,
            java_installer: None,
            installed_java: StdMutex::new(Vec::new()),
        }
    }

    /// Service-level observation starts only after the OS creates a process.
    pub(crate) fn with_spawned(mut self, sender: mpsc::UnboundedSender<()>) -> Self {
        self.spawned = Some(sender);
        self
    }

    pub(crate) fn with_before_start(
        mut self,
        sender: mpsc::UnboundedSender<tokio::sync::oneshot::Sender<Result<(), String>>>,
    ) -> Self {
        self.before_start = Some(sender);
        self
    }

    /// When no installed Java may run the game, asks `sender` for one
    /// instead of failing.
    pub(crate) fn with_java_installer(
        mut self,
        sender: mpsc::UnboundedSender<JavaRequest>,
    ) -> Self {
        self.java_installer = Some(sender);
        self
    }

    fn note_spawned(&self, event: &crate::process::GameEvent) {
        if matches!(event, crate::process::GameEvent::Spawned { .. })
            && let Some(sender) = &self.spawned
        {
            let _ = sender.send(());
        }
    }

    /// Downloads this many files at once instead of the source chain's choice.
    #[must_use]
    pub fn with_concurrency(mut self, concurrency: Option<u32>) -> Self {
        if let Some(count) = concurrency.filter(|count| *count > 0) {
            self.concurrency = count as usize;
        }
        self
    }

    /// Prepares and starts the game, returning how the process ended.
    ///
    /// Every failure is also reported as a terminal signal, so a session that
    /// only listens to `updates` still ends. Cancelling stops downloads and
    /// kills a running game.
    pub async fn launch(
        &self,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, LaunchServiceError> {
        let result = self.launch_inner(request, &updates, &cancel).await;
        match &result {
            Ok(_) => {}
            Err(LaunchServiceError::Cancelled) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Cancelled));
            }
            Err(error) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Failed(error.failure())));
            }
        }
        result
    }

    /// Installs everything the instance needs and stops there: no Java is
    /// chosen and no process is created. Like [`Self::launch`], every failure
    /// also arrives as a terminal signal.
    pub async fn install(
        &self,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), LaunchServiceError> {
        let result = self.prepare(&request, &updates, &cancel).await.map(drop);
        match &result {
            Ok(()) => {}
            Err(LaunchServiceError::Cancelled) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Cancelled));
            }
            Err(error) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Failed(error.failure())));
            }
        }
        result
    }
}
