//! The list a trailing chip opens (design language §6): a heading, a search
//! field once the list is long, and the choices in a scroll area of their
//! own, so a large library or many accounts stay within reach.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    Anchor, AnyElement, App, ElementId, Entity, Focusable as _, IntoElement, SharedString, Window,
    div, px,
};
use gpui_component::dialog::Confirm;
use gpui_component::input::{Input, InputState};
use gpui_component::popover::{Popover, PopoverState};
use gpui_component::{Icon, Sizable as _, StyledExt as _, h_flex, v_flex};

use super::{CloseHandler, PickHandler};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme::ShellColors;

/// From this many choices the list opens with a search field: about as many
/// as it shows before it scrolls.
pub(super) const SEARCH_FROM: usize = 8;
/// The tallest the choices grow; past it they scroll.
pub(super) const LIST_MAX_HEIGHT: gpui::Pixels = px(320.);

/// The choices a search keeps, as indices in their order: each word of
/// `query` is somewhere in the choice's words, whatever the case.
#[must_use]
pub(super) fn matching(query: &str, words: &[String]) -> Vec<usize> {
    let query = query.to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();
    words
        .iter()
        .enumerate()
        .filter(|(_, words)| {
            let words = words.to_lowercase();
            terms.iter().all(|term| words.contains(term))
        })
        .map(|(index, _)| index)
        .collect()
}

/// One more entry under the choices; choosing it closes the list.
pub(super) struct Footer {
    pub(super) id: &'static str,
    pub(super) label: SharedString,
    pub(super) on_click: CloseHandler,
}

/// What a chip's list holds; [`ChoiceList::popover`] opens it from the chip.
pub(super) struct ChoiceList {
    /// Names the parts: `navigation-instance` gives
    /// `navigation-instance-popover` and `navigation-instance-choice-3`.
    pub(super) id: &'static str,
    pub(super) heading: SharedString,
    /// The search field's placeholder: what is searched.
    pub(super) placeholder: SharedString,
    /// Said in place of the choices when a search keeps none.
    pub(super) no_match: SharedString,
    pub(super) width: gpui::Pixels,
    /// Per choice, the words a search looks in.
    pub(super) words: Vec<String>,
    pub(super) selected: Option<usize>,
    /// The picture and words of the choice at an index.
    pub(super) row: Rc<dyn Fn(usize) -> AnyElement>,
    pub(super) on_choose: PickHandler,
    pub(super) footer: Option<Footer>,
}

impl ChoiceList {
    pub(super) fn popover(
        self,
        trigger: Key,
        colors: ShellColors,
        window: &mut Window,
        cx: &mut App,
    ) -> Popover {
        let id = self.id;
        // The field lives while the chip is drawn, so it can take the focus
        // as the list opens; it is emptied as the list closes. A new
        // language makes a new field with its placeholder in that language.
        let search = (self.words.len() >= SEARCH_FROM).then(|| {
            let placeholder = self.placeholder.clone();
            window.use_keyed_state(
                ElementId::named_usize(format!("{id}-search"), crate::i18n::generation() as usize),
                cx,
                |window, cx| InputState::new(window, cx).placeholder(placeholder),
            )
        });
        let popover = Popover::new(SharedString::from(format!("{id}-popover")))
            .anchor(Anchor::BottomRight)
            .trigger(trigger);
        let popover = match search.clone() {
            Some(search) => popover
                .track_focus(&search.focus_handle(cx))
                .on_open_change(move |open, window, cx| {
                    if !*open {
                        search.update(cx, |search, cx| search.set_value("", window, cx));
                    }
                }),
            None => popover,
        };
        let list = Rc::new(self);
        popover.content(move |_, _, cx| list.clone().render(search.clone(), colors, cx))
    }

    fn render(
        self: Rc<Self>,
        search: Option<Entity<InputState>>,
        colors: ShellColors,
        cx: &mut gpui::Context<PopoverState>,
    ) -> AnyElement {
        let id = self.id;
        let popover = cx.entity();
        let choose: PickHandler = {
            let (on_choose, popover) = (self.on_choose.clone(), popover.clone());
            Rc::new(move |index, window, cx| {
                on_choose(index, window, cx);
                popover.update(cx, |state, cx| state.dismiss(window, cx));
            })
        };
        let query = search
            .as_ref()
            .map(|search| search.read(cx).value())
            .unwrap_or_default();
        let shown = matching(&query, &self.words);
        // With a search typed, Enter takes the first choice left; it shows
        // the focus colour so the key's effect is visible before it is pressed.
        let target = (!query.trim().is_empty())
            .then(|| shown.first().copied())
            .flatten();

        // Rows keep the height their words need at any interface scale, so
        // the list scrolls as a whole rather than as a list of fixed rows.
        let choices = if shown.is_empty() {
            div()
                .debug_selector(move || format!("{id}-no-match"))
                .py_4()
                .text_sm()
                .text_color(colors.muted)
                .text_center()
                .child(self.no_match.clone())
                .into_any_element()
        } else {
            v_flex()
                .id(SharedString::from(format!("{id}-choices")))
                .debug_selector(move || format!("{id}-choices"))
                .max_h(LIST_MAX_HEIGHT)
                .overflow_y_scroll()
                .gap(px(2.))
                .children(shown.into_iter().map(|index| {
                    let choose = choose.clone();
                    let selected = self.selected == Some(index);
                    h_flex()
                        .id(ElementId::named_usize(format!("{id}-choice"), index))
                        .debug_selector(move || format!("{id}-choice-{index}"))
                        .flex_none()
                        .gap_3()
                        .items_center()
                        .px_2()
                        .py(px(5.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(if target == Some(index) {
                            colors.focus
                        } else {
                            gpui::transparent_black()
                        })
                        .cursor_pointer()
                        .when(selected, |row| row.bg(colors.surface_subtle))
                        .hover(|row| row.bg(colors.surface_subtle))
                        .on_click(move |_, window, cx| choose(index, window, cx))
                        .child((self.row)(index))
                        .when(selected, |row| {
                            row.child(div().size(px(6.)).rounded_full().bg(colors.primary))
                        })
                }))
                .into_any_element()
        };

        let footer = self.footer.as_ref().map(|footer| {
            let (on_click, popover) = (footer.on_click.clone(), popover.clone());
            let footer_id = footer.id;
            div()
                .id(footer_id)
                .debug_selector(move || footer_id.into())
                .mt_1()
                .px_2()
                .py(px(8.))
                .rounded(px(8.))
                .border_t_1()
                .border_color(colors.border.opacity(0.6))
                .text_sm()
                .text_color(colors.muted)
                .cursor_pointer()
                .hover(|row| row.bg(colors.surface_subtle))
                .on_click(move |_, window, cx| {
                    on_click(window, cx);
                    popover.update(cx, |state, cx| state.dismiss(window, cx));
                })
                .child(footer.label.clone())
        });

        let field = search.clone().map(|search| {
            div()
                .debug_selector(move || format!("{id}-search"))
                .pb_1()
                .child(
                    Input::new(&search).small().cleanable(true).prefix(
                        Icon::new(UiIcon::Search)
                            .size(px(14.))
                            .text_color(colors.muted),
                    ),
                )
        });

        v_flex()
            .w(self.width)
            .gap(px(2.))
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .font_semibold()
                    .text_color(colors.muted)
                    .child(self.heading.clone()),
            )
            .children(field)
            .child(choices)
            .children(footer)
            // Enter in the field: the popover would close on it, so it
            // chooses the first choice the search keeps, if any, instead.
            .when_some(search, |content, search| {
                let list = self.clone();
                content.on_action(move |_: &Confirm, window, cx| {
                    let query = search.read(cx).value();
                    if query.trim().is_empty() {
                        return;
                    }
                    if let Some(&index) = matching(&query, &list.words).first() {
                        choose(index, window, cx);
                    }
                })
            })
            .into_any_element()
    }
}
