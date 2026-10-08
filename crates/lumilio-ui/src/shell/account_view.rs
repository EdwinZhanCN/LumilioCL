//! The look preview of the account the Accounts detail shows. A new preview
//! is made when another account is shown or its skin choice changes, and it
//! asks for its look once; a look that arrives for an account no longer
//! shown is dropped.

use super::LauncherShell;
use crate::live::LiveIntent;
use crate::route::Route;
use crate::skin_view::{Look, SkinViewer};
use crate::{pages, tr};
use gpui::prelude::*;
use gpui::{Context, Entity, Window};

pub(super) struct AccountViewer {
    key: String,
    skin: Option<lumilio_core::SkinChoice>,
    pub(super) view: Entity<SkinViewer>,
}

impl LauncherShell {
    pub(super) fn ensure_account_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self
            .live
            .as_ref()
            .filter(|_| self.route == Route::Accounts)
            .and_then(|model| pages::live::shown_account(model, &self.view))
            .map(|(_, row)| (row.key.clone(), row.skin.clone()));
        let Some((key, skin)) = shown else {
            self.account_viewer = None;
            return;
        };
        if self
            .account_viewer
            .as_ref()
            .is_some_and(|viewer| viewer.key == key && viewer.skin == skin)
        {
            return;
        }
        let view = cx.new(SkinViewer::new);
        self.account_viewer = Some(AccountViewer {
            key: key.clone(),
            skin,
            view,
        });
        if let Some(handler) = self.live_handler.clone() {
            window.defer(cx, move |window, cx| {
                handler(LiveIntent::LoadAccountLook(key), window, cx);
            });
        }
    }

    /// The look of an account, as core loaded it (or why it could not).
    pub fn account_look(
        &mut self,
        key: &str,
        look: Result<lumilio_core::AccountLook, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(viewer) = self
            .account_viewer
            .as_ref()
            .filter(|viewer| viewer.key == key)
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
