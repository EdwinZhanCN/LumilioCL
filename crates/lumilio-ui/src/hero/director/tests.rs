use super::{Director, HeroMode, UNLOAD_SECONDS};
use crate::hero::scenes::Scene;

fn run(director: &mut Director, seconds: f32) {
    for _ in 0..(seconds / 0.05).round() as usize {
        director.tick(0.05, false);
    }
}

#[test]
fn a_launch_unloads_the_world_then_reloads_it_with_progress() {
    let mut director = Director::new();
    director.set_mode(HeroMode::Focus(Scene::Portal), false);
    run(&mut director, 2.);
    let age_before = director.frame(false).age;

    director.set_mode(
        HeroMode::Loading {
            scene: Scene::Portal,
            progress: 0.,
        },
        false,
    );
    assert!(
        director.frame(false).age >= age_before,
        "scene keeps running"
    );
    run(&mut director, UNLOAD_SECONDS / 2.);
    assert!(director.reveal() < 1. && director.reveal() > 0.);
    run(&mut director, UNLOAD_SECONDS);
    assert_eq!(director.reveal(), 0.);

    director.set_mode(
        HeroMode::Loading {
            scene: Scene::Portal,
            progress: 0.6,
        },
        false,
    );
    let mut last = director.reveal();
    for _ in 0..80 {
        director.tick(0.05, false);
        assert!(director.reveal() >= last, "reveal never retreats");
        assert!(director.reveal() <= 0.6, "reveal never passes progress");
        last = director.reveal();
    }
    assert!(last > 0.55);
}

#[test]
fn progress_updates_do_not_restart_the_unload() {
    let mut director = Director::new();
    director.set_mode(
        HeroMode::Loading {
            scene: Scene::Dawn,
            progress: 0.1,
        },
        false,
    );
    run(&mut director, 1.);
    let revealed = director.reveal();
    assert!(revealed > 0.);
    director.set_mode(
        HeroMode::Loading {
            scene: Scene::Dawn,
            progress: 0.2,
        },
        false,
    );
    director.tick(0.05, false);
    assert!(director.reveal() >= revealed);
}

#[test]
fn a_still_frame_stops_asking_for_frames_once_settled() {
    let mut director = Director::new();
    director.set_mode(
        HeroMode::Still {
            scene: Scene::Caves,
            age: 9.,
            dim: true,
        },
        false,
    );
    assert!(director.animating(), "the chunk transition plays first");
    run(&mut director, 2.);
    assert!(!director.animating());
    let frame = director.frame(false);
    assert!(frame.dim && frame.leaving.is_none() && frame.reveal == 1.);
}

#[test]
fn a_focused_scene_replays_through_a_chunk_reload() {
    let mut director = Director::new();
    director.set_mode(HeroMode::Focus(Scene::Caves), false);
    run(&mut director, super::FOCUS_DWELL + 0.2);
    let frame = director.frame(false);
    assert_eq!(frame.leaving.map(|l| l.0), Some(Scene::Caves));
    assert!(frame.age < 0.5);
}

#[test]
fn the_fire_flares_only_while_focused_and_calms_when_the_button_goes() {
    let hearth = Scene::Hearth(Default::default());
    let mut director = Director::new();
    assert!(!director.set_flare(true, false), "no flare in the showcase");
    director.set_mode(HeroMode::Focus(hearth), false);
    assert!(director.set_flare(true, false));
    run(&mut director, 0.6);
    assert!(director.frame(false).flare > 0.9);

    director.set_mode(
        HeroMode::Loading {
            scene: hearth,
            progress: 0.,
        },
        false,
    );
    run(&mut director, 3.);
    assert_eq!(director.frame(false).flare, 0.);
}

#[test]
fn switching_scene_streams_the_new_one_in() {
    let mut director = Director::new();
    director.set_mode(HeroMode::Focus(Scene::Redstone), false);
    let frame = director.frame(false);
    assert_eq!(frame.scene, Scene::Redstone);
    assert_eq!(frame.leaving.map(|l| l.0), Some(Scene::Dawn));
}

#[test]
fn reduced_motion_jumps_to_composed_frames() {
    let mut director = Director::new();
    director.set_mode(
        HeroMode::Loading {
            scene: Scene::Caves,
            progress: 0.4,
        },
        true,
    );
    let frame = director.frame(true);
    assert_eq!(frame.reveal, 0.4);
    assert_eq!(frame.age, Scene::Caves.still_age());
    assert!(frame.leaving.is_none());
}

#[test]
fn huds_describe_the_mode() {
    let mut director = Director::new();
    assert!(director.hud((150, 60)).is_some());
    director.set_mode(
        HeroMode::Loading {
            scene: Scene::Dawn,
            progress: 1.,
        },
        true,
    );
    assert_eq!(director.hud((160, 64)).as_deref(), Some("区块 40/40"));
    director.set_mode(
        HeroMode::Still {
            scene: Scene::Dawn,
            age: 1.,
            dim: false,
        },
        true,
    );
    assert_eq!(director.hud((160, 64)), None);
}
