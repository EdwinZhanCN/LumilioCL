use super::super::error::ServiceError;
use super::super::types::ContentEffect;
use super::{
    change_subjects, fake_java, instance_with_profile, mod_version, publish_release, sha1_hex,
    world,
};
use crate::activity::CancellationToken;
use crate::activity_log::TaskOutcome;
use crate::content::ContentError;
use crate::discover::ProjectKind;
use crate::history::{ChangeKind, HistoryLog};
use crate::instance::{Loader, NewInstance};
use crate::launch_session::LaunchSignal;
use crate::launcher::LaunchUpdate;
use tokio::sync::mpsc;

#[tokio::test]
async fn switching_versions_replaces_the_file_keeps_it_disabled_and_never_clobbers() {
    let world = world();
    let record = world
        .service
        .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("cool-0.9.jar.disabled"), b"old build").unwrap();
    // Another file already called like the new version is the user's.
    std::fs::write(mods.join("cool.jar"), b"hand made").unwrap();
    let switch = || {
        world.service.switch_content_version(
            &record.id,
            ProjectKind::Mod,
            "cool-0.9.jar.disabled",
            "cool",
            "v1",
            CancellationToken::new(),
        )
    };
    assert!(matches!(
        switch().await,
        Err(ServiceError::Content(ContentError::Conflict(_)))
    ));
    assert_eq!(
        std::fs::read(mods.join("cool-0.9.jar.disabled")).unwrap(),
        b"old build"
    );
    assert_eq!(std::fs::read(mods.join("cool.jar")).unwrap(), b"hand made");

    std::fs::remove_file(mods.join("cool.jar")).unwrap();
    switch().await.unwrap();
    assert!(
        !mods.join("cool-0.9.jar.disabled").exists(),
        "the old file went"
    );
    assert_eq!(
        std::fs::read(mods.join("cool.jar.disabled")).unwrap(),
        b"a mod",
        "the new version, still switched off"
    );
    assert!(
        change_subjects(&world, &record.id)
            .contains(&(ChangeKind::ContentUpdated, "cool.jar".to_owned()))
    );
    // An unknown version is refused.
    assert!(matches!(
        world
            .service
            .switch_content_version(
                &record.id,
                ProjectKind::Mod,
                "cool.jar.disabled",
                "cool",
                "nope",
                CancellationToken::new(),
            )
            .await,
        Err(ServiceError::NoCompatibleVersion)
    ));
}

#[tokio::test]
async fn install_never_replaces_a_same_named_file_with_other_content() {
    let world = world();
    let record = world
        .service
        .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let target = world.service.layout.game(&record.id).join("mods/cool.jar");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"my own build").unwrap();
    let install = || {
        world.service.install_content(
            &record.id,
            ProjectKind::Mod,
            "cool",
            CancellationToken::new(),
        )
    };
    assert!(matches!(
        install().await,
        Err(ServiceError::Content(ContentError::Conflict(_)))
    ));
    assert_eq!(std::fs::read(&target).unwrap(), b"my own build");
    assert!(change_subjects(&world, &record.id).is_empty());
    // Removing it on purpose clears the way.
    world
        .service
        .delete_content(&record.id, ProjectKind::Mod, &["cool.jar".to_owned()])
        .await
        .unwrap();
    install().await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
    // The same content again is simply there already.
    install().await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
}

#[tokio::test]
async fn content_changes_report_each_file_and_record_only_real_changes() {
    let world = world();
    let record = instance_with_profile(&world, "Mods").await;
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"a").unwrap();
    std::fs::write(mods.join("b.jar"), b"b").unwrap();
    std::fs::write(mods.join("b.jar.disabled"), b"twin").unwrap();
    let names = |list: &[&str]| list.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();

    let results = world
        .service
        .set_content_state(
            &record.id,
            ProjectKind::Mod,
            &names(&["a.jar", "b.jar.disabled", "ghost.jar", "../x.jar"]),
            true,
        )
        .await
        .unwrap();
    // a is already enabled; b.jar.disabled collides with b.jar; two are refused.
    assert_eq!(
        results[0].outcome.as_ref().unwrap(),
        &ContentEffect::Unchanged
    );
    assert!(matches!(results[1].outcome, Err(ContentError::Conflict(_))));
    assert!(matches!(results[2].outcome, Err(ContentError::NotFound(_))));
    assert!(matches!(
        results[3].outcome,
        Err(ContentError::UnsafeFileName(_))
    ));
    assert!(change_subjects(&world, &record.id).is_empty());

    let results = world
        .service
        .set_content_state(&record.id, ProjectKind::Mod, &names(&["a.jar"]), false)
        .await
        .unwrap();
    assert_eq!(
        results[0].outcome.as_ref().unwrap(),
        &ContentEffect::Renamed("a.jar.disabled".into())
    );
    assert!(results[0].recorded);
    let listed = world
        .service
        .content(&record.id, ProjectKind::Mod)
        .await
        .unwrap();
    assert!(
        listed
            .iter()
            .any(|item| item.display_name == "a.jar" && !item.enabled)
    );

    let results = world
        .service
        .delete_content(
            &record.id,
            ProjectKind::Mod,
            &names(&["b.jar.disabled", "b.jar"]),
        )
        .await
        .unwrap();
    assert!(results.iter().all(|result| result.outcome.is_ok()));
    assert!(!mods.join("b.jar").exists() && !mods.join("b.jar.disabled").exists());
    assert_eq!(
        change_subjects(&world, &record.id),
        [
            (ChangeKind::ContentRemoved, "b.jar".to_owned()),
            (ChangeKind::ContentRemoved, "b.jar.disabled".to_owned()),
            (ChangeKind::ContentDisabled, "a.jar".to_owned()),
        ]
    );
    // Other instances and unknown ones are not reachable through this call.
    assert!(matches!(
        world.service.content("ghost", ProjectKind::Mod).await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    assert!(matches!(
        world
            .service
            .set_content_state(&record.id, ProjectKind::Modpack, &names(&["a"]), true)
            .await
            .unwrap()[0]
            .outcome,
        Err(ContentError::NotAFileKind)
    ));
}

#[tokio::test]
async fn content_writes_wait_for_a_launching_instance_but_reads_do_not() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"; sleep 30");
    let record = world
        .service
        .create_instance("Busy", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"a").unwrap();
    let stop = CancellationToken::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let launch = world.service.launch(&record.id, tx, stop.clone());
    let probe = async {
        while let Some(update) = rx.recv().await {
            if matches!(update, LaunchUpdate::Signal(LaunchSignal::Running)) {
                break;
            }
        }
        let refused = world
            .service
            .set_content_state(&record.id, ProjectKind::Mod, &["a.jar".to_owned()], false)
            .await;
        assert!(matches!(refused, Err(ServiceError::InstanceBusy(_))));
        assert!(matches!(
            world
                .service
                .delete_content(&record.id, ProjectKind::Mod, &["a.jar".to_owned()])
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert_eq!(
            world
                .service
                .content(&record.id, ProjectKind::Mod)
                .await
                .unwrap()
                .len(),
            1
        );
        stop.cancel();
        while rx.recv().await.is_some() {}
    };
    let (exit, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        tokio::join!(launch, probe)
    })
    .await
    .unwrap();
    exit.unwrap();
    assert!(mods.join("a.jar").exists());
    // Once the game is gone the same change goes through.
    world
        .service
        .set_content_state(&record.id, ProjectKind::Mod, &["a.jar".to_owned()], false)
        .await
        .unwrap();
    assert!(mods.join("a.jar.disabled").exists());
}

#[tokio::test]
async fn installed_projects_list_the_file_and_the_newer_version_if_any() {
    let world = world();
    let record = world
        .service
        .create_instance("Have", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("libx.jar"), b"libx").unwrap();
    std::fs::write(mods.join("mine.jar"), b"mine").unwrap();
    let version = |id: &str, hash: &str| {
        format!(
            r#"{{"id":"{id}","project_id":"LIBX","name":"x","version_number":"1",
                "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",
                "files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                           "primary":true,"size":4,"hashes":{{"sha1":"{hash}"}}}}],
                "dependencies":[]}}"#
        )
    };
    let known = sha1_hex(b"libx");
    // More specific address first: answers are matched by containment.
    world.net.answer(
        "/v2/version_files/update",
        format!(r#"{{"{known}":{}}}"#, version("v-new", "newhash")),
    );
    world.net.answer(
        "/v2/version_files",
        format!(r#"{{"{known}":{}}}"#, version("v-old", &known)),
    );

    let have = world
        .service
        .installed_projects(&record.id, ProjectKind::Mod)
        .await
        .unwrap();
    assert_eq!(have.len(), 1, "a file Modrinth does not know is not listed");
    let libx = &have["LIBX"];
    assert_eq!(libx.file_name, "libx.jar");
    assert_eq!(libx.version_id, "v-old");
    assert_eq!(libx.update.as_deref(), Some("v-new"));

    // Modrinth out of reach is an error, not "nothing installed".
    world.net.forget_all();
    assert!(
        world
            .service
            .installed_projects(&record.id, ProjectKind::Mod)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn installed_content_lands_in_the_profile_and_is_logged() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Mods".to_owned(),
                game_version: "1.0".to_owned(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.0".to_owned()),
            },
            1,
        )
        .unwrap()
        .clone();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));

    let name = world
        .service
        .install_content(
            &record.id,
            ProjectKind::Mod,
            "cool",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(name, "cool.jar");
    let installed = world
        .service
        .layout()
        .game(&record.id)
        .join("mods/cool.jar");
    assert_eq!(std::fs::read(installed).unwrap(), b"a mod");
    let changes = HistoryLog::for_instance(world.service.layout().root(), &record.id)
        .changes()
        .unwrap();
    assert_eq!(changes.len(), 1);

    let activity = world.service.activity(10);
    assert!(activity.active.is_empty());
    assert_eq!(activity.finished.len(), 1);
    assert_eq!(activity.finished[0].outcome, TaskOutcome::Succeeded);
}

#[tokio::test]
async fn content_for_another_game_version_is_refused_and_the_failure_is_logged() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Old".to_owned(),
                game_version: "1.7".to_owned(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.0".to_owned()),
            },
            1,
        )
        .unwrap()
        .clone();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let result = world
        .service
        .install_content(
            &record.id,
            ProjectKind::Mod,
            "cool",
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(result, Err(ServiceError::NoCompatibleVersion)));
    assert!(
        !world
            .service
            .layout()
            .game(&record.id)
            .join("mods")
            .exists()
    );
    let activity = world.service.activity(10);
    assert!(matches!(
        activity.finished[0].outcome,
        TaskOutcome::Failed(_)
    ));
}
