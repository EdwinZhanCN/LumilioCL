//! The look preview of the account the Accounts detail shows. A new preview
//! is made when another account is shown or its skin choice changes, and it
//! asks for its look once; a look that arrives for an account no longer
//! shown is dropped.

use super::LauncherShell;
use crate::live::LiveIntent;
use crate::route::Route;
use crate::skin_view::{Look, SkinViewer};
use crate::{pages, tr};
use crate::{skin_dialog::SkinDialog, wardrobe::Wardrobe};
use gpui::prelude::*;
use gpui::{Context, Entity, Window};
use std::rc::Rc;

pub(super) struct AccountViewer {
    key: String,
    skin: Option<lumilio_core::SkinChoice>,
    pub(super) view: Entity<SkinViewer>,
    revision: u64,
    cancel: lumilio_core::CancellationToken,
    pub(super) wardrobe: Option<Entity<Wardrobe>>,
    pub(super) offline: Option<Entity<SkinDialog>>,
}

impl Drop for AccountViewer {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl LauncherShell {
    /// Asks for the face of every account that does not have one yet. An
    /// offline account answers at once (its skin is local); a Microsoft or
    /// third-party account arrives later, keeping its letter mark until then.
    pub(super) fn ensure_account_faces(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(handler) = self.live_handler.clone() else {
            return;
        };
        let wanted: Vec<String> = {
            let Some(model) = self.live.as_mut() else {
                return;
            };
            if !model.accounts_loaded {
                return;
            }
            let wanted: Vec<String> = model
                .accounts
                .iter()
                .filter(|row| !model.faces.contains_key(&row.key))
                .map(|row| row.key.clone())
                .collect();
            // Marked asked before the answers come, so a slow account is not
            // asked again on the next frame.
            for key in &wanted {
                model.faces.insert(key.clone(), None);
            }
            wanted
        };
        if wanted.is_empty() {
            return;
        }
        window.defer(cx, move |window, cx| {
            for key in wanted {
                handler(LiveIntent::LoadAccountFace(key), window, cx);
            }
        });
    }

    /// A face arrived (or could not). A key that is no longer an account is
    /// dropped; a failure keeps the letter mark and is not asked for again.
    pub fn account_face(
        &mut self,
        key: &str,
        result: Result<Option<lumilio_core::SkinPixels>, String>,
        cx: &mut Context<Self>,
    ) {
        let image = match result {
            Ok(Some(pixels)) => crate::pixels::image(pixels.width, pixels.height, &pixels.rgba),
            _ => None,
        };
        self.update_live(
            |model| {
                if model.accounts.iter().any(|row| row.key == key) {
                    model.faces.insert(key.to_owned(), image);
                }
            },
            cx,
        );
    }

    pub(super) fn ensure_account_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self
            .live
            .as_ref()
            .filter(|_| self.route == Route::Accounts)
            .and_then(|model| pages::live::shown_account(model, &self.view))
            .map(|(_, row)| row.clone());
        let Some(row) = shown else {
            self.account_viewer = None;
            return;
        };
        let (key, skin) = (row.key.clone(), row.skin.clone());
        if self
            .account_viewer
            .as_ref()
            .is_some_and(|viewer| viewer.key == key && viewer.skin == skin)
        {
            return;
        }
        let view = cx.new(SkinViewer::new);
        self.account_revision += 1;
        let revision = self.account_revision;
        let offline = self
            .live_handler
            .clone()
            .filter(|_| !row.signed_in())
            .map(|handler| {
                cx.new(|cx| {
                    SkinDialog::new(
                        Rc::new(move |intent, window, cx| {
                            handler(LiveIntent::SaveInlineSkin { revision, intent }, window, cx)
                        }),
                        key.clone(),
                        row.name,
                        skin.as_ref(),
                        window,
                        cx,
                    )
                    .inline()
                })
            });
        let wardrobe = self
            .live_handler
            .clone()
            .filter(|_| !row.third_party)
            .map(|handler| {
                let viewer = view.downgrade();
                let form = offline.clone();
                cx.new(|cx| {
                    let wardrobe =
                        Wardrobe::new(key.clone(), revision, row.microsoft, handler, viewer, cx);
                    match form {
                        Some(form) => wardrobe.with_offline_form(form),
                        None => wardrobe,
                    }
                })
            });
        self.account_viewer = Some(AccountViewer {
            key: key.clone(),
            skin,
            view,
            revision,
            cancel: lumilio_core::CancellationToken::new(),
            wardrobe,
            offline,
        });
        if let Some(handler) = self.live_handler.clone() {
            window.defer(cx, move |window, cx| {
                handler(LiveIntent::LoadAccountLook(key), window, cx);
            });
        }
    }

    /// A generation and cancellation token for the currently retained detail.
    pub fn account_detail(&self, key: &str) -> Option<(u64, lumilio_core::CancellationToken)> {
        self.account_viewer
            .as_ref()
            .filter(|view| view.key == key)
            .map(|view| (view.revision, view.cancel.clone()))
    }

    pub fn wardrobe_listed(
        &mut self,
        key: &str,
        revision: u64,
        library: Result<Vec<lumilio_core::LibrarySkin>, crate::new_game::Failure>,
        profile: Option<Result<lumilio_core::MojangProfile, crate::new_game::Failure>>,
        warning: Option<crate::new_game::Failure>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.wardrobe.as_ref())
        {
            view.update(cx, |view, cx| view.listed(library, profile, warning, cx));
        }
    }

    pub fn wardrobe_worn(
        &mut self,
        key: &str,
        revision: u64,
        ids: std::collections::HashSet<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.wardrobe.as_ref())
        {
            view.update(cx, |view, cx| view.set_worn(ids, cx));
        }
    }

    /// The pictures of the library skins and the owned capes, for the
    /// wardrobe's tiles.
    pub fn wardrobe_pictures(
        &mut self,
        key: &str,
        revision: u64,
        skins: Vec<(String, lumilio_core::AccountLook)>,
        capes: Option<Vec<(String, lumilio_core::SkinPixels)>>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.wardrobe.as_ref())
        {
            view.update(cx, |view, cx| view.update_pictures(skins, capes, cx));
        }
    }

    pub fn wardrobe_finished(
        &mut self,
        key: &str,
        revision: u64,
        action: crate::wardrobe::WardrobeAction,
        result: Result<(), crate::new_game::Failure>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.wardrobe.as_ref())
        {
            view.update(cx, |view, cx| view.finished(action, result, cx));
        }
    }

    pub fn wardrobe_partially_finished(
        &mut self,
        key: &str,
        revision: u64,
        action: crate::wardrobe::WardrobeAction,
        remaining: crate::wardrobe::WardrobeAction,
        error: crate::new_game::Failure,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.wardrobe.as_ref())
        {
            view.update(cx, |view, cx| {
                view.partially_finished(action, remaining, error, cx)
            });
        }
    }

    pub fn wardrobe_confirmed(
        &mut self,
        key: &str,
        revision: u64,
        result: Result<lumilio_core::MojangProfile, crate::new_game::Failure>,
        matches: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.wardrobe.as_ref())
        {
            view.update(cx, |view, cx| view.confirmed(result, matches, cx));
        }
    }

    pub fn pending_library_skin(&self, key: &str, revision: u64, cx: &gpui::App) -> Option<String> {
        self.account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.wardrobe.as_ref())
            .and_then(|view| view.read(cx).pending_skin())
    }

    pub fn inline_skin_saved(
        &mut self,
        key: &str,
        revision: u64,
        result: Result<(), crate::new_game::Failure>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .account_viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
            .and_then(|view| view.offline.as_ref())
        {
            view.update(cx, |view, cx| view.saved(result, cx));
        }
    }

    /// The look of an account, as core loaded it (or why it could not). A
    /// look read for an earlier detail of the same account (before its skin
    /// changed) is dropped, so it cannot replace the newer one.
    pub fn account_look(
        &mut self,
        key: &str,
        revision: u64,
        look: Result<lumilio_core::AccountLook, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(viewer) = self
            .account_viewer
            .as_ref()
            .filter(|viewer| viewer.key == key && viewer.revision == revision)
        else {
            return;
        };
        let look = look
            .map(|look| Look::from_core(&look))
            .map_err(|detail| (tr!("skin-view-failed").to_owned(), detail));
        viewer
            .view
            .update(cx, |viewer, cx| viewer.set_look(look, cx));
    }
}
