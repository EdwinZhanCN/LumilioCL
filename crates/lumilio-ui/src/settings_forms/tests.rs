use super::*;

fn tuned(parsed: Parsed) -> LaunchTuning {
    match parsed.unwrap() {
        LiveIntent::SetLaunchDefaults(tuning) => tuning,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn memory_accepts_blank_ranges_and_refuses_the_rest() {
    assert_eq!(
        memory("", "4096"),
        Ok(LiveIntent::SetMemory {
            min_mb: None,
            max_mb: Some(4096)
        })
    );
    assert_eq!(
        memory(" 512 ", ""),
        Ok(LiveIntent::SetMemory {
            min_mb: Some(512),
            max_mb: None
        })
    );
    assert!(memory("abc", "").unwrap_err().contains("整数"));
    assert!(memory("0", "").unwrap_err().contains("1 到"));
    assert!(memory("", "99999999999").is_err());
    assert!(memory("8192", "4096").unwrap_err().contains("不能超过"));
}

#[test]
fn a_window_needs_both_sides_and_keeps_the_other_settings() {
    let current = LaunchTuning {
        game_arguments: vec!["--demo".into()],
        ..LaunchTuning::default()
    };
    let next = tuned(window(&current, "1280", "720", 1));
    assert_eq!(
        (next.window_width, next.window_height, next.fullscreen),
        (Some(1280), Some(720), Some(true))
    );
    assert_eq!(next.game_arguments, ["--demo"]);
    let cleared = tuned(window(&next, "", "", 2));
    assert_eq!((cleared.window_width, cleared.fullscreen), (None, None));
    assert!(
        window(&current, "1280", "", 2)
            .unwrap_err()
            .contains("同时填宽和高")
    );
    assert!(window(&current, "x", "720", 2).is_err());
}

#[test]
fn argument_and_environment_text_becomes_lines_and_pairs() {
    let current = LaunchTuning::default();
    assert_eq!(
        tuned(jvm_arguments(&current, " -Xss1m \n\n -Da=b ")).jvm_arguments,
        ["-Xss1m", "-Da=b"]
    );
    let env = tuned(environment(&current, "MESA_DEBUG = 1\nA=b=c"));
    assert_eq!(env.environment[0].name, "MESA_DEBUG");
    assert_eq!(env.environment[0].value, "1");
    assert_eq!(env.environment[1].value, "b=c", "only the first = splits");
    assert!(
        environment(&current, "oops")
            .unwrap_err()
            .contains("第 1 行")
    );
    assert!(
        environment(&current, "=x")
            .unwrap_err()
            .contains("环境变量名")
    );
}

#[test]
fn commands_are_trimmed_and_a_bad_wrapper_is_explained() {
    let current = LaunchTuning::default();
    let next = tuned(commands(&current, " echo hi ", "", "  "));
    assert_eq!(next.pre_launch.as_deref(), Some("echo hi"));
    assert_eq!((next.wrapper, next.post_exit), (None, None));
    assert!(
        commands(&current, "", "nice \"unclosed", "")
            .unwrap_err()
            .contains("引号")
    );
}

#[test]
fn concurrency_mirrors_and_folders_parse() {
    assert_eq!(concurrency(""), Ok(LiveIntent::SetConcurrency(None)));
    assert_eq!(concurrency("8"), Ok(LiveIntent::SetConcurrency(Some(8))));
    assert!(concurrency("0").is_err() && concurrency("33").is_err());
    let parsed = mirrors("https://a.example/ => https://m.example/a/\n").unwrap();
    assert_eq!(
        parsed,
        LiveIntent::SetMirrors(vec![MirrorRule {
            official_prefix: "https://a.example/".into(),
            mirror_prefix: "https://m.example/a/".into()
        }])
    );
    assert!(mirrors("no arrow").unwrap_err().contains("第 1 行"));
    assert!(mirrors("a =>  ").unwrap_err().contains("不能为空"));
    assert_eq!(
        java_roots("/opt/a\n /opt/b ").unwrap(),
        LiveIntent::SetJavaRoots(vec!["/opt/a".into(), "/opt/b".into()])
    );
}
