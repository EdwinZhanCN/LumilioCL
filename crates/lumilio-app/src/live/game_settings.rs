//! Game options and presets: opens their dialogs with fresh data, runs what
//! they decide off the interface thread, and answers the dialog and the page.

use std::future::Future;
use std::rc::Rc;

use gpui_kit::{App, AppContext as _, AsyncApp, Context, WeakEntity, Window};
use lumilio_core::{
    GameSettingsError, InstanceOptions, OptionChange, ServiceError, SettingsPreset,
};
use lumilio_ui::game_settings::{
    ChoosePreset, CreatePreset, NamePreset, Naming, OptionDialog, OptionSpec, PresetIntent,
    PresetScope, SourceGame, option_spec,
};
use lumilio_ui::instance_detail::{InstanceDetailView, InstanceIntent, Operated, Section};
use lumilio_ui::kit::ViewIntent;
use lumilio_ui::pages::live::PRESET_GROUP;
use lumilio_ui::toast::Toast;
use lumilio_ui::{LauncherShell, tr};

use super::instance::load_section;
use super::jobs::{answer, reload};
use super::{Reload, Wiring};

/// A preset dialog's failure, as its persistent toast says it.
fn preset_failure(error: &ServiceError) -> String {
    match error {
        ServiceError::GameSettings(GameSettingsError::Stale) => tr!("preset-stale").into(),
        error => tr!("preset-save-failed-detail", detail = error.to_string()),
    }
}

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
    let wiring = wiring.clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        let Some(output) = answer(&wiring, handle, cx).await else {
            return;
        };
        let _ = cx.update_window(window, |_, window, cx| then(output, window, cx));
    })
    .detach();
}

/// Reads the presets, then does `with` with the one asked for.
fn with_preset(
    wiring: &Wiring,
    id: i64,
    window: &Window,
    cx: &mut App,
    with: impl FnOnce(SettingsPreset, &mut Window, &mut App) + 'static,
) {
    let service = wiring.backend.service.clone();
    let shell = wiring.shell.clone();
    run(
        wiring,
        window,
        cx,
        async move { service.settings_presets().await },
        move |result, window, cx| match result {
            Ok(presets) => match presets.into_iter().find(|preset| preset.id == id) {
                Some(preset) => with(preset, window, cx),
                None => toast(&shell, Toast::error(tr!("preset-stale")).sticky(), cx),
            },
            Err(error) => toast(
                &shell,
                Toast::error(tr!("preset-load-failed-detail", detail = error.to_string())).sticky(),
                cx,
            ),
        },
    );
}

fn toast(shell: &WeakEntity<LauncherShell>, toast: Toast, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| shell.toast(toast, cx));
}

/// Shows this preset in the Library's detail.
fn show_preset(wiring: &Wiring, id: i64, cx: &mut App) {
    if let Ok(id) = usize::try_from(id) {
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.apply_view_intent(ViewIntent::Choose(PRESET_GROUP, id), cx)
        });
    }
}

/// A preset write from one of its dialogs. `succeeded` does what follows a
/// success and words its notice; the dialog is answered, then the lists
/// reload so the page shows the result.
fn preset_write<F: 'static, R: Send + 'static>(
    wiring: &Wiring,
    form: WeakEntity<F>,
    window: &Window,
    cx: &mut App,
    work: impl Future<Output = Result<R, ServiceError>> + Send + 'static,
    finished: fn(&mut F, Result<String, String>, &mut Window, &mut Context<F>),
    succeeded: impl FnOnce(R, &Wiring, &mut App) -> String + 'static,
) {
    let wiring = wiring.clone();
    run(
        &wiring.clone(),
        window,
        cx,
        work,
        move |result, window, cx| {
            let answer = match result {
                Ok(output) => Ok(succeeded(output, &wiring, cx)),
                Err(error) => Err(preset_failure(&error)),
            };
            let _ = form.update(cx, |form, cx| finished(form, answer, window, cx));
            reload(&wiring, Reload::Lists, cx);
        },
    );
}

pub(super) fn preset(wiring: &Wiring, intent: PresetIntent, window: &mut Window, cx: &mut App) {
    let service = wiring.backend.service.clone();
    match intent {
        PresetIntent::New => {
            let wiring = wiring.clone();
            run(
                &wiring.clone(),
                window,
                cx,
                async move {
                    (
                        service.library().await.instances,
                        service.current_instance().await,
                    )
                },
                move |(games, current), window, cx| {
                    let games = games
                        .into_iter()
                        .map(|record| SourceGame {
                            id: record.id,
                            name: record.name,
                            version: record.game_version,
                        })
                        .collect();
                    let service = wiring.backend.service.clone();
                    let commit = Rc::new(
                        move |draft: lumilio_ui::game_settings::NewPreset,
                              form: WeakEntity<CreatePreset>,
                              window: &mut Window,
                              cx: &mut App| {
                            let service = service.clone();
                            let name = draft.name.clone();
                            preset_write(
                                &wiring,
                                form,
                                window,
                                cx,
                                async move {
                                    service
                                        .create_settings_preset(
                                            &draft.source,
                                            draft.name,
                                            &draft.groups,
                                        )
                                        .await
                                },
                                CreatePreset::finished,
                                move |id, wiring, cx| {
                                    show_preset(wiring, id, cx);
                                    tr!("preset-created", name = name.as_str())
                                },
                            );
                        },
                    );
                    CreatePreset::open(games, current, commit, window, cx);
                },
            );
        }
        PresetIntent::Rename(id) | PresetIntent::Copy(id) => {
            let naming = if matches!(intent, PresetIntent::Rename(_)) {
                Naming::Rename
            } else {
                Naming::Copy
            };
            let wiring = wiring.clone();
            with_preset(
                &wiring.clone(),
                id,
                window,
                cx,
                move |preset, window, cx| {
                    let original = preset.clone();
                    let commit = Rc::new(
                        move |name: String,
                              form: WeakEntity<NamePreset>,
                              window: &mut Window,
                              cx: &mut App| {
                            let service = wiring.backend.service.clone();
                            let shown = name.clone();
                            let mut renamed = original.clone();
                            renamed.name = name.clone();
                            preset_write(
                                &wiring,
                                form,
                                window,
                                cx,
                                async move {
                                    match naming {
                                        Naming::Rename => {
                                            service.edit_settings_preset(renamed).await.map(|()| id)
                                        }
                                        Naming::Copy => {
                                            service.copy_settings_preset(id, name).await
                                        }
                                    }
                                },
                                NamePreset::finished,
                                move |id, wiring, cx| {
                                    show_preset(wiring, id, cx);
                                    match naming {
                                        Naming::Rename => {
                                            tr!("preset-renamed", name = shown.as_str())
                                        }
                                        Naming::Copy => tr!("preset-copied", name = shown.as_str()),
                                    }
                                },
                            );
                        },
                    );
                    NamePreset::open(naming, &preset, commit, window, cx);
                },
            );
        }
        PresetIntent::Scope(id) => {
            let wiring = wiring.clone();
            with_preset(
                &wiring.clone(),
                id,
                window,
                cx,
                move |preset, window, cx| {
                    let original = preset.clone();
                    let commit = Rc::new(
                        move |excluded,
                              form: WeakEntity<PresetScope>,
                              window: &mut Window,
                              cx: &mut App| {
                            let service = wiring.backend.service.clone();
                            let mut scoped = original.clone();
                            scoped.excluded = excluded;
                            preset_write(
                                &wiring,
                                form,
                                window,
                                cx,
                                async move { service.edit_settings_preset(scoped).await },
                                PresetScope::finished,
                                |(), _, _| tr!("preset-scope-saved").into(),
                            );
                        },
                    );
                    PresetScope::open(preset, commit, window, cx);
                },
            );
        }
        PresetIntent::Delete(id) => {
            super::jobs::job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.delete_settings_preset(id).await },
                |shell, result, cx| {
                    let notice = match result {
                        Ok(()) => Toast::success(tr!("preset-deleted")).sticky(),
                        Err(error) => Toast::error(tr!(
                            "preset-delete-failed-detail",
                            detail = error.to_string()
                        ))
                        .sticky(),
                    };
                    shell.toast(notice, cx);
                },
            );
        }
        PresetIntent::Edit { preset: id, key } => {
            let wiring = wiring.clone();
            with_preset(
                &wiring.clone(),
                id,
                window,
                cx,
                move |preset, window, cx| {
                    let Some(value) = preset.values.get(&key).cloned() else {
                        return;
                    };
                    let (before, edited) = (value.clone(), key.clone());
                    let commit = Rc::new(
                        move |change: OptionChange,
                              form: WeakEntity<OptionDialog>,
                              window: &mut Window,
                              cx: &mut App| {
                            let OptionChange::Set(value) = change else {
                                return;
                            };
                            let service = wiring.backend.service.clone();
                            let (key, before) = (edited.clone(), before.clone());
                            // The row shows the new value; no notice is needed.
                            preset_write(
                                &wiring,
                                form,
                                window,
                                cx,
                                async move { service.set_preset_value(id, key, before, value).await },
                                OptionDialog::finished,
                                |(), _, _| String::new(),
                            );
                        },
                    );
                    OptionDialog::open(
                        OptionSpec {
                            key,
                            value,
                            follow: None,
                            following: false,
                            note: None,
                        },
                        commit,
                        window,
                        cx,
                    );
                },
            );
        }
        PresetIntent::Set {
            preset,
            key,
            before,
            value,
        } => {
            super::jobs::job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.set_preset_value(preset, key, before, value).await },
                |shell, result, cx| {
                    if let Err(error) = result {
                        shell.toast(Toast::error(preset_failure(&error)).sticky(), cx);
                    }
                },
            );
        }
    }
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
