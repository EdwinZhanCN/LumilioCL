//! The list of authentication servers (IA `accounts.md`): what is there, adding
//! one by its address (the person sees the name it gives before it is kept),
//! and removing one together with the accounts signed in on it. It holds no
//! protocol: every step goes through a [`ServerIntent`] and the answers come
//! back through [`ServersDialog::located`], [`ServersDialog::listed`] and
//! [`ServersDialog::failed`].

use std::rc::Rc;

use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::AuthServer;

use crate::kit;
use crate::new_game::Failure;
use crate::theme::{self, ShellColors};
use crate::tr;

const DIALOG_WIDTH: f32 = 480.;

/// One server of the list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerRow {
    pub url: String,
    pub name: String,
    /// LittleSkin: always there, never removed.
    pub builtin: bool,
    /// Accounts signed in on it; removing the server removes them too.
    pub accounts: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerIntent {
    /// Find the server this address means (nothing is kept yet).
    Locate(String),
    /// Keep a server that was found.
    Add(AuthServer),
    /// Forget a server (by API root) and its accounts.
    Remove(String),
}

pub type ServerHandler = Rc<dyn Fn(ServerIntent, &mut Window, &mut App)>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Phase {
    Idle,
    Locating,
    /// The server answered: the person sees its name before keeping it.
    Found(AuthServer),
}

pub struct ServersDialog {
    handler: ServerHandler,
    servers: Vec<ServerRow>,
    address: Entity<InputState>,
    phase: Phase,
    error: Option<Failure>,
}

impl ServersDialog {
    pub fn new(
        handler: ServerHandler,
        servers: Vec<ServerRow>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let address = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("account-server-address-placeholder"))
        });
        cx.subscribe_in(
            &address,
            window,
            |dialog, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    dialog.error = None;
                    if matches!(dialog.phase, Phase::Found(_)) {
                        dialog.phase = Phase::Idle;
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => dialog.locate(window, cx),
                _ => {}
            },
        )
        .detach();
        Self {
            handler,
            servers,
            address,
            phase: Phase::Idle,
            error: None,
        }
    }

    fn typed(&self, cx: &App) -> String {
        self.address.read(cx).value().trim().to_owned()
    }

    fn locate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let address = self.typed(cx);
        if address.is_empty() || self.phase == Phase::Locating {
            return;
        }
        self.phase = Phase::Locating;
        self.error = None;
        cx.notify();
        (self.handler)(ServerIntent::Locate(address), window, cx);
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Phase::Found(server) = &self.phase {
            let server = server.clone();
            (self.handler)(ServerIntent::Add(server), window, cx);
        }
    }

    /// The server the typed address means, or why it could not be found.
    pub fn located(&mut self, result: Result<AuthServer, Failure>, cx: &mut Context<Self>) {
        if self.phase != Phase::Locating {
            return;
        }
        match result {
            Ok(server) => self.phase = Phase::Found(server),
            Err(failure) => {
                self.phase = Phase::Idle;
                self.error = Some(failure);
            }
        }
        cx.notify();
    }

    /// The list changed (a server was added or removed).
    pub fn listed(&mut self, servers: Vec<ServerRow>, window: &mut Window, cx: &mut Context<Self>) {
        self.servers = servers;
        self.phase = Phase::Idle;
        self.error = None;
        self.address
            .update(cx, |address, cx| address.set_value("", window, cx));
        cx.notify();
    }

    /// A step did not work; the dialog says why.
    pub fn failed(&mut self, failure: Failure, cx: &mut Context<Self>) {
        if self.phase == Phase::Locating {
            self.phase = Phase::Idle;
        }
        self.error = Some(failure);
        cx.notify();
    }

    /// Asks before a server (and its accounts) is removed.
    fn ask_remove(&self, row: &ServerRow, window: &mut Window, cx: &mut App) {
        let (handler, url) = (self.handler.clone(), row.url.clone());
        let description = if row.accounts == 0 {
            tr!("account-server-remove-body").to_owned()
        } else {
            tr!("account-server-remove-body-accounts", count = row.accounts)
        };
        window.open_alert_dialog(cx, {
            let title = tr!("account-server-remove-title", name = row.name.as_str());
            move |alert, _, _| {
                let (handler, url) = (handler.clone(), url.clone());
                alert
                    .title(title.clone())
                    .description(description.clone())
                    .ok_text(tr!("common-remove"))
                    .ok_variant(gpui_component::button::ButtonVariant::Danger)
                    .cancel_text(tr!("common-cancel"))
                    .show_cancel(true)
                    .on_ok(move |_, window, cx| {
                        handler(ServerIntent::Remove(url.clone()), window, cx);
                        true
                    })
            }
        });
    }

    pub fn open(dialog: Entity<Self>, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |view, _, cx| Self::frame(&dialog, view, cx));
    }

    fn frame(dialog: &Entity<Self>, view: Dialog, cx: &mut App) -> Dialog {
        let locating = dialog.read(cx).phase == Phase::Locating;
        theme::dialog(view, cx)
            .title(tr!("account-server-label"))
            .w(px(DIALOG_WIDTH))
            .keyboard(!locating)
            .child(dialog.clone())
            .footer(
                h_flex().w_full().justify_end().child(theme::clickable(
                    Key::new("servers-close")
                        .label(tr!("common-close"))
                        .white()
                        .debug_selector(|| "servers-close".into())
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                    true,
                )),
            )
    }
}

impl Render for ServersDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let entity = cx.entity().downgrade();
        let rows: Vec<_> = self
            .servers
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let trail = (!row.builtin).then(|| {
                    let (entity, row) = (entity.clone(), row.clone());
                    kit::ghost(
                        ("server-remove", index),
                        tr!("common-remove"),
                        move |window, cx| {
                            let _ =
                                entity.update(cx, |dialog, cx| dialog.ask_remove(&row, window, cx));
                        },
                    )
                    .debug_selector(move || format!("server-remove-{index}"))
                    .into_any_element()
                });
                let detail = if row.builtin {
                    tr!("account-server-builtin", url = row.url.as_str())
                } else if row.accounts > 0 {
                    tr!(
                        "account-server-accounts",
                        url = row.url.as_str(),
                        count = row.accounts
                    )
                } else {
                    row.url.clone()
                };
                kit::row(row.name.clone(), detail, None, trail, colors)
            })
            .collect();
        let locating = self.phase == Phase::Locating;
        let typed_empty = self.address.read(cx).value().trim().is_empty();
        let find = {
            let entity = entity.clone();
            kit::action(
                "server-find",
                tr!("account-server-find"),
                None,
                false,
                move |window, cx| {
                    let _ = entity.update(cx, |dialog, cx| dialog.locate(window, cx));
                },
            )
            .loading(locating)
            .disabled(locating || typed_empty)
            .debug_selector(|| "server-find".into())
        };
        let found = match &self.phase {
            Phase::Found(server) => {
                let add = {
                    let entity = entity.clone();
                    kit::action(
                        "server-add",
                        tr!("common-add"),
                        None,
                        true,
                        move |window, cx| {
                            let _ = entity.update(cx, |dialog, cx| dialog.add(window, cx));
                        },
                    )
                    .debug_selector(|| "server-add".into())
                };
                let known = self.servers.iter().any(|row| row.url == server.url);
                Some(
                    v_flex()
                        .gap_2()
                        .debug_selector(|| "server-found".into())
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(colors.foreground)
                                .child(server.display_name().to_owned()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted)
                                .child(server.url.clone()),
                        )
                        .children(server.is_insecure().then(|| {
                            div()
                                .text_xs()
                                .text_color(colors.danger)
                                .debug_selector(|| "server-http-warning".into())
                                .child(tr!("account-http-warning"))
                        }))
                        .child(if known {
                            div()
                                .text_xs()
                                .text_color(colors.muted)
                                .child(tr!("account-server-already-listed"))
                                .into_any_element()
                        } else {
                            h_flex().child(add).into_any_element()
                        }),
                )
            }
            _ => None,
        };
        v_flex()
            .id("auth-servers")
            .w_full()
            .gap_4()
            .child(kit::list(rows, colors))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(tr!("account-server-add-title")),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.address).disabled(locating)),
                            )
                            .child(find),
                    ),
            )
            .children(found)
            .children(self.error.clone().map(|(message, technical)| {
                h_flex()
                    .gap_2()
                    .items_center()
                    .debug_selector(|| "server-error".into())
                    .child(div().text_sm().text_color(colors.danger).child(message))
                    .child(kit::technical("server-technical", technical).xsmall())
            }))
    }
}

#[cfg(test)]
mod tests;
