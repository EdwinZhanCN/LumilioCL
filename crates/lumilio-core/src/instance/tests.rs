use super::*;

fn request(name: &str) -> NewInstance {
    NewInstance {
        name: name.to_owned(),
        game_version: "1.21.1".to_owned(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.0".to_owned()),
    }
}

#[test]
fn collections_are_renamed_and_assigned_without_losing_order_or_members() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    store.create(request("One"), 1).unwrap();
    store.create(request("Two"), 2).unwrap();
    for name in ["A", "B", "C"] {
        store.create_collection(name).unwrap();
    }
    store.set_membership("B", "one", true).unwrap();
    store.set_membership("B", "two", true).unwrap();

    store.rename_collection("B", "  Builds ").unwrap();
    let names: Vec<_> = store
        .collections()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["A", "Builds", "C"], "the place is kept");
    assert_eq!(store.collections()[1].members, ["one", "two"]);
    assert!(matches!(
        store.rename_collection("A", "Builds"),
        Err(StoreError::DuplicateCollection(_))
    ));
    assert!(matches!(
        store.rename_collection("A", "   "),
        Err(StoreError::InvalidName)
    ));
    assert!(matches!(
        store.rename_collection("ghost", "X"),
        Err(StoreError::UnknownCollection(_))
    ));
    store.rename_collection("A", "A").unwrap();

    store
        .set_collections_of("one", &["A".to_owned(), "C".to_owned()])
        .unwrap();
    let members = |store: &InstanceStore, name: &str| {
        store
            .collections()
            .iter()
            .find(|c| c.name == name)
            .unwrap()
            .members
            .clone()
    };
    assert_eq!(members(&store, "A"), ["one"]);
    assert_eq!(members(&store, "Builds"), ["two"]);
    assert_eq!(members(&store, "C"), ["one"]);
    assert!(matches!(
        store.set_collections_of("one", &["nope".to_owned()]),
        Err(StoreError::UnknownCollection(_))
    ));
    assert_eq!(members(&store, "A"), ["one"], "a refusal changes nothing");
    assert!(matches!(
        store.set_collections_of("ghost", &[]),
        Err(StoreError::UnknownInstance(_))
    ));
    let reopened = InstanceStore::open(dir.path()).unwrap();
    assert_eq!(reopened.collections(), store.collections());
}

#[test]
fn failed_library_writes_preserve_memory_disk_and_allow_retry() {
    for operation in 0..13 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = InstanceStore::open(dir.path()).unwrap();
        store.create(request("Original"), 1).unwrap();
        store.create_collection("Keep").unwrap();
        store.set_membership("Keep", "original", true).unwrap();
        let records = store.instances().to_vec();
        let collections = store.collections().to_vec();
        let mutate = |store: &mut InstanceStore| -> Result<(), StoreError> {
            match operation {
                0 => store.create(request("New"), 2).map(|_| ()),
                1 => store.rename("original", "Changed"),
                2 => store.set_favorite("original", true),
                3 => store.mark_installed("original", true),
                4 => store.update_settings(
                    "original",
                    InstanceSettings {
                        max_memory_mb: Some(4096),
                        ..InstanceSettings::default()
                    },
                ),
                5 => store.record_session("original", 10, 60),
                6 => store.remove("original").map(|_| ()),
                7 => store.create_collection("New"),
                8 => store.delete_collection("Keep"),
                9 => store.set_membership("Keep", "original", false),
                10 => store.rename_collection("Keep", "Renamed"),
                11 => store.set_collections_of("original", &[]),
                _ => store.set_runtime("original", "1.21.4", Loader::NeoForge, Some("21.4.1")),
            }
        };
        store.db.pragma_update(None, "query_only", true).unwrap();
        assert!(mutate(&mut store).is_err());
        assert_eq!(store.instances(), records, "operation {operation}");
        assert_eq!(store.collections(), collections, "operation {operation}");
        let reopened = InstanceStore::open(dir.path()).unwrap();
        assert_eq!(reopened.instances(), records);
        assert_eq!(reopened.collections(), collections);
        drop(reopened);
        store.db.pragma_update(None, "query_only", false).unwrap();
        mutate(&mut store).unwrap();
        let reopened = InstanceStore::open(dir.path()).unwrap();
        assert_eq!(reopened.instances(), store.instances());
        assert_eq!(reopened.collections(), store.collections());
    }
}

#[test]
fn a_schema_1_library_upgrades_in_place_and_keeps_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("launcher.db");
    {
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE instances (
                position INTEGER NOT NULL, id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL, game_version TEXT NOT NULL, loader TEXT NOT NULL,
                loader_version TEXT, favorite INTEGER NOT NULL, created_at INTEGER NOT NULL,
                last_played INTEGER, play_seconds INTEGER NOT NULL, installed INTEGER NOT NULL,
                java_path TEXT, max_memory_mb INTEGER, min_memory_mb INTEGER,
                jvm_arguments TEXT NOT NULL);
             CREATE TABLE collections (position INTEGER NOT NULL, name TEXT PRIMARY KEY NOT NULL);
             CREATE TABLE collection_members (collection TEXT NOT NULL, position INTEGER NOT NULL,
                instance TEXT NOT NULL, PRIMARY KEY (collection, instance));
             INSERT INTO instances VALUES (0,'old','Old','1.21.1','fabric','0.16.0',1,5,NULL,90,1,
                NULL,4096,NULL,'[\"-Dold=1\"]');
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let mut store = InstanceStore::open(dir.path()).unwrap();
    assert!(!store.recovered_from_damage());
    let old = store.get("old").unwrap().clone();
    assert_eq!(old.name, "Old");
    assert_eq!(old.settings.max_memory_mb, Some(4096));
    assert_eq!(old.settings.jvm_arguments, ["-Dold=1"]);
    assert!(old.settings.launch.is_default());
    assert!(
        dir.path().join("launcher.db.v1").is_file(),
        "a copy of the old file"
    );

    // The new column is usable and survives a reopen.
    let mut settings = old.settings.clone();
    settings.launch.fullscreen = Some(true);
    store.update_settings("old", settings).unwrap();
    let reopened = InstanceStore::open(dir.path()).unwrap();
    assert_eq!(
        reopened.get("old").unwrap().settings.launch.fullscreen,
        Some(true)
    );
    assert_eq!(reopened.get("old").unwrap().name, "Old");
}

#[test]
fn a_schema_2_library_gains_the_source_column_and_a_modpack_remembers_its_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("launcher.db");
    {
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE instances (
                position INTEGER NOT NULL, id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL, game_version TEXT NOT NULL, loader TEXT NOT NULL,
                loader_version TEXT, favorite INTEGER NOT NULL, created_at INTEGER NOT NULL,
                last_played INTEGER, play_seconds INTEGER NOT NULL, installed INTEGER NOT NULL,
                java_path TEXT, max_memory_mb INTEGER, min_memory_mb INTEGER,
                jvm_arguments TEXT NOT NULL, tuning TEXT);
             CREATE TABLE collections (position INTEGER NOT NULL, name TEXT PRIMARY KEY NOT NULL);
             CREATE TABLE collection_members (collection TEXT NOT NULL, position INTEGER NOT NULL,
                instance TEXT NOT NULL, PRIMARY KEY (collection, instance));
             INSERT INTO instances VALUES (0,'old','Old','1.21.1','fabric','0.16.0',1,5,NULL,90,1,
                NULL,4096,NULL,'[]',NULL);
             PRAGMA user_version = 2;",
        )
        .unwrap();
    }
    let mut store = InstanceStore::open(dir.path()).unwrap();
    assert_eq!(store.get("old").unwrap().source_project, None);
    assert!(
        dir.path().join("launcher.db.v2").is_file(),
        "a copy of the old file"
    );
    store.set_source_project("old", "1KVo5zza").unwrap();
    let reopened = InstanceStore::open(dir.path()).unwrap();
    assert_eq!(
        reopened.get("old").unwrap().source_project.as_deref(),
        Some("1KVo5zza")
    );
}

#[test]
fn launch_overrides_persist_are_normalized_and_bad_ones_are_refused() {
    use crate::tuning::{AfterLaunch, QuickPlay};
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    store.create(request("Mine"), 1).unwrap();
    let settings = InstanceSettings {
        launch: InstanceLaunch {
            window_width: Some(800),
            window_height: Some(600),
            game_arguments: Some(vec![" --demo ".into(), "".into()]),
            pre_launch: Some("  ".into()),
            after_launch: Some(AfterLaunch::Hide),
            quick_play: Some(QuickPlay::Server(" mc.example.com:25565 ".into())),
            ..InstanceLaunch::default()
        },
        ..InstanceSettings::default()
    };
    store.update_settings("mine", settings).unwrap();
    let reopened = InstanceStore::open(dir.path()).unwrap();
    let launch = &reopened.get("mine").unwrap().settings.launch;
    assert_eq!(
        launch.game_arguments.as_deref(),
        Some(&["--demo".to_owned()][..])
    );
    assert_eq!(launch.pre_launch.as_deref(), Some(""), "none on purpose");
    assert_eq!(launch.after_launch, Some(AfterLaunch::Hide));
    assert_eq!(
        launch.quick_play,
        Some(QuickPlay::Server("mc.example.com:25565".into()))
    );

    let before = store.instances().to_vec();
    for bad in [
        InstanceLaunch {
            window_width: Some(800),
            ..InstanceLaunch::default()
        },
        InstanceLaunch {
            quick_play: Some(QuickPlay::World("../x".into())),
            ..InstanceLaunch::default()
        },
    ] {
        let result = store.update_settings(
            "mine",
            InstanceSettings {
                launch: bad,
                ..InstanceSettings::default()
            },
        );
        assert!(matches!(result, Err(StoreError::InvalidLaunch(_))));
    }
    assert_eq!(
        store.instances(),
        before,
        "a refused change changes nothing"
    );
}

#[test]
fn transaction_and_commit_failures_preserve_the_last_saved_library() {
    for commit_failure in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = InstanceStore::open(dir.path()).unwrap();
        store.create(request("Original"), 1).unwrap();
        let records = store.instances().to_vec();
        if commit_failure {
            store
                .db
                .execute_batch(
                    "PRAGMA foreign_keys = ON;
                 CREATE TABLE guard_parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE guard_child (id INTEGER REFERENCES guard_parent(id)
                     DEFERRABLE INITIALLY DEFERRED);
                 CREATE TEMP TRIGGER reject_write AFTER INSERT ON instances
                     BEGIN INSERT INTO guard_child VALUES (1); END;",
                )
                .unwrap();
        } else {
            store
                .db
                .execute_batch(
                    "CREATE TEMP TRIGGER reject_write AFTER INSERT ON instances
                     BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
                )
                .unwrap();
        }
        assert!(store.rename("original", "Changed").is_err());
        assert_eq!(store.instances(), records);
        assert_eq!(
            InstanceStore::open(dir.path()).unwrap().instances(),
            records
        );
        store
            .db
            .execute_batch("DROP TRIGGER reject_write;")
            .unwrap();
        store.rename("original", "Changed").unwrap();
        assert_eq!(
            InstanceStore::open(dir.path())
                .unwrap()
                .get("original")
                .unwrap()
                .name,
            "Changed"
        );
    }
}

#[test]
fn memory_validation_checks_overrides_against_inherited_defaults() {
    let mut settings = InstanceSettings {
        min_memory_mb: Some(4096),
        ..InstanceSettings::default()
    };
    assert!(settings.validate_memory(None, Some(2048)).is_err());
    settings.max_memory_mb = Some(8192);
    settings.validate_memory(None, Some(2048)).unwrap();
    settings.min_memory_mb = None;
    settings.max_memory_mb = Some(128);
    assert!(settings.validate_memory(Some(512), None).is_err());
    settings.max_memory_mb = None;
    settings.validate_memory(Some(512), Some(2048)).unwrap();
}

#[test]
fn invalid_instance_memory_is_rejected_without_changing_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    store.create(request("Original"), 1).unwrap();
    let before = store.instances().to_vec();
    for (min, max) in [
        (Some(0), None),
        (None, Some(crate::MAX_MEMORY_MB + 1)),
        (Some(4096), Some(1024)),
    ] {
        assert!(
            store
                .update_settings(
                    "original",
                    InstanceSettings {
                        min_memory_mb: min,
                        max_memory_mb: max,
                        ..InstanceSettings::default()
                    }
                )
                .is_err()
        );
        assert_eq!(store.instances(), before);
        assert_eq!(InstanceStore::open(dir.path()).unwrap().instances(), before);
    }
}

#[test]
fn creates_persists_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    let id = store
        .create(request("生存 Season 3"), 100)
        .unwrap()
        .id
        .clone();
    assert_eq!(id, "season-3");
    store.set_favorite(&id, true).unwrap();
    store.record_session(&id, 500, 60).unwrap();
    store.record_session(&id, 900, 40).unwrap();

    let reopened = InstanceStore::open(dir.path()).unwrap();
    let record = reopened.get(&id).unwrap();
    assert!(record.favorite);
    assert_eq!(record.loader, Loader::Fabric);
    assert_eq!(record.last_played, Some(900));
    assert_eq!(record.play_seconds, 100);
    assert!(!reopened.recovered_from_damage());
}

#[test]
fn ids_are_unique_and_never_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    let a = store.create(request("世界"), 1).unwrap().id.clone();
    let b = store.create(request("世界"), 2).unwrap().id.clone();
    let c = store.create(request("Same"), 3).unwrap().id.clone();
    let d = store.create(request("same"), 4).unwrap().id.clone();
    assert_eq!((a.as_str(), b.as_str()), ("instance", "instance-2"));
    assert_eq!((c.as_str(), d.as_str()), ("same", "same-2"));
}

#[test]
fn a_new_instance_never_adopts_a_profile_folder_already_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    let orphan = Layout::new(dir.path()).game("orphan");
    fs::create_dir_all(orphan.join("saves")).unwrap();
    fs::write(orphan.join("saves/level.dat"), b"precious").unwrap();
    let created = store.create(request("Orphan"), 1).unwrap().id.clone();
    assert_eq!(created, "orphan-2");
    assert_eq!(
        fs::read(orphan.join("saves/level.dat")).unwrap(),
        b"precious"
    );
}

#[test]
fn create_as_refuses_taken_ids_and_carries_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    let id = store.suggest_id("Copy").unwrap();
    assert_eq!(id, "copy");
    // The id is claimed by someone else while work was being staged.
    store.create(request("Copy"), 1).unwrap();
    assert!(matches!(
        store.create_as(&id, request("Copy"), InstanceSettings::default(), false, 2),
        Err(StoreError::IdTaken(_))
    ));
    // ... or a profile folder appeared for it.
    let free = store.suggest_id("Other").unwrap();
    fs::create_dir_all(Layout::new(dir.path()).game(&free)).unwrap();
    assert!(matches!(
        store.create_as(
            &free,
            request("Other"),
            InstanceSettings::default(),
            false,
            2
        ),
        Err(StoreError::IdTaken(_))
    ));
    // Settings and the installed flag go in with the record, atomically.
    let settings = InstanceSettings {
        max_memory_mb: Some(4096),
        ..InstanceSettings::default()
    };
    let made = store
        .create_as("fresh", request("Fresh"), settings.clone(), true, 3)
        .unwrap()
        .clone();
    assert!(made.installed && made.settings == settings && !made.favorite);
    assert_eq!(
        InstanceStore::open(dir.path()).unwrap().get("fresh"),
        Some(&made)
    );
    assert!(matches!(
        store.suggest_id("  "),
        Err(StoreError::InvalidName)
    ));
}

#[test]
fn rejects_blank_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    assert!(matches!(
        store.create(request("   "), 1),
        Err(StoreError::InvalidName)
    ));
    assert!(store.instances().is_empty());
}

#[test]
fn recent_orders_by_last_played_then_creation() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    for (name, at) in [("a", 1), ("b", 2), ("c", 3)] {
        store.create(request(name), at).unwrap();
    }
    store.record_session("a", 50, 1).unwrap();
    let order: Vec<_> = store.recent().iter().map(|r| r.id.as_str()).collect();
    assert_eq!(order, ["a", "c", "b"]);
}

#[test]
fn collections_track_membership_and_forget_removed_instances() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    store.create(request("a"), 1).unwrap();
    store.create(request("b"), 2).unwrap();
    store.create_collection("Redstone").unwrap();
    assert!(matches!(
        store.create_collection("Redstone"),
        Err(StoreError::DuplicateCollection(_))
    ));
    store.set_membership("Redstone", "a", true).unwrap();
    store.set_membership("Redstone", "a", true).unwrap();
    store.set_membership("Redstone", "b", true).unwrap();
    assert_eq!(store.collections()[0].members, ["a", "b"]);
    assert!(matches!(
        store.set_membership("Redstone", "ghost", true),
        Err(StoreError::UnknownInstance(_))
    ));
    store.remove("a").unwrap();
    assert_eq!(store.collections()[0].members, ["b"]);
    store.delete_collection("Redstone").unwrap();
    assert!(store.collections().is_empty());
}

#[test]
fn a_damaged_database_is_preserved_and_the_library_starts_empty() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("launcher.db"),
        "this is not sqlite at all, really",
    )
    .unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    assert!(store.recovered_from_damage());
    assert!(store.instances().is_empty());
    assert_eq!(
        fs::read_to_string(dir.path().join("launcher.db.broken")).unwrap(),
        "this is not sqlite at all, really"
    );
    store.create(request("fresh"), 1).unwrap();
    let reopened = InstanceStore::open(dir.path()).unwrap();
    assert!(!reopened.recovered_from_damage());
    assert_eq!(reopened.instances().len(), 1);
}

#[test]
fn a_newer_schema_is_refused_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("launcher.db");
    {
        let db = Connection::open(&path).unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
    }
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        InstanceStore::open(dir.path()),
        Err(StoreError::NewerSchema(99))
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!dir.path().join("launcher.db.broken").exists());
}

#[test]
fn a_legacy_json_library_is_imported_once_and_set_aside() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = r#"{"schema":1,"instances":[{"id":"old","name":"Old","game_version":"1.20.1",
        "loader":"quilt","loader_version":"0.26.0","favorite":true,"created_at":5,
        "settings":{"max_memory_mb":4096,"jvm_arguments":["-Xss1m"]}}],
        "collections":[{"name":"Keep","members":["old"]}]}"#;
    fs::write(dir.path().join("library.json"), legacy).unwrap();
    let store = InstanceStore::open(dir.path()).unwrap();
    let record = store.get("old").unwrap();
    assert!(record.favorite);
    assert_eq!(record.loader, Loader::Quilt);
    assert_eq!(record.settings.max_memory_mb, Some(4096));
    assert_eq!(record.settings.jvm_arguments, ["-Xss1m"]);
    assert_eq!(store.collections()[0].members, ["old"]);
    assert!(!dir.path().join("library.json").exists());
    assert!(dir.path().join("library.json.imported").exists());

    let again = InstanceStore::open(dir.path()).unwrap();
    assert_eq!(again.instances().len(), 1);
}

#[test]
fn every_field_survives_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    store.create(request("Full"), 7).unwrap();
    let settings = InstanceSettings {
        java_path: Some(PathBuf::from("/opt/jdk/bin/java")),
        max_memory_mb: Some(6144),
        min_memory_mb: Some(1024),
        jvm_arguments: vec!["-XX:+UseZGC".to_owned(), "-Dx=y z".to_owned()],
        launch: InstanceLaunch {
            fullscreen: Some(true),
            game_arguments: Some(vec!["--demo".to_owned()]),
            wrapper: Some("nice".to_owned()),
            ..InstanceLaunch::default()
        },
    };
    store.update_settings("full", settings.clone()).unwrap();
    store.mark_installed("full", true).unwrap();
    let record = InstanceStore::open(dir.path())
        .unwrap()
        .get("full")
        .cloned()
        .unwrap();
    assert_eq!(record.settings, settings);
    assert!(record.installed);
    assert_eq!(record.loader_version.as_deref(), Some("0.16.0"));
    assert_eq!(record.created_at, 7);
}

#[test]
fn game_files_are_shared_per_release_and_instance_files_are_isolated() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(dir.path()).unwrap();
    let a = store.create(request("A"), 1).unwrap().clone();
    let b = store.create(request("B"), 2).unwrap().clone();
    let (da, db) = (store.directories(&a), store.directories(&b));
    assert_eq!(da.libraries(), dir.path().join("meta/libraries"));
    assert_eq!(da.versions(), db.versions());
    assert_eq!(da.assets(), db.assets());
    assert_eq!(da.natives(), db.natives());
    assert_eq!(
        da.natives(),
        dir.path().join("meta/natives/fabric-loader-0.16.0-1.21.1")
    );
    assert_eq!(da.game(), dir.path().join("profiles/a/game"));
    assert_ne!(da.game(), db.game());
}
