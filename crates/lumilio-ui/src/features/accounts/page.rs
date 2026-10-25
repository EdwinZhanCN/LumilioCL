use std::rc::Rc;

use gpui::{Context, IntoElement, Render, Window};

use super::accounts_actions::Accounts;
use super::view::{AccountsCtx, accounts};
use crate::kit::{Emit, ViewIntent};
use crate::live::{AccountRow, LiveHandler};
use crate::theme::ShellColors;

/// The view-state group that remembers which account the detail shows.
pub const ACCOUNT_GROUP: u8 = 220;

/// What the page shows of the rest of the launcher, as the shell knows it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountsInputs {
    pub accounts: Vec<AccountRow>,
    /// The accounts have been read at least once.
    pub loaded: bool,
}

/// Which account the detail shows, by its place in the list; `None` is the
/// current account.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AccountsView {
    pub shown: Option<usize>,
}

impl AccountsView {
    pub(super) fn apply(&mut self, intent: ViewIntent) {
        if let ViewIntent::Choose(ACCOUNT_GROUP, index) = intent {
            self.shown = Some(index);
        }
    }

    /// The account the detail shows: the one last chosen in the list, or the
    /// current account.
    #[must_use]
    pub fn shown_account<'a>(&self, accounts: &'a [AccountRow]) -> Option<(usize, &'a AccountRow)> {
        let current = accounts.iter().position(|row| row.selected);
        let index = self
            .shown
            .unwrap_or(current.unwrap_or(0))
            .min(accounts.len().checked_sub(1)?);
        accounts.get(index).map(|row| (index, row))
    }
}

impl Accounts {
    /// Where the page sends what other features answer: the shell's handler.
    pub fn set_handler(&mut self, handler: LiveHandler) {
        self.handler = Some(handler);
    }

    /// What the page shows, as the shell knows it. Accounts without a face
    /// yet are asked for one.
    pub fn set_inputs(&mut self, inputs: AccountsInputs, cx: &mut Context<Self>) {
        if self.inputs != inputs {
            self.inputs = inputs;
            self.ask_faces(cx);
            cx.notify();
        }
    }

    #[must_use]
    pub fn view(&self) -> &AccountsView {
        &self.view
    }

    /// Shows the current account in the detail, as the corner chip's
    /// 管理账户 does.
    pub fn show_current(&mut self, cx: &mut Context<Self>) {
        self.view.shown = None;
        cx.notify();
    }

    /// Applies a choice made on the page: the account the detail shows.
    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.view.apply(intent);
        cx.notify();
    }
}

impl Render for Accounts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_viewer(window, cx);
        let this = cx.entity().downgrade();
        let emit: Emit = Rc::new(move |intent, _, cx| {
            let _ = this.update(cx, |accounts, cx| accounts.apply_view_intent(intent, cx));
        });
        let ctx = AccountsCtx {
            colors: ShellColors::current(cx),
            inputs: &self.inputs,
            view: &self.view,
            faces: &self.faces,
            emit,
            handler: self
                .handler
                .clone()
                .unwrap_or_else(|| Rc::new(|_, _, _| {})),
            viewer: self.viewer.as_ref().map(|viewer| &viewer.view),
            wardrobe: self
                .viewer
                .as_ref()
                .and_then(|viewer| viewer.wardrobe.as_ref()),
            window_width: f32::from(window.viewport_size().width),
        };
        accounts(&ctx).into_any_element()
    }
}
