use std::time::{Duration, Instant};

use lumilio_core::{LaunchFailure, LaunchPhase, LaunchSignal};

use super::{
    BAR_SEGMENTS, HomeIntent, HomePresentation, RecentEntry, RecoveryDetail, Subject, WorldHint,
    filled_segments, play_minutes, recovery_sentence,
};
use crate::hero::{HeroMode, Landmark, Scene};

fn subject() -> Subject {
    Subject {
        title: "我的世界".into(),
        metadata: "1.21 · Fabric".into(),
        world: WorldHint::Nether,
    }
}

fn continuing() -> HomePresentation {
    HomePresentation::Continue {
        subject: subject(),
        recent: vec![RecentEntry {
            id: None,
            title: "空岛".into(),
            metadata: "昨天".into(),
        }],
    }
}

#[test]
fn each_home_state_has_at_most_one_primary_intent() {
    assert_eq!(HomePresentation::Loading.primary_intent(), None);
    assert_eq!(HomePresentation::Ambient.primary_intent(), None);
    assert_eq!(
        HomePresentation::FirstUse.primary_intent(),
        Some(HomeIntent::Import)
    );
    assert_eq!(continuing().primary_intent(), Some(HomeIntent::Continue));
    assert_eq!(continuing().begin_launch().primary_intent(), None);
}

#[test]
fn production_home_starts_without_first_use_actions() {
    assert_eq!(super::initial_presentation(), HomePresentation::Ambient);
    assert!(!HomePresentation::Ambient.has_body());
    assert_eq!(HomePresentation::Ambient.hero_mode(), HeroMode::Showcase);
}

#[test]
fn a_successful_launch_walks_continue_launching_playing_continue() {
    let now = Instant::now();
    assert_eq!(
        continuing().hero_mode(),
        HeroMode::Focus(Scene::Hearth(Landmark::Portal))
    );
    let mut home = continuing().begin_launch();
    assert!(matches!(home, HomePresentation::Launching { .. }));

    for signal in [
        LaunchSignal::Phase(LaunchPhase::Libraries),
        LaunchSignal::Progress { done: 3, total: 9 },
        LaunchSignal::Phase(LaunchPhase::Starting),
    ] {
        home = home.on_launch_signal(signal, now);
    }
    assert!(matches!(
        home.hero_mode(),
        HeroMode::Loading { scene: Scene::Hearth(Landmark::Portal), progress } if progress > 0.8
    ));

    home = home.on_launch_signal(LaunchSignal::Running, now);
    assert!(matches!(home, HomePresentation::Playing { since, .. } if since == now));
    assert!(matches!(
        home.hero_mode(),
        HeroMode::Still { dim: true, .. }
    ));

    home = home.on_launch_signal(LaunchSignal::Exited { code: Some(0) }, now);
    assert_eq!(home, continuing());
}

#[test]
fn failures_land_in_recovery_with_a_plain_reason() {
    let now = Instant::now();
    let failed = continuing()
        .begin_launch()
        .on_launch_signal(LaunchSignal::Phase(LaunchPhase::Assets), now)
        .on_launch_signal(
            LaunchSignal::Failed(LaunchFailure::Step {
                message: "checksum".into(),
            }),
            now,
        );
    let HomePresentation::Recovery { detail, .. } = &failed else {
        panic!("expected recovery, got {failed:?}");
    };
    assert_eq!(recovery_sentence(detail), "在「资源」这一步停了下来。");
    assert!(!recovery_sentence(detail).contains("checksum"));

    let crashed = continuing()
        .begin_launch()
        .on_launch_signal(LaunchSignal::Running, now)
        .on_launch_signal(LaunchSignal::Exited { code: Some(-1) }, now);
    assert!(matches!(
        crashed,
        HomePresentation::Recovery {
            detail: RecoveryDetail::Crashed { code: Some(-1) },
            ..
        }
    ));
    assert!(matches!(
        crashed.begin_launch(),
        HomePresentation::Launching { .. }
    ));
}

#[test]
fn cancelling_returns_to_continue_and_stray_signals_are_ignored() {
    let now = Instant::now();
    assert_eq!(continuing().begin_launch().cancel_launch(), continuing());
    assert_eq!(
        continuing().on_launch_signal(LaunchSignal::Running, now),
        continuing()
    );
    assert_eq!(
        HomePresentation::FirstUse.begin_launch(),
        HomePresentation::FirstUse
    );
}

#[test]
fn enabled_buttons_show_the_pointer_and_disabled_ones_do_not() {
    let handler: super::HomeIntentHandler = std::rc::Rc::new(|_, _, _| {});
    let pointer = |handler| {
        super::base_button(
            "test",
            "继续",
            super::LocalActionIcon::Continue,
            HomeIntent::Continue,
            handler,
        )
        .shows_pointer()
    };
    assert!(pointer(Some(handler)));
    assert!(!pointer(None));
}

#[test]
fn the_bar_only_shows_earned_blocks() {
    assert_eq!(filled_segments(0.), 0);
    assert_eq!(filled_segments(0.999), BAR_SEGMENTS - 1);
    assert_eq!(filled_segments(1.), BAR_SEGMENTS);
    assert_eq!(filled_segments(7.), BAR_SEGMENTS);
}

#[test]
fn play_time_counts_whole_minutes() {
    let start = Instant::now();
    assert_eq!(play_minutes(start, start + Duration::from_secs(59)), 0);
    assert_eq!(play_minutes(start, start + Duration::from_secs(125)), 2);
    assert_eq!(play_minutes(start + Duration::from_secs(5), start), 0);
}
