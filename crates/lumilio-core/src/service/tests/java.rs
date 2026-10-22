use super::super::error::ServiceError;
use super::{fake_java, launch_to_end, publish_java, publish_release, world};
use crate::activity::CancellationToken;
use crate::activity_log::{RetryAction, TaskOutcome};
use crate::instance::Loader;
use crate::java_runtime::JavaBuild;

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
        .install_java(Some(21), JavaBuild::Any, CancellationToken::new())
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
            .install_java(Some(21), JavaBuild::Any, CancellationToken::new())
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
        Some(RetryAction::InstallJava {
            major: Some(21),
            source: crate::java_runtime::JavaSource::Mojang,
            build: JavaBuild::Any,
        })
    );

    publish_java(&world, false);
    let java = world
        .service
        .install_java(Some(21), JavaBuild::Any, CancellationToken::new())
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
        .install_java(Some(21), JavaBuild::Any, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(again.home(), java.home());
    assert!(matches!(
        world
            .service
            .install_java(Some(99), JavaBuild::Any, CancellationToken::new())
            .await,
        Err(ServiceError::Install(_))
    ));
}

/// Answers Oracle's checksum and archive addresses with a fake GraalVM 21.
/// A wrong checksum stands for a damaged download.
fn publish_graalvm(world: &super::World, wrong_checksum: bool) {
    use sha2::{Digest, Sha256};
    use std::io::Write as _;
    let mut builder = tar::Builder::new(Vec::new());
    for (path, body, mode) in [
        (
            "graalvm-jdk-21.0.8+12.1/release",
            &b"JAVA_VERSION=\"21.0.8\"\nIMPLEMENTOR=\"Oracle Corporation\"\n"[..],
            0o644,
        ),
        (
            "graalvm-jdk-21.0.8+12.1/bin/java",
            &b"#!/bin/sh\n"[..],
            0o755,
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(mode);
        builder.append_data(&mut header, path, body).unwrap();
    }
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&builder.into_inner().unwrap()).unwrap();
    let archive = encoder.finish().unwrap();
    let digest: String = Sha256::digest(if wrong_checksum {
        &b"other"[..]
    } else {
        &archive
    })
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect();
    world.net.forget_all();
    // The checksum's address contains the archive's: it must be answered first.
    world
        .net
        .answer(".sha256", format!("{digest}  graalvm-jdk-21_bin.tar.gz\n"));
    world.net.answer("graalvm/21/latest/", archive);
}

#[tokio::test]
async fn graalvm_comes_from_its_archive_and_a_bad_checksum_leaves_nothing() {
    use crate::java_runtime::JavaSource;
    let world = world();
    world
        .service
        .set_java_source(JavaSource::OracleGraalVm)
        .await
        .unwrap();
    let runtimes = world.service.layout.runtimes();

    publish_graalvm(&world, true);
    assert!(
        world
            .service
            .install_java(Some(21), JavaBuild::Any, CancellationToken::new())
            .await
            .is_err()
    );
    assert!(!runtimes.join("graalvm-21").exists());
    let leftovers: Vec<_> = std::fs::read_dir(&runtimes)
        .map(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    let failed = world.service.activity(5).finished.remove(0);
    assert_eq!(
        failed.retry,
        Some(RetryAction::InstallJava {
            major: Some(21),
            source: JavaSource::OracleGraalVm,
            build: JavaBuild::Any,
        })
    );

    publish_graalvm(&world, false);
    let java = world
        .service
        .install_java(Some(21), JavaBuild::Any, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(java.home(), runtimes.join("graalvm-21"));
    assert_eq!((java.major(), java.vendor()), (21, Some("Oracle")));
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(java.executable())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "the launcher is executable");
    }
    // Java 8 is not offered by GraalVM: the download falls back to Mojang.
    publish_java(&world, false);
    assert!(matches!(
        world
            .service
            .install_java(Some(8), JavaBuild::Any, CancellationToken::new())
            .await,
        Err(ServiceError::Install(why)) if why.contains("Java 8")
    ));
    let offers = world.service.java_offers().await;
    assert_eq!(offers[1], (JavaSource::OracleGraalVm, vec![21, 25]));

    // The launcher's own Java can be deleted; anything else only turned off.
    world
        .service
        .set_java_disabled(java.home(), true)
        .await
        .unwrap();
    world.service.remove_java(java.home()).await.unwrap();
    assert!(!runtimes.join("graalvm-21").exists());
    assert!(world.service.settings().await.disabled_java.is_empty());
    let elsewhere = world._dir.path().join("jdks/zulu-17");
    std::fs::create_dir_all(elsewhere.join("bin")).unwrap();
    std::fs::write(elsewhere.join("release"), "JAVA_VERSION=\"17.0.9\"\n").unwrap();
    std::fs::write(elsewhere.join("bin/java"), "").unwrap();
    assert!(matches!(
        world.service.remove_java(&elsewhere).await,
        Err(ServiceError::NoJavaAt(_))
    ));
    assert!(elsewhere.exists());
    assert!(world.service.remove_java(&runtimes).await.is_err());
}

#[tokio::test]
async fn a_launch_without_a_fitting_java_downloads_one_unless_turned_off() {
    let world = world();
    publish_release(&world);
    publish_java(&world, false);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();

    world.service.set_auto_install_java(false).await.unwrap();
    let error = launch_to_end(&world, &record.id).await.unwrap_err();
    assert!(
        matches!(
            error,
            ServiceError::Launch(crate::launcher::LaunchServiceError::NoJava {
                required: Some(21)
            })
        ),
        "{error}"
    );
    assert!(
        !world
            .service
            .layout
            .runtimes()
            .join("java-runtime-delta")
            .exists()
    );

    world.service.set_auto_install_java(true).await.unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    assert!(
        world
            .service
            .layout
            .runtimes()
            .join("java-runtime-delta/bin/java")
            .is_file()
    );
    let tasks = world.service.activity(10).finished;
    assert!(tasks.iter().any(|task| matches!(
        task.retry,
        Some(RetryAction::InstallJava {
            major: Some(21),
            ..
        })
    ) && matches!(task.outcome, TaskOutcome::Succeeded)));
}

#[tokio::test]
async fn a_launch_can_once_set_the_chosen_java_aside_for_the_automatic_one() {
    use std::os::unix::fs::PermissionsExt;
    let world = world();
    publish_release(&world);
    fake_java(&world, super::ECHO_ARGS);
    // The person's own Java 17 cannot run a game that needs 21.
    let mine = world._dir.path().join("jdks/old");
    std::fs::create_dir_all(mine.join("bin")).unwrap();
    std::fs::write(mine.join("release"), "JAVA_VERSION=\"17.0.9\"\n").unwrap();
    std::fs::write(mine.join("bin/java"), "#!/bin/sh\nexit 3\n").unwrap();
    std::fs::set_permissions(
        mine.join("bin/java"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let mut settings = record.settings.clone();
    settings.java_path = Some(mine.clone());
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    let args = world.service.layout.game(&record.id).join("args.txt");
    let _ = launch_to_end(&world, &record.id).await;
    assert!(!args.exists(), "the person's choice wins by default");
    // Installed now, the release says what it needs.
    let java = world.service.instance_java(&record.id).await.unwrap();
    let chosen = java.choice.unwrap();
    assert!(chosen.chosen_by_user && chosen.breaks_required());
    assert_eq!(java.automatic.unwrap().runtime.major(), 21);

    world.service.launch_next_with_automatic_java(&record.id);
    launch_to_end(&world, &record.id).await.unwrap();
    assert!(args.exists(), "this once, the automatic Java ran");
    std::fs::remove_file(&args).unwrap();
    let _ = launch_to_end(&world, &record.id).await;
    assert!(!args.exists(), "only once");
    assert_eq!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .settings
            .java_path,
        Some(mine),
        "the setting itself is unchanged"
    );
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
