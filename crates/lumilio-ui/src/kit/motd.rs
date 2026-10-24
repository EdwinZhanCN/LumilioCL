//! A server's message of the day (design language §12): the server's own
//! words, so they keep the game's colours and styles, beside the server's
//! name, in a pixel face like the game's. Each colour is moved only as far
//! as the page needs to read it.

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, FontFeatures, FontStyle, FontWeight, HighlightStyle, Hsla, Pixels, StrikethroughStyle,
    StyledText, UnderlineStyle, Window, div, px,
};
use gpui_component::v_flex;
use lumilio_core::{Motd, MotdSpan};

use crate::assets::PixelFace;
use crate::theme;
use crate::theme::ShellColors;

/// The game's server list shows two lines.
const LINES: usize = 2;
/// The pixel face draws on a grid of nine glyph pixels to the em.
const EM_PIXELS: f32 = 9.;
/// A line holds the face's ascent and descent, so an accent over a capital
/// is not cut off by the line's clip.
const LINE_PIXELS: f32 = 12.;
/// At 100 % a glyph pixel is 1.5 px: capitals stand as tall as body text's.
const GLYPH_PIXEL: f32 = 1.5;
/// The game draws its colours on black; on the page each one is darkened or
/// lightened until it reads at this ratio (WCAG AA for small text).
const CONTRAST: f32 = 4.5;
/// Glyphs an obfuscated character is drawn as. The game changes them every
/// frame; the row holds still (§1), so each position keeps one.
const SCRAMBLE: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// The lines a row shows: the first two of the message, without the space
/// around them, empty ones left out. A space-padded line is centred for the
/// game's font; in the row the padding only pushes it aside.
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

/// The colour a span is drawn in on the page: the game's colour made
/// legible, or muted text where the game would use its default grey.
pub(super) fn motd_color(span: &MotdSpan, colors: ShellColors) -> Hsla {
    span.color.map_or(colors.muted, |rgb| {
        theme::legible(gpui::rgb(rgb).into(), colors.background, CONTRAST)
    })
}

/// The message as text lines, or `None` when it has nothing to show.
pub fn motd_text(motd: &Motd, colors: ShellColors) -> Option<gpui::Div> {
    let lines = motd_lines(motd);
    if lines.is_empty() {
        return None;
    }
    Some(
        v_flex()
            .w_full()
            .text_xs()
            .text_color(colors.muted)
            .children(lines.into_iter().map(|spans| MotdLine { spans, colors })),
    )
}

/// The pixel face's size: each glyph pixel a whole number of screen pixels,
/// as near to [`GLYPH_PIXEL`] at the interface scale as that allows, so its
/// edges stay sharp.
pub(super) fn pixel_size(scale_factor: f32, rem: Pixels) -> Pixels {
    let wanted = GLYPH_PIXEL * f32::from(rem) / theme::BASE_FONT_SIZE * scale_factor;
    px(EM_PIXELS * wanted.round().max(1.) / scale_factor)
}

/// The pixel face's ligatures join `->` and `==` into arrows; a message
/// shows the characters the server sent.
fn no_ligatures() -> FontFeatures {
    FontFeatures(Arc::new(vec![("liga".into(), 0), ("calt".into(), 0)]))
}

/// One line of a message: in the pixel face once it is registered,
/// otherwise in the interface face around it, with the same colours.
#[derive(IntoElement)]
struct MotdLine {
    spans: Vec<MotdSpan>,
    colors: ShellColors,
}

impl RenderOnce for MotdLine {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let line = div()
            .w_full()
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap();
        let line = if cx.has_global::<PixelFace>() {
            let size = pixel_size(window.scale_factor(), window.rem_size());
            line.font_family(theme::PIXEL_FONT)
                .font_features(no_ligatures())
                .text_size(size)
                .line_height(size * (LINE_PIXELS / EM_PIXELS))
        } else {
            line
        };
        line.child(styled_line(&self.spans, self.colors))
    }
}

fn styled_line(spans: &[MotdSpan], colors: ShellColors) -> StyledText {
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
        let color = Some(motd_color(span, colors));
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
