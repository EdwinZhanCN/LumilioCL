//! The add-offline-account dialog (IA `accounts.md`): a name checked as it is
//! typed, and an advanced profile id. It holds no business logic: saving goes
//! through an [`AccountRequest`] and the answer comes back through
//! [`AccountForm::saved`].

use std::rc::Rc;

use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, WeakEntity, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::{MAX_PROFILE_NAME, OfflineProfile, ProfileError, ProfileId};

use crate::kit;
use crate::new_game::Failure;
use crate::theme::{self, ShellColors};
use crate::tr;

const DIALOG_WIDTH: f32 = 440.;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRequest {
    pub name: String,
    /// A chosen profile id; `None` derives it from the name.
    pub uuid: Option<String>,
}

pub type AccountHandler = Rc<dyn Fn(AccountRequest, &mut Window, &mut App)>;

/// Why a typed name cannot be used, in one sentence.
#[must_use]
pub fn name_problem(name: &str) -> Option<String> {
    match OfflineProfile::new(name) {
        Ok(_) => None,
        Err(ProfileError::Empty) => Some(tr!("account-name-required").to_owned()),
        Err(ProfileError::TooLong) => Some(tr!("account-name-too-long", count = MAX_PROFILE_NAME)),
        Err(ProfileError::InvalidCharacter(ch)) => {
            Some(tr!("account-name-invalid-char", character = ch.to_string()))
        }
        Err(ProfileError::InvalidId) => None,
    }
}

/// Why a typed profile id cannot be used; blank is fine.
#[must_use]
pub fn uuid_problem(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    ProfileId::parse(text)
        .err()
        .map(|_| tr!("account-uuid-problem").to_owned())
}

/// The request the typed values make, or `None` while something is wrong.
#[must_use]
pub fn request_from(name: &str, uuid: &str, advanced: bool) -> Option<AccountRequest> {
    if name_problem(name).is_some() || (advanced && uuid_problem(uuid).is_some()) {
        return None;
    }
    let uuid = uuid.trim();
    Some(AccountRequest {
        name: name.trim().to_owned(),
        uuid: (advanced && !uuid.is_empty()).then(|| uuid.to_owned()),
    })
}

pub struct AccountForm {
    handler: AccountHandler,
    name: Entity<InputState>,
    uuid: Entity<InputState>,
    advanced: bool,
    /// The first account becomes the current one; the dialog says so.
    first: bool,
    busy: bool,
    error: Option<Failure>,
    close: bool,
}

impl AccountForm {
    pub fn new(
        handler: AccountHandler,
        first: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("account-name-placeholder")));
        let uuid =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("account-uuid-placeholder")));
        for input in [&name, &uuid] {
            cx.subscribe_in(
                input,
                window,
                |form, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        form.error = None;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => form.submit(window, cx),
                    _ => {}
                },
            )
            .detach();
        }
        Self {
            handler,
            name,
            uuid,
            advanced: false,
            first,
            busy: false,
            error: None,
            close: false,
        }
    }

    fn typed(&self, cx: &App) -> (String, String) {
        (
            self.name.read(cx).value().to_string(),
            self.uuid.read(cx).value().to_string(),
        )
    }

    /// The finished request, or `None` while something is missing or wrong.
    pub fn request(&self, cx: &App) -> Option<AccountRequest> {
        let (name, uuid) = self.typed(cx);
        request_from(&name, &uuid, self.advanced)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(request) = self.request(cx) else {
            return;
        };
        self.busy = true;
        self.error = None;
        cx.notify();
        (self.handler)(request, window, cx);
    }

    /// The save finished. Success closes the dialog; a failure stays in it
    /// with what was typed.
    pub fn saved(&mut self, result: Result<(), Failure>, cx: &mut Context<Self>) {
        self.busy = false;
        match result {
            Ok(()) => self.close = true,
            Err(failure) => self.error = Some(failure),
        }
        cx.notify();
    }

    /// Opens the dialog around `form`.
    pub fn open(form: Entity<Self>, window: &mut Window, cx: &mut App) {
        let name = form.read(cx).name.clone();
        window.open_dialog(cx, move |dialog, _, cx| Self::dialog(&form, dialog, cx));
        name.update(cx, |name, cx| name.focus(window, cx));
    }

    fn dialog(form: &Entity<Self>, dialog: Dialog, cx: &mut App) -> Dialog {
        let this = form.read(cx);
        let busy = this.busy;
        let ready = this.request(cx).is_some();
        let weak = form.downgrade();
        let add = theme::clickable(
            Key::new("account-add")
                .label(tr!("common-add"))
                .primary()
                .disabled(busy || !ready)
                .loading(busy)
                .debug_selector(|| "account-add".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |form, cx| form.submit(window, cx));
                }),
            !busy && ready,
        );
        let cancel = theme::clickable(
            Key::new("account-cancel")
                .label(tr!("common-cancel"))
                .white()
                .disabled(busy)
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        theme::dialog(dialog, cx)
            .title(tr!("account-add-offline"))
            .w(px(DIALOG_WIDTH))
            .keyboard(!busy)
            .overlay_closable(!busy)
            .close_button(!busy)
            .child(form.clone())
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(cancel)
                    .child(add),
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

impl Render for AccountForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        let colors = ShellColors::current(cx);
        let busy = self.busy;
        let (name, uuid) = self.typed(cx);
        // An empty box is not nagged about; the Add button just stays off.
        let name_problem = if name.trim().is_empty() {
            None
        } else {
            name_problem(&name)
        };
        let uuid_problem = if self.advanced {
            uuid_problem(&uuid)
        } else {
            None
        };
        let entity: WeakEntity<Self> = cx.entity().downgrade();
        let toggle = {
            let entity = entity.clone();
            Key::new("account-advanced")
                .label(if self.advanced {
                    tr!("account-advanced-collapse")
                } else {
                    tr!("account-advanced-expand")
                })
                .ghost()
                .xsmall()
                .on_click(move |_, _, cx| {
                    let _ = entity.update(cx, |form, cx| {
                        form.advanced = !form.advanced;
                        cx.notify();
                    });
                })
        };
        v_flex()
            .id("account-form")
            .w_full()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .child(label(tr!("account-name-label"), colors))
                    .child(Input::new(&self.name).disabled(busy))
                    .child(match name_problem {
                        Some(problem) => div()
                            .text_xs()
                            .text_color(colors.danger)
                            .debug_selector(|| "account-name-problem".into())
                            .child(problem),
                        None => {
                            let help = if self.first {
                                tr!("account-name-help-first", count = MAX_PROFILE_NAME)
                            } else {
                                tr!("account-name-help", count = MAX_PROFILE_NAME)
                            };
                            div().text_xs().text_color(colors.muted).child(help)
                        }
                    }),
            )
            .child(h_flex().child(toggle))
            .when(self.advanced, |form| {
                form.child(
                    v_flex()
                        .gap_2()
                        .child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .child(label("UUID", colors))
                                .child(kit::info("account-uuid-info", tr!("account-uuid-help"))),
                        )
                        .child(Input::new(&self.uuid).disabled(busy))
                        .children(uuid_problem.map(|problem| {
                            div()
                                .text_xs()
                                .text_color(colors.danger)
                                .debug_selector(|| "account-uuid-problem".into())
                                .child(problem)
                        })),
                )
            })
            .children(self.error.clone().map(|(message, technical)| {
                h_flex()
                    .gap_2()
                    .items_center()
                    .debug_selector(|| "account-error".into())
                    .child(div().text_sm().text_color(colors.danger).child(message))
                    .child(kit::technical("account-technical", technical).xsmall())
            }))
    }
}

#[cfg(test)]
mod tests;
