use super::{ECHO_ARGS, fake_java, launch_to_end, publish_versions, repair_to_end, world};
use crate::activity::CancellationToken;
use crate::activity_log::TaskCategory;
use crate::history::{ChangeKind, HistoryEvent};
use crate::instance::Loader;
use tokio::sync::mpsc;

#[tokio::test]
async fn changing_the_version_prepares_the_new_files_then_commits_and_records_it() {
    let world = world();
    publish_versions(&world, &["1.0", "2.0"]);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let versions = world.service.layout().versions();
    assert!(versions.join("1.0/1.0.jar").is_file());
    assert!(!versions.join("2.0/2.0.jar").exists());

    let (tx, _rx) = mpsc::unbounded_channel();
    let changed = world
        .service
        .change_runtime(
            &record.id,
            "2.0",
            Loader::Vanilla,
            None,
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(changed.game_version, "2.0");
    assert!(changed.installed);
    assert!(
        versions.join("2.0/2.0.jar").is_file(),
        "the new files are fetched"
    );
    assert!(
        versions.join("1.0/1.0.jar").is_file(),
        "the old shared files stay"
    );
    let stored = world.service.instance(&record.id).await.unwrap();
    assert_eq!(stored.game_version, "2.0");

    // It starts on the new version, offline, and the history says what changed.
    world.net.forget_all();
    launch_to_end(&world, &record.id).await.unwrap();
    let history = world.service.history(&record.id).await.unwrap();
    assert!(
        history.events.iter().any(|event| matches!(
            event,
            HistoryEvent::Change { kind: ChangeKind::GameVersionChanged, subject, .. }
                if subject.contains("1.0") && subject.contains("2.0")
        )),
        "{:?}",
        history.events
    );

    // The same combination again is refused, not repeated.
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .change_runtime(
                &record.id,
                "2.0",
                Loader::Vanilla,
                None,
                tx,
                CancellationToken::new()
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_version_change_that_cannot_finish_leaves_the_old_game_launchable() {
    let world = world();
    publish_versions(&world, &["1.0", "2.0"]);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let ask = |version: &'static str, cancel: CancellationToken| {
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .change_runtime(&record.id, version, Loader::Vanilla, None, tx, cancel)
    };

    // The new client cannot be downloaded: nothing changes.
    std::fs::remove_file(world.server.join("2.0.jar")).unwrap();
    assert!(ask("2.0", CancellationToken::new()).await.is_err());
    assert_eq!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .game_version,
        "1.0"
    );

    // Cancelled before it starts: nothing changes.
    std::fs::write(world.server.join("2.0.jar"), b"client of 2.0").unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(ask("2.0", cancel).await.is_err());
    assert_eq!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .game_version,
        "1.0"
    );

    // The library cannot be written: the files were fetched but the
    // record stays on the old version.
    world.service.store.lock().await.set_read_only(true);
    assert!(ask("2.0", CancellationToken::new()).await.is_err());
    world.service.store.lock().await.set_read_only(false);
    assert_eq!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .game_version,
        "1.0"
    );

    // Through all of it, the old version still starts without the network.
    world.net.forget_all();
    launch_to_end(&world, &record.id).await.unwrap();
    assert!(
        world
            .service
            .history(&record.id)
            .await
            .unwrap()
            .events
            .iter()
            .all(|event| {
                !matches!(
                    event,
                    HistoryEvent::Change {
                        kind: ChangeKind::GameVersionChanged,
                        ..
                    }
                )
            }),
        "no change was recorded"
    );
}

#[tokio::test]
async fn a_loader_needs_its_version_and_an_unsupported_one_is_refused() {
    let world = world();
    publish_versions(&world, &["1.0"]);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let error = world
        .service
        .change_runtime(
            &record.id,
            "1.0",
            Loader::Fabric,
            None,
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("loader version"), "{error}");
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .change_runtime(
                "ghost",
                "1.0",
                Loader::Vanilla,
                None,
                tx,
                CancellationToken::new()
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn repairing_refetches_only_what_is_damaged_and_needs_no_network_when_intact() {
    let world = world();
    publish_versions(&world, &["1.0"]);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let jar = world.service.layout().versions().join("1.0/1.0.jar");
    let good = std::fs::read(&jar).unwrap();

    // Damaged: put back, and a worlds/mods folder stays as it was.
    let saves = world.service.layout().game(&record.id).join("saves/keep");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(&jar, b"broken").unwrap();
    repair_to_end(&world, &record.id).await.unwrap();
    assert_eq!(std::fs::read(&jar).unwrap(), good);
    assert!(saves.is_dir());

    // Intact and offline: nothing to fetch, nothing to fail.
    world.net.forget_all();
    repair_to_end(&world, &record.id).await.unwrap();
    assert_eq!(std::fs::read(&jar).unwrap(), good);

    // Missing: restored too, and both repairs are in the history.
    std::fs::remove_file(&jar).unwrap();
    world.net.answer(
        "piston-meta.mojang.com/mc/game/version_manifest",
        r#"{"latest":{"release":"1.0","snapshot":"1.0"},"versions":[]}"#.to_owned(),
    );
    repair_to_end(&world, &record.id).await.unwrap();
    assert_eq!(std::fs::read(&jar).unwrap(), good);
    let repaired = world
        .service
        .history(&record.id)
        .await
        .unwrap()
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                HistoryEvent::Change {
                    kind: ChangeKind::Repaired,
                    ..
                }
            )
        })
        .count();
    assert_eq!(repaired, 3);
    let activity = world.service.activity(10);
    assert!(
        activity
            .finished
            .iter()
            .any(|task| task.category == TaskCategory::Repair)
    );
}
