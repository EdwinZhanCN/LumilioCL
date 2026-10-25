//! What the 预设 tab asks: opens a preset's dialogs with fresh data, saves
//! what they decide off the interface thread, and answers the dialog.

use std::future::Future;
use std::rc::Rc;

use gpui::{App, AppContext as _, Context, WeakEntity, Window};
use lumilio_core::{GameSettingsError, OptionChange, ServiceError, SettingsPreset};

use super::library_actions::{Library, LibraryEvent};
use crate::game_settings::{
    CreatePreset, NamePreset, Naming, NewPreset, OptionDialog, OptionSpec, PresetIntent,
    PresetScope, SourceGame,
};
use crate::runtime::answer;
use crate::toast::{self, Toast};
use crate::tr;

/// A preset dialog's failure, as its persistent toast says it.
fn preset_failure(error: &ServiceError) -> String {
    match error {
        ServiceError::GameSettings(GameSettingsError::Stale) => tr!("preset-stale").into(),
        error => tr!("preset-save-failed-detail", detail = error.to_string()),
    }
}

impl Library {
    pub fn preset(&mut self, intent: PresetIntent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service() else {
            return;
        };
        let this = cx.entity().downgrade();
        match intent {
            PresetIntent::New => self.in_window(
                window,
                cx,
                async move {
                    (
                        service.library().await.instances,
                        service.current_instance().await,
                    )
                },
                move |_, (games, current), window, cx| {
                    let games = games
                        .into_iter()
                        .map(|record| SourceGame {
                            id: record.id,
                            name: record.name,
                            version: record.game_version,
                        })
                        .collect();
                    let commit = Rc::new(
                        move |draft: NewPreset,
                              form: WeakEntity<CreatePreset>,
                              window: &mut Window,
                              cx: &mut App| {
                            let _ = this.update(cx, |library, cx| {
                                library.create_preset(draft, form, window, cx)
                            });
                        },
                    );
                    CreatePreset::open(games, current, commit, window, cx);
                },
            ),
            PresetIntent::Rename(id) | PresetIntent::Copy(id) => {
                let naming = if matches!(intent, PresetIntent::Rename(_)) {
                    Naming::Rename
                } else {
                    Naming::Copy
                };
                self.with_preset(id, window, cx, move |preset, window, cx| {
                    let original = preset.clone();
                    let commit = Rc::new(
                        move |name: String,
                              form: WeakEntity<NamePreset>,
                              window: &mut Window,
                              cx: &mut App| {
                            let original = original.clone();
                            let _ = this.update(cx, |library, cx| {
                                library.name_preset(naming, original, name, form, window, cx)
                            });
                        },
                    );
                    NamePreset::open(naming, &preset, commit, window, cx);
                });
            }
            PresetIntent::Scope(id) => {
                self.with_preset(id, window, cx, move |preset, window, cx| {
                    let original = preset.clone();
                    let commit = Rc::new(
                        move |excluded,
                              form: WeakEntity<PresetScope>,
                              window: &mut Window,
                              cx: &mut App| {
                            let mut scoped = original.clone();
                            scoped.excluded = excluded;
                            let _ = this.update(cx, |library, cx| {
                                let Some(service) = library.service() else {
                                    return;
                                };
                                library.preset_write(
                                    form,
                                    window,
                                    cx,
                                    async move { service.edit_settings_preset(scoped).await },
                                    PresetScope::finished,
                                    |_, (), _| tr!("preset-scope-saved").into(),
                                );
                            });
                        },
                    );
                    PresetScope::open(preset, commit, window, cx);
                });
            }
            PresetIntent::Delete(id) => self.change(
                cx,
                async move { service.delete_settings_preset(id).await },
                |result, cx| {
                    let notice = match result {
                        Ok(()) => Toast::success(tr!("preset-deleted")).sticky(),
                        Err(error) => Toast::error(tr!(
                            "preset-delete-failed-detail",
                            detail = error.to_string()
                        ))
                        .sticky(),
                    };
                    toast::post(notice, cx);
                },
            ),
            PresetIntent::Edit { preset: id, key } => {
                self.with_preset(id, window, cx, move |preset, window, cx| {
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
                            let (key, before) = (edited.clone(), before.clone());
                            let _ = this.update(cx, |library, cx| {
                                let Some(service) = library.service() else {
                                    return;
                                };
                                // The row shows the new value; no notice is needed.
                                library.preset_write(
                                    form,
                                    window,
                                    cx,
                                    async move {
                                        service.set_preset_value(id, key, before, value).await
                                    },
                                    OptionDialog::finished,
                                    |_, (), _| String::new(),
                                );
                            });
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
                });
            }
            PresetIntent::Set {
                preset,
                key,
                before,
                value,
            } => self.change(
                cx,
                async move { service.set_preset_value(preset, key, before, value).await },
                |result, cx| {
                    if let Err(error) = result {
                        toast::post(Toast::error(preset_failure(&error)).sticky(), cx);
                    }
                },
            ),
        }
    }

    fn create_preset(
        &mut self,
        draft: NewPreset,
        form: WeakEntity<CreatePreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service() else {
            return;
        };
        let name = draft.name.clone();
        self.preset_write(
            form,
            window,
            cx,
            async move {
                service
                    .create_settings_preset(&draft.source, draft.name, &draft.groups)
                    .await
            },
            CreatePreset::finished,
            move |library, id, cx| {
                library.show_preset(id, cx);
                tr!("preset-created", name = name.as_str())
            },
        );
    }

    fn name_preset(
        &mut self,
        naming: Naming,
        original: SettingsPreset,
        name: String,
        form: WeakEntity<NamePreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service() else {
            return;
        };
        let id = original.id;
        let shown = name.clone();
        let mut renamed = original;
        renamed.name = name.clone();
        self.preset_write(
            form,
            window,
            cx,
            async move {
                match naming {
                    Naming::Rename => service.edit_settings_preset(renamed).await.map(|()| id),
                    Naming::Copy => service.copy_settings_preset(id, name).await,
                }
            },
            NamePreset::finished,
            move |library, id, cx| {
                library.show_preset(id, cx);
                match naming {
                    Naming::Rename => tr!("preset-renamed", name = shown.as_str()),
                    Naming::Copy => tr!("preset-copied", name = shown.as_str()),
                }
            },
        );
    }

    /// Runs `work` off the interface thread, then `then` in the window.
    fn in_window<R: Send + 'static>(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
        work: impl Future<Output = R> + Send + 'static,
        then: impl FnOnce(&mut Self, R, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let handle = worker.spawn(work);
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let Some(output) = answer(handle, cx).await else {
                return;
            };
            let _ = cx.update_window(window, |_, window, cx| {
                let _ = this.update(cx, |this, cx| then(this, output, window, cx));
            });
        })
        .detach();
    }

    /// Reads the presets, then does `with` with the one asked for.
    fn with_preset(
        &self,
        id: i64,
        window: &Window,
        cx: &mut Context<Self>,
        with: impl FnOnce(SettingsPreset, &mut Window, &mut App) + 'static,
    ) {
        let Some(service) = self.service() else {
            return;
        };
        self.in_window(
            window,
            cx,
            async move { service.settings_presets().await },
            move |_, result, window, cx| match result {
                Ok(presets) => match presets.into_iter().find(|preset| preset.id == id) {
                    Some(preset) => with(preset, window, cx),
                    None => toast::post(Toast::error(tr!("preset-stale")).sticky(), cx),
                },
                Err(error) => toast::post(
                    Toast::error(tr!("preset-load-failed-detail", detail = error.to_string()))
                        .sticky(),
                    cx,
                ),
            },
        );
    }

    /// A preset write from one of its dialogs. `succeeded` does what follows a
    /// success and words its notice; the dialog is answered, then the lists
    /// are read again so the page shows the result.
    fn preset_write<F: 'static, R: Send + 'static>(
        &self,
        form: WeakEntity<F>,
        window: &Window,
        cx: &mut Context<Self>,
        work: impl Future<Output = Result<R, ServiceError>> + Send + 'static,
        finished: fn(&mut F, Result<String, String>, &mut Window, &mut Context<F>),
        succeeded: impl FnOnce(&mut Self, R, &mut Context<Self>) -> String + 'static,
    ) {
        self.in_window(window, cx, work, move |this, result, window, cx| {
            let answer = match result {
                Ok(output) => Ok(succeeded(this, output, cx)),
                Err(error) => Err(preset_failure(&error)),
            };
            let _ = form.update(cx, |form, cx| finished(form, answer, window, cx));
            cx.emit(LibraryEvent::ListsChanged);
        });
    }
}
