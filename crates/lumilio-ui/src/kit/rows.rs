use gpui::prelude::*;
use gpui::{AnyElement, ClickEvent, IntoElement, SharedString, div, px};
use gpui_component::{Icon, Sizable as _, StyledExt as _, h_flex, v_flex};

use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme;
use crate::theme::ShellColors;
use crate::toast;
use crate::toast::Toast;

/// A quiet 技术详情 that opens the raw cause in a dialog (§8, §11).
pub fn technical(id: impl Into<gpui::ElementId>, detail: impl Into<SharedString>) -> Key {
    let detail: SharedString = detail.into();
    Key::new(id)
        .label(crate::tr!("common-technical-details"))
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
/// read-only value and an optional quick control on the trailing side. A
/// value too long for the room left wraps there: notes and permissions are
/// sentences, and an ellipsis would hide what they say.
pub fn value_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    help: Option<SharedString>,
    value: impl Into<SharedString>,
    control: Option<AnyElement>,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    let value = value_text(colors).child(value.into());
    row_frame(id, label, help, value.into_any_element(), control, colors)
}

/// A [`value_row`] whose value is a path (§10). A long path stays on one
/// line and gives way in the middle, keeping the drive and the last folders;
/// hovering it shows the whole path, and a key beside it copies the whole
/// path.
pub fn path_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    help: Option<SharedString>,
    path: &std::path::Path,
    control: Option<AnyElement>,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let copy_id = id.clone();
    let shown: SharedString = path.display().to_string().into();
    let copied = shown.clone();
    let value = h_flex()
        .min_w_0()
        .gap_1()
        .items_center()
        .child(
            middle_line("path", shown)
                .font_family(theme::mono_font())
                .text_xs()
                .text_color(colors.muted),
        )
        .child(
            Key::new("copy")
                .icon(Icon::new(UiIcon::Copy).size(px(14.)))
                .ghost()
                .compact()
                .tooltip(crate::tr!("common-copy-path"))
                .debug_selector(move || format!("{copy_id}-copy"))
                .on_click(move |_: &ClickEvent, _, cx| {
                    toast::copy_text(copied.to_string(), cx);
                    toast::post(Toast::success(crate::tr!("common-path-copied")), cx);
                }),
        );
    row_frame(id, label, help, value.into_any_element(), control, colors)
}

/// One line that gives way in the middle and shows all of itself on hover:
/// a path, or a line that ends in one (§10).
pub fn middle_line(
    id: impl Into<gpui::ElementId>,
    text: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let text = text.into();
    let whole = text.clone();
    div()
        .id(id)
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis_middle()
        .child(text)
        .tooltip(move |window, cx| {
            gpui_component::tooltip::Tooltip::new(whole.clone()).build(window, cx)
        })
}

/// How a row's read-only value reads: quiet mono, able to give way.
fn value_text(colors: ShellColors) -> gpui::Div {
    div()
        .min_w_0()
        .font_family(theme::mono_font())
        .text_xs()
        .text_color(colors.muted)
}

/// The shared row: the value takes only the room the label and the control
/// leave, so a long value gives way first and the control never moves out
/// of the row.
fn row_frame(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    help: Option<SharedString>,
    value: AnyElement,
    control: Option<AnyElement>,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let label_id = id.clone();
    h_flex()
        .id(id)
        .w_full()
        .items_center()
        .gap_3()
        .py(px(11.))
        .child(
            h_flex()
                .min_w_0()
                .gap_1()
                .items_center()
                .debug_selector(move || format!("{label_id}-label"))
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
        .child(h_flex().flex_1().min_w_0().justify_end().child(value))
        .children(control.map(|control| h_flex().flex_none().child(control)))
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
                .font_family(theme::mono_font())
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

/// [`section`] whose label carries the group's own action on its trailing
/// edge ("添加…"), so the action sits above what it changes and the body
/// starts right under the label.
pub fn section_with(
    label: impl Into<SharedString>,
    colors: ShellColors,
    action: impl IntoElement,
    body: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_3()
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .child(div().flex_1().min_w_0().child(section_label(label, colors)))
                .child(action),
        )
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

/// One row of a list block. `lead` and `trail` are optional slots; `detail`
/// is text, or a [`middle_line`] when it ends in a path.
pub fn row(
    title: impl Into<SharedString>,
    detail: impl IntoElement,
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
                .child(div().text_xs().text_color(colors.muted).child(detail)),
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
        detail.into(),
        None,
        Some(control.into_any_element()),
        colors,
    )
}
