use std::collections::BTreeSet;
use std::sync::Mutex as StdMutex;

use lumilio_plugin_api::{LaunchEvent, LaunchOutcome, LaunchTarget};
use tokio::sync::{mpsc, oneshot};

use super::{GameExit, LaunchParts, LaunchServiceError, LaunchUpdate, LogStream, SessionMarker};
use crate::activity::CancellationToken;
use crate::clock::now;
use crate::context::{ContextHub, SessionPhase};
use crate::error::ServiceError;
use crate::history::{SessionOutcome, finish_session, record_attempt};
use crate::instances::StoreError;
use crate::storage::Layout;
use crate::transfer::Transport;
use crate::tuning::QuickPlay;

/// The launch state the launcher keeps between launches.
#[derive(Default)]
pub(crate) struct Launching {
    /// Instances whose next launch picks its Java automatically, whatever
    /// their settings say (the person chose 本次改用 when asked).
    automatic_java_once: StdMutex<BTreeSet<String>>,
}

impl Launching {
    /// Lets the next launch of `id` pick its Java automatically, once.
    pub(crate) fn automatic_java_once(&self, id: &str) {
        self.automatic_java().insert(id.to_owned());
    }

    fn automatic_java(&self) -> std::sync::MutexGuard<'_, BTreeSet<String>> {
        self.automatic_java_once
            .lock()
            .expect("launch state lock was poisoned")
    }
}

impl<T: Transport + Clone> LaunchParts<'_, T> {
    /// Prepares and starts `id`, publishing its session and recording it in
    /// the instance history afterwards.
    pub(crate) async fn launch(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let _lease = self.instances.reserve(id)?;
        let started = now();
        // Any launch uses the one-time choice up, even one that fails.
        let automatic_java = self.state.automatic_java().remove(id);
        if self.instances.lock().await.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.to_owned()));
        }
        let session = Published::begin(self.context, id);
        let result = self
            .run(id, started, automatic_java, quick_play, updates, cancel)
            .await;
        session.end(match &result {
            Ok(exit) => matches!(
                SessionOutcome::of(exit),
                SessionOutcome::Clean | SessionOutcome::Stopped
            ),
            Err(error) => matches!(
                error,
                ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled)
            ),
        });
        result
    }

    /// Starts the game and goes straight into a saved world. The instance's
    /// own direct-start choice is not changed.
    pub(crate) async fn launch_world(
        &self,
        id: &str,
        world: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let target = QuickPlay::World(world.to_owned());
        if crate::tuning::quick_play_problem(&target).is_some() {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                "the world name is not a valid save folder".to_owned(),
            )));
        }
        self.launch(id, Some(Some(target)), updates, cancel).await
    }

    /// Starts the game and goes straight onto a multiplayer server. The
    /// instance's own direct-start choice is not changed.
    pub(crate) async fn launch_server(
        &self,
        id: &str,
        address: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let target = QuickPlay::Server(address.trim().to_owned());
        if let Some(problem) = crate::tuning::quick_play_problem(&target) {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                problem.as_str().to_owned(),
            )));
        }
        self.launch(id, Some(Some(target)), updates, cancel).await
    }

    async fn run(
        &self,
        id: &str,
        started: u64,
        automatic_java: bool,
        quick_play: Option<Option<QuickPlay>>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        // A folder moved or deleted outside the launcher is not quietly
        // replaced by an empty one (plan game-folder-missing).
        let game = self.layout.game(id);
        if !game.is_dir() {
            let error = ServiceError::GameFolderMissing(game);
            return Err(note_attempt(self.layout, id, started, error));
        }
        // Dropped on every way out; only the launcher dying leaves it behind.
        let _marker = SessionMarker::place(self.layout, id, started)?;
        let (mut request, chain) = match self.request(id, quick_play, &cancel).await {
            Ok(prepared) => prepared,
            Err(error) => return Err(note_attempt(self.layout, id, started, error)),
        };
        if automatic_java {
            request.instance.settings.java_path = None;
        }
        let event = LaunchEvent::Started {
            instance_name: request.instance.name.clone(),
            game_version: request.instance.game_version.clone(),
            loader: format!("{:?}", request.instance.loader),
            target: request.quick_play.as_ref().map(|target| match target {
                QuickPlay::World(name) => LaunchTarget::World(name.clone()),
                QuickPlay::Server(address) => LaunchTarget::Server(address.clone()),
            }),
        };
        let events = self.plugins.launch_events();
        let (spawned, mut spawned_rx) = mpsc::unbounded_channel();
        let (before_start, mut preparation_rx) =
            mpsc::unbounded_channel::<oneshot::Sender<Result<(), String>>>();
        let (launcher, java_requests) = self.launcher(chain).await;
        let launcher = launcher
            .with_spawned(spawned)
            .with_before_start(before_start);
        let launch = self.serving_java(
            java_requests,
            &cancel,
            launcher.launch(request, updates.clone(), cancel.clone()),
        );
        tokio::pin!(launch);
        let mut announced = false;
        let mut settings_prepared = false;
        let result = loop {
            tokio::select! {
                biased;
                Some(reply) = preparation_rx.recv() => {
                    let result = self.game_settings.apply_game_settings(id).await.map_err(|e| e.to_string());
                    settings_prepared = result.is_ok();
                    let _ = reply.send(result);
                }
                Some(()) = spawned_rx.recv(), if !announced => {
                    announced = true;
                    self.context.session(id, Some(SessionPhase::Playing));
                    let _ = events.send(event.clone());
                }
                result = &mut launch => break result,
            }
        };
        // A short-lived process can finish in the same poll as its spawn.
        if !announced && spawned_rx.try_recv().is_ok() {
            announced = true;
            self.context.session(id, Some(SessionPhase::Playing));
            let _ = events.send(event);
        }
        // Capturing changes is separate from the game's result and history. Failure is
        // retained on the binding for review instead of turning a clean exit into a crash.
        if settings_prepared && let Err(error) = self.game_settings.capture_game_settings(id).await
        {
            let _ = updates.send(LaunchUpdate::Log {
                stream: LogStream::Stderr,
                text: format!("game settings review: {error}"),
            });
        }
        let exit = match result {
            Ok(exit) => exit,
            Err(error) => {
                if announced {
                    let _ = events.send(LaunchEvent::Exited {
                        outcome: LaunchOutcome::Crashed,
                        played_seconds: 0,
                    });
                }
                return Err(note_attempt(self.layout, id, started, error.into()));
            }
        };
        // Queue exit even if history persistence fails: presence must not leak.
        let outcome = match SessionOutcome::of(&exit) {
            SessionOutcome::Clean => LaunchOutcome::Clean,
            SessionOutcome::Stopped => LaunchOutcome::Stopped,
            SessionOutcome::FailedToStart => LaunchOutcome::FailedToStart,
            _ => LaunchOutcome::Crashed,
        };
        let _ = events.send(LaunchEvent::Exited {
            outcome,
            played_seconds: exit.ran_for.as_secs(),
        });
        let mut store = self.instances.lock().await;
        store.mark_installed(id, true)?;
        finish_session(&mut store, id, started, &exit)?;
        Ok(exit)
    }
}

/// Writes a launch that never produced a process to the instance history,
/// then hands the error back. An unknown instance has no history to write.
fn note_attempt(layout: &Layout, id: &str, started: u64, error: ServiceError) -> ServiceError {
    let outcome = match &error {
        ServiceError::NoSuchInstance(_) => return error,
        ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled) => {
            SessionOutcome::Cancelled
        }
        _ => SessionOutcome::FailedToPrepare,
    };
    // History is a record, not part of the result: a write failure must
    // not replace the error the user needs to see.
    let _ = record_attempt(layout.root(), id, started, outcome);
    error
}

/// The session of one launch, published as it goes (plan global-context,
/// T6). A launch that is dropped half way publishes that it ended.
struct Published<'a> {
    hub: &'a ContextHub,
    id: &'a str,
    ended: bool,
}

impl<'a> Published<'a> {
    fn begin(hub: &'a ContextHub, id: &'a str) -> Self {
        hub.session(id, Some(SessionPhase::Preparing));
        Self {
            hub,
            id,
            ended: false,
        }
    }

    /// A clean end or a stop leaves no session; anything else stays as the
    /// failure Home offers to recover from.
    fn end(mut self, clean: bool) {
        self.ended = true;
        let phase = (!clean).then_some(SessionPhase::Failed);
        self.hub.session(self.id, phase);
    }
}

impl Drop for Published<'_> {
    fn drop(&mut self) {
        if !self.ended {
            self.hub.session(self.id, None);
        }
    }
}

#[cfg(test)]
mod tests;
