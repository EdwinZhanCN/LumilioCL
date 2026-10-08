use super::*;
use lumilio_plugin_api::{
    DiscordActivity, LaunchEvent, LaunchObserver, LaunchOutcome, NativeCapability, Words,
};

struct Observer {
    id: &'static str,
    permissions: Vec<Permission>,
    events: tokio::sync::mpsc::UnboundedSender<LaunchEvent>,
    delay: Duration,
    panics: bool,
}

impl Plugin for Observer {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: self.id.into(),
            name: Words::new("观察者", "Observer"),
            description: Words::default(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: self.permissions.clone(),
            settings: vec![],
        }
    }
    fn launch_observer(&self) -> Option<&dyn LaunchObserver> {
        Some(self)
    }
}
impl LaunchObserver for Observer {
    fn observe(&self, _: &dyn HostContext, event: &LaunchEvent) -> Result<(), PluginError> {
        if self.panics {
            panic!("observer panic");
        }
        std::thread::sleep(self.delay);
        let _ = self.events.send(event.clone());
        Ok(())
    }
}
fn event() -> LaunchEvent {
    LaunchEvent::Exited {
        outcome: LaunchOutcome::Clean,
        played_seconds: 1,
    }
}
fn observer(
    id: &'static str,
    permissions: Vec<Permission>,
) -> (Observer, tokio::sync::mpsc::UnboundedReceiver<LaunchEvent>) {
    let (events, receiver) = tokio::sync::mpsc::unbounded_channel();
    (
        Observer {
            id,
            permissions,
            events,
            delay: Duration::ZERO,
            panics: false,
        },
        receiver,
    )
}
async fn wait_failed(host: &PluginHost, id: &str) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if host.list().await.iter().any(|info| {
                info.manifest.id == id && matches!(info.status, PluginStatus::Failed { .. })
            }) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn observer_panic_and_timeout_do_not_delay_other_observers_and_only_fail_the_culprit() {
    for panics in [true, false] {
        let (mut bad, _) = observer("test.bad", vec![Permission::LaunchEvents]);
        bad.panics = panics;
        if !panics {
            bad.delay = Duration::from_millis(800);
        }
        let (good, mut received) = observer("test.good", vec![Permission::LaunchEvents]);
        let mut host = PluginHost::new(vec![Arc::new(bad), Arc::new(good)], BTreeMap::new());
        host.timeout = Duration::from_millis(500);
        let host = Arc::new(host);
        host.list().await;
        let events = host.launch_events();
        events.send(event()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(400), received.recv())
                .await
                .unwrap(),
            Some(event())
        );
        wait_failed(&host, "test.bad").await;
        assert_eq!(host.list().await[1].status, PluginStatus::Enabled);
    }
}

#[tokio::test]
async fn disabled_or_ungranted_observers_receive_nothing() {
    let (disabled, mut a) = observer("test.disabled", vec![Permission::LaunchEvents]);
    let (ungranted, mut b) = observer("test.ungranted", vec![]);
    let host = Arc::new(PluginHost::new(
        vec![Arc::new(disabled), Arc::new(ungranted)],
        BTreeMap::from([(
            "test.disabled".into(),
            PluginState {
                enabled: Some(false),
                ..Default::default()
            },
        )]),
    ));
    let events = host.launch_events();
    events.send(event()).unwrap();
    drop(events);
    drop(host);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), a.recv())
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), b.recv())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn native_trust_cannot_be_claimed_by_a_manifest_id_and_every_operation_requires_a_grant() {
    let (plugin, _) = observer(
        "lumilio.discord",
        vec![Permission::Native(NativeCapability::DiscordIpc)],
    );
    let host = PluginHost::new(vec![Arc::new(plugin)], BTreeMap::new());
    assert!(host.list().await.is_empty());
    assert!(host.rejected().await[0].contains("core plugin"));
    let (plugin, _) = observer("test.core", vec![]);
    let host = PluginHost::new_core(vec![Arc::new(plugin)], BTreeMap::new());
    assert_eq!(
        host.call("test.core", |_, ctx| Ok(ctx.discord_activity(None)))
            .await,
        Some(Err(PluginError::PermissionDenied))
    );
}

fn activity() -> DiscordActivity {
    DiscordActivity::new("123456789012345678", Some("Minecraft".into()), None)
}

#[tokio::test]
async fn late_native_operations_are_denied_after_disable_or_timeout() {
    for disable in [true, false] {
        let (plugin, _) = observer(
            "test.core",
            vec![Permission::Native(NativeCapability::DiscordIpc)],
        );
        let mut host = PluginHost::new_core(vec![Arc::new(plugin)], BTreeMap::new());
        host.timeout = Duration::from_millis(50);
        host.list().await;
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (late_tx, late_rx) = tokio::sync::oneshot::channel();
        let call = host.call("test.core", move |_, ctx| {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            let result = ctx.discord_activity(Some(activity()));
            late_tx.send(result).unwrap();
            Ok(())
        });
        let revoke = async {
            started_rx.await.unwrap();
            if disable {
                host.set_state(
                    "test.core".into(),
                    PluginState {
                        enabled: Some(false),
                        ..Default::default()
                    },
                );
            } else {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            release_tx.send(()).unwrap();
        };
        tokio::join!(call, revoke);
        assert_eq!(late_rx.await.unwrap(), Err(PluginError::PermissionDenied));
        if disable {
            assert_eq!(host.list().await[0].status, PluginStatus::Disabled);
        }
    }
}
