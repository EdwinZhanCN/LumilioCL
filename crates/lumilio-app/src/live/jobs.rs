use super::{ACTIVITY_LIMIT, POLL, Reload, Wiring};
use gpui_kit::{App, AsyncApp, Context};
use lumilio_core::{CancellationToken, ProjectKind, unix_now};
use lumilio_ui::LauncherShell;
use lumilio_ui::home::HomePresentation;
use lumilio_ui::live::{
    CollectionRow, account_rows, activity_rows, attention_rows, home_places, home_presentation,
    library_cards,
};
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;
use std::future::Future;
use tokio::task::{JoinError, JoinHandle};

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

/// Waits for work on the worker runtime. Work that panicked is told on the
/// shell, whatever page is open; work aborted on purpose says nothing. Either
/// way there is no answer.
pub(super) async fn answer<T>(
    wiring: &Wiring,
    handle: JoinHandle<T>,
    cx: &mut AsyncApp,
) -> Option<T> {
    match handle.await {
        Ok(output) => Some(output),
        Err(error) => {
            let error = lumilio_core::ServiceError::from(error);
            if matches!(error, lumilio_core::ServiceError::Panicked(_)) {
                let toast = Toast::error(tr!("common-work-stopped")).technical(error.to_string());
                let _ = wiring.shell.update(cx, |shell, cx| shell.toast(toast, cx));
            }
            None
        }
    }
}

/// Reads again what the target game already has of the searched kind, when
/// the game or the kind changed or something was installed. Failing to ask
/// just shows nothing as installed.
pub(super) fn refresh_installed(wiring: &Wiring, cx: &mut App) {
    let wanted = wiring
        .shell
        .read_with(cx, |shell, _| {
            shell.live().and_then(|model| {
                Some((
                    model.install_target.clone()?,
                    model.query.kind,
                    model.query.source.clone(),
                ))
            })
        })
        .ok()
        .flatten();
    let Some((target, kind, source)) = wanted else {
        return;
    };
    if kind == ProjectKind::Modpack {
        wiring.state.borrow_mut().installed_key = None;
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.installed.clear(), cx)
        });
        return;
    }
    let key = (target.clone(), kind, source.clone());
    if wiring.state.borrow().installed_key.as_ref() == Some(&key) {
        return;
    }
    wiring.state.borrow_mut().installed_key = Some(key);
    let service = wiring.backend.service.clone();
    let (asked, from) = (target.clone(), source.clone());
    let handle = wiring
        .backend
        .spawn(async move { service.installed_projects(&asked, kind, &from).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let found = answer(&wiring, handle, cx)
            .await
            .and_then(Result::ok)
            .unwrap_or_default();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    // Only if still about the same game, kind and source.
                    if model.install_target.as_ref() == Some(&target)
                        && model.query.kind == kind
                        && model.query.source == source
                    {
                        model.installed = found;
                    }
                },
                cx,
            )
        });
    })
    .detach();
}

/// The result of an install: like [`outcome`], except that a file whose
/// author allows downloading it only on the source's website says so and
/// offers that page, and a CurseForge pack without the CurseForge source
/// says where to turn it on.
pub(super) fn install_outcome<T>(
    result: Result<T, lumilio_core::ServiceError>,
    ok: impl FnOnce(T) -> String,
    failed: String,
) -> Toast {
    use lumilio_core::ServiceError;
    match result {
        Err(ServiceError::ManualDownload { file, page }) => {
            let toast = Toast::error(tr!("discover-manual-download", file = file.as_str()));
            match page {
                Some(page) => toast.link(tr!("discover-open-download-page"), page),
                None => toast,
            }
        }
        Err(ServiceError::NoCatalogSource(lumilio_core::Catalog::CurseForge)) => {
            Toast::error(tr!("discover-needs-curseforge"))
        }
        other => outcome(other, ok, failed),
    }
}

/// The result of an operation: success in words, failure as one plain
/// sentence with the raw cause behind 技术详情 (design language §8, §11).
pub(super) fn outcome<T, E: std::fmt::Display>(
    result: Result<T, E>,
    ok: impl FnOnce(T) -> String,
    failed: String,
) -> Toast {
    match result {
        Ok(value) => Toast::success(ok(value)),
        Err(error) => Toast::error(failed).technical(error.to_string()),
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

/// Reloads the library, Activity and (optionally) Home from the service.
pub(super) fn reload(wiring: &Wiring, what: Reload, cx: &mut App) {
    if what == Reload::Nothing {
        return;
    }
    let full = matches!(what, Reload::All | Reload::Selection);
    let (seq, home_seq) = {
        let mut state = wiring.state.borrow_mut();
        state.reload_seq += 1;
        if full {
            state.home_seq += 1;
            if let Some(task) = state.home_diagnostics.take() {
                task.abort();
            }
        }
        (state.reload_seq, state.home_seq)
    };
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        // Saved records suffice to offer Continue. Disk diagnostics arrive
        // separately and must never hold the first usable Home hostage.
        let (records, summary) = if full {
            let (records, summary) = service.home_snapshot().await;
            (records, Some(summary))
        } else {
            (service.library().await.instances, None)
        };
        (
            records,
            summary,
            service.activity(ACTIVITY_LIMIT),
            service.settings().await,
            service.current_instance().await,
            service.library().await.collections,
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
        let Some((records, summary, activity, settings, current, collections, presets)) =
            answer(&wiring, handle, cx).await
        else {
            return;
        };
        if (full && wiring.state.borrow().home_seq != home_seq)
            || (!full && wiring.state.borrow().reload_seq != seq)
        {
            return;
        }
        let now = unix_now();
        let cards = library_cards(&records, now);
        let rows = activity_rows(&activity, now);
        let accounts = account_rows(&settings);
        let home =
            summary.map(|summary| home_presentation(&records, &summary, current.as_deref(), now));
        let hint = match &home {
            Some((_, id)) => {
                wiring.state.borrow_mut().continue_id.clone_from(id);
                id.clone()
            }
            None => {
                // The current instance is what Continue starts.
                if let Some(id) = &current {
                    wiring.state.borrow_mut().continue_id = Some(id.clone());
                }
                wiring.state.borrow().continue_id.clone()
            }
        };
        let hint = current.or(hint);
        let continued = home
            .as_ref()
            .and_then(|(_, id)| id.as_ref())
            .and_then(|id| records.iter().find(|record| &record.id == id))
            .cloned();
        let _ = wiring.shell.update(cx, |shell, cx| {
            // A launch in progress, or a failure being explained, keeps Home.
            if let Some((home, _)) = home
                && matches!(
                    shell.home(),
                    HomePresentation::Loading
                        | HomePresentation::Ambient
                        | HomePresentation::FirstUse
                        | HomePresentation::Continue { .. }
                )
            {
                shell.set_home(home, cx);
            }
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
                    model.set_library(cards, hint);
                    model
                        .attention
                        .retain(|row| records.iter().any(|record| record.id == row.instance));
                    if full {
                        model.home_places = None;
                    }
                    model.collections = collections
                        .into_iter()
                        .map(|collection| CollectionRow {
                            name: collection.name,
                            members: collection.members,
                        })
                        .collect();
                    model.set_activity(rows, unix_millis());
                    model.set_accounts(accounts);
                },
                cx,
            );
        });
        if let Some(record) = continued {
            cx.update(|cx| refresh_places(&wiring, record, home_seq, cx));
        }
        if what == Reload::All {
            cx.update(|cx| refresh_attention(&wiring, home_seq, cx));
        }
    })
    .detach();
}

/// Only the attention rows depend on background diagnostics; the current
/// game's identity stays owned by the saved selection and fast snapshot.
fn refresh_attention(wiring: &Wiring, seq: u64, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move { service.home().await });
    wiring.state.borrow_mut().home_diagnostics = Some(handle.abort_handle());
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some((records, summary)) = answer(&wiring, handle, cx).await else {
            return;
        };
        if wiring.state.borrow().home_seq != seq {
            return;
        }
        wiring.state.borrow_mut().home_diagnostics = None;
        let cards = library_cards(&records, unix_now());
        let attention = attention_rows(&summary.needs_attention, &cards);
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.attention = attention, cx);
        });
    })
    .detach();
}

/// Reads the worlds and servers of the game Home continues. A game that
/// cannot be read just lists nothing; an answer for a game Home has moved
/// away from meanwhile is dropped.
fn refresh_places(wiring: &Wiring, record: lumilio_core::InstanceRecord, seq: u64, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let id = record.id.clone();
    let handle = wiring.backend.spawn(async move {
        (
            service.worlds(&id).await.unwrap_or_default(),
            service.servers(&id).await.unwrap_or_default(),
        )
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some((worlds, servers)) = answer(&wiring, handle, cx).await else {
            return;
        };
        if wiring.state.borrow().home_seq != seq
            || wiring.state.borrow().continue_id.as_ref() != Some(&record.id)
        {
            return;
        }
        let places = home_places(&record, &worlds, &servers);
        let addresses = places.server_addresses();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.home_places = Some(places), cx)
        });
        for address in addresses {
            cx.update(|cx| ask_place(&wiring, record.id.clone(), address, seq, cx));
        }
    })
    .detach();
}

/// Asks one of Home's servers how it is, as the Worlds tab does. An answer
/// for places Home has read again meanwhile is dropped.
fn ask_place(wiring: &Wiring, id: String, address: String, seq: u64, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let (instance, asked) = (id.clone(), address.clone());
    let handle = wiring
        .backend
        .spawn(async move { service.server_status(&instance, &asked).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let answer = answer(&wiring, handle, cx)
            .await
            .and_then(Result::ok)
            .ok_or(());
        if wiring.state.borrow().home_seq != seq {
            return;
        }
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    if let Some(places) = model
                        .home_places
                        .as_mut()
                        .filter(|places| places.instance == id)
                    {
                        places.server_answered(&address, answer);
                    }
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
    let state = wiring.state.clone();
    job(
        wiring,
        cx,
        true,
        Reload::All,
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
            state.borrow_mut().installed_key = None;
            shell.toast(
                outcome(
                    result,
                    |()| tr!("activity-retry-ok").to_owned(),
                    tr!("activity-retry-failed").to_owned(),
                ),
                cx,
            )
        },
    );
}

/// Milliseconds since the Unix epoch, for working out speeds.
pub(super) fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}
