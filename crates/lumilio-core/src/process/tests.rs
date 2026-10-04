use super::*;
use crate::environment::HostProfile;
use crate::launch::{LaunchContext, LaunchDirectories};
use crate::release::ReleaseManifest;

fn plan() -> LaunchPlan {
    let manifest = ReleaseManifest::decode_json(
        r#"{
            "id": "1.0",
            "mainClass": "net.example.Main",
            "minecraftArguments": "--username ${auth_player_name}",
            "libraries": []
        }"#,
    )
    .unwrap();
    let context = LaunchContext::new(
        HostProfile::current(),
        LaunchDirectories::under("/tmp/launcher"),
    )
    .with_value("auth_player_name", "Steve");
    manifest.build_launch_plan(&context).unwrap()
}

fn position(line: &[String], needle: &str) -> usize {
    line.iter().position(|item| item == needle).unwrap()
}

#[test]
fn command_line_orders_java_memory_user_plan_main_and_game() {
    let plan = plan();
    let options = GameOptions::new("/jdk/bin/java")
        .with_memory(Some(512), Some(2048))
        .with_jvm_arguments(["-Dcustom=1".to_owned()]);
    let line = command_line(&plan, &options);
    assert_eq!(line[0], "/jdk/bin/java");
    assert_eq!(line[1], "-Xmx2048m");
    assert_eq!(line[2], "-Xms512m");
    assert!(position(&line, "-Dcustom=1") < position(&line, "-cp"));
    assert!(position(&line, "-cp") < position(&line, "net.example.Main"));
    let main = position(&line, "net.example.Main");
    assert_eq!(line[main + 1..], ["--username", "Steve"]);
    assert!(line.contains(&"-Dlog4j2.formatMsgNoLookups=true".to_owned()));
}

#[test]
fn user_arguments_override_launcher_defaults() {
    let plan = plan();
    let options = GameOptions::new("java")
        .with_memory(Some(512), Some(2048))
        .with_jvm_arguments([
            "-Xmx8g".to_owned(),
            "-Xms1g".to_owned(),
            "-Dlog4j2.formatMsgNoLookups=false".to_owned(),
        ]);
    let line = command_line(&plan, &options);
    assert_eq!(line.iter().filter(|a| a.starts_with("-Xmx")).count(), 1);
    assert_eq!(line.iter().filter(|a| a.starts_with("-Xms")).count(), 1);
    assert!(line.contains(&"-Xmx8g".to_owned()));
    assert!(!line.contains(&"-Dlog4j2.formatMsgNoLookups=true".to_owned()));
}

#[test]
fn a_minimum_above_the_maximum_is_dropped() {
    let options = GameOptions::new("java").with_memory(Some(4096), Some(1024));
    let line = command_line(&plan(), &options);
    assert!(line.contains(&"-Xmx1024m".to_owned()));
    assert!(!line.iter().any(|a| a.starts_with("-Xms")));
    let zero = GameOptions::new("java").with_memory(Some(0), Some(0));
    let line = command_line(&plan(), &zero);
    assert!(!line.iter().any(|a| a.starts_with("-Xm")));
}

#[test]
fn events_map_to_launch_signals() {
    assert_eq!(
        GameEvent::Running.signal(false),
        Some(LaunchSignal::Running)
    );
    let exit = GameEvent::Exited {
        code: Some(1),
        ran_for: Duration::ZERO,
    };
    assert_eq!(
        exit.signal(true),
        Some(LaunchSignal::Exited { code: Some(1) })
    );
    assert_eq!(
        exit.signal(false),
        Some(LaunchSignal::Failed(LaunchFailure::ExitedEarly {
            code: Some(1)
        }))
    );
    assert_eq!(GameEvent::Spawned { pid: None }.signal(false), None);
}

#[cfg(unix)]
mod supervised {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, body: &str) -> Vec<String> {
        let path = dir.join("fake-java.sh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        vec![path.to_string_lossy().into_owned()]
    }

    async fn collect(mut rx: mpsc::UnboundedReceiver<GameEvent>) -> Vec<GameEvent> {
        let mut all = Vec::new();
        while let Some(event) = rx.recv().await {
            all.push(event);
        }
        all
    }

    #[tokio::test]
    async fn reports_start_marker_output_and_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let line = script(
            dir.path(),
            "echo 'Setting user: Steve'\necho 'oops' >&2\nexit 3",
        );
        let (tx, rx) = mpsc::unbounded_channel();
        let exit = run(
            &line,
            &dir.path().join("game"),
            tx,
            Duration::from_secs(30),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(exit.code, Some(3));
        assert!(exit.was_running && !exit.killed);

        let events = collect(rx).await;
        assert!(matches!(events[0], GameEvent::Spawned { pid: Some(_) }));
        let running = events
            .iter()
            .position(|e| *e == GameEvent::Running)
            .unwrap();
        let marker = events
            .iter()
            .position(
                |e| matches!(e, GameEvent::Line { text, .. } if text.contains("Setting user")),
            )
            .unwrap();
        assert!(
            running < marker,
            "Running is announced with the marker line"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            GameEvent::Line { stream: LogStream::Stderr, text } if text == "oops"
        )));
        assert!(matches!(
            events.last(),
            Some(GameEvent::Exited { code: Some(3), .. })
        ));
    }

    /// Output that is only collected after the process ended (the reader
    /// tasks can lag behind the exit under load) must still be able to mark
    /// the game as started. A background subshell prints the marker after
    /// its parent is already gone, which forces exactly that path.
    #[tokio::test]
    async fn a_marker_collected_after_the_exit_still_counts_as_running() {
        let dir = tempfile::tempdir().unwrap();
        let line = script(
            dir.path(),
            "(sleep 0.2; echo 'Setting user: Steve') &\nexit 0",
        );
        let (tx, rx) = mpsc::unbounded_channel();
        let exit = run(
            &line,
            dir.path(),
            tx,
            Duration::from_secs(30),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(exit.was_running);
        let events = collect(rx).await;
        let running = events
            .iter()
            .position(|e| *e == GameEvent::Running)
            .unwrap();
        let exited = events
            .iter()
            .position(|e| matches!(e, GameEvent::Exited { .. }))
            .unwrap();
        assert!(running < exited, "Running precedes the exit event");
    }

    #[tokio::test]
    async fn an_early_exit_never_counts_as_running() {
        let dir = tempfile::tempdir().unwrap();
        let line = script(dir.path(), "echo boom >&2\nexit 1");
        let (tx, rx) = mpsc::unbounded_channel();
        let exit = run(
            &line,
            dir.path(),
            tx,
            Duration::from_secs(30),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(!exit.was_running);
        let events = collect(rx).await;
        assert!(!events.contains(&GameEvent::Running));
        let last = events.last().unwrap();
        assert_eq!(
            last.signal(exit.was_running),
            Some(LaunchSignal::Failed(LaunchFailure::ExitedEarly {
                code: Some(1)
            }))
        );
    }

    #[tokio::test]
    async fn surviving_the_settle_time_counts_as_running_and_cancel_kills() {
        let dir = tempfile::tempdir().unwrap();
        let line = script(dir.path(), "sleep 30");
        let (tx, rx) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let stopper = cancel.clone();
        let canceller = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            stopper.cancel();
        });
        let exit = run(&line, dir.path(), tx, Duration::from_millis(100), &cancel)
            .await
            .unwrap();
        canceller.await.unwrap();
        assert!(exit.killed && exit.was_running);
        assert!(exit.ran_for < Duration::from_secs(10));
        assert!(collect(rx).await.contains(&GameEvent::Running));
    }

    #[tokio::test]
    async fn a_missing_program_is_a_spawn_error() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let error = run(
            &["/definitely/not/here".to_owned()],
            dir.path(),
            tx,
            Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, ProcessError::Spawn { .. }));
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(matches!(
            run(
                &[],
                dir.path(),
                tx,
                Duration::ZERO,
                &CancellationToken::new()
            )
            .await
            .unwrap_err(),
            ProcessError::EmptyCommand
        ));
    }
}

#[test]
fn tuning_puts_the_wrapper_first_and_the_window_and_user_game_arguments_last() {
    let tuning = LaunchTuning {
        window_width: Some(1280),
        window_height: Some(720),
        fullscreen: Some(true),
        game_arguments: vec!["--demo".into()],
        wrapper: Some("nice -n 5".into()),
        ..LaunchTuning::default()
    };
    let line = command_line(
        &plan(),
        &GameOptions::new("/jdk/bin/java").with_tuning(&tuning),
    );
    assert_eq!(&line[..4], ["nice", "-n", "5", "/jdk/bin/java"]);
    let tail = &line[line.len() - 6..];
    assert_eq!(
        tail,
        [
            "--width",
            "1280",
            "--height",
            "720",
            "--fullscreen",
            "--demo"
        ]
    );
    assert!(position(&line, "--width") > position(&line, "--username"));
    assert!(position(&line, "--demo") > position(&line, "--fullscreen"));
}

#[test]
fn a_window_flag_the_user_already_gave_is_not_added_twice() {
    let tuning = LaunchTuning {
        window_width: Some(1280),
        window_height: Some(720),
        game_arguments: vec!["--width".into(), "640".into()],
        ..LaunchTuning::default()
    };
    let line = command_line(&plan(), &GameOptions::new("java").with_tuning(&tuning));
    assert_eq!(line.iter().filter(|item| *item == "--width").count(), 1);
    assert_eq!(line.iter().filter(|item| *item == "--height").count(), 1);
    assert!(!line.contains(&"1280".to_owned()), "the user's width wins");
    // Off or unset fullscreen adds nothing.
    let off = LaunchTuning {
        fullscreen: Some(false),
        ..LaunchTuning::default()
    };
    let line = command_line(&plan(), &GameOptions::new("java").with_tuning(&off));
    assert!(!line.contains(&"--fullscreen".to_owned()));
}

#[tokio::test]
async fn a_hook_reports_its_exit_code_output_and_stops_on_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let never = CancellationToken::new();
    let outcome = run_hook(
        "echo out; echo err 1>&2; echo $GREETING; exit 4",
        dir.path(),
        &[("GREETING".to_owned(), "hi".to_owned())],
        None,
        &never,
    )
    .await
    .unwrap();
    assert_eq!(outcome.code, Some(4));
    assert!(!outcome.succeeded());
    let texts: Vec<&str> = outcome
        .lines
        .iter()
        .map(|(_, text)| text.as_str())
        .collect();
    assert!(texts.contains(&"out") && texts.contains(&"err") && texts.contains(&"hi"));

    let ok = run_hook("true", dir.path(), &[], None, &never)
        .await
        .unwrap();
    assert!(ok.succeeded());

    let slow = run_hook(
        "sleep 30",
        dir.path(),
        &[],
        Some(Duration::from_millis(100)),
        &never,
    )
    .await
    .unwrap();
    assert!(slow.stopped && !slow.succeeded());
    let cancel = CancellationToken::new();
    cancel.cancel();
    let stopped = run_hook("sleep 30", dir.path(), &[], None, &cancel)
        .await
        .unwrap();
    assert!(stopped.stopped);
}
