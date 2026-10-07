use super::super::error::ServiceError;
use super::{fake_java, launch_to_end, publish_java, publish_release, world};
use crate::activity::CancellationToken;
use crate::activity_log::{RetryAction, TaskOutcome};
use crate::instance::Loader;

#[tokio::test]
async fn java_metadata_falls_back_from_bad_index_and_manifest() {
    let world = world();
    publish_java(&world, false);
    world
        .net
        .answer("bad-index", b"<html>unavailable</html>".to_vec());
    let bad_manifest = world.server.join("bad-jmanifest.json");
    std::fs::write(&bad_manifest, r#"{"error":"unavailable"}"#).unwrap();
    world
        .service
        .set_mirrors(
            vec![
                crate::MirrorRule {
                    official_prefix: crate::java_runtime::INDEX_URL.to_owned(),
                    mirror_prefix: "https://mirror.test/bad-index".into(),
                },
                crate::MirrorRule {
                    official_prefix: super::file_url(&world.server.join("jmanifest.json")),
                    mirror_prefix: super::file_url(&bad_manifest),
                },
            ],
            true,
        )
        .await
        .unwrap();
    let java = world
        .service
        .install_java(Some(21), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(java.major(), 21);
    assert!(java.executable().is_file());
}

#[tokio::test]
async fn java_is_installed_whole_found_by_discovery_and_a_bad_file_leaves_nothing() {
    let world = world();
    publish_java(&world, true);
    let runtimes = world.service.layout.runtimes();
    assert!(
        world
            .service
            .install_java(Some(21), CancellationToken::new())
            .await
            .is_err()
    );
    assert!(!runtimes.join("java-runtime-delta").exists());
    let staging = runtimes.join(".java-runtime-delta.installing");
    assert!(!staging.exists(), "a failed install leaves no half runtime");
    let failed = world.service.activity(5).finished.remove(0);
    assert!(matches!(failed.outcome, TaskOutcome::Failed(_)));
    assert_eq!(
        failed.retry,
        Some(RetryAction::InstallJava { major: Some(21) })
    );

    publish_java(&world, false);
    let java = world
        .service
        .install_java(Some(21), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(java.major(), 21);
    assert!(java.executable().is_file());
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(java.executable())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "the launcher is executable");
    }
    assert!(runtimes.join("java-runtime-delta/bin/jre").is_symlink());
    // The ordinary discovery finds it, and asking again changes nothing.
    let found = world.service.java_installations().await;
    assert!(
        found
            .iter()
            .any(|(runtime, _)| runtime.home().ends_with("java-runtime-delta"))
    );
    let again = world
        .service
        .install_java(Some(21), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(again.home(), java.home());
    assert!(matches!(
        world
            .service
            .install_java(Some(99), CancellationToken::new())
            .await,
        Err(ServiceError::Install(_))
    ));
}

#[tokio::test]
async fn adding_a_java_by_its_executable_or_folder_remembers_the_folder() {
    let world = world();
    let elsewhere = world._dir.path().join("jdks/zulu-17");
    std::fs::create_dir_all(elsewhere.join("bin")).unwrap();
    std::fs::write(elsewhere.join("release"), "JAVA_VERSION=\"17.0.9\"\n").unwrap();
    std::fs::write(elsewhere.join("bin/java"), "#!/bin/sh\n").unwrap();
    assert!(world.service.java_installations().await.is_empty());

    let added = world
        .service
        .add_java(&elsewhere.join("bin/java"))
        .await
        .unwrap();
    assert_eq!(added.home(), elsewhere);
    assert_eq!(world.service.java_installations().await.len(), 1);
    assert_eq!(
        world.service.settings().await.extra_java_roots,
        std::slice::from_ref(&elsewhere)
    );
    // The same one again changes nothing; a folder of installations works too.
    world.service.add_java(&elsewhere).await.unwrap();
    assert_eq!(world.service.settings().await.extra_java_roots.len(), 1);
    world
        .service
        .add_java(elsewhere.parent().unwrap())
        .await
        .unwrap();

    let nothing = world._dir.path().join("empty");
    std::fs::create_dir_all(&nothing).unwrap();
    assert!(matches!(
        world.service.add_java(&nothing).await,
        Err(ServiceError::NoJavaAt(_))
    ));
}

#[tokio::test]
async fn a_java_the_user_turned_off_is_listed_but_never_chosen() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let installed = world.service.java_installations().await;
    assert_eq!(installed.len(), 1);
    assert!(!installed[0].1);
    let home = installed[0].0.home().to_owned();

    world.service.set_java_disabled(&home, true).await.unwrap();
    assert!(
        world.service.java_installations().await[0].1,
        "still listed, marked off"
    );
    let error = launch_to_end(&world, &record.id).await.unwrap_err();
    assert!(error.to_string().contains("Java"), "{error}");

    world.service.set_java_disabled(&home, false).await.unwrap();
    assert!(!world.service.java_installations().await[0].1);
    launch_to_end(&world, &record.id).await.unwrap();
}

#[tokio::test]
async fn an_instance_java_must_exist_and_a_bad_override_changes_nothing() {
    use crate::tuning::InstanceLaunch;
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let mut settings = record.settings.clone();
    settings.java_path = Some(world._dir.path().join("no-such-java"));
    assert!(matches!(
        world
            .service
            .update_instance_settings(&record.id, settings.clone())
            .await,
        Err(ServiceError::NoJavaAt(_))
    ));
    settings.java_path = None;
    settings.launch = InstanceLaunch {
        window_width: Some(800),
        ..InstanceLaunch::default()
    };
    assert!(
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await
            .is_err()
    );
    assert!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .settings
            .launch
            .is_default()
    );
}
