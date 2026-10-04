use crate::theme;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{IntoElement, div, px};
use gpui_component::{h_flex, v_flex};

/// Bars on a display's meter.
pub(super) const METER_BARS: usize = 24;

/// How many whole bars light for a fraction: monotonic in `fraction`, so a
/// display never steps back while progress only moves forward (§3).
pub fn lit_bars(fraction: f32, bars: usize) -> usize {
    let fraction = if fraction.is_nan() {
        0.
    } else {
        fraction.clamp(0., 1.)
    };
    ((fraction * bars as f32).floor() as usize).min(bars)
}

/// A live number on a black display (design language §12): segment digits
/// over unlit eights, and a meter that lights one whole bar at a time.
pub fn progress(
    id: impl Into<gpui::ElementId>,
    fraction: f32,
    colors: ShellColors,
) -> impl IntoElement {
    let body = colors.body;
    let lit = lit_bars(fraction, METER_BARS);
    let percent = (fraction.clamp(0., 1.) * 100.).floor() as u32;
    let id: gpui::ElementId = id.into();
    v_flex()
        .id(id)
        .flex_none()
        .w(px(168.))
        .gap(px(6.))
        .p(px(8.))
        .rounded(px(6.))
        .bg(body.display)
        .shadow(theme::display_shadow())
        .child(
            h_flex()
                .justify_end()
                .items_baseline()
                .gap(px(3.))
                .child(
                    div()
                        .relative()
                        .font_family(theme::LCD_FONT)
                        .text_size(px(18.))
                        .line_height(px(18.))
                        // Unlit segments sit behind the lit ones, as on a real
                        // display.
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .text_color(body.display_dim)
                                .child("888"),
                        )
                        .child(
                            div()
                                .relative()
                                .text_color(body.display_ink)
                                .child(format!("{percent:03}")),
                        ),
                )
                .child(
                    div()
                        .font_family(theme::MONO_FONT)
                        .text_size(px(10.))
                        .text_color(body.display_label)
                        .child("%"),
                ),
        )
        .child(
            h_flex()
                .h(px(8.))
                .gap(px(1.))
                .children((0..METER_BARS).map(|bar| {
                    div().flex_1().h_full().bg(if bar < lit {
                        body.display_ink
                    } else {
                        body.display_dim
                    })
                })),
        )
}
