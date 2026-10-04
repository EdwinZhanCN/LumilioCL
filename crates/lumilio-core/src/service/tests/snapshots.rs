use super::super::error::ServiceError;
use super::{change_subjects, operation_dirs, reopen, world};
use crate::history::ChangeKind;
use crate::instance::Loader;
use crate::recovery::RecoveryNote;
use crate::snapshots::{SnapshotError, SnapshotScope};

#[tokio::test]
async fn snapshots_back_up_restore_and_undo_a_failed_restore_under_the_lease() {
    use std::os::unix::fs::PermissionsExt;
    let world = world();
    let record = world
        .service
        .create_instance("Backup", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("saves/W")).unwrap();
    std::fs::write(game.join("saves/W/level.dat"), b"v1").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:70").unwrap();

    // Nothing to back up is an error, not an empty snapshot.
    let empty = world
        .service
        .create_instance("Empty", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    assert!(matches!(
        world
            .service
            .create_snapshot(&empty.id, SnapshotScope::Full, "")
            .await,
        Err(ServiceError::Snapshot(SnapshotError::Empty))
    ));

    let snapshot = world
        .service
        .create_snapshot(&record.id, SnapshotScope::Full, "before")
        .await
        .unwrap();
    assert_eq!(world.service.snapshots(&record.id).await.unwrap().len(), 1);
    std::fs::write(game.join("saves/W/level.dat"), b"v2").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:110").unwrap();

    // Busy instance: writes refused, listing still works.
    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world.service.restore_snapshot(&record.id, &snapshot).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert!(matches!(
            world.service.delete_snapshot(&record.id, &snapshot).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert_eq!(world.service.snapshots(&record.id).await.unwrap().len(), 1);
    }

    // A restore that cannot finish leaves the game exactly as it was.
    std::fs::set_permissions(&game, std::fs::Permissions::from_mode(0o555)).unwrap();
    let failed = world.service.restore_snapshot(&record.id, &snapshot).await;
    std::fs::set_permissions(&game, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(failed, Err(ServiceError::Snapshot(_))));
    assert_eq!(
        std::fs::read(game.join("saves/W/level.dat")).unwrap(),
        b"v2"
    );
    assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"fov:110");
    assert!(operation_dirs(&world).is_empty());
    assert!(
        !change_subjects(&world, &record.id)
            .iter()
            .any(|(kind, _)| *kind == ChangeKind::SnapshotRestored)
    );

    let units = world
        .service
        .restore_snapshot(&record.id, &snapshot)
        .await
        .unwrap();
    assert!(units.contains(&std::path::PathBuf::from("saves/W")));
    assert_eq!(
        std::fs::read(game.join("saves/W/level.dat")).unwrap(),
        b"v1"
    );
    assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"fov:70");
    world
        .service
        .delete_snapshot(&record.id, &snapshot)
        .await
        .unwrap();
    assert!(
        world
            .service
            .snapshots(&record.id)
            .await
            .unwrap()
            .is_empty()
    );
    let kinds: Vec<_> = change_subjects(&world, &record.id)
        .into_iter()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(
        kinds,
        [
            ChangeKind::SnapshotDeleted,
            ChangeKind::SnapshotRestored,
            ChangeKind::SnapshotCreated
        ]
    );
}

#[tokio::test]
async fn a_restore_the_launcher_died_in_is_rolled_back_when_the_service_opens() {
    let world = world();
    let record = world
        .service
        .create_instance("Crash", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("saves/W")).unwrap();
    std::fs::write(game.join("saves/W/level.dat"), b"original").unwrap();
    // Rehearse: the original was moved aside, the launcher died before the
    // staged world was placed.
    let operation = world
        .service
        .layout
        .operations()
        .join(format!("restore-{}-snap", record.id));
    std::fs::create_dir_all(operation.join("new/saves/W")).unwrap();
    std::fs::write(operation.join("new/saves/W/level.dat"), b"restored").unwrap();
    std::fs::create_dir_all(operation.join("old/saves")).unwrap();
    std::fs::rename(game.join("saves/W"), operation.join("old/saves/W")).unwrap();
    std::fs::write(
        operation.join("restore.json"),
        format!(
            r#"{{"schema":1,"instance_id":"{}","snapshot":"snap","units":["saves/W"]}}"#,
            record.id
        ),
    )
    .unwrap();
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert_eq!(
        service.startup_notes(),
        [RecoveryNote::RestoreRolledBack {
            instance_id: record.id.clone(),
            snapshot: "snap".into(),
            units: vec![std::path::PathBuf::from("saves/W")],
        }]
    );
    assert_eq!(
        std::fs::read(service.layout.game(&record.id).join("saves/W/level.dat")).unwrap(),
        b"original"
    );
    assert!(!operation.exists());
}
