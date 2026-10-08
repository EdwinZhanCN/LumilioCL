//! The version picker (IA `patterns.md` P-VERSION and P-LOADER-VERSION): a
//! dropdown with a search field, only release versions by default, and an
//! optional "show all versions" switch. Every place that picks a game or
//! loader version uses it.

use std::rc::Rc;

use crate::key::Key;
use gpui::{
    Anchor, Context, Entity, EventEmitter, IntoElement, Render, StyleRefinement, Window, div,
    prelude::*, px, uniform_list,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::Popover;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};

use crate::assets::UiIcon;
use crate::theme::{self, ShellColors};
use crate::tr;

/// One version that can be chosen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choice {
    pub id: String,
    /// Shown by default; the rest appear with "show all versions" or a search.
    pub primary: bool,
    /// A short label after the id: "快照", "预发布", "稳定"…
    pub tag: Option<&'static str>,
}

/// Where the list stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Choices {
    Loading,
    /// One plain sentence; the raw cause stays with the caller.
    Failed(String),
    Ready(Vec<Choice>),
}

pub enum PickerEvent {
    Chosen(String),
    Retry,
}

/// The choices a picker shows: a search matches every version (so a typed
/// snapshot is found with the switch off); without one, only the primary
/// ones unless `show_all`.
#[must_use]
pub fn visible<'a>(choices: &'a [Choice], query: &str, show_all: bool) -> Vec<&'a Choice> {
    let query = query.trim().to_lowercase();
    choices
        .iter()
        .filter(|choice| {
            if query.is_empty() {
                show_all || choice.primary
            } else {
                choice.id.to_lowercase().contains(&query)
            }
        })
        .collect()
}

const ROW_HEIGHT: f32 = 32.;
const LIST_HEIGHT: f32 = 280.;

pub struct VersionPicker {
    id: &'static str,
    choices: Choices,
    selected: Option<String>,
    show_all: bool,
    /// Loader versions do without the switch: "stable / latest" already
    /// said which kind is wanted.
    offers_show_all: bool,
    disabled: bool,
    open: bool,
    query: Entity<InputState>,
    placeholder: &'static str,
    /// The menu matches the trigger; set by the owner, who knows its width.
    menu_width: gpui::Pixels,
}

impl EventEmitter<PickerEvent> for VersionPicker {}

impl VersionPicker {
    pub fn new(
        id: &'static str,
        offers_show_all: bool,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("discover-version-search")));
        cx.subscribe(&query, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        Self {
            id,
            choices: Choices::Loading,
            selected: None,
            show_all: false,
            offers_show_all,
            disabled: false,
            open: false,
            query,
            placeholder,
            menu_width: px(360.),
        }
    }

    /// The menu's width; give the trigger's so the two line up.
    pub fn menu_width(mut self, width: gpui::Pixels) -> Self {
        self.menu_width = width;
        self
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn choices(&self) -> &Choices {
        &self.choices
    }

    /// Replaces the list. The selection survives when it is still listed;
    /// otherwise it becomes `fallback`.
    pub fn set_choices(
        &mut self,
        choices: Choices,
        fallback: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let keeps = match (&choices, &self.selected) {
            (Choices::Ready(list), Some(id)) => list.iter().any(|choice| &choice.id == id),
            _ => false,
        };
        if !keeps {
            self.selected = fallback;
        }
        self.choices = choices;
        cx.notify();
    }

    pub fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected = Some(id.clone());
        self.open = false;
        cx.emit(PickerEvent::Chosen(id));
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.open = false;
        }
        cx.notify();
    }

    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.open = open;
        if !open {
            self.query
                .update(cx, |query, cx| query.set_value("", window, cx));
        }
        cx.notify();
    }

    fn list(&self, colors: ShellColors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Choices::Ready(all) = &self.choices else {
            return div().into_any_element();
        };
        let query = self.query.read(cx).value().to_string();
        let shown: Vec<Choice> = visible(all, &query, self.show_all)
            .into_iter()
            .cloned()
            .collect();
        if shown.is_empty() {
            return div()
                .py_4()
                .text_sm()
                .text_color(colors.muted)
                .text_center()
                .child(tr!("version-picker-no-match"))
                .into_any_element();
        }
        let shown = Rc::new(shown);
        let selected = self.selected.clone();
        let view = cx.entity().downgrade();
        let id = self.id;
        let count = shown.len();
        let height = (count as f32 * ROW_HEIGHT).min(LIST_HEIGHT);
        uniform_list((id, 0usize), count, move |range, _, _| {
            range
                .map(|index| {
                    let choice = &shown[index];
                    let chosen = selected.as_deref() == Some(choice.id.as_str());
                    let view = view.clone();
                    let value = choice.id.clone();
                    h_flex()
                        .id((id, index + 1))
                        .w_full()
                        .debug_selector(move || format!("{id}-choice-{index}"))
                        .h(px(ROW_HEIGHT))
                        .px_2()
                        .gap_2()
                        .items_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_sm()
                        .text_color(colors.foreground)
                        .when(chosen, |row| row.bg(colors.surface_subtle))
                        .hover(|row| row.bg(colors.surface_subtle))
                        .on_click(move |_, _, cx| {
                            let value = value.clone();
                            let _ = view.update(cx, |picker, cx| picker.select(value, cx));
                        })
                        .child(div().flex_1().min_w_0().child(choice.id.clone()))
                        .children(
                            choice
                                .tag
                                .map(|tag| div().text_xs().text_color(colors.muted).child(tag)),
                        )
                        .when(chosen, |row| {
                            row.child(div().size(px(6.)).rounded_full().bg(colors.primary))
                        })
                })
                .collect()
        })
        .h(px(height))
        .into_any_element()
    }
}

impl Render for VersionPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let id = self.id;
        match &self.choices {
            Choices::Loading => {
                return field(colors)
                    .child(
                        div()
                            .text_color(colors.muted)
                            .child(tr!("version-picker-loading")),
                    )
                    .into_any_element();
            }
            Choices::Failed(message) => {
                let view = cx.entity().downgrade();
                return field(colors)
                    .justify_between()
                    .child(div().text_color(colors.danger).child(message.clone()))
                    .child(theme::clickable(
                        Key::new((id, 9_000usize))
                            .label(tr!("common-retry"))
                            .ghost()
                            .xsmall()
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |_, cx| cx.emit(PickerEvent::Retry));
                            }),
                        true,
                    ))
                    .into_any_element();
            }
            Choices::Ready(_) => {}
        }

        let label = self
            .selected
            .clone()
            .unwrap_or_else(|| self.placeholder.to_owned());
        let trigger = theme::clickable(
            Key::new(id)
                .white()
                .w_full()
                .disabled(self.disabled)
                .debug_selector(move || id.into())
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(div().text_sm().child(label))
                        .child(
                            Icon::new(UiIcon::Expand)
                                .size(px(14.))
                                .text_color(colors.muted),
                        ),
                ),
            !self.disabled,
        );
        let view = cx.entity().downgrade();
        let show_view = cx.entity().downgrade();
        let opened = cx.entity().downgrade();
        let query = self.query.clone();
        let show_all = self.show_all;
        let offers_show_all = self.offers_show_all;
        let menu_width = self.menu_width;
        Popover::new((id, 9_001usize))
            .anchor(Anchor::TopLeft)
            .trigger_style(StyleRefinement::default().w_full())
            .open(self.open)
            .on_open_change(move |open, window, cx| {
                let open = *open;
                let _ = opened.update(cx, |picker, cx| picker.set_open(open, window, cx));
            })
            .trigger(trigger)
            .content(move |_, _, cx| {
                let toggle = show_view.clone();
                let list = view
                    .upgrade()
                    .map(|picker| picker.update(cx, |picker, cx| picker.list(colors, cx)));
                v_flex()
                    .w(menu_width)
                    .gap_2()
                    .child(
                        Input::new(&query).small().prefix(
                            Icon::new(UiIcon::Search)
                                .size(px(14.))
                                .text_color(colors.muted),
                        ),
                    )
                    .children(list)
                    .when(offers_show_all, |content| {
                        content.child(
                            h_flex()
                                .pt_1()
                                .border_t_1()
                                .border_color(colors.border)
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(colors.muted)
                                        .child(tr!("discover-all-versions")),
                                )
                                .child(crate::kit::switch(
                                    (id, 9_002usize),
                                    show_all,
                                    tr!("discover-all-versions"),
                                    move |_, cx| {
                                        let _ = toggle.update(cx, |picker, cx| {
                                            picker.show_all = !picker.show_all;
                                            cx.notify();
                                        });
                                    },
                                )),
                        )
                    })
            })
            .into_any_element()
    }
}

fn field(colors: ShellColors) -> gpui::Div {
    h_flex()
        .w_full()
        .h(px(32.))
        .px_3()
        .items_center()
        .rounded(px(6.))
        .border_1()
        .border_color(colors.border)
        .text_sm()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(id: &str, primary: bool) -> Choice {
        Choice {
            id: id.to_owned(),
            primary,
            tag: None,
        }
    }

    #[test]
    fn only_primary_versions_show_until_asked_but_a_search_finds_any() {
        let all = [
            choice("26.4-snapshot-1", false),
            choice("26.3", true),
            choice("26.3-rc-1", false),
            choice("26.2", true),
        ];
        let ids = |shown: Vec<&Choice>| shown.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(visible(&all, "", false)), ["26.3", "26.2"]);
        assert_eq!(visible(&all, "", true).len(), 4);
        assert_eq!(ids(visible(&all, "rc", false)), ["26.3-rc-1"]);
        assert_eq!(ids(visible(&all, " 26.3 ", false)), ["26.3", "26.3-rc-1"]);
        assert!(visible(&all, "1.12", true).is_empty());
    }
}
