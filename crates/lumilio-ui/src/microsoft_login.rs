//! The Microsoft sign-in dialog (IA `accounts.md`): one sentence, then
//! "在浏览器中登录", then the code to type while the launcher waits, and a
//! failure in words. It holds no protocol: starting and cancelling go through
//! [`SignInIntent`], and the answers come back through
//! [`SignInDialog::code_arrived`], [`SignInDialog::failed`] and
//! [`SignInDialog::signed_in`].

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, px};
use gpui_component::dialog::Dialog;
use gpui_component::{Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex};

use crate::key::Key;
use crate::new_game::Failure;
use crate::theme::ShellColors;
use crate::{kit, theme, tr};

const DIALOG_WIDTH: f32 = 440.;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignInIntent {
    /// Ask for a code and start waiting for the browser.
    Start,
    /// Stop waiting; nothing is saved.
    Cancel,
}

pub type SignInHandler = Rc<dyn Fn(SignInIntent, &mut Window, &mut App)>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Phase {
    /// Nothing asked for yet.
    Intro,
    /// Waiting for the code to arrive.
    Requesting,
    /// The code is on screen; waiting for the browser.
    Waiting {
        code: String,
        address: String,
    },
    Cancelled,
    Failed(Failure),
}

pub struct SignInDialog {
    handler: SignInHandler,
    phase: Phase,
    /// A finished sign-in closes the dialog on the next render.
    close: bool,
}

impl SignInDialog {
    pub fn new(handler: SignInHandler) -> Self {
        Self {
            handler,
            phase: Phase::Intro,
            close: false,
        }
    }

    /// Whether the dialog is busy with a sign-in and must be left through
    /// "取消" (so the wait really stops).
    fn active(&self) -> bool {
        matches!(self.phase, Phase::Requesting | Phase::Waiting { .. })
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active() {
            return;
        }
        self.phase = Phase::Requesting;
        cx.notify();
        (self.handler)(SignInIntent::Start, window, cx);
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active() {
            self.phase = Phase::Cancelled;
            (self.handler)(SignInIntent::Cancel, window, cx);
        }
        window.close_dialog(cx);
    }

    /// The code to type, and the address to type it at. Returns whether this
    /// sign-in still accepts the code, so its browser can open.
    pub fn code_arrived(&mut self, code: &str, address: &str, cx: &mut Context<Self>) -> bool {
        if self.close || self.phase != Phase::Requesting {
            return false;
        }
        self.phase = Phase::Waiting {
            code: code.to_owned(),
            address: address.to_owned(),
        };
        crate::toast::copy_text(code, cx);
        cx.notify();
        true
    }

    /// The sign-in did not work; the dialog says why and offers a retry.
    pub fn failed(&mut self, failure: Failure, cx: &mut Context<Self>) {
        self.phase = Phase::Failed(failure);
        cx.notify();
    }

    /// The account is in: the dialog closes itself.
    pub fn signed_in(&mut self, cx: &mut Context<Self>) {
        self.close = true;
        cx.notify();
    }

    /// Opens the dialog around `form`.
    pub fn open(form: Entity<Self>, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |dialog, _, cx| Self::dialog(&form, dialog, cx));
    }

    fn dialog(form: &Entity<Self>, dialog: Dialog, cx: &mut App) -> Dialog {
        let this = form.read(cx);
        let active = this.active();
        let waiting_for_code = this.phase == Phase::Requesting;
        let retry = matches!(this.phase, Phase::Failed(_));
        let weak = form.downgrade();
        let start = {
            let weak = weak.clone();
            theme::clickable(
                Key::new("signin-start")
                    .label(if retry {
                        tr!("account-microsoft-retry")
                    } else {
                        tr!("account-microsoft-sign-in")
                    })
                    .primary()
                    .loading(waiting_for_code)
                    .disabled(active)
                    .debug_selector(|| "signin-start".into())
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |form, cx| form.start(window, cx));
                    }),
                !active,
            )
        };
        let cancel = theme::clickable(
            Key::new("signin-cancel")
                .label(if active {
                    tr!("account-microsoft-cancel-sign-in")
                } else {
                    tr!("common-close")
                })
                .white()
                .debug_selector(|| "signin-cancel".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |form, cx| form.cancel(window, cx));
                }),
            true,
        );
        theme::dialog(dialog, cx)
            .title(tr!("account-microsoft-title"))
            .w(px(DIALOG_WIDTH))
            .keyboard(!active)
            .overlay_closable(!active)
            .close_button(!active)
            .child(form.clone())
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(cancel)
                    .child(start),
            )
    }
}

impl Render for SignInDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        let colors = ShellColors::current(cx);
        let body = match &self.phase {
            Phase::Intro => v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.foreground)
                        .child(tr!("account-microsoft-intro")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!("account-microsoft-storage-note")),
                )
                .into_any_element(),
            Phase::Requesting => div()
                .text_sm()
                .text_color(colors.muted)
                .child(tr!("account-microsoft-requesting"))
                .into_any_element(),
            Phase::Cancelled => div().into_any_element(),
            Phase::Waiting { code, address } => {
                let (copy_code, open_address) = (code.clone(), address.clone());
                v_flex()
                    .gap_3()
                    .child(div().text_sm().text_color(colors.muted).child(tr!(
                        "account-microsoft-code-lead",
                        address = address.as_str()
                    )))
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .rounded(px(10.))
                            .bg(colors.surface_subtle)
                            .debug_selector(|| "signin-code".into())
                            .text_size(crate::theme::font_px(28.))
                            .font_semibold()
                            .text_color(colors.foreground)
                            .child(code.clone()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                kit::ghost(
                                    "signin-copy",
                                    tr!("account-microsoft-copy-code"),
                                    move |_, cx| {
                                        crate::toast::copy_text(copy_code.clone(), cx);
                                    },
                                )
                                .debug_selector(|| "signin-copy".into()),
                            )
                            .child(
                                kit::ghost(
                                    "signin-open",
                                    tr!("account-microsoft-reopen"),
                                    move |_, cx| {
                                        crate::platform::open_address(&open_address, cx);
                                    },
                                )
                                .debug_selector(|| "signin-open".into()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(tr!("account-microsoft-waiting")),
                    )
                    .into_any_element()
            }
            Phase::Failed((message, technical)) => h_flex()
                .gap_2()
                .items_center()
                .debug_selector(|| "signin-error".into())
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.danger)
                        .child(message.clone()),
                )
                .child(kit::technical("signin-technical", technical.clone()).xsmall())
                .into_any_element(),
        };
        v_flex().id("signin").w_full().gap_3().child(body)
    }
}

#[cfg(test)]
mod tests;
