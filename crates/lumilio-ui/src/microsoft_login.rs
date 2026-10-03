//! The Microsoft sign-in dialog (IA `accounts.md`): one sentence, then
//! "在浏览器中登录", then the code to type while the launcher waits, and a
//! failure in words. It holds no protocol: starting and cancelling go through
//! [`SignInIntent`], and the answers come back through
//! [`SignInDialog::code_arrived`], [`SignInDialog::failed`] and
//! [`SignInDialog::signed_in`].

use std::rc::Rc;

use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};

use crate::kit;
use crate::new_game::Failure;
use crate::theme::{self, ShellColors};

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
            (self.handler)(SignInIntent::Cancel, window, cx);
        }
        window.close_dialog(cx);
    }

    /// The code to type, and the address to type it at.
    pub fn code_arrived(&mut self, code: &str, address: &str, cx: &mut Context<Self>) {
        if self.phase == Phase::Requesting {
            self.phase = Phase::Waiting {
                code: code.to_owned(),
                address: address.to_owned(),
            };
            cx.notify();
        }
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
                        "再试一次"
                    } else {
                        "在浏览器中登录"
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
                .label(if active { "取消登录" } else { "关闭" })
                .white()
                .debug_selector(|| "signin-cancel".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |form, cx| form.cancel(window, cx));
                }),
            true,
        );
        theme::dialog(dialog, cx)
            .title("登录 Microsoft")
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
        let colors = ShellColors::from_theme(cx.theme());
        let body = match &self.phase {
            Phase::Intro => v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.foreground)
                        .child("点“在浏览器中登录”，在打开的页面输入一个代码，再回到这里。"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child("登录信息只保存在系统凭据库里，不会写进启动器的文件。"),
                )
                .into_any_element(),
            Phase::Requesting => div()
                .text_sm()
                .text_color(colors.muted)
                .child("正在向 Microsoft 要一个登录代码…")
                .into_any_element(),
            Phase::Waiting { code, address } => {
                let (copy_code, open_address) = (code.clone(), address.clone());
                v_flex()
                    .gap_3()
                    .child(div().text_sm().text_color(colors.muted).child(format!(
                        "在 {address} 输入这个代码，然后在页面上登录并确认："
                    )))
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .rounded(px(10.))
                            .bg(colors.surface_subtle)
                            .debug_selector(|| "signin-code".into())
                            .text_size(px(28.))
                            .font_semibold()
                            .text_color(colors.foreground)
                            .child(code.clone()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                kit::ghost("signin-copy", "复制代码", move |_, cx| {
                                    crate::toast::copy_text(copy_code.clone(), cx);
                                })
                                .debug_selector(|| "signin-copy".into()),
                            )
                            .child(
                                kit::ghost("signin-open", "重新打开页面", move |_, cx| {
                                    crate::platform::open_address(&open_address, cx);
                                })
                                .debug_selector(|| "signin-open".into()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child("正在等你在浏览器里完成登录…"),
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
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Host;

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    #[gpui::test]
    fn the_dialog_walks_from_intro_to_code_to_success_and_cancels_what_it_started(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::Modifiers;
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let (_, cx) = cx.add_window_view(|window, cx| {
            let host = cx.new(|_| Host);
            gpui_component::Root::new(host, window, cx)
        });
        let seen: Rc<RefCell<Vec<SignInIntent>>> = Rc::default();
        let sink = seen.clone();
        let form = cx.update(|_, cx| {
            cx.new(|_| {
                SignInDialog::new(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
            })
        });
        cx.update(|window, cx| SignInDialog::open(form.clone(), window, cx));
        cx.run_until_parked();
        let click = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is not on screen"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
        };

        // Nothing is asked until the person says so; pressing twice asks once.
        assert!(seen.borrow().is_empty());
        click(cx, "signin-start");
        click(cx, "signin-start");
        assert_eq!(seen.borrow().as_slice(), [SignInIntent::Start]);

        // The code shows with its helpers, and the dialog can only be left by cancelling.
        form.update(cx, |form, cx| {
            form.code_arrived("AB12CD", "https://www.microsoft.com/link", cx)
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        for selector in ["signin-code", "signin-copy", "signin-open"] {
            assert!(cx.debug_bounds(selector).is_some(), "{selector}");
        }

        // A failure says why and offers another try.
        form.update(cx, |form, cx| {
            form.failed(
                ("代码已经过期，请重新开始登录".into(), "expired".into()),
                cx,
            )
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("signin-error").is_some());
        click(cx, "signin-start");
        assert_eq!(
            seen.borrow().as_slice(),
            [SignInIntent::Start, SignInIntent::Start]
        );

        // Cancelling while it waits tells the application and closes the dialog.
        click(cx, "signin-cancel");
        assert_eq!(seen.borrow().last(), Some(&SignInIntent::Cancel));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
    }

    #[gpui::test]
    fn a_finished_sign_in_closes_the_dialog_and_a_late_code_after_a_failure_is_ignored(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let (_, cx) = cx.add_window_view(|window, cx| {
            let host = cx.new(|_| Host);
            gpui_component::Root::new(host, window, cx)
        });
        let form = cx.update(|_, cx| cx.new(|_| SignInDialog::new(Rc::new(|_, _, _| {}))));
        cx.update(|window, cx| SignInDialog::open(form.clone(), window, cx));
        cx.run_until_parked();
        form.update(cx, |form, cx| {
            form.failed(("x".into(), "y".into()), cx);
            form.code_arrived("LATE", "https://x", cx);
        });
        form.read_with(cx, |form, _| {
            assert!(
                matches!(form.phase, Phase::Failed(_)),
                "a stale code changes nothing"
            );
        });
        form.update(cx, |form, cx| form.signed_in(cx));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
    }
}
