use super::super::error::ServiceError;
use super::world;
use crate::activity::CancellationToken;
use crate::activity_log::RetryAction;
use crate::instance::{InstanceSettings, Loader};

#[tokio::test]
async fn a_whole_game_is_backed_up_and_restored_as_a_new_one_with_nothing_touched() {
    let world = world();
    let record = world
        .service
        .create_instance("生存", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .service
        .store
        .lock()
        .await
        .update_settings(
            &record.id,
            InstanceSettings {
                max_memory_mb: Some(4096),
                java_path: Some("/somewhere/java".into()),
                ..InstanceSettings::default()
            },
        )
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::create_dir_all(game.join("saves/W")).unwrap();
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
    std::fs::write(game.join("saves/W/level.dat"), b"world").unwrap();
    std::fs::write(game.join("logs/latest.log"), b"log").unwrap();
    let archive = world._dir.path().join("b.zip");

    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world
                .service
                .backup_instance(&record.id, &archive, CancellationToken::new())
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(
        world
            .service
            .backup_instance(&record.id, &archive, CancellationToken::new())
            .await
            .unwrap()
            > 0
    );

    let restored = world
        .service
        .restore_backup(&archive, CancellationToken::new())
        .await
        .unwrap();
    assert_ne!(restored.id, record.id);
    assert_eq!(restored.name, "生存（恢复）");
    assert_eq!(
        (restored.game_version.as_str(), restored.loader),
        ("1.0", Loader::Fabric)
    );
    assert_eq!(restored.settings.max_memory_mb, Some(4096));
    assert_eq!(
        restored.settings.java_path, None,
        "a Java path is per machine"
    );
    assert!(!restored.installed);
    let there = world.service.layout.game(&restored.id);
    assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(
        std::fs::read(there.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(!there.join("logs").exists());
    // The original is as it was.
    assert_eq!(
        std::fs::read(game.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert_eq!(world.service.library().await.instances.len(), 2);

    // A file that is not a backup changes nothing.
    let junk = world._dir.path().join("junk.zip");
    std::fs::write(&junk, b"nope").unwrap();
    assert!(
        world
            .service
            .restore_backup(&junk, CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(world.service.library().await.instances.len(), 2);
    let failed = world.service.activity(5).finished.remove(0);
    assert_eq!(
        failed.retry,
        Some(RetryAction::RestoreBackup {
            path: junk.display().to_string()
        })
    );
}

#[tokio::test]
async fn a_game_from_another_launcher_becomes_a_new_one_and_the_source_stays_as_it_was() {
    let world = world();
    let source = world._dir.path().join("prism-inst");
    let write = |path: &str, body: &str| {
        let full = source.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, body).unwrap();
    };
    write("instance.cfg", "name=我的生存\n");
    write(
        "mmc-pack.json",
        r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},
            {"uid":"net.fabricmc.fabric-loader","version":"0.15.7"}]}"#,
    );
    write(".minecraft/mods/a.jar", "mod");
    write(".minecraft/saves/W/level.dat", "world");
    write(".minecraft/logs/latest.log", "log");

    let found = world.service.detect_games(&source).await.unwrap();
    assert_eq!(found.len(), 1);
    let record = world
        .service
        .import_game(found[0].clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(record.name, "我的生存");
    assert_eq!(
        (
            record.game_version.as_str(),
            record.loader,
            record.loader_version.as_deref()
        ),
        ("1.20.1", Loader::Fabric, Some("0.15.7"))
    );
    assert!(!record.installed);
    let there = world.service.layout.game(&record.id);
    assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(
        std::fs::read(there.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(!there.join("logs").exists());
    assert!(
        source.join(".minecraft/mods/a.jar").is_file(),
        "the source is untouched"
    );

    // Not a launcher's folder; a cancelled import; a modded game with no loader version.
    let empty = world._dir.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(world.service.detect_games(&empty).await.is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        world.service.import_game(found[0].clone(), cancel).await,
        Err(ServiceError::Cancelled)
    ));
    let mut nameless = found[0].clone();
    nameless.loader_version = None;
    assert!(
        world
            .service
            .import_game(nameless, CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(
        world.service.library().await.instances.len(),
        1,
        "failures add no game"
    );
    let leftovers = std::fs::read_dir(world.service.layout.operations())
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "no staging folder is left behind");
}

#[tokio::test]
async fn a_game_exported_as_a_prism_zip_comes_back_in_through_the_pack_importer() {
    let world = world();
    let record = world
        .service
        .create_instance("Round", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:70").unwrap();
    let spec = crate::pack_export::ExportSpec {
        format: crate::pack_export::PackFormat::Prism,
        name: "Round".into(),
        version: "1".into(),
        summary: None,
        include: vec!["mods".into(), "options.txt".into()],
    };
    let zip = world._dir.path().join("round.zip");
    let report = world
        .service
        .export_modpack(&record.id, spec, &zip, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!((report.linked, report.bundled), (0, 2));

    let back = world
        .service
        .import_modpack_file(&zip, CancellationToken::new())
        .await
        .unwrap();
    assert_ne!(back.id, record.id);
    assert_eq!(
        (
            back.game_version.as_str(),
            back.loader,
            back.loader_version.as_deref()
        ),
        ("1.0", Loader::Fabric, Some("0.16.0"))
    );
    let there = world.service.layout.game(&back.id);
    assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(std::fs::read(there.join("options.txt")).unwrap(), b"fov:70");
    // The scratch folder is gone, and the finished task knows the new game.
    let scratch = world.service.layout.root().join("cache");
    let leftovers = std::fs::read_dir(&scratch).map(|e| e.count()).unwrap_or(0);
    assert_eq!(leftovers, 0);
    assert_eq!(
        world.service.activity(5).finished[0].instance_id.as_deref(),
        Some(back.id.as_str())
    );
}
