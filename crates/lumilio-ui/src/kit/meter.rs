use crate::theme;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{IntoElement, SharedString, div, px};
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

/// One number on a record display: what it counts, the count, and its unit
/// (§8 allows Latin units such as `H`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reading {
    pub label: SharedString,
    pub value: u64,
    /// How many digits the display has; a larger value shows all eights lit
    /// rather than spilling over.
    pub digits: usize,
    pub unit: Option<&'static str>,
}

/// The lit text of a reading: zero-padded to the display's width, or every
/// segment lit when the value does not fit.
pub fn reading_digits(value: u64, digits: usize) -> String {
    let text = format!("{value:0digits$}");
    if text.len() > digits {
        "8".repeat(digits)
    } else {
        text
    }
}

/// Segment digits over unlit eights, as on a real display.
fn segment_digits(text: String, size: f32, body: theme::Body) -> impl IntoElement {
    div()
        .relative()
        .font_family(theme::LCD_FONT)
        .text_size(px(size))
        .line_height(px(size))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .text_color(body.display_dim)
                .child("8".repeat(text.len())),
        )
        .child(div().relative().text_color(body.display_ink).child(text))
}

/// Numbers that are a record, not progress, on a black display (design
/// language §12): each reading's label in the display's legend colour over
/// its segment digits. Nothing on it moves.
pub fn record(
    id: impl Into<gpui::ElementId>,
    readings: &[Reading],
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    let body = colors.body;
    h_flex()
        .id(id.into())
        .flex_none()
        .items_end()
        .gap(px(20.))
        .px(px(14.))
        .py(px(12.))
        .rounded(px(6.))
        .bg(body.display)
        .shadow(theme::display_shadow())
        .children(readings.iter().map(|reading| {
            v_flex()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(crate::theme::font_px(11.))
                        .text_color(body.display_label)
                        .child(reading.label.clone()),
                )
                .child(
                    h_flex()
                        .items_baseline()
                        .gap(px(3.))
                        .child(segment_digits(
                            reading_digits(reading.value, reading.digits),
                            22.,
                            body,
                        ))
                        .children(reading.unit.map(|unit| {
                            div()
                                .font_family(theme::mono_font())
                                .text_size(crate::theme::font_px(10.))
                                .text_color(body.display_label)
                                .child(unit)
                        })),
                )
        }))
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
                .child(segment_digits(format!("{percent:03}"), 18., body))
                .child(
                    div()
                        .font_family(theme::mono_font())
                        .text_size(crate::theme::font_px(10.))
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
