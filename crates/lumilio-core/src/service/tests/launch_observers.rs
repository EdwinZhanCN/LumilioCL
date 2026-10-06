use super::{World, fake_java, publish_modern_release, publish_release, world};
use crate::activity::CancellationToken;
use crate::history::{HistoryEvent, HistoryLog};
use crate::instance::Loader;
use crate::plugins::{PluginHost, PluginStatus};
use lumilio_plugin_api::{
    API_VERSION, HostContext, LaunchEvent, LaunchObserver, LaunchOutcome, LaunchTarget, Manifest,
    Permission, Plugin, PluginError,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

struct Recorder {
    events: mpsc::UnboundedSender<(u64, LaunchEvent)>,
    caller: std::thread::ThreadId,
    panics: bool,
}
impl Plugin for Recorder {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.observer".into(),
            name: "测试观察者".into(),
            description: String::new(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![Permission::LaunchEvents],
            settings: vec![],
        }
    }
    fn launch_observer(&self) -> Option<&dyn LaunchObserver> {
        Some(self)
    }
}
impl LaunchObserver for Recorder {
    fn observe(&self, ctx: &dyn HostContext, event: &LaunchEvent) -> Result<(), PluginError> {
        assert_ne!(self.caller, std::thread::current().id());
        if self.panics {
            panic!("broken observer");
        }
        self.events
            .send((ctx.launch_id().unwrap(), event.clone()))
            .unwrap();
        Ok(())
    }
}
fn observed_world(panics: bool) -> (World, mpsc::UnboundedReceiver<(u64, LaunchEvent)>) {
    let mut world = world();
    let (events, receiver) = mpsc::unbounded_channel();
    world.service.plugins = Arc::new(PluginHost::new(
        vec![Arc::new(Recorder {
            events,
            panics,
            caller: std::thread::current().id(),
        })],
        Default::default(),
    ));
    (world, receiver)
}
async fn next(receiver: &mut mpsc::UnboundedReceiver<(u64, LaunchEvent)>) -> (u64, LaunchEvent) {
    tokio::time::timeout(Duration::from_secs(3), receiver.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn menu_world_and_server_observations_follow_spawn_and_match_the_written_session() {
    for target in [
        None,
        Some(LaunchTarget::World("My World".into())),
        Some(LaunchTarget::Server("mc.test:25565".into())),
    ] {
        let (world, mut events) = observed_world(false);
        publish_modern_release(&world);
        fake_java(&world, "echo 'Setting user: x'");
        let record = world
            .service
            .create_instance("Observed", None, Loader::Vanilla, None)
            .await
            .unwrap();
        std::fs::create_dir_all(
            world
                .service
                .layout()
                .game(&record.id)
                .join("saves/My World"),
        )
        .unwrap();
        let (updates, _rx) = mpsc::unbounded_channel();
        let exit = match &target {
            None => {
                world
                    .service
                    .launch(&record.id, updates, CancellationToken::new())
                    .await
            }
            Some(LaunchTarget::World(name)) => {
                world
                    .service
                    .launch_world(&record.id, name, updates, CancellationToken::new())
                    .await
            }
            Some(LaunchTarget::Server(address)) => {
                world
                    .service
                    .launch_server(&record.id, address, updates, CancellationToken::new())
                    .await
            }
        }
        .unwrap();
        let (id, started) = next(&mut events).await;
        assert_eq!(
            started,
            LaunchEvent::Started {
                instance_name: "Observed".into(),
                game_version: "1.0".into(),
                loader: "Vanilla".into(),
                target
            }
        );
        let (ended_id, ended) = next(&mut events).await;
        assert_eq!(id, ended_id);
        assert_eq!(
            ended,
            LaunchEvent::Exited {
                outcome: LaunchOutcome::Clean,
                played_seconds: exit.ran_for.as_secs()
            }
        );
        assert!(
            matches!(HistoryLog::for_instance(world.service.layout.root(), &record.id).sessions().unwrap()[0],
            HistoryEvent::Session { seconds, .. } if seconds == exit.ran_for.as_secs())
        );
        assert!(events.try_recv().is_err());
    }
}

#[tokio::test]
async fn preparation_failure_cancellation_install_and_failed_spawn_emit_no_started_event() {
    let (world, mut events) = observed_world(false);
    publish_release(&world);
    let record = world
        .service
        .create_instance("No process", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let (updates, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .launch(&record.id, updates, CancellationToken::new())
            .await
            .is_err()
    );
    let (updates, _rx) = mpsc::unbounded_channel();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(
        world
            .service
            .launch(&record.id, updates, cancelled)
            .await
            .is_err()
    );
    let (updates, _rx) = mpsc::unbounded_channel();
    world
        .service
        .install_instance(&record.id, updates, CancellationToken::new())
        .await
        .unwrap();
    fake_java(&world, "exit 0");
    // Java probing sees a runtime, but the configured wrapper cannot spawn.
    let mut settings = record.settings;
    settings.launch.wrapper = Some("/lumilio-test-does-not-exist".into());
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    let (updates, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .launch(&record.id, updates, CancellationToken::new())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), events.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn short_process_crash_and_user_stop_emit_the_correct_exit() {
    let (world, mut events) = observed_world(false);
    publish_release(&world);
    let record = world
        .service
        .create_instance("Outcomes", None, Loader::Vanilla, None)
        .await
        .unwrap();
    for (script, expected) in [
        ("exit 1", LaunchOutcome::FailedToStart),
        ("echo 'Setting user: x'; exit 3", LaunchOutcome::Crashed),
    ] {
        fake_java(&world, script);
        let (updates, _rx) = mpsc::unbounded_channel();
        world
            .service
            .launch(&record.id, updates, CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(
            next(&mut events).await.1,
            LaunchEvent::Started { .. }
        ));
        assert!(
            matches!(next(&mut events).await.1, LaunchEvent::Exited { outcome, .. } if outcome == expected)
        );
    }
    fake_java(&world, "echo 'Setting user: x'; sleep 30");
    let cancel = CancellationToken::new();
    let (updates, _rx) = mpsc::unbounded_channel();
    let launch = world.service.launch(&record.id, updates, cancel.clone());
    let stop = async {
        assert!(matches!(
            next(&mut events).await.1,
            LaunchEvent::Started { .. }
        ));
        cancel.cancel();
    };
    let (exit, ()) = tokio::join!(launch, stop);
    assert!(exit.unwrap().killed);
    assert!(matches!(
        next(&mut events).await.1,
        LaunchEvent::Exited {
            outcome: LaunchOutcome::Stopped,
            ..
        }
    ));
}

#[tokio::test]
async fn observer_panic_does_not_change_launch_or_history() {
    let (world, _) = observed_world(true);
    publish_release(&world);
    fake_java(&world, "echo 'Setting user: x'");
    let record = world
        .service
        .create_instance("Panic", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let (updates, _rx) = mpsc::unbounded_channel();
    assert_eq!(
        world
            .service
            .launch(&record.id, updates, CancellationToken::new())
            .await
            .unwrap()
            .code,
        Some(0)
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if matches!(
                world.service.plugins().await[0].status,
                PluginStatus::Failed { .. }
            ) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        HistoryLog::for_instance(world.service.layout.root(), &record.id)
            .sessions()
            .unwrap()
            .len(),
        1
    );
}
