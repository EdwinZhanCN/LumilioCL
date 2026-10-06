use super::launching::{render_continue, render_launching, render_playing};
use super::lists::{render_attention, render_first_use, render_recent};
use super::recovery::render_recovery;
use super::{HomeIntentHandler, HomeLinks, HomePresentation, HoverHandler, WorldHint};
use crate::key::KeyKind;
use crate::theme::motion;
use gpui::AnimationExt as _;
use gpui::prelude::*;
use gpui::{Animation, AnyElement, Hsla, IntoElement, SharedString, div, ease_out_quint, px};
use gpui_component::StyledExt as _;
use gpui_component::{h_flex, v_flex};

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
/// the game (its worlds, record and recent cards); without it the recent
/// entries are drawn plainly.
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
            .child("正在准备")
            .into_any_element(),
        HomePresentation::FirstUse => render_first_use(intent_handler, colors).into_any_element(),
        HomePresentation::Ambient => div().into_any_element(),
        other => v_flex()
            .gap_6()
            .children(render_attention(
                &links.attention,
                links.on_act.clone(),
                colors,
            ))
            .children(live.or_else(|| {
                (!other.recent().is_empty()).then(|| {
                    render_recent(other.recent(), links.on_open.clone(), colors).into_any_element()
                })
            }))
            .into_any_element(),
    }
}

/// Foreground placed on the world for states that are about one instance.
pub fn render_overlay(
    presentation: &HomePresentation,
    intent_handler: Option<HomeIntentHandler>,
    on_continue_hover: Option<HoverHandler>,
) -> Option<AnyElement> {
    let art = ArtButtons::new();
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

/// The small heading over each group below the world ("需要留意", "最近").
pub fn body_label(text: impl Into<SharedString>, muted: Hsla) -> impl IntoElement {
    div()
        .text_xs()
        .font_semibold()
        .text_color(muted)
        .child(text.into())
}

/// Foreground on world art is always light-on-dark, whatever the theme; the
/// scrim behind it guarantees the contrast.
pub(super) fn on_art() -> Hsla {
    gpui::white()
}

/// Which key a button on the world is. Keys keep their colours over art
/// (design language §12): orange for the one primary, black for the rest.
#[derive(Clone, Copy)]
pub(super) struct ArtStyle(pub(super) KeyKind);

#[derive(Clone, Copy)]
pub(super) struct ArtButtons {
    /// The orange key: the one bright thing on the art.
    pub(super) primary: ArtStyle,
    /// The black key: secondary actions over the world.
    pub(super) glass: ArtStyle,
}

impl ArtButtons {
    pub(super) fn new() -> Self {
        Self {
            primary: ArtStyle(KeyKind::Orange),
            glass: ArtStyle(KeyKind::Black),
        }
    }
}

pub(super) fn eyebrow(label: impl Into<SharedString>, accent: Hsla) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_center()
        .child(div().size(px(8.)).bg(accent))
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(on_art().opacity(0.78))
                .child(label.into()),
        )
}

pub(super) fn headline(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(30.))
        .line_height(px(38.))
        .font_semibold()
        .text_color(on_art())
        .child(text.into())
}

pub(super) fn caption(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(on_art().opacity(0.74))
        .child(text.into())
}

pub(super) fn world_accent(world: WorldHint) -> Hsla {
    crate::hero::accent(world.scene())
}
