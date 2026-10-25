//! The look preview of the account the detail shows, and every account's
//! face. A new preview is made when another account is shown or its skin
//! choice changes, and it asks for its look once; a look that arrives for an
//! account no longer shown is dropped.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{Context, Entity, RenderImage, Window};

use super::accounts_actions::{Accounts, AccountsEvent};
use crate::kit::Failure;
use crate::skin_dialog::SkinDialog;
use crate::skin_view::{Look, SkinViewer};
use crate::tr;
use crate::wardrobe::{Wardrobe, WardrobeAction};

/// Each account's skin face, by key. `Some(None)` is "asked, no face", so an
/// account that has none is not asked for again and again.
pub(super) type Faces = HashMap<String, Option<Arc<RenderImage>>>;

/// The face of an account, if one has been loaded.
pub(super) fn face_of(faces: &Faces, key: &str) -> Option<Arc<RenderImage>> {
    faces.get(key).cloned().flatten()
}

pub(super) struct AccountViewer {
    key: String,
    skin: Option<lumilio_core::SkinChoice>,
    pub(super) view: Entity<SkinViewer>,
    pub(super) revision: u64,
    pub(super) cancel: lumilio_core::CancellationToken,
    pub(super) wardrobe: Option<Entity<Wardrobe>>,
    pub(super) offline: Option<Entity<SkinDialog>>,
}

impl Drop for AccountViewer {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Accounts {
    /// The face of an account, for the corner chip.
    #[must_use]
    pub fn face_of(&self, key: &str) -> Option<Arc<RenderImage>> {
        face_of(&self.faces, key)
    }

    /// Whether the page is on screen. Leaving it drops the detail, which
    /// stops what it was still reading.
    pub fn set_shown(&mut self, shown: bool) {
        if !shown {
            self.viewer = None;
        }
    }

    /// Forgets the faces of accounts that are gone and asks for the face of
    /// every account that does not have one yet. An offline account answers
    /// at once (its skin is local); a Microsoft or third-party account
    /// arrives later, keeping its letter mark until then.
    pub(super) fn ask_faces(&mut self, cx: &mut Context<Self>) {
        let keys: HashSet<&String> = self.inputs.accounts.iter().map(|row| &row.key).collect();
        self.faces.retain(|key, _| keys.contains(key));
        let Some(worker) = self.worker.clone().filter(|_| self.inputs.loaded) else {
            return;
        };
        let wanted: Vec<String> = self
            .inputs
            .accounts
            .iter()
            .filter(|row| !self.faces.contains_key(&row.key))
            .map(|row| row.key.clone())
            .collect();
        for key in wanted {
            // Marked asked before the answer comes, so a slow account is not
            // asked again by the next reading.
            self.faces.insert(key.clone(), None);
            let service = worker.service().clone();
            let wanted = key.clone();
            let face = worker.spawn(async move { service.account_face(&wanted).await });
            cx.spawn(async move |this, cx| {
                let result = match face.await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                let _ = this.update(cx, |accounts, cx| accounts.face_arrived(&key, result, cx));
            })
            .detach();
        }
    }

    /// A face arrived (or could not). A key that is no longer an account is
    /// dropped; a failure keeps the letter mark and is not asked for again.
    pub(super) fn face_arrived(
        &mut self,
        key: &str,
        result: Result<Option<lumilio_core::SkinPixels>, String>,
        cx: &mut Context<Self>,
    ) {
        let image = match result {
            Ok(Some(pixels)) => crate::pixels::image(pixels.width, pixels.height, &pixels.rgba),
            _ => None,
        };
        if image.is_some() && self.inputs.accounts.iter().any(|row| row.key == key) {
            self.faces.insert(key.to_owned(), image);
            cx.emit(AccountsEvent::FacesChanged);
            cx.notify();
        }
    }

    /// Makes the detail's preview, wardrobe and offline form for the account
    /// shown, unless they are already for it and its skin choice.
    pub(super) fn ensure_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, row)) = self.view.shown_account(&self.inputs.accounts) else {
            self.viewer = None;
            return;
        };
        let row = row.clone();
        let (key, skin) = (row.key.clone(), row.skin.clone());
        if self
            .viewer
            .as_ref()
            .is_some_and(|viewer| viewer.key == key && viewer.skin == skin)
        {
            return;
        }
        let view = cx.new(SkinViewer::new);
        self.revision += 1;
        let revision = self.revision;
        let this = cx.entity().downgrade();
        let offline = (!row.signed_in()).then(|| {
            let this = this.clone();
            cx.new(|cx| {
                SkinDialog::new(
                    Rc::new(move |intent, _, cx| {
                        let _ = this.update(cx, |accounts, cx| {
                            accounts.save_inline_skin(revision, intent, cx);
                        });
                    }),
                    key.clone(),
                    row.name.clone(),
                    skin.as_ref(),
                    window,
                    cx,
                )
                .inline()
            })
        });
        let wardrobe = (!row.third_party).then(|| {
            let viewer = view.downgrade();
            let form = offline.clone();
            let shown = key.clone();
            cx.new(|cx| {
                let wardrobe = Wardrobe::new(
                    row.microsoft,
                    Rc::new(move |action, _, cx| {
                        let key = shown.clone();
                        let _ = this.update(cx, |accounts, cx| {
                            accounts.run_wardrobe(key, revision, action, cx);
                        });
                    }),
                    viewer,
                    cx,
                );
                match form {
                    Some(form) => wardrobe.with_offline_form(form),
                    None => wardrobe,
                }
            })
        });
        self.viewer = Some(AccountViewer {
            key: key.clone(),
            skin,
            view,
            revision,
            cancel: lumilio_core::CancellationToken::new(),
            wardrobe,
            offline,
        });
        self.list_wardrobe(key.clone(), revision, true, cx);
        self.load_look(key, revision, cx);
    }

    /// The revision and cancellation token of the detail, while it shows
    /// this account.
    #[must_use]
    pub fn account_detail(&self, key: &str) -> Option<(u64, lumilio_core::CancellationToken)> {
        self.viewer
            .as_ref()
            .filter(|view| view.key == key)
            .map(|view| (view.revision, view.cancel.clone()))
    }

    /// The detail, while it is still the one made at `revision` for `key`.
    pub(super) fn detail_at(&self, key: &str, revision: u64) -> Option<&AccountViewer> {
        self.viewer
            .as_ref()
            .filter(|view| view.key == key && view.revision == revision)
    }

    /// Its wardrobe, likewise.
    pub(crate) fn wardrobe_at(&self, key: &str, revision: u64) -> Option<Entity<Wardrobe>> {
        self.detail_at(key, revision)
            .and_then(|view| view.wardrobe.clone())
    }

    /// Reads what an account looks like, for the detail made at `revision`.
    pub(super) fn load_look(&mut self, key: String, revision: u64, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let wanted = key.clone();
        let look = worker.spawn(async move { service.account_look(&wanted).await });
        cx.spawn(async move |this, cx| {
            let look = match look.await {
                Ok(result) => result.map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            let _ = this.update(cx, |accounts, cx| {
                accounts.look_arrived(&key, revision, look, cx);
            });
        })
        .detach();
    }

    /// The look of an account, as core loaded it (or why it could not). A
    /// look read for an earlier detail of the same account (before its skin
    /// changed) is dropped, so it cannot replace the newer one.
    pub fn look_arrived(
        &mut self,
        key: &str,
        revision: u64,
        look: Result<lumilio_core::AccountLook, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(viewer) = self.detail_at(key, revision) else {
            return;
        };
        let look = look
            .map(|look| Look::from_core(&look))
            .map_err(|detail| (tr!("skin-view-failed").to_owned(), detail));
        viewer
            .view
            .update(cx, |viewer, cx| viewer.set_look(look, cx));
    }

    /// What the wardrobe lists, for the detail made at `revision`.
    pub fn wardrobe_listed(
        &mut self,
        key: &str,
        revision: u64,
        library: Result<Vec<lumilio_core::LibrarySkin>, Failure>,
        profile: Option<Result<lumilio_core::MojangProfile, Failure>>,
        warning: Option<Failure>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self.wardrobe_at(key, revision) {
            view.update(cx, |view, cx| view.listed(library, profile, warning, cx));
        }
    }

    pub(super) fn wardrobe_finished(
        &mut self,
        key: &str,
        revision: u64,
        action: WardrobeAction,
        result: Result<(), Failure>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self.wardrobe_at(key, revision) {
            view.update(cx, |view, cx| view.finished(action, result, cx));
        }
    }

    pub(super) fn inline_skin_saved(
        &mut self,
        key: &str,
        revision: u64,
        result: Result<(), Failure>,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self
            .detail_at(key, revision)
            .and_then(|view| view.offline.clone())
        {
            view.update(cx, |view, cx| view.saved(result, cx));
        }
    }
}
