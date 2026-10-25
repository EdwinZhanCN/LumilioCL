use gpui_kit::{App, AppContext as _, Window};
use lumilio_core::{CancellationToken, LaunchFailure, LaunchSignal, LaunchUpdate, ServiceError};
use lumilio_ui::home::{HomePresentation, RecoveryDetail, Subject};
use lumilio_ui::instance_detail::TAB_DIAGNOSTICS;
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;

use super::instance::{around_the_game, open_instance_on};
use super::jobs::reload;
use super::{GAME_LOG_FLUSH, GAME_LOG_LINES, Reload, Wiring};

pub(super) fn start_launch(wiring: &Wiring, window: gpui_kit::AnyWindowHandle, cx: &mut App) {
    // Continue starts the current game, which core owns.
    let Some(id) = wiring.backend.service.context().current_instance else {
        return;
    };
    let cancel = CancellationToken::new();
    wiring.state.borrow_mut().cancel = Some(cancel.clone());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let service = wiring.backend.service.clone();
    let after = {
        let (service, id) = (service.clone(), id.clone());
        wiring
            .backend
            .spawn(async move { service.after_launch_for(&id).await })
    };
    let (world, server) = {
        let mut state = wiring.state.borrow_mut();
        (state.next_world.take(), state.next_server.take())
    };
    let instance = id.clone();
    let handle = wiring.backend.spawn(async move {
        match (world, server) {
            (Some(world), _) => service.launch_world(&id, &world, tx, cancel).await,
            (None, Some(address)) => service.launch_server(&id, &address, tx, cancel).await,
            (None, None) => service.launch(&id, tx, cancel).await,
        }
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let after = lumilio_ui::runtime::answer(after, cx)
            .await
            .unwrap_or_default();
        wiring.state.borrow_mut().game_log = Some((instance.clone(), Default::default()));
        // The page can tell the game is on its way before it prints anything.
        push_game_log(&wiring, true, cx);
        let mut unsent = false;
        // The channel closes when the launch is over, so this ends with it.
        loop {
            let update = if unsent {
                let next = futures::future::select(
                    Box::pin(rx.recv()),
                    cx.background_executor().timer(GAME_LOG_FLUSH),
                )
                .await;
                match next {
                    futures::future::Either::Left((update, _)) => update,
                    futures::future::Either::Right(_) => {
                        unsent = false;
                        push_game_log(&wiring, true, cx);
                        continue;
                    }
                }
            } else {
                rx.recv().await
            };
            let Some(update) = update else { break };
            if let LaunchUpdate::Log { text, .. } = &update {
                if let Some((_, lines)) = wiring.state.borrow_mut().game_log.as_mut() {
                    if lines.len() == GAME_LOG_LINES {
                        lines.pop_front();
                    }
                    lines.push_back(text.clone());
                }
                unsent = true;
            }
            if let LaunchUpdate::Signal(signal) = update {
                log_signal(&instance, &signal);
                around_the_game(&wiring, window, &signal, after, cx);
                if wiring
                    .shell
                    .update(cx, |shell, cx| shell.apply_launch_signal(signal, cx))
                    .is_err()
                {
                    return;
                }
            }
        }
        // Launcher errors already ended the session with a signal of their own;
        // anything earlier (unknown instance, no catalog) has not, and neither
        // has work that panicked on the way.
        if let Err(error) = handle.await.unwrap_or_else(|error| Err(error.into()))
            && !matches!(error, ServiceError::Launch(_))
        {
            let signal = LaunchSignal::Failed(error.launch_failure());
            log_signal(&instance, &signal);
            let _ = wiring
                .shell
                .update(cx, |shell, cx| shell.apply_launch_signal(signal, cx));
        }
        wiring.state.borrow_mut().cancel = None;
        push_game_log(&wiring, false, cx);
        wiring.state.borrow_mut().game_log = None;
        cx.update(|cx| reload(&wiring, Reload::Lists, cx));
    })
    .detach();
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

/// Dev hook: `LUMILIO_HOME_RECOVERY=failed|details|crashed|crashed-details|exited|exited-details|away`
/// puts the current game through a launch that goes wrong, by the same
/// signals a real one sends, so Recovery can be reviewed without starting a
/// game: `failed` stops at a step (the account must sign in again),
/// `crashed` quits while playing without an exit code, `exited` quits while
/// starting after printing a Java error, `away` fails while Library is open,
/// and the `details` cases also press 技术详情.
pub(super) fn review_recovery(
    wiring: &Wiring,
    which: String,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut App,
) {
    // Home explains a failure only for the current game (D8).
    let current = wiring.backend.service.context().current_instance;
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.library().await.instances });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(records) = lumilio_ui::runtime::answer(handle, cx).await else {
            return;
        };
        let cards = lumilio_ui::live::library_cards(&records, lumilio_core::unix_now());
        let Some(card) = cards
            .into_iter()
            .find(|card| Some(&card.id) == current.as_ref())
        else {
            return;
        };
        let signals = match which.as_str() {
            "crashed" | "crashed-details" => {
                vec![LaunchSignal::Running, LaunchSignal::Exited { code: None }]
            }
            "exited" | "exited-details" => vec![
                LaunchSignal::Phase(lumilio_core::LaunchPhase::Starting),
                LaunchSignal::Failed(LaunchFailure::ExitedEarly {
                    code: Some(1),
                    output: [
                        "Exception in thread \"main\" java.lang.IllegalStateException: Module named org.objectweb.asm was already on the JVMs module path loaded from /launcher/meta/libraries/org/ow2/asm/asm/9.8/asm-9.8.jar but class-path contains it at location /launcher/meta/libraries/org/ow2/asm/asm/9.6/asm-9.6.jar",
                        "\tat cpw.mods.bootstraplauncher@2.0.2/cpw.mods.bootstraplauncher.BootstrapLauncher.run(BootstrapLauncher.java:139)",
                        "\tat cpw.mods.bootstraplauncher@2.0.2/cpw.mods.bootstraplauncher.BootstrapLauncher.main(BootstrapLauncher.java:69)",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                }),
            ],
            _ => vec![
                LaunchSignal::Phase(lumilio_core::LaunchPhase::Verifying),
                LaunchSignal::Failed(ServiceError::SignInRequired("Steve".into()).launch_failure()),
            ],
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            if which == "away" {
                shell.show(lumilio_ui::route::Route::Library);
            }
            shell.set_home(
                HomePresentation::Continue {
                    subject: Subject {
                        id: Some(card.id),
                        title: card.name,
                        metadata: format!("{} · {}", card.meta, card.played),
                        world: card.world,
                    },
                }
                .begin_launch(),
                cx,
            );
            for signal in signals {
                shell.apply_launch_signal(signal, cx);
            }
        });
        if which.ends_with("details") {
            let _ = cx.update_window(window, |_, window, cx| {
                technical_details(&wiring, window, cx)
            });
        }
    })
    .detach();
}

/// 技术详情 on Home's recovery: why a preparation step failed, or what a
/// game that quit while starting printed, in a dialog to read and copy; for
/// a game that crashed, quit silently or did not end cleanly, that game's
/// Diagnostics tab, where its logs and crash reports are.
pub(super) fn technical_details(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    // Deferred: this runs inside one of the shell's own updates.
    window.defer(cx, move |window, cx| {
        let Ok(HomePresentation::Recovery {
            subject, detail, ..
        }) = wiring.shell.read_with(cx, |shell, _| shell.home().clone())
        else {
            return;
        };
        let technical = match &detail {
            RecoveryDetail::Failed { failure, .. } => failure.technical_detail(),
            _ => None,
        };
        match technical {
            Some(text) => lumilio_ui::toast::technical_dialog(text, window, cx),
            None => {
                if let Some(id) = subject.id {
                    open_instance_on(&wiring, id, TAB_DIAGNOSTICS, window, cx);
                }
            }
        }
    });
}

/// Shows the game's output so far on the open page of that game.
pub(super) fn push_game_log(wiring: &Wiring, running: bool, cx: &mut gpui_kit::AsyncApp) {
    let Some((id, lines)) = wiring
        .state
        .borrow()
        .game_log
        .as_ref()
        .map(|(id, lines)| (id.clone(), lines.iter().cloned().collect::<Vec<_>>()))
    else {
        return;
    };
    let _ = wiring
        .shell
        .update(cx, |shell, cx| shell.game_output(&id, lines, running, cx));
}

/// Launches a chosen instance through Home's launch moment. It becomes the
/// current game first (decision D4), so Continue, the chip and the next
/// start agree on what is playing.
pub(super) fn play(wiring: &Wiring, id: String, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    // Deferred: this may be running inside one of the shell's own updates.
    window.defer(cx, move |window, cx| play_now(&wiring, id, window, cx));
}

fn play_now(wiring: &Wiring, id: String, window: &mut Window, cx: &mut App) {
    let can_start = wiring
        .shell
        .read_with(cx, |shell, _| shell.can_start())
        .unwrap_or(false);
    if !can_start {
        // A world or server asked for with a launch that does not happen must
        // not carry over to the next one.
        forget_target(wiring);
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::info(tr!("launch-already-running")), cx)
        });
        return;
    }
    let Ok(saved) = wiring
        .shell
        .update(cx, |shell, _| shell.save_current(id.clone()))
    else {
        return;
    };
    let wiring = wiring.clone();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let failed = match saved.await {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            // The saving worker stopped without answering.
            Err(_) => Some(tr!("common-work-stopped").to_owned()),
        };
        if let Some(cause) = failed {
            forget_target(&wiring);
            let _ = wiring.shell.update(cx, |shell, cx| {
                shell.toast(
                    Toast::error(tr!("library-current-save-failed")).technical(cause),
                    cx,
                );
            });
            return;
        }
        let _ = cx.update_window(window, |_, window, cx| {
            let _ = wiring.shell.update(cx, |shell, cx| {
                // Another launch began while the choice was saved.
                if !shell.can_start() {
                    forget_target(&wiring);
                    return;
                }
                let Some(card) = shell
                    .live()
                    .and_then(|model| model.library.iter().find(|card| card.id == id))
                    .cloned()
                else {
                    forget_target(&wiring);
                    return;
                };
                shell.set_home(
                    HomePresentation::Continue {
                        subject: Subject {
                            id: Some(id),
                            title: card.name,
                            metadata: format!("{} · {}", card.meta, card.played),
                            world: card.world,
                        },
                    },
                    cx,
                );
                shell.request_continue(window, cx);
            });
        });
    })
    .detach();
}

fn forget_target(wiring: &Wiring) {
    let mut state = wiring.state.borrow_mut();
    state.next_world = None;
    state.next_server = None;
}
