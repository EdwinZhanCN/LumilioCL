use super::super::LauncherService;
use super::super::error::ServiceError;
use super::{ECHO_ARGS, fake_java, launch_to_end, publish_release, world};
use crate::activity::CancellationToken;
use crate::instance::{InstanceSettings, Loader, NewInstance};
use crate::settings::SettingsError;
use tokio::sync::mpsc;

#[tokio::test]
async fn plugin_failure_and_disable_do_not_affect_the_launch_chain() {
    use lumilio_plugin_api::{API_VERSION, Manifest, Plugin, PluginState};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    struct Observer;
    impl Plugin for Observer {
        fn manifest(&self) -> Manifest {
            Manifest {
                id: "test.observer".into(),
                name: "测试".into(),
                description: String::new(),
                version: "1".into(),
                api: API_VERSION,
                default_enabled: true,
                permissions: Vec::new(),
                settings: Vec::new(),
            }
        }
    }

    let mut world = world();
    world.service.plugins = Arc::new(crate::PluginHost::new(
        vec![Arc::new(Observer)],
        BTreeMap::new(),
    ));
    publish_release(&world);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    world
        .service
        .plugins
        .call::<(), _>("test.observer", |_, _| panic!("broken observer"))
        .await;
    assert!(matches!(
        world.service.plugins().await[0].status,
        crate::PluginStatus::Failed { .. }
    ));
    assert_eq!(
        launch_to_end(&world, &record.id).await.unwrap().code,
        Some(0)
    );
    world
        .service
        .set_plugin(
            "test.observer",
            PluginState {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        launch_to_end(&world, &record.id).await.unwrap().code,
        Some(0)
    );
}

#[tokio::test]
async fn instance_memory_overrides_are_validated_and_can_resume_inheritance() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Memory".into(),
                game_version: "1.0".into(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            1,
        )
        .unwrap()
        .clone();
    world
        .service
        .set_default_memory(Some(512), Some(2048))
        .await
        .unwrap();
    let settings = InstanceSettings {
        min_memory_mb: Some(4096),
        ..InstanceSettings::default()
    };
    assert!(matches!(
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await,
        Err(ServiceError::Settings(SettingsError::InvalidMemory))
    ));
    assert_eq!(world.service.library().await.instances[0], record);
    let settings = InstanceSettings {
        max_memory_mb: Some(4096),
        ..InstanceSettings::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings.clone())
        .await
        .unwrap();
    assert_eq!(
        world.service.library().await.instances[0].settings,
        settings
    );
    world
        .service
        .update_instance_settings(&record.id, InstanceSettings::default())
        .await
        .unwrap();
    assert_eq!(
        world.service.library().await.instances[0].settings,
        InstanceSettings::default()
    );
    let settings = InstanceSettings {
        min_memory_mb: Some(1024),
        ..InstanceSettings::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    world
        .service
        .set_default_memory(Some(128), Some(512))
        .await
        .unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    // There is no catalog answer: memory validation must win over network failure.
    assert!(matches!(
        world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await,
        Err(ServiceError::Settings(SettingsError::InvalidMemory))
    ));
    assert!(!world.service.layout().game(&record.id).exists());
    let expected_library = world.service.library().await;
    let expected_settings = world.service.settings().await;
    let root = world.service.layout().root().to_path_buf();
    drop(world.service);
    let reopened = LauncherService::open(root, world.net.clone(), super::stock_plugins()).unwrap();
    assert_eq!(reopened.library().await, expected_library);
    assert_eq!(reopened.settings().await, expected_settings);
}

#[tokio::test]
async fn service_settings_expose_only_saved_defaults_after_a_write_failure() {
    let world = world();
    world
        .service
        .set_default_memory(Some(512), Some(2048))
        .await
        .unwrap();
    let before = world.service.settings().await;
    std::fs::create_dir(world.service.layout().root().join("settings.json.tmp")).unwrap();
    assert!(
        world
            .service
            .set_default_memory(Some(1024), Some(4096))
            .await
            .is_err()
    );
    assert_eq!(world.service.settings().await, before);
}

#[tokio::test]
async fn an_instance_overrides_some_defaults_and_follows_the_rest() {
    use crate::tuning::{InstanceLaunch, LaunchTuning};
    let world = world();
    publish_release(&world);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    world
        .service
        .set_launch_defaults(LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            game_arguments: vec!["--demo".into()],
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    let mut settings = record.settings.clone();
    settings.launch = InstanceLaunch {
        fullscreen: Some(true),
        ..InstanceLaunch::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings.clone())
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let game = world.service.layout().game(&record.id);
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(
        args.contains("--width 1280 --height 720"),
        "follows the defaults: {args}"
    );
    assert!(args.contains("--fullscreen"), "its own override: {args}");
    assert!(args.contains("--demo"), "{args}");

    // Overriding the arguments with "none" drops them; clearing the
    // override follows the defaults again.
    settings.launch.game_arguments = Some(Vec::new());
    world
        .service
        .update_instance_settings(&record.id, settings.clone())
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(!args.contains("--demo"), "{args}");
    settings.launch = InstanceLaunch::default();
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(
        args.contains("--demo") && !args.contains("--fullscreen"),
        "{args}"
    );
}

#[tokio::test]
async fn a_failing_command_before_launch_stops_the_game_but_one_after_exit_does_not() {
    use crate::tuning::LaunchTuning;
    let world = world();
    publish_release(&world);
    fake_java(
        &world,
        "echo \"Setting user: x\"\necho ran > \"$INST_DIR/ran.txt\"",
    );
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    world
        .service
        .set_launch_defaults(LaunchTuning {
            pre_launch: Some("exit 3".into()),
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    let error = launch_to_end(&world, &record.id).await.unwrap_err();
    assert!(error.to_string().contains("before launch"), "{error}");
    assert!(error.to_string().contains('3'), "{error}");
    assert!(!game.join("ran.txt").exists(), "the game must not start");

    world
        .service
        .set_launch_defaults(LaunchTuning {
            post_exit: Some("exit 9".into()),
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    let exit = launch_to_end(&world, &record.id).await.unwrap();
    assert_eq!(
        exit.code,
        Some(0),
        "a failed command after exit changes nothing"
    );
    assert!(game.join("ran.txt").exists());
}

#[tokio::test]
async fn the_window_follows_the_instance_choice_then_the_preference() {
    use crate::tuning::{AfterLaunch, InstanceLaunch, Preferences};
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert_eq!(
        world.service.after_launch_for(&record.id).await,
        AfterLaunch::Keep
    );
    world
        .service
        .set_preferences(Preferences {
            after_launch: AfterLaunch::Hide,
            ..Preferences::default()
        })
        .await
        .unwrap();
    assert_eq!(
        world.service.after_launch_for(&record.id).await,
        AfterLaunch::Hide
    );
    let mut settings = record.settings.clone();
    settings.launch = InstanceLaunch {
        after_launch: Some(AfterLaunch::Keep),
        ..InstanceLaunch::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    assert_eq!(
        world.service.after_launch_for(&record.id).await,
        AfterLaunch::Keep
    );
    assert_eq!(
        world.service.after_launch_for("ghost").await,
        AfterLaunch::Hide
    );
}

#[tokio::test]
async fn the_current_instance_persists_and_reads_as_none_once_deleted() {
    let world = world();
    publish_release(&world);
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
    assert_eq!(world.service.current_instance().await, None);
    assert!(matches!(
        world.service.set_current_instance("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    world.service.set_current_instance(&b.id).await.unwrap();
    assert_eq!(world.service.current_instance().await, Some(b.id.clone()));
    world.service.delete_instance(&b.id).await.unwrap();
    assert_eq!(world.service.current_instance().await, None);
    world.service.set_current_instance(&a.id).await.unwrap();
    assert_eq!(world.service.current_instance().await, Some(a.id.clone()));
}
