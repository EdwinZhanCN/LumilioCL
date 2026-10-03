//! The two small dialogs of Library collections (IA `library.md`): a name for
//! a new or renamed collection, and the list a game is added to. They hold no
//! business logic: the answer goes out through a handler and the dialog
//! closes; the application tells a refusal in a toast.

use std::rc::Rc;

use crate::controls::Checkbox;
use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, StyledExt as _, WindowExt as _, h_flex, v_flex};

use crate::theme::{self, ShellColors};

const DIALOG_WIDTH: f32 = 400.;

/// Why a collection name cannot be used, in one sentence. `original` is the
/// name being renamed, which may be kept.
#[must_use]
pub fn name_problem(name: &str, taken: &[String], original: Option<&str>) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return Some("请输入名称".to_owned());
    }
    taken
        .iter()
        .any(|other| other == name && Some(other.as_str()) != original)
        .then(|| format!("已经有一个叫“{name}”的合集"))
}

fn close_with(window: &mut Window, cx: &mut App) {
    window.close_dialog(cx);
}

/// Asks before a collection goes. Only the label is removed; the games stay.
pub fn confirm_delete(
    name: &str,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let name = name.to_owned();
    let on_ok = Rc::new(on_ok);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_ok = on_ok.clone();
        alert
            .title(format!("删除合集“{name}”？"))
            .description("只会删除这个合集，里面的游戏都还在游戏库里。")
            .ok_text("删除")
            .ok_variant(gpui_component::button::ButtonVariant::Danger)
            .cancel_text("取消")
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
    });
}

/// Asks before a game is deleted from the library.
pub fn confirm_game_delete(
    name: &str,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let name = name.to_owned();
    let on_ok = Rc::new(on_ok);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_ok = on_ok.clone();
        alert
            .title(format!("删除“{name}”？"))
            .description("游戏目录、存档和历史会一起删除，之后不能找回。")
            .ok_text("删除")
            .ok_variant(gpui_component::button::ButtonVariant::Danger)
            .cancel_text("取消")
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
    });
}

/// Offers to remove shared game files nobody uses, saying how much, and what
/// was left alone on purpose.
pub fn confirm_reclaim(
    size: &str,
    kept: &[String],
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let description = if kept.is_empty() {
        "没有游戏用到它们。删掉后，以后需要时会重新下载。".to_owned()
    } else {
        format!(
            "没有游戏用到它们。删掉后，以后需要时会重新下载。\n没清理的部分：{}",
            kept.join("；")
        )
    };
    let title = format!("清理 {size} 没用的游戏文件？");
    let on_ok = Rc::new(on_ok);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_ok = on_ok.clone();
        alert
            .title(title.clone())
            .description(description.clone())
            .ok_text("清理")
            .ok_variant(gpui_component::button::ButtonVariant::Danger)
            .cancel_text("取消")
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
    });
}

// ── name ────────────────────────────────────────────────────────────────

pub type NameHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;

pub struct NamePrompt {
    input: Entity<InputState>,
    taken: Vec<String>,
    original: Option<String>,
    handler: NameHandler,
}

impl NamePrompt {
    /// `original` is the name being changed (shown at first); `taken` are the
    /// names that exist.
    pub fn new(
        original: Option<String>,
        taken: Vec<String>,
        handler: NameHandler,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let start = original.clone().unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如 生存、整合包")
                .default_value(start)
        });
        cx.subscribe_in(
            &input,
            window,
            |form, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => form.submit(window, cx),
                _ => {}
            },
        )
        .detach();
        Self {
            input,
            taken,
            original,
            handler,
        }
    }

    fn typed(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }

    /// The name to use, or `None` while it is missing or taken.
    pub fn name(&self, cx: &App) -> Option<String> {
        let typed = self.typed(cx);
        name_problem(&typed, &self.taken, self.original.as_deref())
            .is_none()
            .then(|| typed.trim().to_owned())
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(name) = self.name(cx) {
            (self.handler)(name, window, cx);
            close_with(window, cx);
        }
    }

    pub fn open(
        form: Entity<Self>,
        title: &'static str,
        confirm: &'static str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let input = form.read(cx).input.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let ready = form.read(cx).name(cx).is_some();
            let weak = form.downgrade();
            let ok = theme::clickable(
                Key::new("collection-name-ok")
                    .label(confirm)
                    .primary()
                    .disabled(!ready)
                    .debug_selector(|| "collection-name-ok".into())
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |form, cx| form.submit(window, cx));
                    }),
                ready,
            );
            theme::dialog(dialog, cx)
                .title(title)
                .w(px(DIALOG_WIDTH))
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Key::new("collection-name-cancel")
                                .label("取消")
                                .white()
                                .on_click(|_, window, cx| close_with(window, cx)),
                        )
                        .child(ok),
                )
        });
        input.update(cx, |input, cx| input.focus(window, cx));
    }
}

impl Render for NamePrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let typed = self.typed(cx);
        // An empty box is not nagged about; the button just stays off.
        let problem = if typed.trim().is_empty() {
            None
        } else {
            name_problem(&typed, &self.taken, self.original.as_deref())
        };
        v_flex()
            .w_full()
            .gap_2()
            .child(Input::new(&self.input))
            .children(problem.map(|problem| {
                div()
                    .text_xs()
                    .text_color(colors.danger)
                    .debug_selector(|| "collection-name-problem".into())
                    .child(problem)
            }))
    }
}

// ── picker ──────────────────────────────────────────────────────────────

/// The collections a game ends up in, and a new one to make for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Membership {
    pub collections: Vec<String>,
    pub new: Option<String>,
}

pub type PickHandler = Rc<dyn Fn(Membership, &mut Window, &mut App)>;

pub struct CollectionPicker {
    /// Each collection and whether the game is in it.
    rows: Vec<(String, bool)>,
    new_name: Entity<InputState>,
    handler: PickHandler,
}

impl CollectionPicker {
    pub fn new(
        rows: Vec<(String, bool)>,
        handler: PickHandler,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let new_name = cx.new(|cx| InputState::new(window, cx).placeholder("或者新建一个合集"));
        cx.subscribe_in(
            &new_name,
            window,
            |picker, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => picker.submit(window, cx),
                _ => {}
            },
        )
        .detach();
        Self {
            rows,
            new_name,
            handler,
        }
    }

    fn typed(&self, cx: &App) -> String {
        self.new_name.read(cx).value().to_string()
    }

    fn taken(&self) -> Vec<String> {
        self.rows.iter().map(|(name, _)| name.clone()).collect()
    }

    /// The answer the dialog gives, or `None` while the new name is wrong.
    pub fn membership(&self, cx: &App) -> Option<Membership> {
        let typed = self.typed(cx);
        let new = (!typed.trim().is_empty()).then(|| typed.trim().to_owned());
        if new.is_some() && name_problem(&typed, &self.taken(), None).is_some() {
            return None;
        }
        Some(Membership {
            collections: self
                .rows
                .iter()
                .filter(|(_, member)| *member)
                .map(|(name, _)| name.clone())
                .collect(),
            new,
        })
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(membership) = self.membership(cx) {
            (self.handler)(membership, window, cx);
            close_with(window, cx);
        }
    }

    pub fn open(form: Entity<Self>, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |dialog, _, cx| {
            let ready = form.read(cx).membership(cx).is_some();
            let weak = form.downgrade();
            let ok = theme::clickable(
                Key::new("collection-pick-ok")
                    .label("完成")
                    .primary()
                    .disabled(!ready)
                    .debug_selector(|| "collection-pick-ok".into())
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |form, cx| form.submit(window, cx));
                    }),
                ready,
            );
            theme::dialog(dialog, cx)
                .title("加入合集")
                .w(px(DIALOG_WIDTH))
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Key::new("collection-pick-cancel")
                                .label("取消")
                                .white()
                                .on_click(|_, window, cx| close_with(window, cx)),
                        )
                        .child(ok),
                )
        });
    }
}

impl Render for CollectionPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let typed = self.typed(cx);
        let problem = if typed.trim().is_empty() {
            None
        } else {
            name_problem(&typed, &self.taken(), None)
        };
        let weak = cx.entity().downgrade();
        v_flex()
            .w_full()
            .gap_3()
            .children(self.rows.iter().enumerate().map(|(index, (name, member))| {
                let weak = weak.clone();
                Checkbox::new(("collection-member", index))
                    .checked(*member)
                    .label(name.clone())
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |picker, cx| {
                            if let Some(row) = picker.rows.get_mut(index) {
                                row.1 = !row.1;
                            }
                            cx.notify();
                        });
                    })
            }))
            .child(Input::new(&self.new_name))
            .children(problem.map(|problem| {
                div()
                    .text_xs()
                    .text_color(colors.danger)
                    .debug_selector(|| "collection-new-problem".into())
                    .child(problem)
            }))
            .children(self.rows.is_empty().then(|| {
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .font_medium()
                    .child("还没有合集，在下面起个名字就会新建一个")
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_must_exist_and_be_new_unless_it_is_the_one_being_renamed() {
        let taken = vec!["生存".to_owned(), "建筑".to_owned()];
        assert_eq!(name_problem("  ", &taken, None), Some("请输入名称".into()));
        assert!(
            name_problem("生存", &taken, None)
                .unwrap()
                .contains("已经有")
        );
        assert!(
            name_problem(" 生存 ", &taken, None).is_some(),
            "spaces are trimmed"
        );
        assert_eq!(
            name_problem("生存", &taken, Some("生存")),
            None,
            "keeping it is fine"
        );
        assert!(name_problem("建筑", &taken, Some("生存")).is_some());
        assert_eq!(name_problem("新的", &taken, None), None);
    }
}
