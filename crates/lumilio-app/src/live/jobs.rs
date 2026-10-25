use std::future::Future;

use gpui_kit::{App, Context};
use lumilio_core::{CancellationToken, unix_now};
use lumilio_ui::live::{CollectionRow, account_rows, activity_rows, library_cards};
use lumilio_ui::runtime::answer;
use lumilio_ui::toast::Toast;
pub(super) use lumilio_ui::toast::outcome;
use lumilio_ui::{LauncherShell, tr};
use tokio::task::JoinError;

use super::{ACTIVITY_LIMIT, POLL, Reload, Wiring};

/// Runs `work` on the worker runtime, then hands the result to `done` on the
/// interface thread and reloads what changed. `started` floats up when it
/// begins; `poll` keeps Activity fresh meanwhile. Work that panics fails like
/// any other, so `done` still runs and says so.
pub(super) fn job<T, E>(
    wiring: &Wiring,
    cx: &mut App,
    poll: bool,
    then: Reload,
    started: Option<Toast>,
    work: impl Future<Output = Result<T, E>> + Send + 'static,
    done: impl FnOnce(&mut LauncherShell, Result<T, E>, &mut Context<LauncherShell>) + 'static,
) where
    T: Send + 'static,
    E: From<JoinError> + Send + 'static,
{
    let handle = wiring.backend.spawn(work);
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        if let Some(toast) = started {
            let _ = wiring.shell.update(cx, |shell, cx| shell.toast(toast, cx));
        }
        while poll && !handle.is_finished() {
            cx.background_executor().timer(POLL).await;
            cx.update(|cx| reload(&wiring, Reload::Lists, cx));
        }
        let output = handle.await.unwrap_or_else(|error| Err(E::from(error)));
        let _ = wiring.shell.update(cx, |shell, cx| done(shell, output, cx));
        cx.update(|cx| {
            reload(&wiring, then, cx);
            refresh_installed(&wiring, cx);
        });
    })
    .detach();
}

/// Reads again what the game installs go into already has of what Discover
/// lists, if that may have changed.
pub(super) fn refresh_installed(wiring: &Wiring, cx: &mut App) {
    if let Ok(discover) = wiring
        .shell
        .read_with(cx, |shell, _| shell.discover().clone())
    {
        discover.update(cx, |discover, cx| discover.refresh_installed(cx));
    }
}

/// What the game installs go into has changed outside Discover: it reads that
/// again.
pub(super) fn installed_changed(wiring: &Wiring, cx: &mut App) {
    if let Ok(discover) = wiring
        .shell
        .read_with(cx, |shell, _| shell.discover().clone())
    {
        discover.update(cx, |discover, cx| discover.invalidate_installed(cx));
    }
}

/// The result of a quick change whose success shows by itself (a star, a
/// shorter list): only a failure is told.
pub(super) fn failed_only<T, E: std::fmt::Display>(
    shell: &mut LauncherShell,
    result: Result<T, E>,
    failed: &str,
    cx: &mut Context<LauncherShell>,
) {
    if let Err(error) = result {
        shell.toast(Toast::error(failed).technical(error.to_string()), cx);
    }
}

/// Reloads the library, Activity, accounts and game settings from the
/// service. Home and what follows the current game are kept up to date by
/// [`super::context::follow`].
pub(super) fn reload(wiring: &Wiring, what: Reload, cx: &mut App) {
    if what == Reload::Nothing {
        return;
    }
    let seq = {
        let mut state = wiring.state.borrow_mut();
        state.reload_seq += 1;
        state.reload_seq
    };
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        let library = service.library().await;
        (
            library.instances,
            library.collections,
            service.activity(ACTIVITY_LIMIT),
            service.settings().await,
            async {
                Ok::<_, lumilio_core::ServiceError>((
                    service.settings_presets().await?,
                    service.all_game_settings().await?,
                ))
            }
            .await,
        )
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some((records, collections, activity, settings, presets)) = answer(handle, cx).await
        else {
            return;
        };
        if wiring.state.borrow().reload_seq != seq {
            return;
        }
        let now = unix_now();
        let cards = library_cards(&records, now);
        let rows = activity_rows(&activity, now);
        let traffic = activity.traffic;
        let accounts = account_rows(&settings);
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    match presets {
                        Ok((presets, instances)) => {
                            model.settings_presets = presets;
                            model.game_settings = instances;
                            model.presets_error = None;
                        }
                        Err(error) => model.presets_error = Some(error.to_string()),
                    }
                    model.set_library(cards);
                    model.collections = collections
                        .into_iter()
                        .map(|collection| CollectionRow {
                            name: collection.name,
                            members: collection.members,
                        })
                        .collect();
                    let now_ms = unix_millis();
                    model.set_activity(rows, now_ms);
                    model.set_traffic(traffic, now_ms);
                    model.set_accounts(accounts);
                },
                cx,
            );
        });
    })
    .detach();
}

/// Runs a failed or cancelled task again from its original input. It is a
/// task of its own in Activity; the old entry stays.
pub(super) fn retry_task(wiring: &Wiring, action: lumilio_core::RetryAction, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let after = wiring.clone();
    job(
        wiring,
        cx,
        true,
        Reload::Lists,
        Some(Toast::info(tr!("activity-retry-started"))),
        async move {
            // Progress lives in Activity; the channel only has to drain.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(async move { while rx.recv().await.is_some() {} });
            service
                .retry_task(action, tx, CancellationToken::new())
                .await
        },
        move |shell, result, cx| {
            shell.toast(
                outcome(
                    result,
                    |()| tr!("activity-retry-ok").to_owned(),
                    tr!("activity-retry-failed").to_owned(),
                ),
                cx,
            );
            // A retried install may have changed what the game has.
            cx.defer(move |cx| installed_changed(&after, cx));
        },
    );
}

/// Milliseconds since the Unix epoch, for working out speeds.
pub(super) fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}
