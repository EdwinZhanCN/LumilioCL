use super::super::error::ServiceError;
use super::{
    ECHO_ARGS, assert_send, fake_java, is_installed, launch_to_end, outcomes_of,
    publish_modern_release, publish_release, reopen, reopen_service, world,
};
use crate::activity::CancellationToken;
use crate::activity_log::TaskOutcome;
use crate::discover::ProjectKind;
use crate::history::{HistoryEvent, HistoryLog, SessionOutcome};
use crate::instance::Loader;
use crate::launch_session::{LaunchSession, LaunchSignal};
use crate::launcher::{LaunchServiceError, LaunchUpdate};
use crate::recovery::{RecoveryNote, SessionMarker};
use tokio::sync::mpsc;

#[tokio::test]
async fn pending_launch_rejects_conflicting_writes_and_drop_releases_target() {
    let world = world();
    let mut records = Vec::new();
    for name in ["Target", "Independent"] {
        records.push(
            world
                .service
                .create_instance(name, Some("1.0"), Loader::Vanilla, None)
                .await
                .unwrap(),
        );
    }
    let target = &records[0].id;
    // Pause launch at the settings boundary after it reserves its target.
    let defaults = world.service.settings.lock().await;
    let (updates, _receiver) = mpsc::unbounded_channel();
    let mut launch = Box::pin(
        world
            .service
            .launch(target, updates, CancellationToken::new()),
    );
    assert!(futures_util::poll!(&mut launch).is_pending());
    assert!(
        matches!(world.service.delete_instance(target).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
    );
    assert!(
        matches!(world.service.install_content(target, ProjectKind::Mod, "project", CancellationToken::new()).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
    );
    let (updates, _receiver) = mpsc::unbounded_channel();
    assert!(
        matches!(world.service.launch(target, updates, CancellationToken::new()).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
    );
    assert_eq!(world.service.instance(target).await.unwrap().name, "Target");
    world.service.rename(target, "Renamed").await.unwrap();
    world.service.delete_instance(&records[1].id).await.unwrap();
    assert!(world.service.instance(target).await.is_ok());
    drop(launch);
    drop(defaults);
    world.service.delete_instance(target).await.unwrap();
    assert!(matches!(
        world.service.instance(target).await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn failed_launch_does_not_leave_an_instance_reserved() {
    let world = world();
    let (updates, _receiver) = mpsc::unbounded_channel();
    assert!(matches!(
        world
            .service
            .launch("missing", updates, CancellationToken::new())
            .await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    assert!(matches!(
        world.service.delete_instance("missing").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn launching_installs_runs_records_and_then_works_offline() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let launch = world
        .service
        .launch(&record.id, tx, CancellationToken::new());
    assert_send(&launch);
    let exit = launch.await.unwrap();
    assert_eq!(exit.code, Some(0));
    let mut session = LaunchSession::new();
    while let Ok(update) = rx.try_recv() {
        if let LaunchUpdate::Signal(signal) = update {
            session.apply(signal);
        }
    }
    assert!(session.is_finished());

    let library = world.service.library().await;
    let stored = &library.instances[0];
    assert!(stored.installed);
    assert!(stored.last_played.is_some());
    let history = HistoryLog::for_instance(world.service.layout().root(), &record.id)
        .sessions()
        .unwrap();
    assert_eq!(history.len(), 1);
    assert!(
        world
            .service
            .layout()
            .versions()
            .join("1.0/1.0.jar")
            .is_file()
    );

    // The network is gone; the installed release still launches.
    world.net.forget_all();
    let (tx, _rx) = mpsc::unbounded_channel();
    let again = world
        .service
        .launch(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(again.code, Some(0));
}

#[tokio::test]
async fn installing_is_separate_cancellable_and_only_success_marks_installed() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Pack", None, Loader::Vanilla, None)
        .await
        .unwrap();

    // Cancelled mid-download: not installed, recorded as cancelled.
    let cancel = CancellationToken::new();
    *world.net.cancel_on_file.lock().unwrap() = Some(cancel.clone());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let cancelled = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        world.service.install_instance(&record.id, tx, cancel),
    )
    .await
    .expect("cancelling must not wait for a stalled download");
    assert!(matches!(
        cancelled,
        Err(ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled))
    ));
    assert!(!is_installed(&world, &record.id).await);
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Cancelled
    );
    let mut session = LaunchSession::new();
    while let Ok(LaunchUpdate::Signal(signal)) = rx.try_recv() {
        session.apply(signal);
    }
    assert!(session.is_finished());

    // A failing download: not installed, recorded as failed with a reason.
    let client = world.server.join("client.jar");
    let kept = std::fs::read(&client).unwrap();
    std::fs::remove_file(&client).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .install_instance(&record.id, tx, CancellationToken::new())
            .await
            .is_err()
    );
    assert!(!is_installed(&world, &record.id).await);
    assert!(matches!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Failed(_)
    ));

    // Retry succeeds; no game process or play session came with it.
    std::fs::write(&client, kept).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .install_instance(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert!(is_installed(&world, &record.id).await);
    let stored = world.service.instance(&record.id).await.unwrap();
    assert!(stored.last_played.is_none());
    assert!(
        HistoryLog::for_instance(world.service.layout.root(), &record.id)
            .sessions()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Succeeded
    );

    // Installed once, it launches with the network gone (AC-OFFLINE-01).
    fake_java(&world, "echo \"Setting user: x\"");
    world.net.forget_all();
    let (tx, _rx) = mpsc::unbounded_channel();
    let exit = world
        .service
        .launch(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(exit.code, Some(0));
}

#[tokio::test]
async fn every_way_a_launch_can_end_leaves_its_own_record() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Ends", None, Loader::Vanilla, None)
        .await
        .unwrap();

    // No Java: preparation fails before any process exists.
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await
            .is_err()
    );
    // Cancelled before it starts: no process, its own outcome.
    let cancel = CancellationToken::new();
    cancel.cancel();
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(world.service.launch(&record.id, tx, cancel).await.is_err());
    let stored = world.service.instance(&record.id).await.unwrap();
    assert!(stored.last_played.is_none());
    assert_eq!(stored.play_seconds, 0);

    // Dies at once, crashes after starting, exits cleanly.
    for script in [
        "exit 1",
        "echo \"Setting user: x\"; exit 3",
        "echo \"Setting user: x\"",
    ] {
        fake_java(&world, script);
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await
            .unwrap();
    }

    // Running, then the user stops it: the process really ends.
    fake_java(&world, "echo \"Setting user: x\"; sleep 30");
    let stop = CancellationToken::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let launch = world.service.launch(&record.id, tx, stop.clone());
    let stopper = async {
        while let Some(update) = rx.recv().await {
            if matches!(update, LaunchUpdate::Signal(LaunchSignal::Running)) {
                stop.cancel();
                break;
            }
        }
        // Keep the receiver open so later updates do not fail the send.
        while rx.recv().await.is_some() {}
    };
    let (exit, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        tokio::join!(launch, stopper)
    })
    .await
    .expect("stopping must end the process promptly");
    let exit = exit.unwrap();
    assert!(exit.killed && exit.was_running);

    assert_eq!(
        outcomes_of(&world, &record.id).await,
        [
            SessionOutcome::Stopped,
            SessionOutcome::Clean,
            SessionOutcome::Crashed,
            SessionOutcome::FailedToStart,
            SessionOutcome::Cancelled,
            SessionOutcome::FailedToPrepare,
        ]
    );
    // Nothing is left holding the instance.
    assert!(world.service.reserve_instance(&record.id).is_ok());
}

#[tokio::test]
async fn a_launcher_that_died_mid_launch_reports_the_session_and_touches_no_process() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Died", None, Loader::Vanilla, None)
        .await
        .unwrap();
    // While a launch is in progress the marker exists; a normal return removes it.
    fake_java(&world, "echo \"Setting user: x\"");
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .launch(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert!(!world.service.layout.sessions().join("died.json").exists());
    // A launcher killed mid-launch cannot clean up: rehearse by leaving the marker.
    let marker = SessionMarker::place(&world.service.layout, &record.id, 42).unwrap();
    std::mem::forget(marker);
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert!(
        service
            .startup_notes()
            .contains(&RecoveryNote::SessionInterrupted {
                instance_id: record.id.clone(),
                started: 42
            })
    );
    let history = HistoryLog::for_instance(&root, &record.id)
        .sessions()
        .unwrap();
    assert!(matches!(
        history[..],
        [
            HistoryEvent::Session {
                started: 42,
                outcome: SessionOutcome::Interrupted,
                ..
            },
            ..
        ]
    ));
    assert!(!service.layout.sessions().join("died.json").exists());
    // Reported once; nothing was relaunched.
    let (service, _dir) = reopen_service(service, &root);
    assert!(service.startup_notes().is_empty());
    assert_eq!(
        service.instance(&record.id).await.unwrap().play_seconds,
        0,
        "an interrupted session counts no play time"
    );
    // An id that is not plain is refused before any file is named after it.
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(matches!(
        service
            .launch("../escape", tx, CancellationToken::new())
            .await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn launch_defaults_reach_the_real_process_and_its_commands() {
    use crate::tuning::{EnvVar, LaunchTuning};
    let world = world();
    publish_release(&world);
    fake_java(
        &world,
        concat!(
            "echo \"Setting user: x\"\n",
            "echo \"$*\" > \"$INST_DIR/args.txt\"\n",
            "echo \"FOO=$FOO WRAPPED=$WRAPPED NAME=$INST_NAME\" > \"$INST_DIR/env.txt\"",
        ),
    );
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    world
        .service
        .set_launch_defaults(LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            fullscreen: Some(true),
            jvm_arguments: vec!["-Dtuned=1".into()],
            game_arguments: vec!["--demo".into()],
            environment: vec![EnvVar {
                name: "FOO".into(),
                value: "bar".into(),
            }],
            wrapper: Some("env WRAPPED=yes".into()),
            pre_launch: Some("echo before > \"$INST_DIR/pre.txt\"".into()),
            post_exit: Some("echo after > \"$INST_DIR/post.txt\"".into()),
        })
        .await
        .unwrap();

    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(args.contains("-Dtuned=1"), "{args}");
    assert!(args.contains("--width 1280 --height 720"), "{args}");
    assert!(args.contains("--fullscreen"), "{args}");
    assert!(args.trim_end().ends_with("--demo"), "{args}");
    let env = std::fs::read_to_string(game.join("env.txt")).unwrap();
    assert_eq!(env.trim(), "FOO=bar WRAPPED=yes NAME=Run");
    assert_eq!(
        std::fs::read_to_string(game.join("pre.txt"))
            .unwrap()
            .trim(),
        "before"
    );
    assert_eq!(
        std::fs::read_to_string(game.join("post.txt"))
            .unwrap()
            .trim(),
        "after"
    );
}

#[tokio::test]
async fn launching_into_a_world_adds_the_release_s_quick_play_arguments_only_where_declared() {
    let world = world();
    publish_modern_release(&world);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    std::fs::create_dir_all(game.join("saves/My World")).unwrap();
    let go = |name: &'static str| {
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .launch_world(&record.id, name, tx, CancellationToken::new())
    };

    go("My World").await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(args.contains("--quickPlaySingleplayer My World"), "{args}");
    assert!(
        args.contains("--quickPlayPath quickPlay/lumilio.json"),
        "{args}"
    );
    assert!(!args.contains("--quickPlayMultiplayer"), "{args}");

    // A world that is not there never starts the game.
    std::fs::remove_file(game.join("args.txt")).unwrap();
    let error = go("Gone").await.unwrap_err();
    assert!(error.to_string().contains("no saved world"), "{error}");
    assert!(go("../escape").await.is_err());
    assert!(!game.join("args.txt").exists());

    // The ordinary launch goes to the menu: no quick-play arguments.
    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(!args.contains("quickPlay"), "{args}");
}

#[tokio::test]
async fn a_version_without_quick_play_says_so_instead_of_starting_at_the_menu() {
    let world = world();
    publish_release(&world);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    std::fs::create_dir_all(game.join("saves/My World")).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let error = world
        .service
        .launch_world(&record.id, "My World", tx, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot start directly in singleplayer"),
        "{error}"
    );
    assert!(!game.join("args.txt").exists(), "the game must not start");
}

#[tokio::test]
async fn a_chosen_server_uses_quick_play_where_declared_and_the_old_arguments_elsewhere() {
    use crate::tuning::{InstanceLaunch, QuickPlay};
    for (modern, expected) in [
        (true, "--quickPlayMultiplayer mc.example.com:25565"),
        (false, "--server mc.example.com --port 25565"),
    ] {
        let world = world();
        if modern {
            publish_modern_release(&world);
        } else {
            publish_release(&world);
        }
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let mut settings = record.settings.clone();
        settings.launch = InstanceLaunch {
            quick_play: Some(QuickPlay::Server("mc.example.com:25565".into())),
            ..InstanceLaunch::default()
        };
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let args =
            std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt"))
                .unwrap();
        assert!(args.contains(expected), "modern={modern}: {args}");
    }
}

#[tokio::test]
async fn launching_an_unknown_instance_is_an_error() {
    let world = world();
    let (tx, _rx) = mpsc::unbounded_channel();
    let result = world
        .service
        .launch("ghost", tx, CancellationToken::new())
        .await;
    assert!(matches!(result, Err(ServiceError::NoSuchInstance(_))));
}
