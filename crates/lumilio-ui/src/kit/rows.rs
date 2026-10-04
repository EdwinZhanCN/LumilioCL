use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{AnyElement, ClickEvent, IntoElement, SharedString, div, px};
use gpui_component::Sizable as _;
use gpui_component::StyledExt as _;
use gpui_component::{Icon, h_flex, v_flex};

/// A quiet 技术详情 that opens the raw cause in a dialog (§8, §11).
pub fn technical(id: impl Into<gpui::ElementId>, detail: impl Into<SharedString>) -> Key {
    let detail: SharedString = detail.into();
    Key::new(id)
        .label("技术详情")
        .ghost()
        .small()
        .on_click(move |_: &ClickEvent, window, cx| {
            crate::toast::technical_dialog(detail.clone(), window, cx)
        })
}

/// A muted ⓘ after a label that shows its help as a tooltip (§10).
pub fn info(id: impl Into<gpui::ElementId>, help: impl Into<SharedString>) -> impl IntoElement {
    Key::new(id)
        .icon(Icon::new(UiIcon::Info).size(px(14.)))
        .ghost()
        .compact()
        .tooltip(help.into())
}

/// A settings row (§10): label and optional help on the leading side, the
/// read-only value and an optional quick control on the trailing side.
pub fn value_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    help: Option<SharedString>,
    value: impl Into<SharedString>,
    control: Option<AnyElement>,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(id)
        .w_full()
        .items_center()
        .gap_3()
        .py(px(11.))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(colors.foreground)
                        .child(label.into()),
                )
                // The row's own id scopes this one.
                .children(help.map(|help| info("info", help))),
        )
        .child(
            div()
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(colors.muted)
                .child(value.into()),
        )
        .children(control)
}

/// Silkscreen for a titled group (design language §12): an optional orange
/// mono index and the ink title.
pub fn section_label(text: impl Into<SharedString>, colors: ShellColors) -> impl IntoElement {
    silk_label(None, text, colors)
}

/// The numbered silkscreen label alone, for a section whose body the caller
/// shows or hides (an accordion head).
pub fn section_head(
    index: usize,
    text: impl Into<SharedString>,
    colors: ShellColors,
) -> impl IntoElement {
    silk_label(Some(index), text, colors)
}

pub(super) fn silk_label(
    index: Option<usize>,
    text: impl Into<SharedString>,
    colors: ShellColors,
) -> gpui::Div {
    h_flex()
        .w_full()
        .items_center()
        .gap(px(10.))
        .children(index.map(|index| {
            div()
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(colors.body.orange_text)
                .child(format!("{index:02}"))
        }))
        .child(
            div()
                .text_xs()
                .text_color(colors.foreground)
                .child(text.into()),
        )
}

pub fn section(
    label: impl Into<SharedString>,
    colors: ShellColors,
    body: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_3()
        .child(section_label(label, colors))
        .child(body)
}

/// [`section`] with a numbered silkscreen index, for pages with several
/// groups.
pub fn section_at(
    index: usize,
    label: impl Into<SharedString>,
    colors: ShellColors,
    body: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_3()
        .child(silk_label(Some(index), label, colors))
        .child(body)
}

/// The base surface of a card or list block.
pub fn surface(colors: ShellColors) -> gpui::Div {
    div()
        .rounded(px(6.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface)
}

/// One row of a list block. `lead` and `trail` are optional slots.
pub fn row(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    lead: Option<AnyElement>,
    trail: Option<AnyElement>,
    colors: ShellColors,
) -> gpui::Div {
    h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .py(px(11.))
        .children(lead)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(colors.foreground)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(detail.into()),
                ),
        )
        .children(trail)
}

/// Stacks rows into a hairline table with no surface (design language §12):
/// an ink rule on top and a hairline under each row.
pub fn list<E: IntoElement>(rows: Vec<E>, colors: ShellColors) -> impl IntoElement {
    div()
        .w_full()
        .border_t_1()
        .border_color(colors.foreground)
        .children(rows.into_iter().map(move |row| {
            div()
                .w_full()
                .border_b_1()
                .border_color(colors.border)
                .child(row)
        }))
}

/// A collection of things (mods, worlds, downloads) as a panel of rows with
/// hairline separators.
pub fn panel_list<E: IntoElement>(rows: Vec<E>, colors: ShellColors) -> impl IntoElement {
    let last = rows.len().saturating_sub(1);
    surface(colors)
        .overflow_hidden()
        .children(rows.into_iter().enumerate().map(move |(index, row)| {
            div()
                .w_full()
                .px_4()
                .when(index < last, |wrap| {
                    wrap.border_b_1().border_color(colors.border)
                })
                .child(row)
        }))
}

/// One small vignette, one sentence (design language §7).
pub fn empty(
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    colors: ShellColors,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .items_center()
        .gap_1()
        .py_10()
        .child(
            div()
                .text_base()
                .font_semibold()
                .text_color(colors.foreground)
                .child(title.into()),
        )
        .child(div().text_sm().text_color(colors.muted).child(body.into()))
}

/// A settings row: title and explanation on the left, the control on the right.
pub fn setting(
    title: &'static str,
    detail: impl Into<SharedString>,
    control: impl IntoElement,
    colors: ShellColors,
) -> gpui::Div {
    row(
        title,
        detail,
        None,
        Some(control.into_any_element()),
        colors,
    )
}
