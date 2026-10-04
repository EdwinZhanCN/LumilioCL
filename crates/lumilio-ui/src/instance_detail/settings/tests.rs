use super::super::InstanceIntent;
use super::saves::Saved;
use super::saves::after_launch;
use super::saves::commands;
use super::saves::environment;
use super::saves::game_arguments;
use super::saves::java;
use super::saves::jvm_arguments;
use super::saves::quick_play;
use super::saves::window;
use super::texts::command_text;
use super::texts::followed;
use super::texts::quick_play_text;
use super::texts::window_text;
use lumilio_core::{AfterLaunch, InstanceSettings, QuickPlay};
use std::path::PathBuf;

fn saved(result: Saved) -> InstanceSettings {
    match result.unwrap() {
        InstanceIntent::SaveSettings(settings) => settings,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn rows_say_where_a_value_comes_from() {
    assert_eq!(followed(None, "1280 × 720"), "跟随默认 · 1280 × 720");
    assert_eq!(
        followed(Some("800 × 600".into()), "1280 × 720"),
        "800 × 600"
    );
    assert_eq!(
        window_text(Some(800), Some(600)).as_deref(),
        Some("800 × 600")
    );
    assert_eq!(window_text(Some(800), None), None);
    assert_eq!(quick_play_text(None), "无（进入主菜单）");
    assert_eq!(
        quick_play_text(Some(&QuickPlay::World("My World".into()))),
        "世界 · My World"
    );
    assert_eq!(
        command_text(Some(&String::new()), Some(&"nice".to_owned())),
        "不使用"
    );
    assert_eq!(
        command_text(None, Some(&"nice".to_owned())),
        "跟随默认 · nice"
    );
    assert_eq!(command_text(None, None), "跟随默认 · 无");
}

#[test]
fn the_window_and_fullscreen_are_overrides_that_can_be_cleared() {
    let base = InstanceSettings::default();
    let next = saved(window(&base, "800", "600", 1));
    assert_eq!(
        (
            next.launch.window_width,
            next.launch.window_height,
            next.launch.fullscreen
        ),
        (Some(800), Some(600), Some(true))
    );
    // Clearing removes the override instead of storing the default.
    let back = saved(window(&next, "", "", 0));
    assert!(back.launch.is_default());
    assert!(
        window(&base, "800", "", 0)
            .unwrap_err()
            .contains("同时填宽和高")
    );
    assert!(window(&base, "x", "600", 0).is_err());
    assert_eq!(
        saved(window(&base, "", "", 2)).launch.fullscreen,
        Some(false)
    );
}

#[test]
fn a_direct_start_needs_a_usable_target() {
    let base = InstanceSettings::default();
    assert_eq!(
        saved(quick_play(&base, 1, " My World ")).launch.quick_play,
        Some(QuickPlay::World("My World".into()))
    );
    assert_eq!(
        saved(quick_play(&base, 2, "mc.example.com:25565"))
            .launch
            .quick_play,
        Some(QuickPlay::Server("mc.example.com:25565".into()))
    );
    assert!(
        saved(quick_play(&base, 0, "ignored"))
            .launch
            .quick_play
            .is_none()
    );
    assert!(quick_play(&base, 1, "").unwrap_err().contains("请填写"));
    assert!(quick_play(&base, 1, "../x").is_err());
    assert!(quick_play(&base, 2, "host:99999").is_err());
}

#[test]
fn lists_follow_the_defaults_or_are_set_even_when_empty() {
    let base = InstanceSettings::default();
    assert_eq!(
        saved(game_arguments(&base, 0, "--demo"))
            .launch
            .game_arguments,
        None
    );
    assert_eq!(
        saved(game_arguments(&base, 1, " --demo \n"))
            .launch
            .game_arguments,
        Some(vec!["--demo".to_owned()])
    );
    assert_eq!(
        saved(game_arguments(&base, 1, "")).launch.game_arguments,
        Some(Vec::new()),
        "own and empty means none on purpose"
    );
    let env = saved(environment(&base, 1, "A=1\nB = two"));
    assert_eq!(env.launch.environment.as_ref().unwrap()[1].value, "two");
    assert_eq!(saved(environment(&env, 0, "A=1")).launch.environment, None);
    assert!(
        environment(&base, 1, "oops")
            .unwrap_err()
            .contains("第 1 行")
    );
    assert!(
        environment(&base, 1, "=x")
            .unwrap_err()
            .contains("环境变量名")
    );
    assert_eq!(
        saved(jvm_arguments(&base, "-Da\n\n-Db")).jvm_arguments,
        ["-Da", "-Db"]
    );
    assert!(
        saved(jvm_arguments(&saved(jvm_arguments(&base, "-Da")), ""))
            .jvm_arguments
            .is_empty()
    );
}

#[test]
fn a_command_field_follows_when_blank_and_dash_means_none() {
    let base = InstanceSettings::default();
    let next = saved(commands(&base, "  ", "nice", "-"));
    assert_eq!(next.launch.pre_launch, None);
    assert_eq!(next.launch.wrapper.as_deref(), Some("nice"));
    assert_eq!(next.launch.post_exit.as_deref(), Some(""));
    assert!(
        commands(&base, "", "unclosed \"quote", "")
            .unwrap_err()
            .contains("引号")
    );
}

#[test]
fn java_and_after_launch_can_be_set_and_cleared() {
    let base = InstanceSettings::default();
    let with = saved(java(&base, " /opt/jdk-21 "));
    assert_eq!(with.java_path, Some(PathBuf::from("/opt/jdk-21")));
    assert_eq!(saved(java(&with, "")).java_path, None);
    assert_eq!(
        saved(after_launch(&base, 2)).launch.after_launch,
        Some(AfterLaunch::Hide)
    );
    assert_eq!(
        saved(after_launch(&base, 1)).launch.after_launch,
        Some(AfterLaunch::Keep)
    );
    assert_eq!(saved(after_launch(&base, 0)).launch.after_launch, None);
}
