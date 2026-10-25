//! A game's options: opens their dialogs with fresh data, runs what they
//! decide off the interface thread, and answers the dialog and the page. The
//! presets themselves are the Library's (`lumilio_ui::features::library`).

use std::future::Future;
use std::rc::Rc;

use gpui_kit::{App, AppContext as _, AsyncApp, WeakEntity, Window};
use lumilio_core::{
    GameSettingsError, InstanceOptions, OptionChange, ServiceError, SettingsPreset,
};
use lumilio_ui::game_settings::{ChoosePreset, OptionDialog, option_spec};
use lumilio_ui::instance_detail::{InstanceDetailView, InstanceIntent, Operated, Section};
use lumilio_ui::toast::Toast;
use lumilio_ui::{LauncherShell, tr};

use super::instance::load_section;
use super::jobs::reload;
use super::{Reload, Wiring};

/// Runs `work` off the interface thread, then `then` in the window.
fn run<R: Send + 'static>(
    wiring: &Wiring,
    window: &Window,
    cx: &mut App,
    work: impl Future<Output = R> + Send + 'static,
    then: impl FnOnce(R, &mut Window, &mut App) + 'static,
) {
    let handle = wiring.backend.spawn(work);
    let window = window.window_handle();
    cx.spawn(async move |cx: &mut AsyncApp| {
        let Some(output) = lumilio_ui::runtime::answer(handle, cx).await else {
            return;
        };
        let _ = cx.update_window(window, |_, window, cx| then(output, window, cx));
    })
    .detach();
}

fn toast(shell: &WeakEntity<LauncherShell>, toast: Toast, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| shell.toast(toast, cx));
}

/// Reads a game's options and the presets for a dialog about that game.
fn with_options(
    wiring: &Wiring,
    id: String,
    window: &Window,
    cx: &mut App,
    with: impl FnOnce(InstanceOptions, Vec<SettingsPreset>, &mut Window, &mut App) + 'static,
) {
    let service = wiring.backend.service.clone();
    let shell = wiring.shell.clone();
    run(
        wiring,
        window,
        cx,
        async move {
            let options = service.game_options(&id).await?;
            Ok::<_, ServiceError>((options, service.settings_presets().await?))
        },
        move |result, window, cx| match result {
            Ok((options, presets)) => with(options, presets, window, cx),
            Err(error) => toast(
                &shell,
                Toast::error(tr!("options-load-failed")).technical(error.to_string()),
                cx,
            ),
        },
    );
}

/// A game's options changed: the page reads them again, and so do the lists
/// (Home's attention and the preset page).
fn options_changed(wiring: &Wiring, id: &str, view: &WeakEntity<InstanceDetailView>, cx: &mut App) {
    load_section(
        wiring,
        id.to_owned(),
        view.clone(),
        Section::GameOptions,
        cx,
    );
    reload(wiring, Reload::Lists, cx);
}

/// What the game's options page failure says.
fn options_failure(error: &ServiceError) -> String {
    match error {
        ServiceError::GameSettings(GameSettingsError::Stale) => tr!("options-stale").into(),
        _ => tr!("options-save-failed").into(),
    }
}

pub(super) fn choose_preset(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    with_options(
        &wiring.clone(),
        id.clone(),
        window,
        cx,
        move |options, presets, window, cx| {
            let names: Vec<(i64, String)> = presets
                .iter()
                .map(|preset| (preset.id, preset.name.clone()))
                .collect();
            let commit = Rc::new(
                move |chosen: Option<i64>,
                      form: WeakEntity<ChoosePreset>,
                      window: &mut Window,
                      cx: &mut App| {
                    let service = wiring.backend.service.clone();
                    let (target, done, view, id) =
                        (id.clone(), wiring.clone(), view.clone(), id.clone());
                    let notice = match chosen
                        .and_then(|chosen| names.iter().find(|(id, _)| *id == chosen))
                    {
                        Some((_, name)) => tr!("options-bound", name = name.as_str()),
                        None => tr!("options-unbound").into(),
                    };
                    run(
                        &wiring,
                        window,
                        cx,
                        async move { service.bind_settings_preset(&target, chosen).await },
                        move |result, window, cx| {
                            let answer = result
                                .map(|()| notice)
                                .map_err(|error| format!("{}\n{error}", options_failure(&error)));
                            let _ = form.update(cx, |form, cx| form.finished(answer, window, cx));
                            options_changed(&done, &id, &view, cx);
                        },
                    );
                },
            );
            ChoosePreset::open(presets, options, commit, window, cx);
        },
    );
}

pub(super) fn edit_option(
    wiring: &Wiring,
    id: String,
    key: String,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    with_options(
        &wiring.clone(),
        id.clone(),
        window,
        cx,
        move |options, _, window, cx| {
            let Some(spec) = option_spec(&options, &key) else {
                return;
            };
            let expected = options.view;
            let commit = Rc::new(
                move |change: OptionChange,
                      form: WeakEntity<OptionDialog>,
                      window: &mut Window,
                      cx: &mut App| {
                    let service = wiring.backend.service.clone();
                    let (expected, key, done, view, id) = (
                        expected.clone(),
                        key.clone(),
                        wiring.clone(),
                        view.clone(),
                        id.clone(),
                    );
                    run(
                        &wiring,
                        window,
                        cx,
                        async move { service.edit_game_option(expected, key, change).await },
                        move |result, window, cx| {
                            // The table shows the new value; no notice is needed.
                            let answer = result
                                .map(|()| String::new())
                                .map_err(|error| format!("{}\n{error}", options_failure(&error)));
                            let _ = form.update(cx, |form, cx| form.finished(answer, window, cx));
                            options_changed(&done, &id, &view, cx);
                        },
                    );
                },
            );
            OptionDialog::open(spec, commit, window, cx);
        },
    );
}

/// A write the options page started itself (a switch, a decision on the
/// game's changes, a recovery check). The page is disabled until it answers.
pub(super) fn write_option(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    intent: InstanceIntent,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let target = id.clone();
    let work = async move {
        match intent {
            InstanceIntent::SetOption {
                expected,
                key,
                change,
            } => service
                .edit_game_option(expected, key, change)
                .await
                .map(|()| String::new()),
            InstanceIntent::ResolveOptions {
                expected,
                selected,
                resolution,
            } => {
                let count = selected.len();
                service
                    .resolve_game_settings(expected, selected, resolution)
                    .await
                    .map(|()| tr!("options-resolved", count = count))
            }
            InstanceIntent::RecoverOptions => service
                .recover_game_settings(&target)
                .await
                .map(|()| tr!("options-checked").into()),
            _ => unreachable!("only option writes reach this function"),
        }
    };
    let handle = wiring.backend.spawn(work);
    let wiring = wiring.clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        let result = handle.await.unwrap_or_else(|error| Err(error.into()));
        let conflicts = match &result {
            Err(ServiceError::GameSettings(GameSettingsError::Conflict(keys))) => keys.clone(),
            _ => Vec::new(),
        };
        let done = match result {
            Ok(notice) => Operated {
                notice,
                technical: None,
                refresh: vec![Section::GameOptions],
            },
            Err(error) => Operated {
                notice: if conflicts.is_empty() {
                    options_failure(&error)
                } else {
                    tr!("options-conflict", count = conflicts.len())
                },
                technical: Some(error.to_string()),
                // A refused write still shows what is true now.
                refresh: vec![Section::GameOptions],
            },
        };
        let stale = view
            .update(cx, |view, cx| {
                if !conflicts.is_empty() {
                    view.options_conflicted(conflicts, cx);
                }
                view.operated(done, cx)
            })
            .unwrap_or_default();
        cx.update(|cx| {
            for section in stale {
                load_section(&wiring, id.clone(), view.clone(), section, cx);
            }
            reload(&wiring, Reload::Lists, cx);
        });
    })
    .detach();
}
