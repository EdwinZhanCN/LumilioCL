//! A server's message of the day in a display window (design language §12):
//! the server's own words, so they keep the game's colours and styles, on the
//! dark ground the game draws them on. Outside the window the interface keeps
//! its one accent.

use crate::theme::{self, ShellColors};
use gpui::prelude::*;
use gpui::{
    FontStyle, FontWeight, HighlightStyle, Hsla, StrikethroughStyle, StyledText, UnderlineStyle,
    div, px,
};
use gpui_component::v_flex;
use lumilio_core::{Motd, MotdSpan};

/// The game draws a message's uncoloured text in this grey.
const DEFAULT_COLOR: u32 = 0x80_8080;
/// The game's server list shows two lines.
const LINES: usize = 2;
/// Glyphs an obfuscated character is drawn as. The game changes them every
/// frame; the window holds still (§1), so each position keeps one.
const SCRAMBLE: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// The lines a window shows: the first two of the message, without the space
/// around them, empty ones left out. A space-padded line is centred for the
/// game's font; in the window the padding only pushes it aside.
#[must_use]
pub fn motd_lines(motd: &Motd) -> Vec<Vec<MotdSpan>> {
    motd.lines()
        .into_iter()
        .take(LINES)
        .map(trimmed)
        .filter(|line| !line.is_empty())
        .collect()
}

/// A line without the white space at either end, across its runs.
fn trimmed(mut line: Vec<MotdSpan>) -> Vec<MotdSpan> {
    while let Some(first) = line.first_mut() {
        let kept = first.text.trim_start().len();
        if kept == 0 {
            line.remove(0);
        } else {
            first.text.drain(..first.text.len() - kept);
            break;
        }
    }
    while let Some(last) = line.last_mut() {
        last.text.truncate(last.text.trim_end().len());
        if last.text.is_empty() {
            line.pop();
        } else {
            break;
        }
    }
    line
}

/// The window, or `None` when the message has nothing to show.
pub fn motd_window(motd: &Motd, colors: ShellColors) -> Option<gpui::Div> {
    let lines = motd_lines(motd);
    if lines.is_empty() {
        return None;
    }
    let ground: Hsla = gpui::rgb(DEFAULT_COLOR).into();
    Some(
        v_flex()
            .w_full()
            .max_w(px(480.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(3.))
            .bg(colors.body.display)
            .font_family(theme::mono_font())
            .text_size(theme::font_px(12.))
            .line_height(px(16.))
            .text_color(ground)
            .children(lines.iter().map(|line| {
                div()
                    .w_full()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(styled_line(line))
            })),
    )
}

fn styled_line(spans: &[MotdSpan]) -> StyledText {
    let mut text = String::new();
    let mut highlights = Vec::with_capacity(spans.len());
    for span in spans {
        let start = text.len();
        if span.obfuscated {
            text.extend(
                span.text
                    .chars()
                    .enumerate()
                    .map(|(at, ch)| scramble(ch, at)),
            );
        } else {
            text.push_str(&span.text);
        }
        let color: Option<Hsla> = span.color.map(|rgb| gpui::rgb(rgb).into());
        highlights.push((
            start..text.len(),
            HighlightStyle {
                color,
                font_weight: span.bold.then_some(FontWeight::BOLD),
                font_style: span.italic.then_some(FontStyle::Italic),
                underline: span.underlined.then_some(UnderlineStyle {
                    thickness: px(1.),
                    color,
                    wavy: false,
                }),
                strikethrough: span.strikethrough.then_some(StrikethroughStyle {
                    thickness: px(1.),
                    color,
                }),
                ..HighlightStyle::default()
            },
        ));
    }
    StyledText::new(text).with_highlights(highlights)
}

/// The glyph an obfuscated character holds at `at`; spaces stay spaces.
fn scramble(ch: char, at: usize) -> char {
    if ch.is_whitespace() {
        return ch;
    }
    let pick = (at * 31 + ch as usize) % SCRAMBLE.len();
    char::from(SCRAMBLE[pick])
}
