use super::super::error::ServiceError;
use super::{
    World, instance_with_profile, no_leftovers, pack_file, pack_index, sha1_hex, world,
    write_mrpack,
};
use crate::activity::CancellationToken;
use crate::activity_log::TaskOutcome;
use crate::history::{ChangeKind, HistoryEvent, HistoryLog};
use crate::instance::{InstanceSettings, Loader, StoreError};
use std::sync::Arc;

#[tokio::test]
async fn an_exported_pack_carries_everything_when_modrinth_is_out_of_reach_and_needs_the_lease() {
    let world = world();
    let record = world
        .service
        .create_instance("Pack", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:70").unwrap();
    let spec = crate::pack_export::ExportSpec {
        format: crate::pack_export::PackFormat::Modrinth,
        name: "Pack".into(),
        version: "1".into(),
        summary: None,
        include: vec!["mods".into(), "options.txt".into()],
    };
    let pack = world._dir.path().join("pack.mrpack");

    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world
                .service
                .export_modpack(&record.id, spec.clone(), &pack, CancellationToken::new())
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(!pack.exists());
    let report = world
        .service
        .export_modpack(&record.id, spec.clone(), &pack, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!((report.linked, report.bundled), (0, 2));
    assert!(report.lookup_failed && report.size > 0);
    let index = crate::modpack::read_index(&pack).unwrap();
    assert_eq!(
        (index.name.as_str(), index.minecraft.as_str()),
        ("Pack", "1.0")
    );
    assert!(index.files.is_empty());

    let mut nothing = spec;
    nothing.include = vec!["saves".into()];
    assert!(matches!(
        world
            .service
            .export_modpack(&record.id, nothing, &pack, CancellationToken::new())
            .await,
        Err(ServiceError::Export(
            crate::pack_export::ExportError::NothingChosen
        ))
    ));
}

#[tokio::test]
async fn importing_a_local_pack_builds_a_whole_instance_and_keeps_the_file() {
    let world = world();
    let other = instance_with_profile(&world, "Other").await;
    world
        .net
        .answer("cdn.modrinth.com/data/mods/a.jar", "mod-a");
    world
        .net
        .answer("cdn.modrinth.com/data/mods/server-only.jar", "never");
    let index = pack_index(&format!(
        "{},{}",
        pack_file("mods/a.jar", "mod-a", ""),
        pack_file(
            "mods/server-only.jar",
            "never",
            r#","env":{"client":"unsupported","server":"required"}"#
        ),
    ));
    let pack = world._dir.path().join("cool.mrpack");
    write_mrpack(
        &pack,
        &index,
        &[
            ("overrides/config/x.toml", "from-overrides"),
            ("overrides/options.txt", "base"),
            ("client-overrides/options.txt", "client-wins"),
            ("overrides/../escape.txt", "no"),
        ],
    );
    let record = world
        .service
        .import_modpack_file(&pack, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(record.name, "Cool Pack");
    let game = world.service.layout.game(&record.id);
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"mod-a");
    assert!(!game.join("mods/server-only.jar").exists());
    assert_eq!(
        std::fs::read(game.join("config/x.toml")).unwrap(),
        b"from-overrides"
    );
    assert_eq!(
        std::fs::read(game.join("options.txt")).unwrap(),
        b"client-wins"
    );
    assert!(!world.service.layout.profiles().join("escape.txt").exists());
    assert!(pack.is_file(), "a local pack is the user's file");
    no_leftovers(&world);
    // The other instance is exactly as it was.
    assert_eq!(
        std::fs::read(world.service.layout.game(&other.id).join("saves/level.dat")).unwrap(),
        b"world"
    );
    assert_eq!(world.service.library().await.instances.len(), 2);
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Succeeded
    );
}

#[tokio::test]
async fn the_library_stays_usable_while_a_pack_downloads() {
    let world = world();
    world
        .net
        .answer("cdn.modrinth.com/data/mods/a.jar", "mod-a");
    let pack = world._dir.path().join("slow.mrpack");
    write_mrpack(
        &pack,
        &pack_index(&pack_file("mods/a.jar", "mod-a", "")),
        &[],
    );
    let gate = Arc::new(tokio::sync::Notify::new());
    *world.net.gate.lock().unwrap() = Some(gate.clone());
    let import = world
        .service
        .import_modpack_file(&pack, CancellationToken::new());
    let probe = async {
        // Give the download time to start, then use the library.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let library =
            tokio::time::timeout(std::time::Duration::from_secs(2), world.service.library())
                .await
                .expect("the library must not wait for the download");
        assert!(library.instances.is_empty(), "nothing is published yet");
        assert_eq!(world.service.activity(10).active.len(), 1);
        gate.notify_waiters();
    };
    let (record, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(import, probe)
    })
    .await
    .unwrap();
    record.unwrap();
    assert_eq!(world.service.library().await.instances.len(), 1);
}

#[tokio::test]
async fn bad_packs_and_failed_downloads_leave_nothing_and_touch_no_instance() {
    let world = world();
    let other = instance_with_profile(&world, "Keep").await;
    let dir = world._dir.path().to_path_buf();
    let assert_untouched = |world: &World| {
        let library = std::fs::read_dir(world.service.layout.profiles())
            .unwrap()
            .count();
        assert_eq!(library, 1, "only the existing profile is on disk");
        no_leftovers(world);
    };

    // Not an archive at all.
    let junk = dir.join("junk.mrpack");
    std::fs::write(&junk, b"not a zip").unwrap();
    assert!(
        world
            .service
            .import_modpack_file(&junk, CancellationToken::new())
            .await
            .is_err()
    );
    // A path that climbs out of the game folder.
    let climbing = dir.join("climb.mrpack");
    write_mrpack(
        &climbing,
        &pack_index(&pack_file("../evil.jar", "x", "")),
        &[],
    );
    assert!(
        world
            .service
            .import_modpack_file(&climbing, CancellationToken::new())
            .await
            .is_err()
    );
    // Only an untrusted host to download from.
    let untrusted = dir.join("untrusted.mrpack");
    let index = pack_index(&format!(
        r#"{{"path":"mods/a.jar","hashes":{{"sha1":"{}"}},"fileSize":1,"downloads":["https://evil.example/a.jar"]}}"#,
        sha1_hex(b"a")
    ));
    write_mrpack(&untrusted, &index, &[("overrides/config/x.toml", "x")]);
    assert!(matches!(
        world
            .service
            .import_modpack_file(&untrusted, CancellationToken::new())
            .await,
        Err(ServiceError::Install(_))
    ));
    // A download whose content does not match the pack's hash.
    world
        .net
        .answer("cdn.modrinth.com/data/mods/a.jar", "TAMPERED");
    let tampered = dir.join("tampered.mrpack");
    write_mrpack(
        &tampered,
        &pack_index(&pack_file("mods/a.jar", "mod-a", "")),
        &[("overrides/config/x.toml", "x")],
    );
    assert!(
        world
            .service
            .import_modpack_file(&tampered, CancellationToken::new())
            .await
            .is_err()
    );
    // Not a file (a folder).
    assert!(
        world
            .service
            .import_modpack_file(&dir, CancellationToken::new())
            .await
            .is_err()
    );
    assert_untouched(&world);

    // Cancelled while the download is in flight.
    world
        .net
        .answer("cdn.modrinth.com/data/mods/b.jar", "mod-b");
    let slow = dir.join("slow.mrpack");
    write_mrpack(
        &slow,
        &pack_index(&pack_file("mods/b.jar", "mod-b", "")),
        &[("overrides/config/x.toml", "x")],
    );
    *world.net.gate.lock().unwrap() = Some(Arc::new(tokio::sync::Notify::new()));
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(world.service.import_modpack_file(&slow, cancel), async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            stop.cancel();
        })
    })
    .await
    .expect("cancelling must not wait for the download");
    assert!(matches!(result, Err(ServiceError::Cancelled)));
    *world.net.gate.lock().unwrap() = None;
    assert_untouched(&world);
    assert_eq!(world.service.library().await.instances.len(), 1);
    assert_eq!(
        std::fs::read(world.service.layout.game(&other.id).join("saves/level.dat")).unwrap(),
        b"world"
    );
    assert!(
        world
            .service
            .activity(20)
            .finished
            .iter()
            .any(|task| task.outcome == TaskOutcome::Cancelled)
    );
}

#[tokio::test]
async fn a_copy_is_independent_inherits_settings_and_leaves_the_source_alone() {
    let world = world();
    let source = instance_with_profile(&world, "Source").await;
    world.service.toggle_favorite(&source.id).await.unwrap();
    world
        .service
        .update_instance_settings(
            &source.id,
            InstanceSettings {
                max_memory_mb: Some(4096),
                ..InstanceSettings::default()
            },
        )
        .await
        .unwrap();
    world
        .service
        .store
        .lock()
        .await
        .mark_installed(&source.id, true)
        .unwrap();
    let game = world.service.layout.game(&source.id);
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(game.join("logs/latest.log"), b"log").unwrap();
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
    let history = HistoryLog::for_instance(world.service.layout.root(), &source.id);
    history
        .append(&HistoryEvent::Change {
            at: 1,
            kind: ChangeKind::ContentAdded,
            subject: "a.jar".into(),
        })
        .unwrap();
    let before = std::fs::read(game.join("saves/level.dat")).unwrap();

    let with = world
        .service
        .copy_instance(&source.id, "Source", true, CancellationToken::new())
        .await
        .unwrap();
    let without = world
        .service
        .copy_instance(&source.id, "Source", false, CancellationToken::new())
        .await
        .unwrap();
    assert_ne!(with.id, source.id);
    assert_ne!(with.id, without.id, "the same name never reuses an id");
    // Inherits what makes it the same game; starts clean otherwise.
    assert!(with.installed && with.loader == source.loader);
    assert_eq!(with.settings.max_memory_mb, Some(4096));
    assert!(!with.favorite && with.last_played.is_none() && with.play_seconds == 0);
    // Files: worlds only when asked, volatile output never, history not copied.
    let copy = world.service.layout.game(&with.id);
    assert_eq!(std::fs::read(copy.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(std::fs::read(copy.join("saves/level.dat")).unwrap(), before);
    assert!(!copy.join("logs").exists());
    let bare = world.service.layout.game(&without.id);
    assert!(bare.join("mods/a.jar").is_file() && !bare.join("saves").exists());
    assert!(
        HistoryLog::for_instance(world.service.layout.root(), &with.id)
            .read()
            .unwrap()
            .events
            .is_empty()
    );
    // Independent: changing the copy leaves the source as it was.
    std::fs::write(copy.join("mods/a.jar"), b"changed").unwrap();
    std::fs::write(copy.join("saves/level.dat"), b"changed").unwrap();
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(std::fs::read(game.join("saves/level.dat")).unwrap(), before);
    no_leftovers(&world);
    assert_eq!(world.service.library().await.instances.len(), 3);
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Succeeded
    );
}

#[tokio::test]
async fn a_copy_that_is_refused_cancelled_or_fails_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let world = world();
    let source = instance_with_profile(&world, "Source").await;
    let game = world.service.layout.game(&source.id);
    std::fs::write(game.join("secret.txt"), b"s").unwrap();
    let tree_before = std::fs::read_dir(&game).unwrap().count();

    // Blank name: refused up front.
    assert!(matches!(
        world
            .service
            .copy_instance(&source.id, "  ", true, CancellationToken::new())
            .await,
        Err(ServiceError::Store(StoreError::InvalidName))
    ));
    // Unknown source.
    assert!(matches!(
        world
            .service
            .copy_instance("ghost", "X", true, CancellationToken::new())
            .await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    // Busy source (launching or running): no copy of a moving target.
    {
        let _busy = world.service.reserve_instance(&source.id).unwrap();
        assert!(matches!(
            world
                .service
                .copy_instance(&source.id, "X", true, CancellationToken::new())
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    // Cancelled before it starts.
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        world
            .service
            .copy_instance(&source.id, "X", true, cancel)
            .await,
        Err(ServiceError::Cancelled)
    ));
    // An unreadable file stops the copy part-way.
    std::fs::set_permissions(
        game.join("secret.txt"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let failed = world
        .service
        .copy_instance(&source.id, "X", true, CancellationToken::new())
        .await;
    std::fs::set_permissions(
        game.join("secret.txt"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(matches!(failed, Err(ServiceError::Io(_))));

    // Nothing was published, nothing is left behind, the source is as it was.
    assert_eq!(world.service.library().await.instances.len(), 1);
    assert_eq!(
        std::fs::read_dir(world.service.layout.profiles())
            .unwrap()
            .count(),
        1
    );
    no_leftovers(&world);
    assert_eq!(std::fs::read_dir(&game).unwrap().count(), tree_before);
    assert_eq!(std::fs::read(game.join("secret.txt")).unwrap(), b"s");
    // And a plain retry works.
    world
        .service
        .copy_instance(&source.id, "X", true, CancellationToken::new())
        .await
        .unwrap();
}
