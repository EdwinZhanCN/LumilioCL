use crate::error::ServiceError;
use crate::instances::Loader;
use crate::instances::history::ChangeKind;
use crate::testing::{change_subjects, world};
use crate::worlds::WorldError;

#[tokio::test]
async fn worlds_are_copied_and_deleted_under_the_lease_and_recorded() {
    let world = world();
    let record = world
        .service
        .create_instance("Worlds", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let saves = world.service.layout.game(&record.id).join("saves");
    std::fs::create_dir_all(saves.join("w/region")).unwrap();
    std::fs::write(saves.join("w/level.dat"), b"not nbt").unwrap();
    std::fs::write(saves.join("w/region/r.0.0.mca"), b"chunks").unwrap();

    let listed = world.service.worlds(&record.id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].damaged, "a damaged world is still listed");

    // While the instance is busy, writes are refused and reads still work.
    {
        let _busy = world.service.instances.reserve(&record.id).unwrap();
        assert!(matches!(
            world.service.copy_world(&record.id, "w", None).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert!(matches!(
            world.service.delete_world(&record.id, "w").await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert_eq!(world.service.worlds(&record.id).await.unwrap().len(), 1);
    }
    assert!(saves.join("w").is_dir());

    // A leftover half-copy is swept, the copy is whole, and nothing is overwritten.
    std::fs::create_dir_all(saves.join(".w copy.copying")).unwrap();
    let first = world
        .service
        .copy_world(&record.id, "w", None)
        .await
        .unwrap();
    let second = world
        .service
        .copy_world(&record.id, "w", None)
        .await
        .unwrap();
    assert_eq!((first.as_str(), second.as_str()), ("w copy", "w copy 2"));
    assert!(!saves.join(".w copy.copying").exists());
    assert_eq!(
        std::fs::read(saves.join("w copy/region/r.0.0.mca")).unwrap(),
        b"chunks"
    );
    assert!(matches!(
        world
            .service
            .copy_world(&record.id, "w", Some("w copy"))
            .await,
        Err(ServiceError::World(WorldError::AlreadyExists(_)))
    ));
    assert!(matches!(
        world.service.copy_world(&record.id, "ghost", None).await,
        Err(ServiceError::World(WorldError::NotFound(_)))
    ));

    world
        .service
        .delete_world(&record.id, "w copy")
        .await
        .unwrap();
    assert!(!saves.join("w copy").exists());
    assert!(saves.join("w").is_dir() && saves.join("w copy 2").is_dir());
    assert_eq!(
        change_subjects(&world, &record.id),
        [
            (ChangeKind::WorldDeleted, "w copy".to_owned()),
            (ChangeKind::WorldCopied, "w copy 2".to_owned()),
            (ChangeKind::WorldCopied, "w copy".to_owned()),
        ]
    );
    assert!(matches!(
        world.service.worlds("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn a_world_is_exported_and_imported_under_the_lease_and_the_import_is_recorded() {
    let world = world();
    let record = world
        .service
        .create_instance("Zip", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let saves = world.service.layout.game(&record.id).join("saves");
    std::fs::create_dir_all(saves.join("w")).unwrap();
    std::fs::write(saves.join("w/level.dat"), b"not nbt").unwrap();
    let archive = world._dir.path().join("w.zip");

    {
        let _busy = world.service.instances.reserve(&record.id).unwrap();
        assert!(matches!(
            world.service.export_world(&record.id, "w", &archive).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert!(matches!(
            world.service.import_world(&record.id, &archive).await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(!archive.exists());
    assert!(
        world
            .service
            .export_world(&record.id, "w", &archive)
            .await
            .unwrap()
            > 0
    );
    assert!(matches!(
        world
            .service
            .export_world(&record.id, "ghost", &archive)
            .await,
        Err(ServiceError::World(WorldError::NotFound(_)))
    ));

    let imported = world
        .service
        .import_world(&record.id, &archive)
        .await
        .unwrap();
    assert_eq!(imported, "w 2");
    assert!(saves.join("w 2/level.dat").is_file());
    assert_eq!(
        change_subjects(&world, &record.id),
        [(ChangeKind::WorldImported, "w 2".to_owned())]
    );
}

/// Issue #128: while a game runs, only the world it has open is held.
#[tokio::test]
async fn while_a_game_runs_only_the_world_it_has_open_is_refused() {
    use crate::instances::snapshot_files::SnapshotScope;
    use std::time::{Duration, UNIX_EPOCH};

    let world = world();
    let record = world
        .service
        .create_instance("Playing", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let id = record.id.as_str();
    let saves = world.service.layout.game(id).join("saves");
    for name in ["open", "idle"] {
        std::fs::create_dir_all(saves.join(name)).unwrap();
        std::fs::write(saves.join(name).join("level.dat"), b"not nbt").unwrap();
        std::fs::write(saves.join(name).join("session.lock"), b"x").unwrap();
    }
    // "idle" was opened by an earlier game; this game opened "open".
    let lock_time = |name: &str| {
        std::fs::metadata(saves.join(name).join("session.lock"))
            .unwrap()
            .modified()
            .unwrap()
    };
    let idle_at = lock_time("idle");
    let started = idle_at + Duration::from_secs(10);
    std::fs::File::options()
        .write(true)
        .open(saves.join("open/session.lock"))
        .unwrap()
        .set_modified(started + Duration::from_secs(5))
        .unwrap();
    let started_ms =
        i64::try_from(started.duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap();
    let game = world
        .service
        .instances
        .reserve(id)
        .unwrap()
        .playing(started_ms);

    fn in_use<T>(result: Result<T, ServiceError>) -> bool {
        matches!(result, Err(ServiceError::WorldInUse(folder)) if folder == "open")
    }
    assert!(in_use(world.service.copy_world(id, "open", None).await));
    assert!(in_use(world.service.delete_world(id, "open").await));
    let zip = world.service.layout.root().join("open.zip");
    assert!(in_use(world.service.export_world(id, "open", &zip).await));
    assert!(in_use(
        world
            .service
            .create_snapshot(id, SnapshotScope::World("open".to_owned()), "")
            .await
    ));

    // The other world is free, and a world can be added.
    let copy = world.service.copy_world(id, "idle", None).await.unwrap();
    let zip = world.service.layout.root().join("idle.zip");
    world.service.export_world(id, "idle", &zip).await.unwrap();
    world
        .service
        .create_snapshot(id, SnapshotScope::World("idle".to_owned()), "")
        .await
        .unwrap();
    world.service.delete_world(id, &copy).await.unwrap();
    world.service.import_world(id, &zip).await.unwrap();

    // Work on the whole game still waits for it to end.
    assert!(matches!(
        world
            .service
            .create_snapshot(id, SnapshotScope::Full, "")
            .await,
        Err(ServiceError::InstanceBusy(_))
    ));
    assert!(matches!(
        world.service.delete_instance(id).await,
        Err(ServiceError::InstanceBusy(_))
    ));

    drop(game);
    world.service.delete_world(id, "open").await.unwrap();
}
