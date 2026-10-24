use gpui::prelude::*;
use gpui::{IntoElement, SharedString, div, px};
use gpui_component::{h_flex, v_flex};

use crate::theme;
use crate::theme::ShellColors;

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

/// Segment digits over unlit eights, as on a real display. A `!` is a dark
/// digit and a `.` the point between digits (DSEG7 draws both).
fn segment_digits(text: String, size: f32, body: theme::Body) -> impl IntoElement {
    let unlit: String = text
        .chars()
        .map(|glyph| if glyph == '.' { '.' } else { '8' })
        .collect();
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
                .child(unlit),
        )
        .child(div().relative().text_color(body.display_ink).child(text))
}

/// A speed on a display: the digits right-aligned in five places with one
/// decimal ("!21.1"), and its unit.
#[must_use]
pub fn speed_digits(bytes_per_second: f64) -> (String, &'static str) {
    const KB: f64 = 1024.;
    let (value, unit) = if bytes_per_second >= KB * KB {
        (bytes_per_second / (KB * KB), "MB/s")
    } else {
        (bytes_per_second.max(0.) / KB, "KB/s")
    };
    let text = format!("{:.1}", value.min(999.9));
    let blanks = 5_usize.saturating_sub(text.len());
    (format!("{}{text}", "!".repeat(blanks)), unit)
}

/// One cell per thing that may be held, lit while held: the connections a
/// file or every download uses. `lit` past `count` lights them all.
fn cells(lit: usize, count: usize, on: gpui::Hsla, off: gpui::Hsla, height: f32) -> gpui::Div {
    h_flex()
        .flex_none()
        .gap(px(2.))
        .children((0..count).map(move |cell| {
            div()
                .w(px(4.))
                .h(px(height))
                .rounded(px(1.))
                .bg(if cell < lit { on } else { off })
        }))
}

/// The connections one file downloads over, as cells on the page: orange
/// while held, LED-off grey otherwise.
pub fn connection_cells(lit: usize, count: usize, colors: ShellColors) -> gpui::Div {
    cells(lit, count, colors.body.orange, colors.body.led_off, 10.)
}

/// Bars in a row's meter as wide as its name column.
pub const ROW_METER_BARS: usize = 40;

/// A running task's progress under its name: a strip of `bars` bars that
/// light one whole bar at a time, orange while it moves and muted while it
/// is held. A narrow strip takes fewer bars, so each stays a bar.
pub fn row_meter(
    id: impl Into<gpui::ElementId>,
    fraction: f32,
    bars: usize,
    paused: bool,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    let body = colors.body;
    let lit = lit_bars(fraction, bars);
    let on = if paused { body.muted } else { body.orange };
    h_flex()
        .id(id.into())
        .w_full()
        .h(px(3.))
        .gap(px(1.))
        .children((0..bars).map(move |bar| {
            div()
                .flex_1()
                .h_full()
                .bg(if bar < lit { on } else { body.led_off })
        }))
}

/// What the download display shows (design language §12, Displays).
pub struct TransferDisplay {
    /// Bytes per second over all downloads, once known.
    pub speed: Option<f64>,
    /// Connections held, and how many may be.
    pub open: usize,
    pub limit: usize,
    /// Its legends: the speed's and the connections'.
    pub legends: (&'static str, &'static str),
    /// Counts after the connections (running, paused).
    pub counts: Vec<Reading>,
}

/// The one display on Activity: the speed of all downloads in segment
/// digits, a cell per connection the setting allows, lit while held, and the
/// counts of tasks.
pub fn transfer_display(
    id: impl Into<gpui::ElementId>,
    display: TransferDisplay,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    let body = colors.body;
    let legend = |text: SharedString| {
        div()
            .text_size(crate::theme::font_px(11.))
            .text_color(body.display_label)
            .child(text)
    };
    let unit = |text: SharedString| {
        div()
            .font_family(theme::mono_font())
            .text_size(crate::theme::font_px(10.))
            .text_color(body.display_label)
            .child(text)
    };
    let (digits, speed_unit) = speed_digits(display.speed.unwrap_or(0.));
    let speed = v_flex()
        .gap(px(6.))
        .child(legend(display.legends.0.into()))
        .child(
            h_flex()
                .items_baseline()
                .gap(px(3.))
                .child(segment_digits(digits, 22., body))
                .child(unit(speed_unit.into())),
        );
    let connections = v_flex()
        .gap(px(6.))
        .child(legend(display.legends.1.into()))
        .child(
            h_flex()
                .h(px(22.))
                .items_center()
                .gap(px(8.))
                .child(cells(
                    display.open,
                    display.limit,
                    body.display_ink,
                    body.display_dim,
                    16.,
                ))
                .child(unit(format!("{}/{}", display.open, display.limit).into())),
        );
    let counts = h_flex()
        .gap(px(20.))
        .children(display.counts.iter().map(|reading| {
            v_flex()
                .gap(px(6.))
                .child(legend(reading.label.clone()))
                .child(segment_digits(
                    reading_digits(reading.value, reading.digits),
                    22.,
                    body,
                ))
        }));
    h_flex()
        .id(id.into())
        .w_full()
        .items_end()
        .flex_wrap()
        .gap_x(px(28.))
        .gap_y(px(12.))
        .px(px(14.))
        .py(px(12.))
        .rounded(px(6.))
        .bg(body.display)
        .shadow(theme::display_shadow())
        .child(speed)
        .child(connections)
        .child(div().flex_1())
        .child(counts)
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
