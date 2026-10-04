//! The sign-in dialog for authentication servers (LittleSkin and others; IA
//! `accounts.md`): pick the server, type the name and password, and — when
//! the account has several characters — pick one. It holds no protocol:
//! signing in goes through a [`ThirdPartyIntent`] and the answers come back
//! through [`ThirdPartyDialog::choose`], [`ThirdPartyDialog::failed`] and
//! [`ThirdPartyDialog::signed_in`].

use std::rc::Rc;

use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::{AuthServer, CharacterProfile, ProfileId};

use crate::kit;
use crate::new_game::Failure;
use crate::theme::{self, ShellColors};

const DIALOG_WIDTH: f32 = 460.;

pub const PASSWORD_HINT: &str =
    "密码只会发给你选的认证服务器；启动器不保存它，只把服务器发来的令牌放进系统凭据库。";
pub const HTTP_WARNING: &str =
    "警告：此服务器使用不安全的 HTTP 协议，你的密码在登录时会被明文传输。";

/// What the dialog asks the application to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThirdPartyIntent {
    SignIn {
        /// The server's API root.
        server: String,
        login: String,
        password: String,
    },
    /// Finish a sign-in that asked which character to use.
    Choose { pending: u64, character: ProfileId },
    /// The person left a sign-in that was waiting for a choice.
    Abandon(u64),
    /// Leave this dialog for the list of authentication servers.
    ManageServers,
}

pub type ThirdPartyHandler = Rc<dyn Fn(ThirdPartyIntent, &mut Window, &mut App)>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Phase {
    Form,
    /// Waiting for the server's answer.
    Working,
    /// The server found several characters: choose one.
    Choose {
        pending: u64,
        characters: Vec<CharacterProfile>,
        chosen: usize,
    },
}

pub struct ThirdPartyDialog {
    handler: ThirdPartyHandler,
    servers: Vec<AuthServer>,
    server: usize,
    login: Entity<InputState>,
    password: Entity<InputState>,
    phase: Phase,
    error: Option<Failure>,
    close: bool,
}

impl ThirdPartyDialog {
    pub fn new(
        handler: ThirdPartyHandler,
        servers: Vec<AuthServer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let login = cx.new(|cx| InputState::new(window, cx));
        let password = cx.new(|cx| InputState::new(window, cx).masked(true));
        for input in [&login, &password] {
            cx.subscribe_in(
                input,
                window,
                |dialog, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        dialog.error = None;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => dialog.submit(window, cx),
                    _ => {}
                },
            )
            .detach();
        }
        Self {
            handler,
            servers,
            server: 0,
            login,
            password,
            phase: Phase::Form,
            error: None,
            close: false,
        }
    }

    fn current(&self) -> Option<&AuthServer> {
        self.servers.get(self.server)
    }

    /// What the name box is called: servers that sign in with a user name say so.
    #[must_use]
    pub fn login_label(&self) -> &'static str {
        if self.current().is_some_and(|server| server.non_email_login) {
            "用户名"
        } else {
            "邮箱"
        }
    }

    /// The finished request, or `None` while something is missing.
    #[must_use]
    pub fn request(&self, cx: &App) -> Option<ThirdPartyIntent> {
        let server = self.current()?.url.clone();
        let login = self.login.read(cx).value().trim().to_owned();
        let password = self.password.read(cx).value().to_string();
        (!login.is_empty() && !password.is_empty()).then_some(ThirdPartyIntent::SignIn {
            server,
            login,
            password,
        })
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != Phase::Form {
            return;
        }
        let Some(request) = self.request(cx) else {
            return;
        };
        self.phase = Phase::Working;
        self.error = None;
        cx.notify();
        (self.handler)(request, window, cx);
    }

    fn pick_character(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Phase::Choose {
            pending,
            characters,
            chosen,
        } = &self.phase
        else {
            return;
        };
        let intent = ThirdPartyIntent::Choose {
            pending: *pending,
            character: characters[*chosen].id,
        };
        self.phase = Phase::Working;
        cx.notify();
        (self.handler)(intent, window, cx);
    }

    fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Phase::Choose { pending, .. } = &self.phase {
            (self.handler)(ThirdPartyIntent::Abandon(*pending), window, cx);
        }
        window.close_dialog(cx);
    }

    /// The account has several characters: the person chooses one.
    pub fn choose(
        &mut self,
        pending: u64,
        characters: Vec<CharacterProfile>,
        cx: &mut Context<Self>,
    ) {
        if self.phase == Phase::Working && !characters.is_empty() {
            self.phase = Phase::Choose {
                pending,
                characters,
                chosen: 0,
            };
            cx.notify();
        }
    }

    /// The sign-in did not work; the dialog says why and keeps what was typed.
    pub fn failed(&mut self, failure: Failure, cx: &mut Context<Self>) {
        self.phase = Phase::Form;
        self.error = Some(failure);
        cx.notify();
    }

    /// The account is in: the dialog closes itself.
    pub fn signed_in(&mut self, cx: &mut Context<Self>) {
        self.close = true;
        cx.notify();
    }

    /// Opens the dialog around `dialog`.
    pub fn open(dialog: Entity<Self>, window: &mut Window, cx: &mut App) {
        let login = dialog.read(cx).login.clone();
        window.open_dialog(cx, move |view, _, cx| Self::frame(&dialog, view, cx));
        login.update(cx, |login, cx| login.focus(window, cx));
    }

    fn frame(dialog: &Entity<Self>, view: Dialog, cx: &mut App) -> Dialog {
        let this = dialog.read(cx);
        let working = this.phase == Phase::Working;
        let choosing = matches!(this.phase, Phase::Choose { .. });
        let ready = this.request(cx).is_some();
        let weak = dialog.downgrade();
        let go = {
            let weak = weak.clone();
            theme::clickable(
                Key::new("tp-go")
                    .label(if choosing { "选择" } else { "登录" })
                    .primary()
                    .loading(working)
                    .disabled(working || (!choosing && !ready))
                    .debug_selector(|| "tp-go".into())
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |dialog, cx| {
                            if dialog.phase == (Phase::Form) {
                                dialog.submit(window, cx);
                            } else {
                                dialog.pick_character(window, cx);
                            }
                        });
                    }),
                !working && (choosing || ready),
            )
        };
        let cancel = theme::clickable(
            Key::new("tp-cancel")
                .label("取消")
                .white()
                .disabled(working)
                .debug_selector(|| "tp-cancel".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |dialog, cx| dialog.leave(window, cx));
                }),
            !working,
        );
        theme::dialog(view, cx)
            .title("登录第三方账户")
            .w(px(DIALOG_WIDTH))
            .keyboard(!working)
            .overlay_closable(false)
            .close_button(!working)
            .on_close({
                let weak = dialog.downgrade();
                move |_, window, cx| {
                    // Closing with the corner button also abandons a pending choice.
                    let _ = weak.update(cx, |dialog, cx| {
                        if let Phase::Choose { pending, .. } = &dialog.phase {
                            (dialog.handler)(ThirdPartyIntent::Abandon(*pending), window, cx);
                        }
                    });
                }
            })
            .child(dialog.clone())
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(cancel)
                    .child(go),
            )
    }
}

fn label(text: &'static str, colors: ShellColors) -> impl IntoElement {
    div()
        .text_sm()
        .font_medium()
        .text_color(colors.foreground)
        .child(text)
}

impl Render for ThirdPartyDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        let colors = ShellColors::from_theme(cx.theme());
        let entity = cx.entity().downgrade();
        let body = match &self.phase {
            Phase::Choose {
                characters, chosen, ..
            } => v_flex()
                .gap_2()
                .child(label("这个账户有多个角色，选一个用来玩", colors))
                .child(v_flex().children(characters.iter().enumerate().map(
                    |(index, character)| {
                        let entity = entity.clone();
                        kit::led_option(
                            ("tp-character", index),
                            character.name.clone(),
                            *chosen == index,
                            colors,
                            move |_, cx| {
                                let _ = entity.update(cx, |dialog, cx| {
                                    if let Phase::Choose { chosen, .. } = &mut dialog.phase {
                                        *chosen = index;
                                    }
                                    cx.notify();
                                });
                            },
                        )
                        .debug_selector(move || format!("tp-character-{index}"))
                    },
                )))
                .into_any_element(),
            Phase::Form | Phase::Working => {
                let busy = self.phase == Phase::Working;
                let server_rows = self.servers.iter().enumerate().map(|(index, server)| {
                    let entity = entity.clone();
                    kit::led_option(
                        ("tp-server", index),
                        server.display_name().to_owned(),
                        self.server == index,
                        colors,
                        move |_, cx| {
                            let _ = entity.update(cx, |dialog, cx| {
                                dialog.server = index;
                                dialog.error = None;
                                cx.notify();
                            });
                        },
                    )
                    .debug_selector(move || format!("tp-server-{index}"))
                });
                let manage = {
                    let entity = entity.clone();
                    kit::ghost(
                        "tp-manage",
                        "添加认证服务器…",
                        move |window, cx| {
                            // This dialog gives way to the list of servers.
                            window.close_dialog(cx);
                            let _ = entity.update(cx, |dialog, cx| {
                                (dialog.handler)(ThirdPartyIntent::ManageServers, window, cx);
                            });
                        },
                    )
                    .disabled(busy)
                    .debug_selector(|| "tp-manage".into())
                };
                v_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(label("认证服务器", colors))
                            .child(v_flex().children(server_rows))
                            .child(h_flex().child(manage)),
                    )
                    .children(
                        self.current()
                            .filter(|server| server.is_insecure())
                            .map(|_| {
                                div()
                                    .text_xs()
                                    .text_color(colors.danger)
                                    .debug_selector(|| "tp-http-warning".into())
                                    .child(HTTP_WARNING)
                            }),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(label(self.login_label(), colors))
                            .child(Input::new(&self.login).disabled(busy)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(label("密码", colors))
                            .child(Input::new(&self.password).disabled(busy))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted)
                                    .child(PASSWORD_HINT),
                            ),
                    )
                    .children(self.error.clone().map(|(message, technical)| {
                        h_flex()
                            .gap_2()
                            .items_center()
                            .debug_selector(|| "tp-error".into())
                            .child(div().text_sm().text_color(colors.danger).child(message))
                            .child(kit::technical("tp-technical", technical).xsmall())
                    }))
                    .into_any_element()
            }
        };
        v_flex().id("third-party").w_full().gap_3().child(body)
    }
}

#[cfg(test)]
mod tests;
