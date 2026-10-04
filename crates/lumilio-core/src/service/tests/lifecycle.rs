use super::super::LauncherService;
use super::super::error::ServiceError;
use super::{
    Scripted, fake_java, instance_with_profile, operation_dirs, publish_release, reopen, world,
};
use crate::activity::CancellationToken;
use crate::activity_log::{FinishedTask, TaskCategory, TaskOutcome};
use crate::deletion::Deletion;
use crate::instance::{Loader, NewInstance, StoreError};
use crate::launch_session::LaunchSignal;
use crate::launcher::LaunchUpdate;
use crate::recovery::RecoveryNote;
use std::os::unix::fs::PermissionsExt;
use tokio::sync::mpsc;

#[tokio::test]
async fn delete_removes_only_that_profile_and_leaves_no_journal() {
    let world = world();
    publish_release(&world);
    let target = instance_with_profile(&world, "Target").await;
    let other = instance_with_profile(&world, "Other").await;
    let meta = world.service.layout.meta();
    std::fs::create_dir_all(&meta).unwrap();
    std::fs::write(meta.join("shared.jar"), b"shared").unwrap();
    world.service.delete_instance(&target.id).await.unwrap();
    assert!(world.service.instance(&target.id).await.is_err());
    assert!(!world.service.layout.profile(&target.id).exists());
    assert!(
        world
            .service
            .layout
            .game(&other.id)
            .join("saves/level.dat")
            .exists()
    );
    assert!(meta.join("shared.jar").exists());
    assert!(operation_dirs(&world).is_empty());
}

#[tokio::test]
async fn failed_isolation_keeps_record_and_profile_and_allows_retry() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    let profiles = world.service.layout.profiles();
    std::fs::set_permissions(&profiles, std::fs::Permissions::from_mode(0o555)).unwrap();
    let failed = world.service.delete_instance(&record.id).await;
    std::fs::set_permissions(&profiles, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(failed, Err(ServiceError::Io(_))));
    assert!(world.service.instance(&record.id).await.is_ok());
    assert!(
        world
            .service
            .layout
            .game(&record.id)
            .join("saves/level.dat")
            .exists()
    );
    assert!(operation_dirs(&world).is_empty());
    world.service.delete_instance(&record.id).await.unwrap();
    assert!(!world.service.layout.profile(&record.id).exists());
}

#[tokio::test]
async fn failed_library_commit_restores_the_profile() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    world.service.store.lock().await.set_read_only(true);
    let failed = world.service.delete_instance(&record.id).await;
    world.service.store.lock().await.set_read_only(false);
    assert!(matches!(failed, Err(ServiceError::Store(_))));
    assert!(world.service.instance(&record.id).await.is_ok());
    assert_eq!(
        std::fs::read(
            world
                .service
                .layout
                .game(&record.id)
                .join("saves/level.dat")
        )
        .unwrap(),
        b"world"
    );
    assert!(operation_dirs(&world).is_empty());
    world.service.delete_instance(&record.id).await.unwrap();
}

#[tokio::test]
async fn failed_cleanup_still_deletes_and_the_next_start_finishes_it() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    // A read-only folder inside the profile makes the final removal fail.
    let locked = world.service.layout.game(&record.id).join("saves");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    world.service.delete_instance(&record.id).await.unwrap();
    assert!(world.service.instance(&record.id).await.is_err());
    let leftovers = operation_dirs(&world);
    assert_eq!(leftovers.len(), 1, "journal and files stay for recovery");
    std::fs::set_permissions(
        leftovers[0].join("profile/game/saves"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert_eq!(
        service.startup_notes(),
        [RecoveryNote::DeleteCompleted {
            instance_id: record.id.clone()
        }]
    );
    assert!(
        service
            .layout
            .operations()
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn crash_after_isolation_before_commit_rolls_back_on_next_start() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    // Rehearse the crash: journal and quarantine happened, the library did not commit.
    let mut crashed = Deletion::begin(&world.service.layout, &record, 7).unwrap();
    crashed.quarantine().unwrap();
    drop(crashed);
    assert!(!world.service.layout.profile(&record.id).exists());
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert_eq!(
        service.startup_notes(),
        [RecoveryNote::DeleteRolledBack {
            instance_id: record.id.clone()
        }]
    );
    assert!(service.instance(&record.id).await.is_ok());
    assert_eq!(
        std::fs::read(service.layout.game(&record.id).join("saves/level.dat")).unwrap(),
        b"world"
    );
    assert!(
        service
            .layout
            .operations()
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn crash_after_commit_completes_and_conflicts_or_bad_journals_are_kept() {
    let world = world();
    publish_release(&world);
    let committed = instance_with_profile(&world, "Committed").await;
    let conflict = instance_with_profile(&world, "Conflict").await;
    let broken = instance_with_profile(&world, "Broken").await;
    let layout = world.service.layout.clone();
    let mut crashed = Deletion::begin(&layout, &committed, 1).unwrap();
    crashed.quarantine().unwrap();
    world
        .service
        .store
        .lock()
        .await
        .remove(&committed.id)
        .unwrap();
    drop(crashed);
    // Both copies exist for the conflicting one.
    let mut both = Deletion::begin(&layout, &conflict, 2).unwrap();
    both.quarantine().unwrap();
    std::fs::create_dir_all(layout.profile(&conflict.id)).unwrap();
    drop(both);
    // An unreadable journal beside real files must not be removed.
    let mut bad = Deletion::begin(&layout, &broken, 3).unwrap();
    bad.quarantine().unwrap();
    drop(bad);
    let bad_dir = layout.operations().join(format!("delete-{}-3", broken.id));
    std::fs::write(bad_dir.join("delete.json"), b"{ not json").unwrap();
    let root = layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    let notes = service.startup_notes();
    assert_eq!(notes.len(), 3, "{notes:?}");
    assert!(notes.contains(&RecoveryNote::DeleteCompleted {
        instance_id: committed.id.clone()
    }));
    assert!(notes.iter().any(|note| matches!(
        note,
        RecoveryNote::DeleteConflict { instance_id, .. } if *instance_id == conflict.id
    )));
    assert!(notes.iter().any(|note| matches!(
        note,
        RecoveryNote::JournalUnusable { path, .. } if *path == bad_dir
    )));
    assert!(
        layout
            .operations()
            .join(format!("delete-{}-2", conflict.id))
            .join("profile")
            .exists()
    );
    assert!(bad_dir.join("profile/game/saves/level.dat").exists());
    assert!(
        !layout
            .operations()
            .join(format!("delete-{}-1", committed.id))
            .exists()
    );
}

#[tokio::test]
async fn damaged_library_is_kept_reported_and_never_overwritten_by_a_second_failure() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Lost").await;
    let root = world.service.layout.root().to_path_buf();
    let (service, dir) = reopen(world, &root);
    drop(service);
    let database = root.join("launcher.db");
    std::fs::write(&database, b"first damage").unwrap();
    let service = LauncherService::open(&root, Scripted::default()).unwrap();
    let [
        RecoveryNote::LibraryRecovered {
            preserved,
            candidates,
        },
    ] = service.startup_notes()
    else {
        panic!("{:?}", service.startup_notes());
    };
    assert_eq!(std::fs::read(preserved).unwrap(), b"first damage");
    assert_eq!(candidates, std::slice::from_ref(&record.id));
    let preserved = preserved.clone();
    assert!(service.library().await.instances.is_empty());
    assert!(root.join("profiles").join(&record.id).exists());
    drop(service);
    std::fs::write(&database, b"second damage").unwrap();
    let service = LauncherService::open(&root, Scripted::default()).unwrap();
    let [
        RecoveryNote::LibraryRecovered {
            preserved: second, ..
        },
    ] = service.startup_notes()
    else {
        panic!("{:?}", service.startup_notes());
    };
    assert_ne!(*second, preserved);
    assert_eq!(std::fs::read(second).unwrap(), b"second damage");
    assert_eq!(std::fs::read(&preserved).unwrap(), b"first damage");
    drop(dir);
}

#[tokio::test]
async fn newer_schema_refuses_to_open_and_leaves_the_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    drop(LauncherService::open(&root, Scripted::default()).unwrap());
    {
        let db = rusqlite::Connection::open(root.join("launcher.db")).unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
    }
    let before = std::fs::read(root.join("launcher.db")).unwrap();
    let refused = LauncherService::open(&root, Scripted::default());
    assert!(matches!(
        refused,
        Err(ServiceError::Store(StoreError::NewerSchema(99)))
    ));
    assert_eq!(std::fs::read(root.join("launcher.db")).unwrap(), before);
    assert!(!root.join("launcher.db.broken").exists());
    // The refused open must not keep the root locked.
    assert!(matches!(
        LauncherService::open(&root, Scripted::default()),
        Err(ServiceError::Store(StoreError::NewerSchema(99)))
    ));
}

#[tokio::test]
async fn damaged_settings_and_torn_activity_lines_are_reported_with_counts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    drop(LauncherService::open(&root, Scripted::default()).unwrap());
    std::fs::write(root.join("settings.json"), b"{ nope").unwrap();
    let good = FinishedTask {
        category: TaskCategory::Install,
        label: "ok".into(),
        instance_id: None,
        started: 1,
        finished: 2,
        outcome: TaskOutcome::Succeeded,
        retry: None,
    };
    let mut lines = serde_json::to_string(&good).unwrap();
    lines.push_str("\n{torn\nnot even json\n");
    std::fs::write(root.join("activity.jsonl"), lines).unwrap();
    let service = LauncherService::open(&root, Scripted::default()).unwrap();
    let notes = service.startup_notes();
    assert!(notes.iter().any(|note| matches!(
        note,
        RecoveryNote::SettingsRecovered { preserved } if std::fs::read(preserved).unwrap() == b"{ nope"
    )));
    assert!(notes.contains(&RecoveryNote::ActivityLogSkipped { count: 2 }));
    assert_eq!(service.activity(10).finished.len(), 1);
}

#[tokio::test]
async fn registered_instance_details_survive_rename_and_missing_profile() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Details".into(),
                game_version: "1.21.1".into(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            1,
        )
        .unwrap()
        .clone();
    world.service.set_favorite(&record.id, true).await.unwrap();
    {
        let mut store = world.service.store.lock().await;
        store.create_collection("Keep").unwrap();
        store.set_membership("Keep", &record.id, true).unwrap();
    }
    assert!(!world.service.layout().profile(&record.id).exists());
    world.service.rename(&record.id, "生存游戏").await.unwrap();
    let detail = world.service.instance(&record.id).await.unwrap();
    assert_eq!(detail.name, "生存游戏");
    assert_eq!(detail.id, record.id);
    assert!(detail.favorite);
    assert_eq!(
        world.service.library().await.collections[0].members,
        std::slice::from_ref(&record.id)
    );
    let root = world.service.layout().root().to_path_buf();
    drop(world.service);
    let reopened = LauncherService::open(root, world.net.clone()).unwrap();
    assert_eq!(reopened.instance(&record.id).await.unwrap(), detail);
    assert!(matches!(
        reopened.instance("missing").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn a_new_instance_defaults_to_the_newest_release_and_recommended_loader() {
    let world = world();
    publish_release(&world);
    world.net.answer(
        "meta.fabricmc.net/v2/versions/loader/1.0",
        r#"[{"loader":{"version":"0.17.0-beta","stable":false}},
            {"loader":{"version":"0.16.0","stable":true}}]"#,
    );
    let vanilla = world
        .service
        .create_instance("Plain", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert_eq!(vanilla.game_version, "1.0");
    assert_eq!(vanilla.loader_version, None);
    let fabric = world
        .service
        .create_instance("Modded", None, Loader::Fabric, None)
        .await
        .unwrap();
    assert_eq!(fabric.loader_version.as_deref(), Some("0.16.0"));
    assert_eq!(world.service.library().await.instances.len(), 2);

    // A chosen Forge build is recorded as chosen; installing it is the
    // launcher's job (its installer runs then).
    let forge = world
        .service
        .create_instance("Forge", Some("1.0"), Loader::Forge, Some("1.0.0"))
        .await
        .unwrap();
    assert_eq!(forge.loader, Loader::Forge);
    assert_eq!(forge.loader_version.as_deref(), Some("1.0.0"));
    assert_eq!(world.service.library().await.instances.len(), 3);
}

#[tokio::test]
async fn an_unreachable_catalog_creates_nothing() {
    let world = world();
    let result = world
        .service
        .create_instance("X", None, Loader::Vanilla, None)
        .await;
    assert!(matches!(result, Err(ServiceError::Remote(_))));
    assert!(world.service.library().await.instances.is_empty());
}

#[tokio::test]
async fn creating_does_not_install_and_bad_requests_publish_nothing() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Only", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert!(!record.installed);
    assert!(!world.service.layout.versions().exists());
    assert!(world.service.activity(10).finished.is_empty());
    // With no catalog, "newest" cannot be resolved: no guessed version.
    world.net.forget_all();
    assert!(matches!(
        world
            .service
            .create_instance("Guess", None, Loader::Vanilla, None)
            .await,
        Err(ServiceError::Remote(_))
    ));
    assert_eq!(world.service.library().await.instances.len(), 1);
    // The same name again gets its own id and never touches the first profile.
    let again = world
        .service
        .create_instance("Only", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    assert_ne!(again.id, record.id);
}

#[tokio::test]
async fn favorites_toggle_and_persist() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Fav", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert!(world.service.toggle_favorite(&record.id).await.unwrap());
    assert!(world.service.library().await.instances[0].favorite);
    assert!(!world.service.toggle_favorite(&record.id).await.unwrap());
    assert!(matches!(
        world.service.toggle_favorite("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn deleting_an_instance_removes_its_profile_and_keeps_shared_files() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let a = world
        .service
        .create_instance("A", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let b = world
        .service
        .create_instance("B", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .launch(&a.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert!(world.service.layout().game(&a.id).is_dir());

    world.service.delete_instance(&a.id).await.unwrap();
    assert!(!world.service.layout().profile(&a.id).exists());
    assert!(
        world
            .service
            .layout()
            .versions()
            .join("1.0/1.0.jar")
            .is_file()
    );
    assert_eq!(world.service.library().await.instances.len(), 1);

    // B never installed anything itself, and launches from the shared files.
    world.net.forget_all();
    let (tx, mut rx) = mpsc::unbounded_channel();
    world
        .service
        .launch(&b.id, tx, CancellationToken::new())
        .await
        .unwrap();
    let mut downloaded = false;
    while let Ok(update) = rx.try_recv() {
        downloaded |= matches!(
            update,
            LaunchUpdate::Signal(LaunchSignal::Phase(
                crate::launch_session::LaunchPhase::Libraries
            ))
        );
    }
    assert!(!downloaded);
}
