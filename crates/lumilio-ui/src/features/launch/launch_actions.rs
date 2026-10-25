use std::collections::VecDeque;
use std::time::Duration;

use futures_util::future::{Either, select};
use gpui::{AnyWindowHandle, AppContext as _, AsyncApp, Context, EventEmitter, Task};
use lumilio_core::{
    AfterLaunch, CancellationToken, LaunchFailure, LaunchSignal, LaunchUpdate, ServiceError,
};

use crate::java_conflict::JavaConflict;
use crate::live::{PlaceTarget, account_rows};
use crate::runtime::{self, Worker};
use crate::{platform, tr};

/// How many lines of the running game's output are kept.
const GAME_LOG_LINES: usize = 2000;
/// The longest a new line waits before the open page shows it.
const GAME_LOG_FLUSH: Duration = Duration::from_millis(150);

/// What a launch tells the shell and the application.
#[derive(Clone, Debug, PartialEq)]
pub enum LaunchEvent {
    /// How the launch is going, for Home's launch moment.
    Signal(LaunchSignal),
    /// What the game has printed so far, for the open page of that game.
    Output {
        id: String,
        lines: Vec<String>,
        running: bool,
    },
    /// The launch, or the game, is over: the lists are read again.
    Ended,
}

/// Whether the current game can start now, and if not, what to ask first.
#[derive(Clone, Debug, PartialEq)]
pub enum Readiness {
    Ready,
    /// Its folder was moved or deleted.
    MissingFolder {
        id: String,
        name: String,
    },
    /// No account is chosen.
    NoAccount,
    /// The Java chosen for it cannot run it.
    Java(JavaConflict),
}

/// One launch at a time: how to stop it, where it goes, and what the game
/// prints while it runs.
pub struct Launch {
    worker: Option<Worker>,
    /// Stops the launch or the running game.
    cancel: Option<CancellationToken>,
    /// A world or server the next launch goes straight into, once.
    target: Option<PlaceTarget>,
    /// The game whose Java conflict was just answered: its next check does
    /// not ask again.
    java_answered: Option<String>,
    /// What the running game has printed: the game and its newest lines.
    log: Option<(String, VecDeque<String>)>,
}

impl EventEmitter<LaunchEvent> for Launch {}

impl Launch {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            cancel: None,
            target: None,
            java_answered: None,
            log: None,
        }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// The next launch goes straight into this world or onto this server.
    pub fn aim(&mut self, target: PlaceTarget) {
        self.target = Some(target);
    }

    /// The next launch goes nowhere in particular: a world or server asked
    /// for with a launch that does not happen must not carry over.
    pub fn forget_target(&mut self) {
        self.target = None;
    }

    /// Takes the target, to give it back with [`Self::answer_java`].
    pub fn take_target(&mut self) -> Option<PlaceTarget> {
        self.target.take()
    }

    /// The person answered the Java question for the current game: the next
    /// launch goes where the asked one was headed, with the automatic Java
    /// this once if `switch`, and without asking again. False when there is
    /// no current game to start.
    pub fn answer_java(&mut self, switch: bool, target: Option<PlaceTarget>) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let Some(id) = worker.service().context().current_instance else {
            return false;
        };
        if switch {
            worker.service().launch_next_with_automatic_java(&id);
        }
        self.java_answered = Some(id);
        self.target = target;
        true
    }

    /// Makes an empty folder for game `id`, whose folder is gone.
    pub fn new_game_folder(&self, id: String, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let Some(worker) = self.worker.clone() else {
            return Task::ready(Err(tr!("common-work-stopped").to_owned()));
        };
        let service = worker.service().clone();
        let made = worker.spawn(async move { service.new_game_folder(&id).await });
        cx.spawn(async move |_, cx| match runtime::answer(made, cx).await {
            Some(made) => made.map_err(|error| error.to_string()),
            None => Err(tr!("common-work-stopped").to_owned()),
        })
    }

    /// Whether a launch is on its way or its game runs.
    #[must_use]
    pub fn launching(&self) -> bool {
        self.cancel.is_some()
    }

    /// Stops the launch or the running game.
    pub fn stop(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
    }

    /// Stops the launch or the running game, if it is game `id`'s.
    pub fn stop_game(&self, id: &str) {
        if self.active(id) {
            self.stop();
        }
    }

    /// What game `id` has printed so far, while it is on its way or runs.
    #[must_use]
    pub fn output_of(&self, id: &str) -> Option<Vec<String>> {
        let (running, lines) = self.log.as_ref()?;
        (self.active(id) && running == id).then(|| lines.iter().cloned().collect())
    }

    /// Whether core's session is game `id`'s, on its way or running.
    fn active(&self, id: &str) -> bool {
        self.worker.as_ref().is_some_and(|worker| {
            worker
                .service()
                .context()
                .session
                .is_some_and(|session| session.is_active() && session.instance == id)
        })
    }

    /// Checks whether the current game can start: its folder, then an
    /// account (asked of the service: one added a moment ago may not have
    /// reached the page yet), then its Java. A check that cannot be made
    /// does not stop the launch.
    pub fn readiness(&mut self, cx: &mut Context<Self>) -> Task<Readiness> {
        let Some(worker) = self.worker.clone() else {
            return Task::ready(Readiness::Ready);
        };
        let service = worker.service().clone();
        let current = service.context().current_instance;
        let answered = self.java_answered.take();
        let check_java = current.clone().filter(|id| answered.as_ref() != Some(id));
        let work = worker.spawn(async move {
            // Whether to play this game at all comes before account and Java.
            if let Some(id) = current.filter(|id| !service.game_dir(id).is_dir())
                && let Ok(record) = service.instance(&id).await
            {
                return Readiness::MissingFolder {
                    id: record.id,
                    name: record.name,
                };
            }
            let has_account = account_rows(&service.settings().await)
                .iter()
                .any(|row| row.selected);
            if !has_account {
                return Readiness::NoAccount;
            }
            let conflict = match check_java {
                Some(id) => service
                    .instance_java(&id)
                    .await
                    .ok()
                    .and_then(|java| JavaConflict::of(&java)),
                None => None,
            };
            conflict.map_or(Readiness::Ready, Readiness::Java)
        });
        cx.spawn(async move |_, cx| runtime::answer(work, cx).await.unwrap_or(Readiness::Ready))
    }

    /// Starts the current game, which core owns, into the world or onto the
    /// server aimed at. `window` hides while the game runs and comes back
    /// when it ends, as the preferences say.
    pub fn start(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let Some(id) = service.context().current_instance else {
            return;
        };
        let cancel = CancellationToken::new();
        self.cancel = Some(cancel.clone());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let around = {
            let (service, id) = (service.clone(), id.clone());
            worker.spawn(async move {
                let after = service.after_launch_for(&id).await;
                let front = service.settings().await.preferences.foreground_on_exit();
                (after, front)
            })
        };
        let target = self.target.take();
        let instance = id.clone();
        let handle = worker.spawn(async move {
            match target {
                Some(PlaceTarget::World(world)) => {
                    service.launch_world(&id, &world, tx, cancel).await
                }
                Some(PlaceTarget::Server(address)) => {
                    service.launch_server(&id, &address, tx, cancel).await
                }
                None => service.launch(&id, tx, cancel).await,
            }
        });
        cx.spawn(async move |this, cx| {
            let around = runtime::answer(around, cx)
                .await
                .unwrap_or((AfterLaunch::default(), true));
            let started = this.update(cx, |launch, cx| {
                launch.log = Some((instance.clone(), VecDeque::new()));
                // The page can tell the game is on its way before it prints anything.
                launch.flush(true, cx);
            });
            if started.is_err() {
                return;
            }
            let mut unsent = false;
            // The channel closes when the launch is over, so this ends with it.
            loop {
                let update = if unsent {
                    let timer = cx.background_executor().timer(GAME_LOG_FLUSH);
                    match select(Box::pin(rx.recv()), timer).await {
                        Either::Left((update, _)) => update,
                        Either::Right(_) => {
                            unsent = false;
                            let _ = this.update(cx, |launch, cx| launch.flush(true, cx));
                            continue;
                        }
                    }
                } else {
                    rx.recv().await
                };
                let Some(update) = update else { break };
                match update {
                    LaunchUpdate::Log { text, .. } => {
                        let _ = this.update(cx, |launch, _| launch.keep_line(text));
                        unsent = true;
                    }
                    LaunchUpdate::Signal(signal) => {
                        log_signal(&instance, &signal);
                        around_the_game(window, &signal, around, cx);
                        if this
                            .update(cx, |_, cx| cx.emit(LaunchEvent::Signal(signal)))
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
            // Launcher errors already ended the session with a signal of their own;
            // anything earlier (unknown instance, no catalog) has not, and neither
            // has work that panicked on the way.
            let failed = match handle.await.unwrap_or_else(|error| Err(error.into())) {
                Err(error) if !matches!(error, ServiceError::Launch(_)) => {
                    let signal = LaunchSignal::Failed(error.launch_failure());
                    log_signal(&instance, &signal);
                    Some(signal)
                }
                _ => None,
            };
            let _ = this.update(cx, |launch, cx| {
                if let Some(signal) = failed {
                    cx.emit(LaunchEvent::Signal(signal));
                }
                launch.cancel = None;
                launch.flush(false, cx);
                launch.log = None;
                cx.emit(LaunchEvent::Ended);
            });
        })
        .detach();
    }

    fn keep_line(&mut self, text: String) {
        if let Some((_, lines)) = self.log.as_mut() {
            if lines.len() == GAME_LOG_LINES {
                lines.pop_front();
            }
            lines.push_back(text);
        }
    }

    /// Shows the game's output so far on the open page of that game.
    fn flush(&self, running: bool, cx: &mut Context<Self>) {
        if let Some((id, lines)) = &self.log {
            cx.emit(LaunchEvent::Output {
                id: id.clone(),
                lines: lines.iter().cloned().collect(),
                running,
            });
        }
    }
}

/// Keeps how a launch went wrong, or how the game ended, in the launcher log.
fn log_signal(instance: &str, signal: &LaunchSignal) {
    match signal {
        LaunchSignal::Running => log::info!("{instance}: the game is running"),
        LaunchSignal::Exited { code: Some(0) } => log::info!("{instance}: the game quit"),
        LaunchSignal::Exited { code } => {
            log::warn!("{instance}: the game ended unexpectedly (exit code {code:?})");
        }
        LaunchSignal::Failed(LaunchFailure::Step { message, .. }) => {
            log::warn!("{instance}: the launch failed: {message}");
        }
        LaunchSignal::Failed(failure @ LaunchFailure::ExitedEarly { code, .. }) => {
            match failure.technical_detail() {
                Some(output) => log::warn!(
                    "{instance}: the game quit while starting (exit code {code:?}), printing:\n{output}"
                ),
                None => log::warn!("{instance}: the game quit while starting (exit code {code:?})"),
            }
        }
        LaunchSignal::Cancelled => log::info!("{instance}: the launch or game was stopped"),
        LaunchSignal::Phase(_) | LaunchSignal::Progress { .. } => {}
    }
}

/// What the window does around the game, as the preferences say: hide while
/// it runs, come back to the front when it ends.
fn around_the_game(
    window: AnyWindowHandle,
    signal: &LaunchSignal,
    (after, front): (AfterLaunch, bool),
    cx: &mut AsyncApp,
) {
    match signal {
        LaunchSignal::Running if after == AfterLaunch::Hide => {
            cx.update(|cx| platform::hide_launcher(cx));
        }
        LaunchSignal::Exited { .. } | LaunchSignal::Failed(_) | LaunchSignal::Cancelled
            if front =>
        {
            let _ = cx.update_window(window, |_, window, cx| platform::bring_to_front(window, cx));
        }
        _ => {}
    }
}
