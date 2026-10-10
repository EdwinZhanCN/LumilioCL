use super::buttons::{LocalActionIcon, art_button};
use super::render::{ArtButtons, caption, eyebrow, headline, on_art, world_accent};
use super::{HomeIntent, HomeIntentHandler, HoverHandler, Subject};
use crate::theme::motion;
use gpui::AnimationExt as _;
use gpui::prelude::*;
use gpui::{Animation, AnyElement, IntoElement, div, ease_out_quint, px};
use gpui_component::Sizable as _;
use gpui_component::StyledExt as _;
use gpui_component::{h_flex, v_flex};
use lumilio_core::{LaunchPhase, LaunchSession};
use std::time::Instant;

pub(super) fn render_continue(
    subject: &Subject,
    intent_handler: Option<HomeIntentHandler>,
    on_hover: Option<HoverHandler>,
    art: ArtButtons,
) -> AnyElement {
    // ia[home]: 继续 | 英雄区主按钮「继续」 | 启动当前游戏，英雄区进入启动时刻；跟随当前游戏（导航右下角芯片）
    let button = art_button(
        "home-continue",
        crate::tr!("home-continue"),
        LocalActionIcon::Continue,
        HomeIntent::Continue,
        art.primary,
        intent_handler,
    )
    .large();
    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow(
            crate::tr!("home-continue-eyebrow"),
            world_accent(subject.world),
        ))
        .child(headline(subject.title.clone()))
        .child(caption(subject.metadata.clone()))
        .child(
            div()
                .id("home-continue-hover")
                .debug_selector(|| "home-continue".into())
                .mt(px(12.))
                .when_some(on_hover, |target, on_hover| {
                    target.on_hover(move |hovered, window, cx| on_hover(*hovered, window, cx))
                })
                .child(button),
        )
        .into_any_element()
}

/// People-facing names of the launch phases, in order.
pub fn phase_label(phase: LaunchPhase) -> &'static str {
    match phase {
        LaunchPhase::Verifying => crate::tr!("home-phase-verifying"),
        LaunchPhase::Libraries => crate::tr!("home-phase-libraries"),
        LaunchPhase::Assets => crate::tr!("home-phase-assets"),
        LaunchPhase::Starting => crate::tr!("home-phase-starting"),
    }
}

pub fn phase_headline(phase: LaunchPhase) -> &'static str {
    match phase {
        LaunchPhase::Verifying => crate::tr!("home-phase-verifying-headline"),
        LaunchPhase::Libraries => crate::tr!("home-phase-libraries-headline"),
        LaunchPhase::Assets => crate::tr!("home-phase-assets-headline"),
        LaunchPhase::Starting => crate::tr!("home-phase-starting-headline"),
    }
}

/// Segments in the launch bar; each is a block that fills discretely.
pub const BAR_SEGMENTS: usize = 24;
pub(super) const SEGMENT: f32 = 14.;
pub(super) const SEGMENT_GAP: f32 = 3.;
/// Side padding inside the launch display.
pub(super) const DISPLAY_PAD: f32 = 12.;
/// Room for the item counts beside the bar.
pub(super) const COUNTS_WIDTH: f32 = 90.;
/// Width of the Continue button the bar grows out of.
pub(super) const MORPH_FROM: f32 = 116.;

/// Filled segments for a progress fraction: never more than earned.
pub fn filled_segments(fraction: f32) -> usize {
    ((fraction.clamp(0., 1.) * BAR_SEGMENTS as f32).floor() as usize).min(BAR_SEGMENTS)
}

pub(super) fn render_launching(
    subject: &Subject,
    session: &LaunchSession,
    intent_handler: Option<HomeIntentHandler>,
    art: ArtButtons,
) -> AnyElement {
    let phase = session.phase().unwrap_or(LaunchPhase::Starting);
    let filled = filled_segments(session.fraction());
    let bar_width = BAR_SEGMENTS as f32 * (SEGMENT + SEGMENT_GAP) - SEGMENT_GAP;
    let full_width = bar_width + 12. + COUNTS_WIDTH + DISPLAY_PAD * 2.;
    // Art is always dark, so its display is the night body's.
    let night = crate::theme::Body::of(true);
    let track = night.display_dim;
    let fill_color = night.display_ink;

    let segments = (0..BAR_SEGMENTS).map(|index| {
        let block = div().w(px(SEGMENT)).h(px(10.)).rounded(px(1.));
        if index < filled {
            // A block that just filled flashes white once, then takes the
            // world's colour.
            block
                .bg(fill_color)
                .with_animation(
                    ("launch-segment-filled", index),
                    Animation::new(motion::SETTLE).with_easing(ease_out_quint()),
                    move |block, delta| block.bg(gpui::white().blend(fill_color.opacity(delta))),
                )
                .into_any_element()
        } else if index == filled {
            // The block being worked on blinks in two hard steps.
            block
                .bg(track)
                .with_animation(
                    ("launch-segment-next", index),
                    Animation::new(motion::SCENE).repeat(),
                    move |block, delta| {
                        block.bg(if delta < 0.5 {
                            fill_color.opacity(0.5)
                        } else {
                            track
                        })
                    },
                )
                .into_any_element()
        } else {
            block.bg(track).into_any_element()
        }
    });

    let counts = session
        .items()
        .map(|(done, total)| format!("{done} / {total}"))
        .unwrap_or_default();

    let bar = h_flex()
        .flex_none()
        .gap(px(12.))
        .items_center()
        .px(px(DISPLAY_PAD))
        .child(h_flex().gap(px(SEGMENT_GAP)).children(segments))
        .child(
            div()
                .font_family(crate::theme::mono_font())
                .text_xs()
                .text_color(night.display_label)
                .child(counts),
        );

    // The orange Continue key stretches into a black display: a clipped window
    // grows from the key's width while its face darkens to the display's.
    let morph = div()
        .h(px(40.))
        .flex()
        .items_center()
        .overflow_hidden()
        .rounded(px(6.))
        .shadow(crate::theme::display_shadow())
        .child(bar)
        .with_animation(
            "launch-morph",
            Animation::new(motion::SETTLE).with_easing(ease_out_quint()),
            move |morph, delta| {
                morph
                    .w(px(MORPH_FROM + (full_width - MORPH_FROM) * delta))
                    .bg(night.orange.blend(night.display.opacity(delta.min(1.))))
            },
        );

    let steps = LaunchPhase::ORDER.map(|step| {
        let label = div().text_xs();
        if step < phase {
            label
                .text_color(on_art().opacity(0.62))
                .child(format!("✓ {}", phase_label(step)))
        } else if step == phase {
            label
                .font_semibold()
                .text_color(on_art())
                .child(phase_label(step))
        } else {
            label
                .text_color(on_art().opacity(0.36))
                .child(phase_label(step))
        }
    });

    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow(
            crate::tr!("home-entering", title = subject.title.to_string()),
            world_accent(subject.world),
        ))
        .child(headline(phase_headline(phase)))
        .child(div().mt(px(8.)).child(morph))
        .child(
            h_flex()
                .mt(px(4.))
                .gap(px(18.))
                .items_center()
                .child(h_flex().gap(px(14.)).children(steps))
                .child(
                    // ia[home]: 取消启动 | 启动时刻里的「取消」 | 停止本次启动，回到继续状态
                    art_button(
                        "home-cancel-launch",
                        crate::tr!("common-cancel"),
                        LocalActionIcon::Cancel,
                        HomeIntent::CancelLaunch,
                        art.glass,
                        intent_handler,
                    )
                    .small(),
                ),
        )
        .into_any_element()
}

/// Whole minutes of play, for the calm playing readout.
pub fn play_minutes(since: Instant, now: Instant) -> u64 {
    now.saturating_duration_since(since).as_secs() / 60
}

pub(super) fn render_playing(
    subject: &Subject,
    since: Instant,
    intent_handler: Option<HomeIntentHandler>,
    art: ArtButtons,
) -> AnyElement {
    let minutes = play_minutes(since, Instant::now());
    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow(
            crate::tr!("home-playing-eyebrow"),
            world_accent(subject.world),
        ))
        .child(headline(subject.title.clone()))
        .child(caption(crate::tr!(
            "home-playing-caption",
            minutes = minutes
        )))
        .child(
            div()
                .mt(px(12.))
                .debug_selector(|| "home-stop-game".into())
                // ia[home]: 结束游戏 | 游戏运行中的「结束游戏」 | 停止游戏进程
                .child(art_button(
                    "home-stop-game",
                    crate::tr!("home-stop-game"),
                    LocalActionIcon::Stop,
                    HomeIntent::StopGame,
                    art.glass,
                    intent_handler,
                )),
        )
        .into_any_element()
}
