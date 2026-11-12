use std::collections::{BTreeMap, VecDeque};
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
    /// How the launch of game `id` is going, for its launch moment.
    Signal { id: String, signal: LaunchSignal },
    /// What the game has printed so far, for the open page of that game.
    /// `started_ms` is when its launch began (Unix milliseconds) while it is
    /// on its way or runs, and `None` once it is over.
    Output {
        id: String,
        lines: Vec<String>,
        started_ms: Option<i64>,
    },
    /// The launch, or the game, of `id` is over: the lists are read again.
    Ended { id: String },
}

/// Whether a game can start now, and if not, what to ask first.
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

/// The launches on their way and the games running, one per game (plan
/// concurrent-sessions): how to stop each, and what each prints.
pub struct Launch {
    worker: Option<Worker>,
    /// Each game's launch, by game id.
    slots: BTreeMap<String, Slot>,
    /// A world or server the next launch goes straight into, once.
    target: Option<PlaceTarget>,
    /// The game whose Java conflict was just answered: its next check does
    /// not ask again.
    java_answered: Option<String>,
}

/// One game's launch.
struct Slot {
    /// Stops the launch or the running game.
    cancel: CancellationToken,
    /// What the game has printed: its newest lines.
    log: VecDeque<String>,
    /// When the launch began, in ms since the Unix epoch. A page opened
    /// later is told this, not when it opened: a world the game opened in
    /// between is still the one it has open (issue #128).
    started_ms: i64,
}

impl EventEmitter<LaunchEvent> for Launch {}

impl Launch {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            slots: BTreeMap::new(),
            target: None,
            java_answered: None,
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

    /// Whether any launch is on its way or any game runs.
    #[must_use]
    pub fn launching(&self) -> bool {
        !self.slots.is_empty()
    }

    /// Whether game `id`'s launch is on its way or its game runs.
    #[must_use]
    pub fn launching_game(&self, id: &str) -> bool {
        self.slots.contains_key(id)
    }

    /// Stops game `id`'s launch or its running game; other games go on.
    pub fn stop_game(&self, id: &str) {
        if let Some(slot) = self.slots.get(id) {
            slot.cancel.cancel();
        }
    }

    /// What game `id` has printed so far and when its launch began (Unix
    /// milliseconds), while it is on its way or runs.
    #[must_use]
    pub fn output_of(&self, id: &str) -> Option<(Vec<String>, i64)> {
        let slot = self.slots.get(id)?;
        self.active(id)
            .then(|| (slot.log.iter().cloned().collect(), slot.started_ms))
    }

    /// Whether core says game `id` is on its way or running.
    fn active(&self, id: &str) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|worker| worker.service().context().is_active(id))
    }

    /// Checks whether game `id` can start: its folder, then an account
    /// (asked of the service: one added a moment ago may not have reached
    /// the page yet), then its Java. A check that cannot be made does not
    /// stop the launch.
    pub fn readiness(&mut self, id: String, cx: &mut Context<Self>) -> Task<Readiness> {
        let Some(worker) = self.worker.clone() else {
            return Task::ready(Readiness::Ready);
        };
        let service = worker.service().clone();
        let check_java = self.java_answered.take().as_ref() != Some(&id);
        let work = worker.spawn(async move {
            // Whether to play this game at all comes before account and Java.
            if !service.game_dir(&id).is_dir()
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
            let conflict = if check_java {
                service
                    .instance_java(&id)
                    .await
                    .ok()
                    .and_then(|java| JavaConflict::of(&java))
            } else {
                None
            };
            conflict.map_or(Readiness::Ready, Readiness::Java)
        });
        cx.spawn(async move |_, cx| runtime::answer(work, cx).await.unwrap_or(Readiness::Ready))
    }

    /// Starts game `id`, which core owns, into the world or onto the server
    /// aimed at, beside any other game on its way or running. `window` hides
    /// while a game runs and comes back when the last one ends, as the
    /// preferences say (plan concurrent-sessions, D2). A game already on its
    /// way or running is not started again, and the world or server aimed
    /// at is dropped with it: the next launch may be another game's.
    pub fn start(&mut self, id: String, window: AnyWindowHandle, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        if self.slots.contains_key(&id) {
            self.target = None;
            return;
        }
        let service = worker.service().clone();
        let cancel = CancellationToken::new();
        self.slots.insert(
            id.clone(),
            Slot {
                cancel: cancel.clone(),
                log: VecDeque::new(),
                started_ms: now_ms(),
            },
        );
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
            // The page can tell the game is on its way before it prints anything.
            let started = this.update(cx, |launch, cx| launch.flush(&instance, true, cx));
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
                            let _ = this.update(cx, |launch, cx| launch.flush(&instance, true, cx));
                            continue;
                        }
                    }
                } else {
                    rx.recv().await
                };
                let Some(update) = update else { break };
                match update {
                    LaunchUpdate::Log { text, .. } => {
                        let _ = this.update(cx, |launch, _| launch.keep_line(&instance, text));
                        unsent = true;
                    }
                    LaunchUpdate::Signal(signal) => {
                        log_signal(&instance, &signal);
                        let Ok(alone) = this.update(cx, |launch, _| launch.slots.len() == 1) else {
                            return;
                        };
                        around_the_game(window, &signal, around, alone, cx);
                        let id = instance.clone();
                        if this
                            .update(cx, |_, cx| cx.emit(LaunchEvent::Signal { id, signal }))
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
                    let id = instance.clone();
                    cx.emit(LaunchEvent::Signal { id, signal });
                }
                launch.flush(&instance, false, cx);
                launch.slots.remove(&instance);
                cx.emit(LaunchEvent::Ended { id: instance });
            });
        })
        .detach();
    }

    fn keep_line(&mut self, id: &str, text: String) {
        if let Some(slot) = self.slots.get_mut(id) {
            if slot.log.len() == GAME_LOG_LINES {
                slot.log.pop_front();
            }
            slot.log.push_back(text);
        }
    }

    /// Shows game `id`'s output so far on the open page of that game.
    fn flush(&self, id: &str, running: bool, cx: &mut Context<Self>) {
        if let Some(slot) = self.slots.get(id) {
            cx.emit(LaunchEvent::Output {
                id: id.to_owned(),
                lines: slot.log.iter().cloned().collect(),
                started_ms: running.then_some(slot.started_ms),
            });
        }
    }
}

/// Now, in ms since the Unix epoch: the unit of a world's lock time.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or_default()
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
/// it runs, come back to the front when it ends, unless another game is still
/// on its way or running (`alone` is false; plan concurrent-sessions, D2).
fn around_the_game(
    window: AnyWindowHandle,
    signal: &LaunchSignal,
    (after, front): (AfterLaunch, bool),
    alone: bool,
    cx: &mut AsyncApp,
) {
    match signal {
        LaunchSignal::Running if after == AfterLaunch::Hide => {
            cx.update(|cx| platform::hide_launcher(cx));
        }
        LaunchSignal::Exited { .. } | LaunchSignal::Failed(_) | LaunchSignal::Cancelled
            if front && alone =>
        {
            let _ = cx.update_window(window, |_, window, cx| platform::bring_to_front(window, cx));
        }
        _ => {}
    }
}
