//! The launch service: from an instance record to a running game.
//!
//! It composes the catalog, release, install, repair-scan, Java, and process
//! pieces and reports everything as [`LaunchUpdate`]s, so a UI only has to
//! render a [`crate::LaunchSession`]. Behavior notes:
//! `docs/behavior/launcher-spine.md`.

mod phases;
mod progress;
mod types;

#[cfg(all(test, unix))]
mod tests;

pub use self::progress::runtimes_root;
pub use self::types::{LaunchRequest, LaunchServiceError, LaunchUpdate};

use crate::activity::CancellationToken;
use crate::launch_session::{LaunchFailure, LaunchSignal};
use crate::process::GameExit;
use crate::transfer::{SourceChain, Transport};
use tokio::sync::mpsc;

/// Shown in the game's debug screen as the launcher brand.
/// Where the game may record how a direct start went, inside the game folder.
const QUICK_PLAY_LOG: &str = "quickPlay/lumilio.json";

const LAUNCHER_NAME: &str = "LumilioCL";

pub struct Launcher<T> {
    transport: T,
    sources: SourceChain,
    concurrency: usize,
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
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Failed(
                    LaunchFailure::Step {
                        message: error.to_string(),
                    },
                )));
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
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Failed(
                    LaunchFailure::Step {
                        message: error.to_string(),
                    },
                )));
            }
        }
        result
    }
}
