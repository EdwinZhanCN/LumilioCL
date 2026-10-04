use super::instance::around_the_game;
use super::jobs::reload;
use super::{GAME_LOG_FLUSH, GAME_LOG_LINES, Reload, Wiring};
use gpui_kit::{App, Window};
use lumilio_core::{CancellationToken, LaunchFailure, LaunchSignal, LaunchUpdate, ServiceError};
use lumilio_ui::home::{HomePresentation, Subject};
use lumilio_ui::live::account_failure;
use lumilio_ui::toast::Toast;

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
    let world = wiring.state.borrow_mut().next_world.take();
    let handle = wiring.backend.spawn(async move {
        match world {
            Some(world) => service.launch_world(&id, &world, tx, cancel).await,
            None => service.launch(&id, tx, cancel).await,
        }
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let after = after.await.unwrap_or_default();
        let instance = wiring
            .state
            .borrow()
            .continue_id
            .clone()
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
        // anything earlier (unknown instance, no catalog) has not.
        if let Ok(Err(error)) = handle.await
            && !matches!(error, ServiceError::Launch(_))
        {
            // Sign-in trouble is told in words, not as the raw cause.
            let message = match &error {
                ServiceError::Auth(_) | ServiceError::SignInRequired(_) => {
                    account_failure(&error).0
                }
                _ => error.to_string(),
            };
            let signal = LaunchSignal::Failed(LaunchFailure::Step { message });
            let _ = wiring
                .shell
                .update(cx, |shell, cx| shell.apply_launch_signal(signal, cx));
        }
        wiring.state.borrow_mut().cancel = None;
        push_game_log(&wiring, false, cx);
        wiring.state.borrow_mut().game_log = None;
        cx.update(|cx| reload(&wiring, Reload::All, cx));
    })
    .detach();
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
        let _ = wiring.shell.update(cx, |shell, cx| {
            if matches!(
                shell.home(),
                HomePresentation::Launching { .. } | HomePresentation::Playing { .. }
            ) {
                shell.toast(Toast::info("已经有游戏在运行"), cx);
                return;
            }
            let Some(card) = shell
                .live()
                .and_then(|model| model.library.iter().find(|card| card.id == id))
                .cloned()
            else {
                return;
            };
            state.borrow_mut().continue_id = Some(id);
            shell.set_home(
                HomePresentation::Continue {
                    subject: Subject {
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
