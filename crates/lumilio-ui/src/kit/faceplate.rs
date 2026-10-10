use crate::theme;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{IntoElement, SharedString, div, px};
use gpui_component::v_flex;

/// Width of a faceplate.
pub const FACEPLATE_WIDTH: gpui::Pixels = px(232.);

/// An instance card's frame (design language §12): a panel with a hairline
/// border, a soft shadow and a screw mark in each corner. Hover turns the
/// border ink.
pub fn faceplate(id: impl Into<gpui::ElementId>, colors: ShellColors) -> gpui::Stateful<gpui::Div> {
    let body = colors.body;
    let screw = |top: bool, left: bool| {
        let screw = div()
            .absolute()
            .size(px(7.))
            .rounded_full()
            .border_1()
            .border_color(body.hairline)
            .bg(body.page);
        let screw = if top {
            screw.top(px(7.))
        } else {
            screw.bottom(px(7.))
        };
        if left {
            screw.left(px(7.))
        } else {
            screw.right(px(7.))
        }
    };
    v_flex()
        .id(id)
        .relative()
        .w(FACEPLATE_WIDTH)
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(6.))
        .bg(body.panel)
        .border_1()
        .border_color(body.hairline)
        .shadow(theme::panel_shadow(body))
        .cursor_pointer()
        .hover(move |card| card.border_color(body.ink))
        .child(screw(true, true))
        .child(screw(true, false))
        .child(screw(false, true))
        .child(screw(false, false))
}

/// A faceplate's top: the name in light type over the loader's orange label.
pub fn faceplate_head(
    name: impl Into<SharedString>,
    loader: impl Into<SharedString>,
    colors: ShellColors,
) -> impl IntoElement {
    v_flex()
        .gap(px(2.))
        .pt(px(4.))
        .child(
            div()
                .text_size(px(20.))
                .line_height(px(26.))
                .font_weight(gpui::FontWeight::LIGHT)
                .text_color(colors.foreground)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(name.into()),
        )
        .child(
            div()
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(colors.body.orange_text)
                .child(SharedString::from(loader.into().to_uppercase())),
        )
}

/// The display window a cover sits in: the one place the world is framed.
/// The cover does not dissolve here.
pub fn display_window(cover: impl IntoElement, colors: ShellColors) -> gpui::Div {
    div()
        .relative()
        .h(px(96.))
        .w_full()
        .rounded(px(3.))
        .overflow_hidden()
        .bg(colors.body.display)
        .child(cover)
}

/// The cover of a faceplate's display: no dissolve, 3 px corners.
pub fn faceplate_cover(
    seed: u32,
    loader: crate::cover::Loader,
    world: crate::home::WorldHint,
    colors: ShellColors,
) -> impl IntoElement {
    crate::cover::element(seed, loader, world, colors.body.display, FACEPLATE_FADE)
}

/// A faceplate's cover has no dissolve into the page: it sits in a window.
pub const FACEPLATE_FADE: gpui::Pixels = px(0.);
