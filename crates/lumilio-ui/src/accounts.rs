//! The add-offline-account dialog (IA `accounts.md`): a name checked as it is
//! typed, and an advanced profile id. It holds no business logic: saving goes
//! through an [`AccountRequest`] and the answer comes back through
//! [`AccountForm::saved`].

use std::rc::Rc;

use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, WeakEntity, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::{MAX_PROFILE_NAME, OfflineProfile, ProfileError, ProfileId};

use crate::kit;
use crate::new_game::Failure;
use crate::theme::{self, ShellColors};

const DIALOG_WIDTH: f32 = 440.;

pub const UUID_HELP: &str = "游戏用它认出你，存档里的玩家数据也按它保存。留空则由名称决定（同名永远得到同一个）；只能在添加时设置，之后不能更改。";

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
        Err(ProfileError::Empty) => Some("请输入名称".to_owned()),
        Err(ProfileError::TooLong) => Some(format!("名称最多 {MAX_PROFILE_NAME} 个字符")),
        Err(ProfileError::InvalidCharacter(ch)) => {
            Some(format!("名称只能用字母、数字和下划线，不能用“{ch}”"))
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
        .map(|_| "UUID 需要 32 位十六进制数字（可带横线）".to_owned())
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
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("例如 Steve"));
        let uuid = cx.new(|cx| InputState::new(window, cx).placeholder("留空：由名称决定"));
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
                .label("添加")
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
                .label("取消")
                .white()
                .disabled(busy)
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        theme::dialog(dialog, cx)
            .title("添加离线账户")
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
        let colors = ShellColors::from_theme(cx.theme());
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
                    "收起高级选项"
                } else {
                    "高级选项"
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
                    .child(label("名称", colors))
                    .child(Input::new(&self.name).disabled(busy))
                    .child(match name_problem {
                        Some(problem) => div()
                            .text_xs()
                            .text_color(colors.danger)
                            .debug_selector(|| "account-name-problem".into())
                            .child(problem),
                        None => div().text_xs().text_color(colors.muted).child(format!(
                            "最多 {MAX_PROFILE_NAME} 位字母、数字或下划线{}",
                            if self.first {
                                "。这是第一个账户，会自动设为当前账户"
                            } else {
                                ""
                            }
                        )),
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
                                .child(kit::info("account-uuid-info", UUID_HELP)),
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
mod tests {
    use super::*;

    #[test]
    fn names_are_explained_one_reason_at_a_time() {
        assert_eq!(name_problem("Steve"), None);
        assert_eq!(name_problem(""), Some("请输入名称".into()));
        assert!(
            name_problem("a_very_long_name_indeed")
                .unwrap()
                .contains("16")
        );
        assert!(name_problem("bad name").unwrap().contains("不能用"));
    }

    #[test]
    fn a_profile_id_is_checked_only_when_typed() {
        assert_eq!(uuid_problem(""), None);
        assert_eq!(uuid_problem("   "), None);
        assert_eq!(uuid_problem("123e4567-e89b-12d3-a456-426614174000"), None);
        assert!(uuid_problem("nope").is_some());
    }

    #[test]
    fn the_request_ignores_a_hidden_id_and_waits_for_a_valid_one() {
        // A leftover id is not sent while the advanced section is closed.
        let request = request_from(" Steve ", "garbage", false).unwrap();
        assert_eq!(request.name, "Steve");
        assert_eq!(request.uuid, None);
        // Open, it must be valid; blank still means "from the name".
        assert_eq!(request_from("Steve", "garbage", true), None);
        assert_eq!(request_from("Steve", " ", true).unwrap().uuid, None);
        let chosen = request_from("Steve", "123e4567e89b12d3a456426614174000", true).unwrap();
        assert_eq!(
            chosen.uuid.as_deref(),
            Some("123e4567e89b12d3a456426614174000")
        );
        assert_eq!(request_from("", "", false), None);
    }

    struct Host;

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    #[gpui::test]
    fn the_dialog_checks_as_you_type_saves_once_and_keeps_the_draft_on_failure(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::Modifiers;
        use std::cell::RefCell;
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let (_, cx) = cx.add_window_view(|window, cx| {
            let host = cx.new(|_| Host);
            gpui_component::Root::new(host, window, cx)
        });
        let seen: Rc<RefCell<Vec<AccountRequest>>> = Rc::default();
        let sink = seen.clone();
        let form = cx.update(|window, cx| {
            cx.new(|cx| {
                AccountForm::new(
                    Rc::new(move |request, _, _| sink.borrow_mut().push(request)),
                    true,
                    window,
                    cx,
                )
            })
        });
        cx.update(|window, cx| AccountForm::open(form.clone(), window, cx));
        cx.run_until_parked();
        let name = form.read_with(cx, |form, _| form.name.clone());
        let type_name = |cx: &mut gpui::VisualTestContext, text: &str| {
            cx.update(|window, cx| {
                name.update(cx, |input, cx| input.set_value(text.to_owned(), window, cx))
            });
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
        };

        // Nothing typed: the button does nothing and nobody is scolded.
        let add = cx.debug_bounds("account-add").expect("dialog on screen");
        cx.simulate_click(add.center(), Modifiers::none());
        assert!(seen.borrow().is_empty());
        assert!(cx.debug_bounds("account-name-problem").is_none());

        // A bad name says why and cannot be saved.
        type_name(cx, "bad name");
        assert!(cx.debug_bounds("account-name-problem").is_some());
        let add = cx.debug_bounds("account-add").unwrap();
        cx.simulate_click(add.center(), Modifiers::none());
        assert!(seen.borrow().is_empty());

        // A good name saves once, however often the button is pressed.
        type_name(cx, "Steve");
        assert!(cx.debug_bounds("account-name-problem").is_none());
        let add = cx.debug_bounds("account-add").unwrap();
        cx.simulate_click(add.center(), Modifiers::none());
        cx.simulate_click(add.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().as_slice(),
            [AccountRequest {
                name: "Steve".into(),
                uuid: None
            }]
        );

        // A refusal stays in the dialog, with what was typed.
        form.update(cx, |form, cx| {
            form.saved(Err(("已经有叫“Steve”的账户了".into(), "dup".into())), cx)
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("account-error").is_some());
        assert!(cx.debug_bounds("dialog-layer").is_some(), "still open");
        assert_eq!(form.read_with(cx, |form, cx| form.typed(cx).0), "Steve");

        // Success closes it.
        form.update(cx, |form, cx| form.saved(Ok(()), cx));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
    }
}
