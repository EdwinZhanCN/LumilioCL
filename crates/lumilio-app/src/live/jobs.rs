use super::{ACTIVITY_LIMIT, POLL, Reload, Wiring};
use gpui_kit::{App, Context};
use lumilio_core::{CancellationToken, ProjectKind, unix_now};
use lumilio_ui::LauncherShell;
use lumilio_ui::live::{CollectionRow, account_rows, activity_rows, library_cards};
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;
use std::future::Future;

/// Runs `work` on the worker runtime, then hands the result to `done` on the
/// interface thread and reloads what changed. `started` floats up when it
/// begins; `poll` keeps Activity fresh meanwhile.
pub(super) fn job<R: Send + 'static>(
    wiring: &Wiring,
    cx: &mut App,
    poll: bool,
    then: Reload,
    started: Option<Toast>,
    work: impl Future<Output = R> + Send + 'static,
    done: impl FnOnce(&mut LauncherShell, R, &mut Context<LauncherShell>) + 'static,
) {
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
        let Ok(output) = handle.await else { return };
        let _ = wiring.shell.update(cx, |shell, cx| done(shell, output, cx));
        cx.update(|cx| {
            reload(&wiring, then, cx);
            refresh_installed(&wiring, cx);
        });
    })
    .detach();
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
        let found = handle.await.ok().and_then(Result::ok).unwrap_or_default();
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
        let Ok((records, collections, activity, settings, presets)) = handle.await else {
            return;
        };
        if wiring.state.borrow().reload_seq != seq {
            return;
        }
        let now = unix_now();
        let cards = library_cards(&records, now);
        let rows = activity_rows(&activity, now);
        let accounts = account_rows(&settings);
        let current = wiring.backend.service.context().current_instance;
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
                    model.set_library(cards, current);
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
