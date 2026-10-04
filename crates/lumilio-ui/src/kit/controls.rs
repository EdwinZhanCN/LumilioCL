use crate::assets::UiIcon;
use crate::controls::{PortTabs, Segments};
use crate::key::Key;
use crate::theme;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{App, ClickEvent, Hsla, IntoElement, SharedString, Window, div, px};
use gpui_component::Sizable as _;
use gpui_component::StyledExt as _;
use gpui_component::{Icon, h_flex};

/// View tabs as port labels (design language §12).
pub fn tabs(
    id: &'static str,
    labels: &[&'static str],
    active: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    PortTabs::new(id, labels, active, on_select)
}

/// Refinement choices, for the second level under a section: segment keys
/// with an LED over the one in use.
pub fn segments(
    id: &'static str,
    labels: &[&'static str],
    active: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Segments::new(id, labels, active, on_select)
}

/// One option of a filter list: an LED and a label on a hairline row. The
/// chosen option lights its LED and reads in ink, semibold; the rest are muted.
pub fn led_option(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    chosen: bool,
    colors: ShellColors,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let body = colors.body;
    h_flex()
        .id(id)
        .w_full()
        .h(px(32.))
        .gap(px(10.))
        .items_center()
        .border_b_1()
        .border_color(colors.border)
        .cursor_pointer()
        .hover(move |row| row.bg(body.panel))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(crate::controls::led(chosen, body))
        .child(
            div()
                .text_sm()
                .when(chosen, |label| label.font_semibold())
                .text_color(if chosen {
                    colors.foreground
                } else {
                    colors.muted
                })
                .child(label.into()),
        )
}

/// A round identity mark: the name's first letter on a colour taken from the
/// name, so the same account always looks the same. Stands in for the skin
/// head until skins exist.
pub fn avatar(name: &str, size: f32) -> gpui::Div {
    let hue = (crate::live::seed_of(name) % 360) as f32 / 360.;
    let initial = name
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().to_string())
        .unwrap_or_default();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .bg(gpui::hsla(hue, 0.42, 0.42, 1.))
        .text_color(gpui::white())
        .text_size(px(size * 0.45))
        .font_semibold()
        .child(initial)
}

/// The kinds of tag (design language §12).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TagKind {
    /// A proper noun such as the loader.
    Ink,
    /// Versions and plain facts.
    Plain,
    /// At most one emphasis tag per region.
    Orange,
}

/// A 20 px mono tag.
pub fn tag(text: impl Into<SharedString>, kind: TagKind, colors: ShellColors) -> gpui::Div {
    let body = colors.body;
    let (bg, fg) = match kind {
        TagKind::Orange => (body.orange, body.on_orange),
        TagKind::Ink => (body.ink, body.page),
        TagKind::Plain => (body.key_grey, body.ink),
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px(20.))
        .px(px(6.))
        .rounded(px(2.))
        .bg(bg)
        .text_color(fg)
        .font_family(theme::MONO_FONT)
        .text_size(px(11.))
        .child(text.into())
}

/// A status is not a tag: it is an LED and a word. Without a `tone` it is a
/// plain tag.
pub fn chip(
    text: impl Into<SharedString>,
    tone: Option<Hsla>,
    colors: ShellColors,
) -> impl IntoElement {
    match tone {
        Some(tone) => h_flex()
            .flex_none()
            .gap(px(6.))
            .items_center()
            .text_xs()
            .text_color(colors.foreground)
            .child(crate::controls::tone_led(tone))
            .child(text.into())
            .into_any_element(),
        None => tag(text, TagKind::Plain, colors).into_any_element(),
    }
}

pub fn tone_ok() -> Hsla {
    gpui::hsla(0.38, 0.55, 0.5, 1.)
}

pub fn tone_warn() -> Hsla {
    gpui::hsla(0.11, 0.85, 0.55, 1.)
}

/// A key on the page: orange when primary, white otherwise (design language
/// §12).
pub fn action(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    icon: Option<UiIcon>,
    primary: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> Key {
    let mut key = Key::new(id).label(label);
    if let Some(icon) = icon {
        key = key.icon(Icon::new(icon));
    }
    let key = if primary { key.primary() } else { key.white() };
    key.on_click(move |_: &ClickEvent, window, cx| on_click(window, cx))
}

/// A quiet text key for row-level actions.
pub fn ghost(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> Key {
    Key::new(id)
        .label(label)
        .ghost()
        .small()
        .on_click(move |_: &ClickEvent, window, cx| on_click(window, cx))
}

/// A labelled on/off preference: a fader with an LED.
pub fn switch(
    id: impl Into<gpui::ElementId>,
    checked: bool,
    label: &'static str,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    crate::controls::Fader::new(id, checked, label, on_toggle)
}
