use super::{ACTIVITY_LIMIT, POLL, Reload, Wiring};
use gpui_kit::{App, Context};
use lumilio_core::{CancellationToken, ProjectKind, unix_now};
use lumilio_ui::LauncherShell;
use lumilio_ui::home::HomePresentation;
use lumilio_ui::live::{
    CollectionRow, account_rows, activity_rows, attention_rows, home_presentation, library_cards,
};
use lumilio_ui::toast::Toast;
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
            shell
                .live()
                .and_then(|model| Some((model.install_target.clone()?, model.query.kind)))
        })
        .ok()
        .flatten();
    let Some((target, kind)) = wanted else {
        return;
    };
    if kind == ProjectKind::Modpack {
        wiring.state.borrow_mut().installed_key = None;
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.installed.clear(), cx)
        });
        return;
    }
    let key = (target.clone(), kind);
    if wiring.state.borrow().installed_key.as_ref() == Some(&key) {
        return;
    }
    wiring.state.borrow_mut().installed_key = Some(key);
    let service = wiring.backend.service.clone();
    let asked = target.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.installed_projects(&asked, kind).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let found = handle.await.ok().and_then(Result::ok).unwrap_or_default();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    // Only if still about the same game and kind.
                    if model.install_target.as_ref() == Some(&target) && model.query.kind == kind {
                        model.installed = found;
                    }
                },
                cx,
            )
        });
    })
    .detach();
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

/// Reloads the library, Activity and (optionally) Home from the service.
pub(super) fn reload(wiring: &Wiring, what: Reload, cx: &mut App) {
    if what == Reload::Nothing {
        return;
    }
    let full = what == Reload::All;
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        let (records, summary) = if full {
            let (records, summary) = service.home().await;
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
        )
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok((records, summary, activity, settings, current, collections)) = handle.await else {
            return;
        };
        let now = unix_now();
        let cards = library_cards(&records, now);
        let rows = activity_rows(&activity, now);
        let attention = summary
            .as_ref()
            .map(|summary| attention_rows(&summary.needs_attention, &cards));
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
                    model.set_library(cards, hint);
                    if let Some(attention) = attention {
                        model.attention = attention;
                    }
                    model.collections = collections
                        .into_iter()
                        .map(|collection| CollectionRow {
                            name: collection.name,
                            members: collection.members,
                        })
                        .collect();
                    model.set_activity(rows, unix_millis());
                    model.accounts = accounts;
                    model.accounts_loaded = true;
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
        Some(Toast::info("已重新开始，进度在动态里")),
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
                    |()| "这次成功了".to_owned(),
                    "还是没有成功".to_owned(),
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
