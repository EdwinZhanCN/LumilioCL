//! A list row that draws each thing it says about its item in the form that
//! thing takes (design language §12): a state is an LED and a word, a mode or
//! a version is a tag, a time or a count is a mono reading on the trailing
//! edge, an address is a muted mono line under the name. Pages hand over the
//! facts; they never join them into one line of text.

use super::controls::{TagKind, tag};
use crate::theme::{self, ShellColors};
use gpui::prelude::*;
use gpui::{
    AnyElement, ElementId, ImageSource, IntoElement, ObjectFit, SharedString, div, img, px,
};
use gpui_component::{h_flex, v_flex};

/// The trailing readings column is at least this wide, so a short time and a
/// long one keep the actions after them in line.
const READINGS_WIDTH: gpui::Pixels = px(88.);
/// A tag holding free text (a server's version name) is cut here.
const TAG_MAX_WIDTH: gpui::Pixels = px(180.);
/// Edge of a row's leading picture.
const LEAD_SIZE: gpui::Pixels = px(40.);

/// How a state's LED shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    /// Lit orange: what is happening now.
    Lit,
    /// Green: it works.
    Ok,
    /// The failure colour: it cannot be used.
    Failure,
    /// Unlit: not known yet, or not there.
    Off,
}

/// A state: an LED and a word. The word carries the meaning, so colour is
/// never the only sign (§9).
pub fn status(word: impl Into<SharedString>, tone: Tone, colors: ShellColors) -> gpui::Div {
    let body = colors.body;
    let (led, ink) = match tone {
        Tone::Lit => (crate::controls::led(true, body), colors.foreground),
        Tone::Ok => (crate::controls::tone_led(body.ok), colors.foreground),
        Tone::Failure => (crate::controls::tone_led(colors.danger), colors.danger),
        Tone::Off => (crate::controls::led(false, body), colors.muted),
    };
    h_flex()
        .flex_none()
        .gap(px(6.))
        .items_center()
        .text_xs()
        .whitespace_nowrap()
        .text_color(ink)
        .child(led)
        .child(word.into())
}

/// A reading for the trailing column: a time or a size, in muted mono.
pub fn reading(text: impl Into<SharedString>, colors: ShellColors) -> gpui::Div {
    div()
        .flex_none()
        .font_family(theme::mono_font())
        .text_xs()
        .whitespace_nowrap()
        .text_color(colors.muted)
        .child(text.into())
}

/// A count out of a limit ("12/100"): the count in ink, the limit muted.
pub fn count_of(count: u64, limit: Option<u64>, colors: ShellColors) -> gpui::Div {
    h_flex()
        .flex_none()
        .font_family(theme::mono_font())
        .text_xs()
        .whitespace_nowrap()
        .child(div().text_color(colors.foreground).child(count.to_string()))
        .children(limit.map(|limit| div().text_color(colors.muted).child(format!("/{limit}"))))
}

/// Lit bars for a round trip, as the game's server list shows them: five
/// under 150 ms, then one fewer at 300, 600 and 1000 ms.
#[must_use]
pub const fn signal_bars(latency_ms: u64) -> usize {
    match latency_ms {
        0..150 => 5,
        150..300 => 4,
        300..600 => 3,
        600..1000 => 2,
        _ => 1,
    }
}

/// Signal bars: five rising bars, lit in ink by [`signal_bars`]. The
/// tooltip gives the round trip in milliseconds.
pub fn signal(
    id: impl Into<ElementId>,
    latency_ms: u64,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    let lit = signal_bars(latency_ms);
    let tip: SharedString = format!("{latency_ms} ms").into();
    h_flex()
        .id(id)
        .flex_none()
        .h(px(12.))
        .items_end()
        .gap(px(1.))
        .children((0..5).map(move |bar| {
            div()
                .w(px(3.))
                .h(px(4. + 2. * bar as f32))
                .rounded(px(1.))
                .bg(if bar < lit {
                    colors.foreground
                } else {
                    colors.body.led_off
                })
        }))
        .tooltip(move |window, cx| {
            gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
        })
}

/// A row's leading picture in a display window (§12): the item's own picture,
/// or a cover seeded by its identity when it has none.
pub fn cover_tile(
    picture: Option<ImageSource>,
    seed: u32,
    loader: crate::cover::Loader,
    colors: ShellColors,
) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .size(LEAD_SIZE)
        .rounded(px(3.))
        .overflow_hidden()
        .bg(colors.body.display)
        .child(match picture {
            Some(source) => img(source)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => crate::cover::element(
                seed,
                loader,
                crate::home::WorldHint::Overworld,
                colors.body.display,
                px(0.),
            )
            .into_any_element(),
        })
}

/// One row of a list: the name with its state, tags and a note under it,
/// anything that needs a line of its own below, readings on the trailing
/// edge, then the actions. Every slot but the name is optional.
#[must_use]
pub struct Item {
    title: SharedString,
    lead: Option<AnyElement>,
    state: Option<(SharedString, Tone)>,
    tags: Vec<(SharedString, TagKind)>,
    note: Option<SharedString>,
    below: Option<AnyElement>,
    readings: Vec<AnyElement>,
    actions: Option<AnyElement>,
}

impl Item {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            lead: None,
            state: None,
            tags: Vec::new(),
            note: None,
            below: None,
            readings: Vec::new(),
            actions: None,
        }
    }

    /// A cover or icon before the name.
    pub fn lead(mut self, lead: impl IntoElement) -> Self {
        self.lead = Some(lead.into_any_element());
        self
    }

    /// The item's state beside its name.
    pub fn state(mut self, word: impl Into<SharedString>, tone: Tone) -> Self {
        self.state = Some((word.into(), tone));
        self
    }

    /// A tag under the name: ink for a mode or proper noun, plain for a
    /// version.
    pub fn tag(mut self, text: impl Into<SharedString>, kind: TagKind) -> Self {
        self.tags.push((text.into(), kind));
        self
    }

    /// A muted mono line after the tags: an address, a folder.
    pub fn note(mut self, text: impl Into<SharedString>) -> Self {
        self.note = Some(text.into());
        self
    }

    /// Something that needs a line of its own under the name.
    pub fn below(mut self, element: impl IntoElement) -> Self {
        self.below = Some(element.into_any_element());
        self
    }

    /// A reading on the trailing edge ([`reading`], [`count_of`], [`signal`],
    /// or a [`status`] that stands in for them).
    pub fn reading(mut self, element: impl IntoElement) -> Self {
        self.readings.push(element.into_any_element());
        self
    }

    /// The row's keys, last on the trailing edge.
    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }

    pub fn render(self, colors: ShellColors) -> gpui::Div {
        let head = h_flex()
            .min_w_0()
            .gap_2()
            .items_center()
            .child(
                div()
                    .min_w_0()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(self.title),
            )
            .children(self.state.map(|(word, tone)| status(word, tone, colors)));
        let facts = (!self.tags.is_empty() || self.note.is_some()).then(|| {
            h_flex()
                .min_w_0()
                .gap(px(6.))
                .items_center()
                .children(self.tags.into_iter().map(move |(text, kind)| {
                    tag(text, kind, colors)
                        .max_w(TAG_MAX_WIDTH)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                }))
                .children(self.note.map(|note| {
                    div()
                        .min_w_0()
                        .font_family(theme::mono_font())
                        .text_xs()
                        .text_color(colors.muted)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(note)
                }))
        });
        // The picture, the name block, the readings and the keys share one
        // line; what needs a line of its own goes under it, so a tall row
        // keeps its picture beside its name.
        let line = h_flex()
            .w_full()
            .min_h(LEAD_SIZE)
            .items_center()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(head)
                    .children(facts),
            )
            .when(!self.readings.is_empty(), |line| {
                line.child(
                    h_flex()
                        .flex_none()
                        .min_w(READINGS_WIDTH)
                        .justify_end()
                        .items_center()
                        .gap_3()
                        .children(self.readings),
                )
            })
            .children(self.actions);
        h_flex()
            .w_full()
            .items_start()
            .gap_3()
            .py(px(10.))
            .children(self.lead)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .child(line)
                    .children(self.below),
            )
    }
}
