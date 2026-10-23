//! The launch service: from an instance record to a running game.
//!
//! It composes the catalog, release, install, repair-scan, Java, and process
//! pieces and reports everything as [`LaunchUpdate`]s, so a UI only has to
//! render a [`crate::LaunchSession`].

mod output;
mod phases;
mod progress;
mod types;

#[cfg(all(test, unix))]
mod tests;

pub use self::output::GAME_OUTPUT_LOG;
pub use self::progress::runtimes_root;
pub use self::types::{JavaRequest, LaunchRequest, LaunchServiceError, LaunchUpdate};

use crate::activity::CancellationToken;
use crate::java::JavaRuntime;
use crate::launch_session::LaunchSignal;
use crate::process::GameExit;
use crate::transfer::{
    ConnectionBudget, SourceChain, TransferEngine, TransferError, TransferEvent, Transport,
};
use std::path::PathBuf;
use std::sync::Mutex as StdMutex;
use tokio::sync::{broadcast, mpsc};

/// Shown in the game's debug screen as the launcher brand.
/// Where the game may record how a direct start went, inside the game folder.
const QUICK_PLAY_LOG: &str = "quickPlay/lumilio.json";

const LAUNCHER_NAME: &str = "LumilioCL";

pub struct Launcher<T> {
    transport: T,
    sources: SourceChain,
    concurrency: usize,
    /// Connections shared with other work; `None` keeps them to this launcher.
    budget: Option<ConnectionBudget>,
    /// Where stopped downloads are noted; `None` discards them.
    partials: Option<PathBuf>,
    /// Hears every transfer of every engine the launcher makes.
    watcher: Option<broadcast::Sender<TransferEvent>>,
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
            budget: None,
            partials: None,
            watcher: None,
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

    /// Downloads on connections shared with other work, which replaces any
    /// concurrency of its own.
    #[must_use]
    pub fn with_budget(mut self, budget: ConnectionBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Keeps stopped downloads for the next launch to continue, noted in
    /// `folder`.
    #[must_use]
    pub fn keeping_partials(mut self, folder: PathBuf) -> Self {
        self.partials = Some(folder);
        self
    }

    /// Reports every transfer to `watcher` as well, so the task this launch
    /// runs under can show its files.
    #[must_use]
    pub(crate) fn watched_by(mut self, watcher: broadcast::Sender<TransferEvent>) -> Self {
        self.watcher = Some(watcher);
        self
    }

    fn transfer_engine(&self) -> Result<TransferEngine<T>, TransferError> {
        let mut engine = match &self.budget {
            Some(budget) => TransferEngine::with_budget(self.transport.clone(), budget.clone()),
            None => TransferEngine::new(self.transport.clone(), self.concurrency)?,
        };
        if let Some(folder) = &self.partials {
            engine = engine.keeping_partials(folder.clone());
        }
        if let Some(watcher) = &self.watcher {
            engine = engine.watched_by(watcher.clone());
        }
        Ok(engine)
    }

    /// An engine for the documents a launch reads into memory.
    fn fetcher(&self) -> Result<TransferEngine<T>, LaunchServiceError> {
        self.transfer_engine()
            .map_err(|error| LaunchServiceError::ManifestUnavailable(error.to_string()))
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
