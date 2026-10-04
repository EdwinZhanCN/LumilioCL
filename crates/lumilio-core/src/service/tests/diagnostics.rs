use super::super::error::ServiceError;
use super::super::types::LOG_TAIL_BYTES;
use super::{fake_java, launch_to_end, publish_release, world};
use crate::activity::CancellationToken;
use crate::activity_log::TaskOutcome;
use crate::history::{ChangeKind, HistoryEvent};
use crate::instance::Loader;

#[tokio::test]
async fn history_and_problems_are_readable_without_the_lease() {
    let world = world();
    let record = world
        .service
        .create_instance("Read", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let saves = world.service.layout.game(&record.id).join("saves");
    std::fs::create_dir_all(saves.join("w")).unwrap();
    world
        .service
        .copy_world(&record.id, "w", None)
        .await
        .unwrap();
    let _busy = world.service.reserve_instance(&record.id).unwrap();
    let read = world.service.history(&record.id).await.unwrap();
    assert!(matches!(
        read.events[..],
        [HistoryEvent::Change {
            kind: ChangeKind::WorldCopied,
            ..
        }]
    ));
    let problems = world.service.problems(&record.id).await.unwrap();
    assert!(
        problems
            .iter()
            .any(|problem| problem.kind == crate::diagnostics::ProblemKind::NotInstalled)
    );
    assert!(matches!(
        world.service.history("nobody").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    assert!(matches!(
        world.service.problems("nobody").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn logs_and_crash_reports_are_read_safely_without_the_lease() {
    let world = world();
    let record = world
        .service
        .create_instance("Logs", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    let none = world.service.logs(&record.id).await.unwrap();
    assert_eq!((none.latest, none.crashes.len()), (None, 0));
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::create_dir_all(game.join("crash-reports")).unwrap();
    let big = "x\n".repeat(LOG_TAIL_BYTES as usize);
    std::fs::write(game.join("logs/latest.log"), &big).unwrap();
    std::fs::write(
        game.join("crash-reports/crash-1.txt"),
        "java.lang.OutOfMemoryError: heap",
    )
    .unwrap();
    let _busy = world.service.reserve_instance(&record.id).unwrap();
    let logs = world.service.logs(&record.id).await.unwrap();
    assert!(logs.latest.unwrap().len() as u64 <= LOG_TAIL_BYTES);
    assert_eq!(logs.crashes[0].file_name, "crash-1.txt");
    let (text, hints) = world
        .service
        .crash_report(&record.id, "crash-1.txt")
        .await
        .unwrap();
    assert!(text.contains("OutOfMemory"));
    assert_eq!(hints, vec![crate::diagnostics::CrashHint::OutOfMemory]);
    assert!(
        world
            .service
            .crash_report(&record.id, "../../launcher.db")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_games_size_counts_its_own_folder_only() {
    let world = world();
    let record = world
        .service
        .create_instance("Size", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    let before = world.service.instance_size(&record.id).await.unwrap();
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), vec![0_u8; 1000]).unwrap();
    std::fs::write(game.join("options.txt"), vec![0_u8; 24]).unwrap();
    assert_eq!(
        world.service.instance_size(&record.id).await.unwrap(),
        before + 1024
    );
    assert!(matches!(
        world.service.instance_size("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn unused_shared_files_are_measured_then_removed_only_when_nothing_is_running() {
    let world = world();
    let record = world
        .service
        .create_instance("Keep", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let meta = world.service.layout.meta();
    let put = |path: &str, body: &str| {
        let full = meta.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, body).unwrap();
    };
    put(
        "versions/1.0/1.0.json",
        r#"{"id":"1.0","assets":"9","assetIndex":{"id":"9"},"libraries":[]}"#,
    );
    put(
        "assets/indexes/9.json",
        r#"{"objects":{"a":{"hash":"aa11"}}}"#,
    );
    put("assets/objects/aa/aa11", "used");
    put("assets/objects/bb/bb22", "unused-bytes");
    put("versions/0.9/0.9.json", "{}");

    let found = world.service.reclaimable().await.unwrap();
    let bytes = found.bytes();
    assert!(bytes > 0);
    assert!(found.unused.iter().any(|(p, _)| p.ends_with("bb22")));
    assert!(
        found
            .unused
            .iter()
            .any(|(p, _)| p.ends_with("versions/0.9"))
    );
    assert!(!found.unused.iter().any(|(p, _)| p.ends_with("aa11")));

    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world.service.reclaim().await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(
        meta.join("assets/objects/bb/bb22").is_file(),
        "nothing removed while busy"
    );
    assert_eq!(world.service.reclaim().await.unwrap(), bytes);
    assert!(!meta.join("assets/objects/bb/bb22").exists());
    assert!(!meta.join("versions/0.9").exists());
    assert!(meta.join("assets/objects/aa/aa11").is_file());
    assert!(meta.join("versions/1.0/1.0.json").is_file());
    assert!(world.service.reclaimable().await.unwrap().unused.is_empty());
}

#[tokio::test]
async fn an_exported_log_hides_the_player_and_the_folders_and_leaves_nothing_half_written() {
    let world = world();
    let record = world
        .service
        .create_instance("Log", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::create_dir_all(game.join("crash-reports")).unwrap();
    std::fs::write(
        game.join("logs/latest.log"),
        format!(
            "[1] [main/INFO]: Setting user: Steve\n[2] [main/INFO]: dir {}\n",
            game.display()
        ),
    )
    .unwrap();
    std::fs::write(
        game.join("crash-reports/crash-1.txt"),
        "Player Steve crashed",
    )
    .unwrap();
    let out = world._dir.path().join("out.txt");

    world
        .service
        .export_log(&record.id, None, &out)
        .await
        .unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("Setting user: <player>") && text.contains("dir <game>"));
    assert!(!text.contains("Steve"));
    assert!(!world._dir.path().join("out.txt.part").exists());

    world
        .service
        .export_log(&record.id, Some("crash-1.txt"), &out)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "Player <player> crashed"
    );

    // No log, an unsafe name, or an unwritable place: an error and no file.
    let empty = world
        .service
        .create_instance("None", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let none = world._dir.path().join("none.txt");
    assert!(
        world
            .service
            .export_log(&empty.id, None, &none)
            .await
            .is_err()
    );
    assert!(
        world
            .service
            .export_log(&record.id, Some("../x"), &none)
            .await
            .is_err()
    );
    let blocked = world._dir.path().join("no-such-folder/out.txt");
    assert!(
        world
            .service
            .export_log(&record.id, None, &blocked)
            .await
            .is_err()
    );
    assert!(!none.exists());
}

#[tokio::test]
async fn a_cancelled_export_is_recorded_as_cancelled_and_leaves_no_pack() {
    let world = world();
    let record = world
        .service
        .create_instance("Stop", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
    let spec = crate::pack_export::ExportSpec {
        format: crate::pack_export::PackFormat::Modrinth,
        name: "Stop".into(),
        version: "1".into(),
        summary: None,
        include: vec!["mods".into()],
    };
    let pack = world._dir.path().join("stop.mrpack");
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        world
            .service
            .export_modpack(&record.id, spec, &pack, cancel)
            .await,
        Err(ServiceError::Cancelled)
    ));
    assert!(!pack.exists() && !world._dir.path().join("stop.mrpack.part").exists());
    assert_eq!(
        world.service.activity(5).finished[0].outcome,
        TaskOutcome::Cancelled
    );
}

#[tokio::test]
async fn clearing_the_cache_frees_space_and_games_still_start() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let natives = world.service.layout().natives("1.0");
    std::fs::create_dir_all(&natives).unwrap();
    std::fs::write(natives.join("lib.so"), vec![1u8; 512]).unwrap();
    let before = world.service.storage_usage().await;
    assert!(before.cache >= 512 && before.shared > 0 && before.runtimes > 0);
    let freed = world.service.clear_cache().await.unwrap();
    assert!(freed >= 512);
    let after = world.service.storage_usage().await;
    assert_eq!(after.cache, 0);
    assert_eq!(after.shared, before.shared, "shared game files stay");
    launch_to_end(&world, &record.id).await.unwrap();
}

#[tokio::test]
async fn the_diagnostics_bundle_hides_the_player_the_folders_and_commands() {
    use crate::tuning::LaunchTuning;
    use std::io::Read;
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: Steve\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    world
        .service
        .set_launch_defaults(LaunchTuning {
            pre_launch: Some("echo secret-command".into()),
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let game = world.service.layout().game(&record.id);
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(
        game.join("logs/latest.log"),
        format!(
            "Steve joined from {}\n",
            world.service.layout().root().display()
        ),
    )
    .unwrap();
    let path = world._dir.path().join("diagnostics.zip");
    world.service.export_diagnostics(&path).await.unwrap();

    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut all = String::new();
    for index in 0..archive.len() {
        archive
            .by_index(index)
            .unwrap()
            .read_to_string(&mut all)
            .unwrap();
    }
    assert!(all.contains("LumilioCL"), "{all}");
    assert!(all.contains("<player> joined from <launcher>"), "{all}");
    assert!(!all.contains("Steve"), "{all}");
    assert!(!all.contains(&world.service.layout().root().display().to_string()));
    assert!(!all.contains("secret-command"), "commands are only counted");
    assert!(all.contains("pre-launch true"));
}
