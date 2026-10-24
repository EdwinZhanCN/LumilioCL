use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt as _, AnyElement, Hsla, IntoElement, SharedString, div, ease_out_quint,
    px,
};
use gpui_component::{StyledExt as _, h_flex, v_flex};

use super::launching::{render_continue, render_launching, render_playing};
use super::lists::{render_attention, render_first_use};
use super::recovery::render_recovery;
use super::{HomeIntentHandler, HomeLinks, HomePresentation, HoverHandler, WorldHint};
use crate::key::KeyKind;
use crate::theme::motion;

/// Colors passed from the shell keep this module's copy and layout independent
/// from gpui-component's global theme shape.
#[derive(Clone, Copy)]
pub struct ShellHomeColors {
    pub foreground: gpui::Hsla,
    pub muted: gpui::Hsla,
    pub border: gpui::Hsla,
    pub surface: gpui::Hsla,
    pub primary: gpui::Hsla,
    pub primary_foreground: gpui::Hsla,
    pub focus: gpui::Hsla,
    pub danger: gpui::Hsla,
}

/// Page content below the world. Uses theme colours only, so it is correct
/// in light and dark appearance alike. `live` is what the library knows about
/// the game (its worlds and record).
pub fn render_body(
    presentation: &HomePresentation,
    intent_handler: Option<HomeIntentHandler>,
    links: &HomeLinks,
    colors: ShellHomeColors,
    live: Option<AnyElement>,
) -> AnyElement {
    match presentation {
        HomePresentation::Loading => div()
            .text_sm()
            .text_color(colors.muted)
            .child(crate::tr!("home-loading"))
            .into_any_element(),
        HomePresentation::FirstUse => render_first_use(intent_handler, colors).into_any_element(),
        HomePresentation::Ambient => div().into_any_element(),
        _ => v_flex()
            .gap_6()
            .children(render_attention(links, colors))
            .children(live)
            .into_any_element(),
    }
}

/// Foreground placed on the world for states that are about one instance.
/// `text` is [`on_art`] over the world, or the body's ink without one.
pub fn render_overlay(
    presentation: &HomePresentation,
    intent_handler: Option<HomeIntentHandler>,
    on_continue_hover: Option<HoverHandler>,
    text: Hsla,
) -> Option<AnyElement> {
    let art = ArtButtons::new(text);
    let (key, content): (usize, _) = match presentation {
        HomePresentation::Continue { subject, .. } => (
            0,
            render_continue(subject, intent_handler, on_continue_hover, art),
        ),
        HomePresentation::Launching {
            subject, session, ..
        } => (1, render_launching(subject, session, intent_handler, art)),
        HomePresentation::Playing { subject, since, .. } => {
            (2, render_playing(subject, *since, intent_handler, art))
        }
        HomePresentation::Recovery {
            subject, detail, ..
        } => (3, render_recovery(subject, detail, intent_handler, art)),
        _ => return None,
    };
    Some(
        v_flex()
            // Children keep their own width: nothing on the world stretches.
            .items_start()
            .child(content)
            .with_animation(
                ("home-overlay", key),
                Animation::new(motion::SCENE).with_easing(ease_out_quint()),
                |overlay, delta| overlay.opacity(delta).mt(motion::LIFT * (1. - delta)),
            )
            .into_any_element(),
    )
}

/// The small heading over each group below the world ("需要留意", "接着玩").
pub fn body_label(text: impl Into<SharedString>, muted: Hsla) -> impl IntoElement {
    div()
        .text_xs()
        .font_semibold()
        .text_color(muted)
        .child(text.into())
}

/// Foreground on world art is always light-on-dark, whatever the theme; the
/// scrim behind it guarantees the contrast. Without a wallpaper the text sits
/// on the body and takes its ink instead.
#[must_use]
pub fn on_art() -> Hsla {
    gpui::white()
}

/// Which key a button on the world is. Keys keep their colours over art
/// (design language §12): orange for the one primary, black for the rest.
#[derive(Clone, Copy)]
pub(super) struct ArtStyle(pub(super) KeyKind);

#[derive(Clone, Copy)]
pub(super) struct ArtButtons {
    /// The colour of words over the world, or over the body without one.
    pub(super) text: Hsla,
    /// The orange key: the one bright thing on the art.
    pub(super) primary: ArtStyle,
    /// The black key: secondary actions over the world.
    pub(super) glass: ArtStyle,
}

impl ArtButtons {
    pub(super) fn new(text: Hsla) -> Self {
        Self {
            text,
            primary: ArtStyle(KeyKind::Orange),
            glass: ArtStyle(KeyKind::Black),
        }
    }
}

pub(super) fn eyebrow(
    art: ArtButtons,
    label: impl Into<SharedString>,
    accent: Hsla,
) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_center()
        .child(div().size(px(8.)).bg(accent))
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(art.text.opacity(0.78))
                .child(label.into()),
        )
}

pub(super) fn headline(art: ArtButtons, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(crate::theme::font_px(30.))
        .line_height(px(38.))
        .font_semibold()
        .text_color(art.text)
        .child(text.into())
}

pub(super) fn caption(art: ArtButtons, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(art.text.opacity(0.74))
        .child(text.into())
}

pub(super) fn world_accent(world: WorldHint) -> Hsla {
    crate::hero::accent(world.scene())
}
