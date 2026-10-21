use super::instance::{around_the_game, open_instance_on};
use super::jobs::{answer, reload};
use super::{GAME_LOG_FLUSH, GAME_LOG_LINES, Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, Window};
use lumilio_core::{CancellationToken, LaunchFailure, LaunchSignal, LaunchUpdate, ServiceError};
use lumilio_ui::home::{HomePresentation, RecoveryDetail, Subject};
use lumilio_ui::instance_detail::TAB_DIAGNOSTICS;
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;

pub(super) fn start_launch(wiring: &Wiring, window: gpui_kit::AnyWindowHandle, cx: &mut App) {
    let Some(id) = wiring.state.borrow().continue_id.clone() else {
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
    let handle = wiring.backend.spawn(async move {
        match (world, server) {
            (Some(world), _) => service.launch_world(&id, &world, tx, cancel).await,
            (None, Some(address)) => service.launch_server(&id, &address, tx, cancel).await,
            (None, None) => service.launch(&id, tx, cancel).await,
        }
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let after = answer(&wiring, after, cx).await.unwrap_or_default();
        let instance = wiring
            .state
            .borrow()
            .continue_id
            .clone()
            .unwrap_or_default();
        wiring.state.borrow_mut().game_log = Some((instance.clone(), Default::default()));
        // The page can tell the game is on its way before it prints anything.
        push_game_log(&wiring, true, cx);
        cx.update(|cx| {
            super::instance_plugins::studio_windows_game_changed(&wiring, &instance, true, cx)
        });
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
            let _ = wiring
                .shell
                .update(cx, |shell, cx| shell.apply_launch_signal(signal, cx));
        }
        wiring.state.borrow_mut().cancel = None;
        push_game_log(&wiring, false, cx);
        let ended = wiring.state.borrow_mut().game_log.take().map(|(id, _)| id);
        if let Some(id) = ended {
            cx.update(|cx| {
                super::instance_plugins::studio_windows_game_changed(&wiring, &id, false, cx)
            });
        }
        cx.update(|cx| reload(&wiring, Reload::All, cx));
    })
    .detach();
}

/// Dev hook: `LUMILIO_HOME_RECOVERY=failed|details|crashed|crashed-details|away`
/// puts the current game through a launch that goes wrong, by the same
/// signals a real one sends, so Recovery can be reviewed without starting a
/// game: `failed` stops at a step (the account must sign in again),
/// `crashed` quits while playing without an exit code, `away` fails while
/// Library is open, and the `details` cases also press 技术详情.
pub(super) fn review_recovery(
    wiring: &Wiring,
    which: String,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        (
            service.library().await.instances,
            service.current_instance().await,
        )
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some((records, current)) = answer(&wiring, handle, cx).await else {
            return;
        };
        let cards = lumilio_ui::live::library_cards(&records, lumilio_core::unix_now());
        let Some(card) = cards
            .iter()
            .find(|card| Some(&card.id) == current.as_ref())
            .or(cards.first())
            .cloned()
        else {
            return;
        };
        wiring.state.borrow_mut().continue_id = Some(card.id.clone());
        let signals = match which.as_str() {
            "crashed" | "crashed-details" => {
                vec![LaunchSignal::Running, LaunchSignal::Exited { code: None }]
            }
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
                    recent: Vec::new(),
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

/// 技术详情 on Home's recovery: why a preparation step failed, in a dialog
/// to read and copy; for a game that quit or a session that did not end
/// cleanly, that game's Diagnostics tab, where its logs and crash reports are.
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
        match detail {
            RecoveryDetail::Failed {
                failure: LaunchFailure::Step { message, .. },
                ..
            } => lumilio_ui::toast::technical_dialog(message, window, cx),
            _ => {
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

/// Launches a chosen instance through Home's launch moment.
pub(super) fn play(wiring: &Wiring, id: String, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    // Deferred: this may be running inside one of the shell's own updates.
    window.defer(cx, move |window, cx| {
        let state = wiring.state.clone();
        // A world or server asked for with a launch that does not happen must
        // not carry over to the next one.
        let forget_target = || {
            let mut state = state.borrow_mut();
            state.next_world = None;
            state.next_server = None;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            if !shell.home().can_start() {
                forget_target();
                shell.toast(Toast::info(tr!("launch-already-running")), cx);
                return;
            }
            let Some(card) = shell
                .live()
                .and_then(|model| model.library.iter().find(|card| card.id == id))
                .cloned()
            else {
                forget_target();
                return;
            };
            state.borrow_mut().continue_id = Some(id.clone());
            shell.set_home(
                HomePresentation::Continue {
                    subject: Subject {
                        id: Some(id),
                        title: card.name,
                        metadata: format!("{} · {}", card.meta, card.played),
                        world: card.world,
                    },
                    recent: Vec::new(),
                },
                cx,
            );
            shell.request_continue(window, cx);
        });
    });
}
