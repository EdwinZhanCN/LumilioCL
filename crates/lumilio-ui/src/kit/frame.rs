use crate::assets::UiIcon;
use crate::theme;
use crate::theme::{ShellColors, motion};
use gpui::AnimationExt as _;
use gpui::prelude::*;
use gpui::{Animation, AnyElement, IntoElement, SharedString, div, ease_out_quint, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use gpui_component::{Icon, TITLE_BAR_HEIGHT, h_flex, v_flex};

/// The scrolling page frame: clears the title bar and the floating capsule and
/// keeps content on the shared column.
pub fn page(id: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .id(id)
        .size_full()
        // The scroll container stays a plain block: a flex row here would
        // stretch the column to the viewport and nothing would ever scroll.
        .overflow_y_scroll()
        .child(
            theme::content_column()
                .mx_auto()
                .pt(TITLE_BAR_HEIGHT + px(12.))
                .pb(theme::BOTTOM_SAFE_AREA)
                .gap_6()
                .child(content),
        )
}

/// Keeps the wheel inside a list that scrolls within a page. Gpui gives a
/// wheel event to every scrollable under the pointer, so without this the
/// page moves along with the list. A list that fits has nothing to scroll and
/// lets the page have the wheel.
pub fn keep_wheel<E: gpui::InteractiveElement>(element: E, scroll: &gpui::ScrollHandle) -> E {
    let scroll = scroll.clone();
    element.on_scroll_wheel(move |_, _, cx| {
        if scroll.max_offset().y > px(0.) {
            cx.stop_propagation();
        }
    })
}

/// Route content enters with a short fade and lift, keyed by what it shows so
/// a new subject restarts the entrance and an unchanged one does not.
pub fn entrance(element: impl IntoElement, key: (&'static str, usize)) -> AnyElement {
    div()
        .w_full()
        .child(element)
        .with_animation(
            key,
            Animation::new(motion::QUICK).with_easing(ease_out_quint()),
            |element, delta| element.opacity(delta).mt(motion::LIFT * (1. - delta)),
        )
        .into_any_element()
}

pub fn header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    trailing: Option<AnyElement>,
    colors: ShellColors,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_size(px(36.))
                        .line_height(px(42.))
                        .font_weight(gpui::FontWeight::LIGHT)
                        .text_color(colors.foreground)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(subtitle.into()),
                ),
        )
        .children(trailing)
}

/// The toolbar row (design language §7): view tabs on the leading side,
/// search on the trailing edge, even when there are no tabs.
pub fn toolbar(tabs: Option<AnyElement>, search: Option<AnyElement>) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().min_w_0().children(tabs))
        .children(search.map(|search| div().flex_none().child(search)))
}

/// The page search field: fixed width, magnifier prefix.
pub fn search_field(state: &gpui::Entity<InputState>, colors: ShellColors) -> AnyElement {
    div()
        .w(SEARCH_WIDTH)
        .child(
            Input::new(state).small().cleanable(true).prefix(
                Icon::new(UiIcon::Search)
                    .size(px(14.))
                    .text_color(colors.muted),
            ),
        )
        .into_any_element()
}

pub const SEARCH_WIDTH: gpui::Pixels = px(240.);
