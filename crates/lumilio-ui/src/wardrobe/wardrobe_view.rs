use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, Context, Entity, FocusHandle, RenderImage, Task, WeakEntity, Window};
use lumilio_core::{LibrarySkin, MojangProfile, PairedCape, SkinModel};
use lumilio_skin_render::Texture;

use super::editor;
use super::pictures::arms;
use crate::collections::NamePrompt;
use crate::key::Key;
use crate::kit::Failure;
use crate::live::{LiveHandler, LiveIntent};
use crate::skin_dialog::SkinDialog;
use crate::skin_view::{SkinViewer, Trial};
use crate::{kit, tr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WardrobeAction {
    Reload,
    Import(Vec<PathBuf>),
    SaveCurrent,
    Wear(String),
    Default,
    Cape(Option<String>),
    Model(String, SkinModel),
    Reorder(Vec<String>),
    Remove(String),
    Rename(String, String),
    Replace(String, PathBuf),
    PairCape(String, PairedCape),
    SaveLibraryDraft {
        id: String,
        model: SkinModel,
        cape: PairedCape,
        replacement: Option<PathBuf>,
    },
    WearDraft {
        id: String,
        model: SkinModel,
        cape: PairedCape,
        replacement: Option<PathBuf>,
    },
    ApplyCurrentDraft {
        model: SkinModel,
        cape: PairedCape,
        replacement: Option<PathBuf>,
        model_changed: bool,
        cape_changed: bool,
    },
}

pub struct Wardrobe {
    pub(super) key: String,
    pub(super) revision: u64,
    pub(super) handler: LiveHandler,
    pub(super) microsoft: bool,
    pub(super) viewer: WeakEntity<SkinViewer>,
    pub(super) profile: Option<MojangProfile>,
    pub(super) library: Vec<LibrarySkin>,
    pub(super) worn: HashSet<String>,
    pub(super) busy: bool,
    pub(super) loaded: bool,
    pub(super) error: Option<Failure>,
    pub(super) profile_error: Option<Failure>,
    pub(super) profile_warning: Option<Failure>,
    pub(super) pending: Option<WardrobeAction>,
    pub(super) partial: Option<WardrobeAction>,
    pub(super) before: Option<MojangProfile>,
    /// Library skins and owned capes as textures, by id.
    pub(super) skins: HashMap<String, Arc<Texture>>,
    pub(super) capes: HashMap<String, Arc<Texture>>,
    /// Their pictures; a skin's is keyed by its arm model too.
    pub(super) skin_pictures: HashMap<(String, SkinModel), Arc<RenderImage>>,
    pub(super) cape_pictures: HashMap<String, Arc<RenderImage>>,
    /// The texture each picture was drawn from. Replacing bytes under the same
    /// id must redraw; the id and arm model alone are not the picture.
    pub(super) skin_sources: HashMap<(String, SkinModel), Arc<Texture>>,
    pub(super) cape_sources: HashMap<String, Arc<Texture>>,
    /// Bumped on every picture update so a late draw cannot restore an older texture.
    pub(super) picture_epoch: u64,
    pub(super) old_pictures: Vec<Arc<RenderImage>>,
    pub(super) drawing: Option<Task<()>>,
    pub(super) editor: Option<editor::Editor>,
    /// An offline account's skin source form, shown in place of the grid
    /// while `offline_editing`.
    pub(super) offline: Option<Entity<SkinDialog>>,
    pub(super) offline_editing: bool,
    /// The edit panel (or offline form) takes focus when it opens, so Escape
    /// reaches it; closing it gives focus back to「编辑外观…」.
    pub(super) editor_focus: FocusHandle,
    pub(super) edit_focus: FocusHandle,
    /// The library skin on trial in the preview.
    pub(super) trying: Option<String>,
    pub(super) release_registered: bool,
}

impl Wardrobe {
    #[cfg(test)]
    pub(crate) fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn new(
        key: String,
        revision: u64,
        microsoft: bool,
        handler: LiveHandler,
        viewer: WeakEntity<SkinViewer>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            key,
            revision,
            handler,
            microsoft,
            viewer,
            profile: None,
            library: Vec::new(),
            worn: HashSet::new(),
            busy: false,
            loaded: false,
            error: None,
            profile_error: None,
            profile_warning: None,
            pending: None,
            partial: None,
            before: None,
            skins: HashMap::new(),
            capes: HashMap::new(),
            skin_pictures: HashMap::new(),
            cape_pictures: HashMap::new(),
            skin_sources: HashMap::new(),
            cape_sources: HashMap::new(),
            picture_epoch: 0,
            old_pictures: Vec::new(),
            drawing: None,
            editor: None,
            offline: None,
            offline_editing: false,
            editor_focus: cx.focus_handle(),
            edit_focus: cx.focus_handle(),
            trying: None,
            release_registered: false,
        }
    }

    /// The skin source form an offline account edits its look with.
    #[must_use]
    pub fn with_offline_form(mut self, form: Entity<SkinDialog>) -> Self {
        self.offline = Some(form);
        self
    }

    pub(super) fn request(
        &mut self,
        action: WardrobeAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        (self.handler)(
            LiveIntent::Wardrobe {
                key: self.key.clone(),
                revision: self.revision,
                action,
            },
            window,
            cx,
        );
        cx.notify();
    }

    pub fn listed(
        &mut self,
        library: Result<Vec<LibrarySkin>, Failure>,
        profile: Option<Result<MojangProfile, Failure>>,
        warning: Option<Failure>,
        cx: &mut Context<Self>,
    ) {
        self.loaded = true;
        match library {
            Ok(entries) => self.library = entries,
            Err(error) => self.error = Some(error),
        }
        if let Some(result) = profile {
            self.profile_warning = warning;
            match result {
                Ok(profile) => {
                    self.profile = Some(profile);
                    self.profile_error = None;
                }
                Err(error) => self.profile_error = Some(error),
            }
        }
        if let Some(id) = &self.trying
            && !self.library.iter().any(|entry| &entry.id == id)
        {
            self.try_on(None, cx);
        }
        cx.notify();
    }

    pub fn set_worn(&mut self, ids: HashSet<String>, cx: &mut Context<Self>) {
        self.worn = ids;
        cx.notify();
    }

    /// Shows a library skin on trial in the preview, or ends the trial. On
    /// a Microsoft account the skin brings its paired cape; elsewhere the
    /// account's cape stays.
    pub(super) fn try_on(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        let trial = id.as_ref().and_then(|id| {
            let entry = self.library.iter().find(|entry| &entry.id == id)?;
            let skin = self.skins.get(id)?.clone();
            let cape = match (&entry.cape, self.microsoft) {
                (PairedCape::Hidden, true) => Some(None),
                (PairedCape::Cape(cape), true) => self.capes.get(cape).cloned().map(Some),
                _ => None,
            };
            Some(Trial {
                skin: Some(skin),
                arms: arms(entry.model),
                cape,
            })
        });
        self.trying = id.filter(|_| trial.is_some());
        let _ = self
            .viewer
            .update(cx, |viewer, cx| viewer.try_on(trial, cx));
        cx.notify();
    }

    pub fn finished(
        &mut self,
        action: WardrobeAction,
        result: Result<(), Failure>,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        match result {
            Err(error) => self.error = Some(error),
            Ok(()) => {
                self.error = None;
                if matches!(action, WardrobeAction::Cape(_)) {
                    self.partial = None;
                }
                if self.microsoft
                    && matches!(
                        action,
                        WardrobeAction::Wear(_)
                            | WardrobeAction::WearDraft { .. }
                            | WardrobeAction::ApplyCurrentDraft { .. }
                            | WardrobeAction::Default
                            | WardrobeAction::Cape(_)
                    )
                {
                    self.before = self.profile.clone();
                    self.pending = Some(action);
                    self.busy = true;
                } else if matches!(
                    action,
                    WardrobeAction::Wear(_) | WardrobeAction::WearDraft { .. }
                ) {
                    // An offline account's look reloads with its new choice.
                    self.try_on(None, cx);
                    self.editor = None;
                } else if matches!(action, WardrobeAction::SaveLibraryDraft { .. }) {
                    self.editor = None;
                }
            }
        }
        cx.notify();
    }

    pub fn partially_finished(
        &mut self,
        action: WardrobeAction,
        remaining: WardrobeAction,
        error: Failure,
        cx: &mut Context<Self>,
    ) {
        self.error = Some(error);
        self.before = self.profile.clone();
        self.pending = Some(action);
        self.partial = Some(remaining);
        self.busy = true;
        if let Some(editor) = &mut self.editor {
            editor.original.model = editor.edited.model;
            editor.original.replacement = None;
            editor.edited.replacement = None;
        }
        cx.notify();
    }

    pub fn pending_skin(&self) -> Option<String> {
        match &self.pending {
            Some(WardrobeAction::Wear(id)) => Some(id.clone()),
            Some(WardrobeAction::WearDraft {
                id,
                replacement: None,
                ..
            }) => Some(id.clone()),
            _ => None,
        }
    }

    pub fn confirmed(
        &mut self,
        result: Result<MojangProfile, Failure>,
        matches: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        match result {
            Ok(profile) => {
                let confirmed = if self.partial.is_some() {
                    self.before.as_ref().and_then(MojangProfile::active_skin)
                        != profile.active_skin()
                } else {
                    match &self.pending {
                        Some(WardrobeAction::Default) => profile.active_skin().is_none(),
                        Some(WardrobeAction::Cape(id)) => {
                            profile.active_cape().map(|cape| &cape.id) == id.as_ref()
                        }
                        Some(WardrobeAction::Wear(id)) => matches.unwrap_or_else(|| {
                            self.library
                                .iter()
                                .find(|entry| &entry.id == id)
                                .is_some_and(|entry| {
                                    profile
                                        .active_skin()
                                        .is_some_and(|skin| skin.model == entry.model)
                                        && self.before.as_ref().and_then(MojangProfile::active_skin)
                                            != profile.active_skin()
                                })
                        }),
                        Some(WardrobeAction::WearDraft {
                            model, replacement, ..
                        }) => {
                            if replacement.is_some() {
                                profile
                                    .active_skin()
                                    .is_some_and(|skin| skin.model == *model)
                                    && self.before.as_ref().and_then(MojangProfile::active_skin)
                                        != profile.active_skin()
                            } else {
                                matches.unwrap_or(false)
                            }
                        }
                        Some(WardrobeAction::ApplyCurrentDraft {
                            model,
                            cape,
                            replacement,
                            model_changed,
                            cape_changed,
                        }) => {
                            let skin_ready = !(replacement.is_some() || *model_changed)
                                || profile
                                    .active_skin()
                                    .is_some_and(|skin| skin.model == *model)
                                    && self.before.as_ref().and_then(MojangProfile::active_skin)
                                        != profile.active_skin();
                            skin_ready
                                && (!cape_changed
                                    || match cape {
                                        PairedCape::Hidden => profile.active_cape().is_none(),
                                        PairedCape::Cape(id) => {
                                            profile.active_cape().is_some_and(|item| &item.id == id)
                                        }
                                        PairedCape::Keep => true,
                                    })
                        }
                        _ => true,
                    }
                };
                if confirmed {
                    if matches!(
                        self.pending,
                        Some(WardrobeAction::Wear(_) | WardrobeAction::WearDraft { .. })
                    ) {
                        // The worn skin is now the account's own look.
                        self.try_on(None, cx);
                    }
                    self.pending = None;
                    self.before = None;
                    if self.partial.is_none() {
                        self.editor = None;
                    }
                }
                self.profile = Some(profile);
                self.profile_error = None;
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    pub(super) fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &'static str,
        action: WardrobeAction,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Key {
        let weak = cx.weak_entity();
        Key::new(id)
            .label(label)
            .white()
            .disabled(self.busy || !enabled)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |view, cx| view.request(action.clone(), window, cx));
            })
    }

    pub(super) fn can_wear(&self) -> bool {
        !self.microsoft
            || self.profile_error.is_none()
                && self.profile_warning.is_none()
                && self.profile.is_some()
    }

    pub(super) fn rename(
        &self,
        skin: &LibrarySkin,
        cx: &Context<Self>,
    ) -> impl Fn(&mut Window, &mut App) + use<> {
        let weak = cx.weak_entity();
        let (id, name) = (skin.id.clone(), skin.name.clone());
        move |window, cx| {
            let weak = weak.clone();
            let id = id.clone();
            let form = cx.new(|cx| {
                NamePrompt::new(
                    Some(name.clone()),
                    Vec::new(),
                    Rc::new(move |name, window, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.request(WardrobeAction::Rename(id.clone(), name), window, cx)
                        });
                    }),
                    window,
                    cx,
                )
                .with_placeholder(tr!("wardrobe-rename-placeholder"), window, cx)
            });
            NamePrompt::open(
                form,
                tr!("wardrobe-rename-title"),
                tr!("collection-rename-confirm"),
                window,
                cx,
            );
        }
    }

    pub(super) fn menu(
        &self,
        ix: usize,
        skin: &LibrarySkin,
        cx: &Context<Self>,
    ) -> Vec<kit::MenuEntry> {
        let mut menu = Vec::new();
        for (label, to) in [
            (tr!("wardrobe-up"), ix.checked_sub(1)),
            (
                tr!("wardrobe-down"),
                (ix + 1 < self.library.len()).then_some(ix + 1),
            ),
        ] {
            if let Some(to) = to {
                let mut ids: Vec<_> = self.library.iter().map(|skin| skin.id.clone()).collect();
                ids.swap(ix, to);
                let weak = cx.weak_entity();
                menu.push(kit::MenuEntry::new(label, move |window, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        view.request(WardrobeAction::Reorder(ids.clone()), window, cx)
                    });
                }));
            }
        }
        // ia[accounts]: 编辑库里的皮肤 | 皮肤 ⋯ 菜单「改名…」「编辑…」 | 改名只改显示的名字；贴图、模型与披风先成草稿，保存后更新库条目
        menu.push(kit::MenuEntry::new(
            tr!("wardrobe-rename"),
            self.rename(skin, cx),
        ));
        let edit = cx.weak_entity();
        let id = skin.id.clone();
        menu.push(kit::MenuEntry::new(
            tr!("wardrobe-edit-skin"),
            move |window, cx| {
                let _ = edit.update(cx, |view, cx| {
                    view.try_on(Some(id.clone()), cx);
                    view.open_editor(window, cx);
                });
            },
        ));
        let remove = skin.id.clone();
        let remove_view = cx.weak_entity();
        menu.push(
            kit::MenuEntry::new(tr!("common-remove"), move |window, cx| {
                let _ = remove_view.update(cx, |view, cx| {
                    view.request(WardrobeAction::Remove(remove.clone()), window, cx)
                });
            })
            .danger(),
        );
        menu
    }
}
