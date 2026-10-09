use super::*;
use gpui::AnyElement;
use lumilio_core::{PairedCape, SkinModel};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Target {
    Current,
    Library(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Values {
    pub(super) model: SkinModel,
    pub(super) cape: PairedCape,
    pub(super) replacement: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(super) struct Editor {
    pub(super) target: Target,
    pub(super) original: Values,
    pub(super) edited: Values,
    replacement_texture: Option<Arc<Texture>>,
}

impl Editor {
    pub(super) fn new(target: Target, model: SkinModel, cape: PairedCape) -> Self {
        let original = Values {
            model,
            cape,
            replacement: None,
        };
        Self {
            target,
            edited: original.clone(),
            original,
            replacement_texture: None,
        }
    }

    pub(super) fn changed(&self) -> bool {
        self.original != self.edited
    }
}

impl Wardrobe {
    pub(super) fn open_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.editor_focus, cx);
        if self.trying.is_none() && !self.microsoft {
            // An offline look is a skin source, not a texture draft.
            self.offline_editing = self.offline.is_some();
            cx.notify();
            return;
        }
        self.editor = if let Some(id) = &self.trying {
            self.library
                .iter()
                .find(|entry| &entry.id == id)
                .map(|entry| {
                    Editor::new(Target::Library(id.clone()), entry.model, entry.cape.clone())
                })
        } else {
            self.profile.as_ref().map(|profile| {
                Editor::new(
                    Target::Current,
                    profile
                        .active_skin()
                        .map_or(SkinModel::Wide, |skin| skin.model),
                    profile
                        .active_cape()
                        .map_or(PairedCape::Hidden, |cape| PairedCape::Cape(cape.id.clone())),
                )
            })
        };
        self.preview_editor(cx);
        cx.notify();
    }

    /// Drops the draft, shows the account's own look again and gives focus
    /// back to「编辑外观…」.
    pub(super) fn cancel_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.offline_editing = false;
        self.editor = None;
        self.try_on(None, cx);
        window.focus(&self.edit_focus, cx);
    }

    pub(super) fn preview_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(viewer) = self.viewer.upgrade() else {
            return;
        };
        let own = viewer.read(cx).own_look().cloned();
        let skin = editor
            .replacement_texture
            .clone()
            .or_else(|| match &editor.target {
                Target::Current => own.as_ref().and_then(|look| look.skin.clone()),
                Target::Library(id) => self.skins.get(id).cloned(),
            });
        let cape = match &editor.edited.cape {
            PairedCape::Keep => own.and_then(|look| look.cape),
            PairedCape::Hidden => None,
            PairedCape::Cape(id) => self.capes.get(id).cloned(),
        };
        viewer.update(cx, |viewer, cx| {
            viewer.try_on(
                Some(Trial {
                    skin,
                    arms: arms(editor.edited.model),
                    cape: Some(cape),
                }),
                cx,
            )
        });
    }

    fn selected_replacement(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.edited.replacement = Some(path.clone());
        editor.replacement_texture = None;
        let task = cx.background_executor().spawn(async move {
            std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    lumilio_core::skin_pixels(&bytes).map_err(|error| error.to_string())
                })
                .and_then(|pixels| texture(&pixels).ok_or_else(|| "invalid texture".into()))
                .map(|texture| (path, texture))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok((path, texture)) => {
                        if let Some(editor) = &mut view.editor
                            && editor.edited.replacement.as_ref() == Some(&path)
                        {
                            editor.replacement_texture = Some(texture);
                            view.error = None;
                            view.preview_editor(cx);
                        }
                    }
                    Err(detail) => {
                        view.error = Some((tr!("wardrobe-editor-invalid-image").into(), detail))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn editor_action(&self, apply: bool) -> Option<WardrobeAction> {
        if self.partial.is_some() {
            return None;
        }
        if apply
            && self.microsoft
            && (self.profile_error.is_some() || self.profile_warning.is_some())
        {
            return None;
        }
        let editor = self.editor.as_ref()?;
        if editor.edited.replacement.is_some() && editor.replacement_texture.is_none() {
            return None;
        }
        match &editor.target {
            Target::Library(id) if apply => Some(WardrobeAction::WearDraft {
                id: id.clone(),
                model: editor.edited.model,
                cape: editor.edited.cape.clone(),
                replacement: editor.edited.replacement.clone(),
            }),
            Target::Library(id) if editor.changed() => Some(WardrobeAction::SaveLibraryDraft {
                id: id.clone(),
                model: editor.edited.model,
                cape: editor.edited.cape.clone(),
                replacement: editor.edited.replacement.clone(),
            }),
            Target::Current if apply && editor.changed() => {
                Some(WardrobeAction::ApplyCurrentDraft {
                    model: editor.edited.model,
                    cape: editor.edited.cape.clone(),
                    replacement: editor.edited.replacement.clone(),
                    model_changed: editor.edited.model != editor.original.model,
                    cape_changed: editor.edited.cape != editor.original.cape,
                })
            }
            _ => None,
        }
    }

    fn draft_button(
        &self,
        id: &'static str,
        label: &'static str,
        action: Option<WardrobeAction>,
        cx: &Context<Self>,
    ) -> Key {
        let weak = cx.weak_entity();
        let disabled = self.busy || action.is_none();
        Key::new(id)
            .label(label)
            .white()
            .disabled(disabled)
            .on_click(move |_, window, cx| {
                if let Some(action) = &action {
                    let _ = weak.update(cx, |view, cx| view.request(action.clone(), window, cx));
                }
            })
    }

    pub(super) fn render_editor(&self, colors: ShellColors, cx: &Context<Self>) -> AnyElement {
        let editor = self.editor.as_ref().expect("editor is open");
        let target = match &editor.target {
            Target::Current => tr!("wardrobe-editor-current").to_owned(),
            Target::Library(id) => self
                .library
                .iter()
                .find(|entry| &entry.id == id)
                .map_or_else(|| id.clone(), |entry| entry.name.clone()),
        };
        let weak = cx.weak_entity();
        let replace = Key::new("wardrobe-editor-replace")
            .label(tr!("wardrobe-editor-replace"))
            .white()
            .disabled(self.busy)
            .on_click(move |_, window, cx| {
                let chosen =
                    crate::platform::pick_path(cx, true, false, tr!("account-skin-pick-picture"));
                let handle = window.window_handle();
                let weak = weak.clone();
                cx.spawn(async move |cx| {
                    if let Some(path) = chosen.await {
                        let _ = cx.update_window(handle, |_, _, cx| {
                            let _ = weak.update(cx, |view, cx| {
                                if view.editor.is_some() {
                                    view.selected_replacement(path, cx);
                                }
                            });
                        });
                    }
                })
                .detach();
            });
        let model_view = cx.weak_entity();
        let model = kit::segments(
            "wardrobe-editor-model",
            tr_all!["account-skin-model-classic", "account-skin-model-slim"],
            usize::from(editor.edited.model == SkinModel::Slim),
            move |ix, _, cx| {
                let _ = model_view.update(cx, |view, cx| {
                    if let Some(editor) = &mut view.editor {
                        editor.edited.model = if ix == 1 {
                            SkinModel::Slim
                        } else {
                            SkinModel::Wide
                        };
                        view.preview_editor(cx);
                    }
                });
            },
        );
        // A cape is its picture with its name under it; no cape is an empty well.
        let cape_tile = |id: String, label: String, cape: PairedCape| {
            let weak = cx.weak_entity();
            let picture = match &cape {
                PairedCape::Cape(id) => self.cape_pictures.get(id),
                _ => None,
            };
            let selector = id.clone();
            v_flex()
                .w(px(CAPE_THUMB.0))
                .gap_1()
                .child(
                    tile(
                        gpui::ElementId::Name(id.into()),
                        CAPE_THUMB,
                        picture,
                        editor.edited.cape == cape,
                        colors,
                    )
                    .role(gpui::Role::Button)
                    .aria_label(label.clone())
                    .debug_selector(move || selector.clone())
                    .when(matches!(cape, PairedCape::Hidden), |tile| {
                        tile.flex().items_center().justify_center().child(
                            gpui_component::Icon::new(crate::assets::UiIcon::Close)
                                .size(px(18.))
                                .text_color(colors.muted),
                        )
                    })
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            if !view.busy
                                && let Some(editor) = &mut view.editor
                            {
                                editor.edited.cape = cape.clone();
                                view.preview_editor(cx);
                            }
                        });
                    }),
                )
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .text_center()
                        .text_color(colors.muted)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(label),
                )
        };
        let mut capes = h_flex().flex_wrap().items_start().gap_3().child(cape_tile(
            "wardrobe-editor-no-cape".into(),
            tr!("wardrobe-no-cape").into(),
            PairedCape::Hidden,
        ));
        if let Some(profile) = &self.profile {
            for cape in &profile.capes {
                capes = capes.child(cape_tile(
                    format!("wardrobe-editor-cape-{}", cape.id),
                    cape.alias.clone(),
                    PairedCape::Cape(cape.id.clone()),
                ));
            }
        }
        let cancel = cx.weak_entity();
        let save = self.editor_action(false);
        let apply = self.editor_action(true);
        // Cancel first, the commit last and orange, as a dialog's footer.
        let mut actions = h_flex().w_full().flex_wrap().justify_end().gap_2().child(
            Key::new("wardrobe-editor-cancel")
                .label(tr!("common-cancel"))
                .white()
                .debug_selector(|| "wardrobe-editor-cancel".into())
                .on_click(move |_, window, cx| {
                    let _ = cancel.update(cx, |view, cx| view.cancel_editor(window, cx));
                }),
        );
        if let Some(action) = self.partial.clone() {
            actions = actions.child(self.button(
                "wardrobe-editor-retry-cape",
                tr!("wardrobe-retry-cape"),
                action,
                true,
                cx,
            ));
        }
        if matches!(editor.target, Target::Library(_)) {
            actions = actions.child(self.draft_button(
                "wardrobe-editor-save",
                tr!("wardrobe-editor-save"),
                save,
                cx,
            ));
        }
        // ia[accounts]: 应用外观草稿 | 编辑面「应用到账户」 | 仅提交变更；已接受的在线写入显示同步中，失败保留草稿
        actions = actions.child(
            self.draft_button(
                "wardrobe-editor-apply",
                tr!("wardrobe-editor-apply"),
                apply,
                cx,
            )
            .primary()
            .debug_selector(|| "wardrobe-editor-apply".into()),
        );
        let field = |label: &'static str, body: AnyElement| {
            v_flex()
                .gap_2()
                .child(kit::section_label(label, colors))
                .child(body)
        };
        let escape = cx.weak_entity();
        v_flex()
            .id("wardrobe-editor")
            .debug_selector(|| "wardrobe-editor".into())
            .track_focus(&self.editor_focus)
            .w_full()
            .gap_5()
            .on_key_down(move |event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    let _ = escape.update(cx, |view, cx| view.cancel_editor(window, cx));
                    cx.stop_propagation();
                }
            })
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_base()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(tr!("wardrobe-edit-title")),
                    )
                    .child(div().text_xs().text_color(colors.muted).child(target)),
            )
            .child(field(
                tr!("wardrobe-editor-texture"),
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(replace)
                    .children(editor.edited.replacement.as_ref().map(|path| {
                        div()
                            .min_w_0()
                            .text_xs()
                            .text_color(colors.muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(
                                path.file_name()
                                    .unwrap_or(path.as_os_str())
                                    .to_string_lossy()
                                    .into_owned(),
                            )
                    }))
                    .into_any_element(),
            ))
            .child(field(
                tr!("wardrobe-editor-arms"),
                h_flex().child(model).into_any_element(),
            ))
            .children(
                self.microsoft
                    .then(|| field(tr!("wardrobe-editor-cape"), capes.into_any_element())),
            )
            .child(actions)
            .children(self.error.clone().map(|(message, detail)| {
                v_flex()
                    .gap_2()
                    .child(div().text_sm().text_color(colors.danger).child(message))
                    .child(kit::technical("wardrobe-editor-technical", detail))
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_changes_do_not_change_its_original_values() {
        let mut draft = Editor::new(Target::Current, SkinModel::Wide, PairedCape::Hidden);
        assert!(!draft.changed());
        draft.edited.model = SkinModel::Slim;
        draft.edited.cape = PairedCape::Cape("owned".into());
        draft.edited.replacement = Some(PathBuf::from("new.png"));
        assert!(draft.changed());
        assert_eq!(draft.original.model, SkinModel::Wide);
        assert_eq!(draft.original.cape, PairedCape::Hidden);
        assert!(draft.original.replacement.is_none());
    }
}
