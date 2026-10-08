use super::*;
use lumilio_plugin_api::{API_VERSION, Manifest, PluginState};
use std::collections::BTreeMap;

struct Fake;
impl Plugin for Fake {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.maps".into(),
            name: "Map".into(),
            description: String::new(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![],
            settings: vec![],
        }
    }
}

#[tokio::test]
async fn faults_stop_only_provider_and_success_resets_counter() {
    let host = PluginHost::new(vec![Arc::new(Fake)], BTreeMap::new());
    for _ in 0..4 {
        assert!(
            host.map_call(
                "test.maps",
                "base:seed",
                None,
                CancellationToken::new(),
                MAP_TIMEOUT,
                |_, _| -> Result<(), PluginError> { panic!("probe") }
            )
            .await
            .is_err()
        );
    }
    assert!(
        host.map_call(
            "test.maps",
            "base:seed",
            None,
            CancellationToken::new(),
            MAP_TIMEOUT,
            |_, _| Ok(())
        )
        .await
        .is_ok()
    );
    for _ in 0..5 {
        assert!(
            host.map_call(
                "test.maps",
                "base:seed",
                None,
                CancellationToken::new(),
                MAP_TIMEOUT,
                |_, _| -> Result<(), PluginError> { Err(PluginError::PermissionDenied) }
            )
            .await
            .is_err()
        );
    }
    assert_eq!(
        host.map_call(
            "test.maps",
            "base:seed",
            None,
            CancellationToken::new(),
            MAP_TIMEOUT,
            |_, _| Ok(())
        )
        .await,
        Err(MapFailure::ProviderStopped)
    );
    assert!(
        host.map_call(
            "test.maps",
            "overlay:points",
            None,
            CancellationToken::new(),
            MAP_TIMEOUT,
            |_, _| Ok(())
        )
        .await
        .is_ok()
    );
    assert!(host.call("test.maps", |_, _| Ok(())).await.is_some());
    assert_eq!(host.list().await[0].status, PluginStatus::Enabled);
}

#[tokio::test]
async fn timeout_cancel_and_disabled_do_not_fail_plugin() {
    let host = PluginHost::new(vec![Arc::new(Fake)], BTreeMap::new());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(
        host.map_call(
            "test.maps",
            "seed",
            None,
            cancel,
            MAP_TIMEOUT,
            |_, _| Ok(())
        )
        .await,
        Err(MapFailure::Cancelled)
    );
    assert!(
        host.map_call(
            "test.maps",
            "seed",
            None,
            CancellationToken::new(),
            Duration::from_millis(5),
            |_, ctx| {
                while !ctx.cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(())
            }
        )
        .await
        .is_err()
    );
    assert_eq!(host.list().await[0].status, PluginStatus::Enabled);
    host.set_state(
        "test.maps".into(),
        PluginState {
            enabled: Some(false),
            values: BTreeMap::new(),
        },
    );
    assert_eq!(
        host.map_call(
            "test.maps",
            "seed",
            None,
            CancellationToken::new(),
            MAP_TIMEOUT,
            |_, _| -> Result<(), PluginError> { panic!("must not dispatch") }
        )
        .await,
        Err(MapFailure::Off)
    );
}
