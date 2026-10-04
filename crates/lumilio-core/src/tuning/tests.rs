use super::*;

fn words(text: &str) -> Vec<String> {
    split_words(text).unwrap()
}

#[test]
fn a_window_needs_both_sides_in_range() {
    let mut tuning = LaunchTuning::default();
    assert_eq!(tuning.validate(), Ok(()));
    tuning.window_width = Some(854);
    assert_eq!(tuning.validate(), Err(TuningError::WindowSize));
    tuning.window_height = Some(480);
    assert_eq!(tuning.validate(), Ok(()));
    for (w, h) in [(0, 480), (854, 0), (MAX_WINDOW_SIDE + 1, 480)] {
        tuning.window_width = Some(w);
        tuning.window_height = Some(h);
        assert_eq!(tuning.validate(), Err(TuningError::WindowSize), "{w}x{h}");
    }
}

#[test]
fn environment_names_and_control_characters_are_refused() {
    let with_env = |name: &str, value: &str| LaunchTuning {
        environment: vec![EnvVar {
            name: name.into(),
            value: value.into(),
        }],
        ..LaunchTuning::default()
    };
    assert_eq!(with_env("MESA_DEBUG", "1").validate(), Ok(()));
    for bad in ["", "A=B", "has space", "tab\t"] {
        assert_eq!(
            with_env(bad, "x").validate(),
            Err(TuningError::EnvironmentName(bad.into()))
        );
    }
    assert_eq!(
        with_env("OK", "a\0b").validate(),
        Err(TuningError::ControlCharacter)
    );
    let args = LaunchTuning {
        game_arguments: vec!["--a\nb".into()],
        ..LaunchTuning::default()
    };
    assert_eq!(args.validate(), Err(TuningError::ControlCharacter));
}

#[test]
fn the_wrapper_must_parse_into_a_program() {
    let wrapper = |text: &str| LaunchTuning {
        wrapper: Some(text.into()),
        ..LaunchTuning::default()
    };
    assert_eq!(wrapper("gamemoderun").validate(), Ok(()));
    assert_eq!(wrapper("env \"A B\" nice").validate(), Ok(()));
    assert_eq!(
        wrapper("unclosed \"quote").validate(),
        Err(TuningError::Wrapper)
    );
    assert_eq!(wrapper("   ").validate(), Err(TuningError::Wrapper));
}

#[test]
fn words_follow_shell_quoting_for_plain_cases() {
    assert_eq!(words("a b  c"), ["a", "b", "c"]);
    assert_eq!(
        words(r#"nice -n 5 "my game" 'it''s'"#),
        ["nice", "-n", "5", "my game", "its"]
    );
    assert_eq!(words(r"a\ b c"), ["a b", "c"]);
    assert_eq!(words(r#"say "he said \"hi\"""#), ["say", r#"he said "hi""#]);
    assert_eq!(words(r#"empty "" end"#), ["empty", "", "end"]);
    assert_eq!(split_words("open 'quote"), None);
    assert_eq!(split_words("trailing\\"), None);
    assert!(words("   ").is_empty());
}

#[test]
fn an_instance_replaces_jvm_arguments_only_when_it_has_some() {
    let defaults = LaunchTuning {
        jvm_arguments: vec!["-XX:+UseG1GC".into()],
        game_arguments: vec!["--demo".into()],
        ..LaunchTuning::default()
    };
    let none = InstanceLaunch::default();
    assert_eq!(
        defaults.for_instance(&[], &none).jvm_arguments,
        ["-XX:+UseG1GC"]
    );
    let own = defaults.for_instance(&["-Dx=1".to_owned()], &none);
    assert_eq!(own.jvm_arguments, ["-Dx=1"]);
    assert_eq!(own.game_arguments, ["--demo"]);
}

#[test]
fn normalizing_gives_nothing_one_spelling() {
    let tuning = LaunchTuning {
        pre_launch: Some("  ".into()),
        wrapper: Some(" nice ".into()),
        jvm_arguments: vec![" -Da ".into(), "".into(), "   ".into()],
        ..LaunchTuning::default()
    }
    .normalized();
    assert_eq!(tuning.pre_launch, None);
    assert_eq!(tuning.wrapper.as_deref(), Some("nice"));
    assert_eq!(tuning.jvm_arguments, ["-Da"]);
}

#[test]
fn a_missing_foreground_preference_means_come_back() {
    assert!(Preferences::default().foreground_on_exit());
    let off = Preferences {
        foreground_on_exit: Some(false),
        ..Preferences::default()
    };
    assert!(!off.foreground_on_exit());
}

#[test]
fn overrides_replace_only_what_they_set_and_empty_means_none_on_purpose() {
    let defaults = LaunchTuning {
        window_width: Some(1280),
        window_height: Some(720),
        fullscreen: Some(false),
        game_arguments: vec!["--demo".into()],
        environment: vec![EnvVar {
            name: "A".into(),
            value: "1".into(),
        }],
        pre_launch: Some("prep".into()),
        wrapper: Some("nice".into()),
        post_exit: Some("clean".into()),
        ..LaunchTuning::default()
    };
    // Nothing set: everything follows.
    assert_eq!(InstanceLaunch::default().apply(defaults.clone()), defaults);
    let own = InstanceLaunch {
        window_width: Some(800),
        window_height: Some(600),
        fullscreen: Some(true),
        game_arguments: Some(Vec::new()),
        pre_launch: Some(String::new()),
        wrapper: Some("gamemoderun".into()),
        ..InstanceLaunch::default()
    };
    let effective = own.apply(defaults.clone());
    assert_eq!(
        (
            effective.window_width,
            effective.window_height,
            effective.fullscreen
        ),
        (Some(800), Some(600), Some(true))
    );
    assert!(effective.game_arguments.is_empty(), "none on purpose");
    assert_eq!(effective.environment, defaults.environment, "still follows");
    assert_eq!(
        effective.pre_launch, None,
        "an empty command switches it off"
    );
    assert_eq!(effective.wrapper.as_deref(), Some("gamemoderun"));
    assert_eq!(effective.post_exit.as_deref(), Some("clean"));
}

#[test]
fn overrides_are_validated_like_defaults() {
    assert_eq!(InstanceLaunch::default().validate(), Ok(()));
    let bad_window = InstanceLaunch {
        window_width: Some(800),
        ..InstanceLaunch::default()
    };
    assert_eq!(bad_window.validate(), Err(TuningError::WindowSize));
    let empty_wrapper = InstanceLaunch {
        wrapper: Some(String::new()),
        ..InstanceLaunch::default()
    };
    assert_eq!(empty_wrapper.validate(), Ok(()), "off on purpose is fine");
    let bad_target = InstanceLaunch {
        quick_play: Some(QuickPlay::World("../escape".into())),
        ..InstanceLaunch::default()
    };
    assert_eq!(bad_target.validate(), Err(TuningError::QuickPlay));
    let kept = InstanceLaunch {
        pre_launch: Some("  ".into()),
        ..InstanceLaunch::default()
    }
    .normalized();
    assert_eq!(
        kept.pre_launch.as_deref(),
        Some(""),
        "blank stays 'none on purpose'"
    );
    assert!(!kept.is_default());
}

#[test]
fn old_versions_are_known_not_to_start_in_a_world() {
    for old in ["1.8.9", "1.12.2", "1.16.5", "1.19.4", "1.19"] {
        assert!(quick_play_world_unsupported(old), "{old}");
    }
    for new in ["1.20", "1.20.1", "1.21.1", "26.3", "23w14a", "weird", ""] {
        assert!(!quick_play_world_unsupported(new), "{new}");
    }
}

#[test]
fn quick_play_targets_are_checked() {
    let world = |name: &str| quick_play_problem(&QuickPlay::World(name.into()));
    let server = |address: &str| quick_play_problem(&QuickPlay::Server(address.into()));
    assert_eq!(world("New World"), None);
    for bad in ["", "..", "a/b", "a\\b", "x\ny"] {
        assert!(world(bad).is_some(), "{bad:?}");
    }
    assert_eq!(server("mc.example.com"), None);
    assert_eq!(server("mc.example.com:25565"), None);
    assert_eq!(server("127.0.0.1:1"), None);
    for bad in [
        "",
        ":25565",
        "has space",
        "host:0",
        "host:99999",
        "host:abc",
        "a/b",
    ] {
        assert!(server(bad).is_some(), "{bad:?}");
    }
}
