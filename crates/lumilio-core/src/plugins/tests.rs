use super::*;
use lumilio_plugin_api::{Permission, SettingField, SettingKind, SettingValue};
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "tests/analysis.rs"]
mod analysis;
#[path = "tests/content.rs"]
mod content;
#[path = "tests/network.rs"]
mod network;
#[path = "tests/tabs.rs"]
mod tabs;

struct Fake {
    id: &'static str,
    api: u32,
    default_enabled: bool,
    manifests: Arc<AtomicUsize>,
    caller: std::thread::ThreadId,
}

impl Plugin for Fake {
    fn manifest(&self) -> Manifest {
        assert_ne!(
            self.caller,
            std::thread::current().id(),
            "metadata also runs off-thread"
        );
        self.manifests.fetch_add(1, Ordering::SeqCst);
        Manifest {
            id: self.id.into(),
            name: "测试插件".into(),
            description: "只用于测试".into(),
            version: "1".into(),
            api: self.api,
            default_enabled: self.default_enabled,
            permissions: vec![Permission::ReadGameFiles {
                under: "schematics".into(),
            }],
            settings: vec![
                SettingField {
                    key: "show".into(),
                    label: "显示".into(),
                    help: String::new(),
                    kind: SettingKind::Toggle { default: true },
                },
                SettingField {
                    key: "mode".into(),
                    label: "模式".into(),
                    help: String::new(),
                    kind: SettingKind::Choice {
                        options: vec!["a".into(), "b".into()],
                        default: "a".into(),
                    },
                },
                SettingField {
                    key: "name".into(),
                    label: "名称".into(),
                    help: String::new(),
                    kind: SettingKind::Text {
                        default: "default".into(),
                    },
                },
                SettingField {
                    key: "count".into(),
                    label: "数量".into(),
                    help: String::new(),
                    kind: SettingKind::Number {
                        min: 1,
                        max: 10,
                        default: 5,
                    },
                },
            ],
        }
    }
}

fn fake(id: &'static str, api: u32, default_enabled: bool) -> Arc<Fake> {
    Arc::new(Fake {
        id,
        api,
        default_enabled,
        manifests: Arc::default(),
        caller: std::thread::current().id(),
    })
}

fn host() -> PluginHost {
    PluginHost::new(
        vec![
            fake("test.fake", API_VERSION, true),
            fake("test.other", API_VERSION, true),
        ],
        BTreeMap::new(),
    )
}

#[tokio::test]
async fn enabled_disabled_and_manifest_defaults_control_dispatch() {
    let host = PluginHost::new(vec![fake("test.fake", API_VERSION, false)], BTreeMap::new());
    assert_eq!(host.list().await[0].status, PluginStatus::Disabled);
    assert_eq!(host.call("test.fake", |_, _| Ok(42)).await, None);
    host.set_state(
        "test.fake".into(),
        PluginState {
            enabled: Some(true),
            ..Default::default()
        },
    );
    assert_eq!(host.call("test.fake", |_, _| Ok(42)).await, Some(42));
    host.set_state(
        "test.fake".into(),
        PluginState {
            enabled: Some(false),
            ..Default::default()
        },
    );
    assert_eq!(host.call("test.fake", |_, _| Ok(42)).await, None);
    host.set_state("test.fake".into(), PluginState::default());
    assert_eq!(host.list().await[0].status, PluginStatus::Disabled);
}

#[tokio::test]
async fn panic_disables_only_the_failing_plugin_until_restart() {
    let host = host();
    assert_eq!(
        host.call::<(), _>("test.fake", |_, _| panic!("broken plugin"))
            .await,
        None
    );
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
    host.set_state(
        "test.fake".into(),
        PluginState {
            enabled: Some(true),
            ..Default::default()
        },
    );
    assert_eq!(host.call("test.fake", |_, _| Ok(42)).await, None);
    assert_eq!(host.call("test.other", |_, _| Ok(42)).await, Some(42));
    let restarted =
        super::PluginHost::new(vec![fake("test.fake", API_VERSION, true)], BTreeMap::new());
    assert_eq!(restarted.call("test.fake", |_, _| Ok(42)).await, Some(42));
}

#[tokio::test]
async fn errors_also_fail_only_the_calling_plugin() {
    let host = host();
    assert_eq!(
        host.call::<(), _>("test.fake", |_, _| Err(PluginError::Unavailable(
            "broken".into()
        )))
        .await,
        None
    );
    assert_eq!(
        host.list().await[0].status,
        PluginStatus::Failed {
            message: "broken".into()
        }
    );
    assert_eq!(host.call("test.other", |_, _| Ok(7)).await, Some(7));
}

#[tokio::test]
async fn timeout_discards_late_results_and_stops_future_dispatch() {
    let mut host = host();
    // Register before shortening the test timeout so thread startup does not
    // masquerade as a plugin invocation timeout.
    host.list().await;
    host.timeout = Duration::from_millis(20);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let call = host.call("test.fake", move |_, _| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(42)
    });
    let observer = async {
        started_rx.await.unwrap();
    };
    let (result, ()) = tokio::join!(call, observer);
    assert_eq!(result, None);
    assert_eq!(
        host.list().await[0].status,
        PluginStatus::Failed {
            message: "plugin call timed out".into()
        }
    );
    release_tx.send(()).unwrap();
    assert_eq!(host.call("test.fake", |_, _| Ok(42)).await, None);
    assert_eq!(host.call("test.other", |_, _| Ok(7)).await, Some(7));
}

#[tokio::test]
async fn unauthorized_io_returns_errors_without_panicking() {
    let host = host();
    assert_eq!(
        host.call("test.fake", |_, ctx| {
            assert_eq!(
                ctx.read_file("../secret"),
                Err(PluginError::PermissionDenied)
            );
            assert_eq!(
                ctx.read_file("schematics/a.litematic"),
                Err(PluginError::PermissionDenied)
            );
            assert_eq!(
                ctx.fetch("https://api.modrinth.com/v2/search"),
                Err(PluginError::PermissionDenied)
            );
            Ok(42)
        })
        .await,
        Some(42)
    );
    assert_eq!(host.list().await[0].status, PluginStatus::Enabled);
}

#[tokio::test]
async fn incompatible_api_and_duplicate_ids_are_rejected() {
    let host = PluginHost::new(
        vec![
            fake("test.old", API_VERSION + 1, true),
            fake("test.fake", API_VERSION, true),
            fake("test.fake", API_VERSION, true),
        ],
        BTreeMap::new(),
    );
    assert_eq!(host.list().await.len(), 1);
    assert_eq!(host.rejected().await.len(), 2);
    assert_eq!(host.call("test.old", |_, _| Ok(42)).await, None);
}

#[tokio::test]
async fn settings_use_valid_saved_values_and_fall_back_to_defaults() {
    let host = host();
    assert_eq!(
        host.call("test.fake", |_, ctx| Ok(ctx.setting("count")))
            .await,
        Some(Some(SettingValue::Number(5)))
    );
    let saved = PluginState {
        enabled: None,
        values: BTreeMap::from([("count".into(), SettingValue::Number(8))]),
    };
    host.validate_state("test.fake", &saved).await.unwrap();
    host.set_state("test.fake".into(), saved);
    assert_eq!(
        host.call("test.fake", |_, ctx| Ok(ctx.setting("count")))
            .await,
        Some(Some(SettingValue::Number(8)))
    );
    // Invalid values from a hand-edited file cannot leak into plugin code.
    host.set_state(
        "test.fake".into(),
        PluginState {
            enabled: None,
            values: BTreeMap::from([("count".into(), SettingValue::Number(99))]),
        },
    );
    assert_eq!(
        host.call("test.fake", |_, ctx| Ok(ctx.setting("count")))
            .await,
        Some(Some(SettingValue::Number(5)))
    );
    assert_eq!(
        host.call("test.fake", |_, ctx| Ok(ctx.setting("unknown")))
            .await,
        Some(None)
    );
}

#[tokio::test]
async fn malformed_settings_are_rejected_before_a_save() {
    let host = host();
    for (key, value) in [
        ("count", SettingValue::Number(0)),
        ("count", SettingValue::Number(11)),
        ("count", SettingValue::Text("5".into())),
        ("show", SettingValue::Number(1)),
        ("mode", SettingValue::Choice("c".into())),
        ("unknown", SettingValue::Toggle(true)),
    ] {
        assert!(
            host.validate_state(
                "test.fake",
                &PluginState {
                    enabled: None,
                    values: BTreeMap::from([(key.into(), value)])
                }
            )
            .await
            .is_err()
        );
    }
    assert!(
        host.validate_state("test.missing", &PluginState::default())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn disabling_a_plugin_discards_an_in_flight_contribution() {
    let host = host();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let call = host.call("test.fake", move |_, _| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(42)
    });
    let disable = async {
        started_rx.await.unwrap();
        host.set_state(
            "test.fake".into(),
            PluginState {
                enabled: Some(false),
                ..Default::default()
            },
        );
        release_tx.send(()).unwrap();
    };
    let (result, ()) = tokio::join!(call, disable);
    assert_eq!(result, None);
}

#[tokio::test]
async fn the_service_persists_preferences_but_never_runtime_failures() {
    use crate::{FileTransport, LauncherService};
    let dir = tempfile::tempdir().unwrap();
    let plugins = || vec![fake("test.fake", API_VERSION, true) as Arc<dyn Plugin>];
    let service = LauncherService::open(dir.path(), FileTransport, plugins()).unwrap();
    let state = PluginState {
        enabled: Some(false),
        values: BTreeMap::from([("show".into(), SettingValue::Toggle(false))]),
    };
    service
        .set_plugin("test.fake", state.clone())
        .await
        .unwrap();
    assert_eq!(service.plugins().await[0].status, PluginStatus::Disabled);
    drop(service);
    let service = LauncherService::open(dir.path(), FileTransport, plugins()).unwrap();
    assert_eq!(service.settings().await.plugins["test.fake"], state);
    service
        .set_plugin("test.fake", PluginState::default())
        .await
        .unwrap();
    service
        .plugins
        .call::<(), _>("test.fake", |_, _| panic!("oops"))
        .await;
    let json = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
    assert!(!json.contains("oops") && !json.contains("Failed"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap()["schema"],
        1
    );
    drop(service);
    let service = LauncherService::open(dir.path(), FileTransport, plugins()).unwrap();
    assert_eq!(service.plugins().await[0].status, PluginStatus::Enabled);
}

#[tokio::test]
async fn failed_persistence_preserves_host_preferences() {
    use crate::{FileTransport, LauncherService};
    let dir = tempfile::tempdir().unwrap();
    let service = LauncherService::open(
        dir.path(),
        FileTransport,
        vec![fake("test.fake", API_VERSION, true)],
    )
    .unwrap();
    service
        .set_plugin("test.fake", PluginState::default())
        .await
        .unwrap();
    std::fs::create_dir(dir.path().join("settings.json.tmp")).unwrap();
    assert!(
        service
            .set_plugin(
                "test.fake",
                PluginState {
                    enabled: Some(false),
                    ..Default::default()
                }
            )
            .await
            .is_err()
    );
    assert_eq!(service.plugins().await[0].status, PluginStatus::Enabled);
    assert_eq!(
        service.settings().await.plugins["test.fake"],
        PluginState::default()
    );
}

#[tokio::test]
async fn independent_field_saves_merge_and_reset_restores_all_defaults() {
    use crate::{FileTransport, LauncherService};
    let dir = tempfile::tempdir().unwrap();
    let service = LauncherService::open(
        dir.path(),
        FileTransport,
        vec![fake("test.fake", API_VERSION, true)],
    )
    .unwrap();
    let (toggle, text, number) = tokio::join!(
        service.set_plugin_enabled("test.fake", false),
        service.set_plugin_value("test.fake", "name", SettingValue::Text("draft".into())),
        service.set_plugin_value("test.fake", "count", SettingValue::Number(9)),
    );
    toggle.unwrap();
    text.unwrap();
    number.unwrap();
    let state = service.settings().await.plugins["test.fake"].clone();
    assert_eq!(state.enabled, Some(false));
    assert_eq!(state.values["name"], SettingValue::Text("draft".into()));
    assert_eq!(state.values["count"], SettingValue::Number(9));
    assert!(
        service
            .set_plugin_value("test.fake", "count", SettingValue::Number(99))
            .await
            .is_err()
    );
    assert_eq!(service.settings().await.plugins["test.fake"], state);
    service.reset_plugin("test.fake").await.unwrap();
    assert_eq!(service.plugins().await[0].status, PluginStatus::Enabled);
    assert_eq!(
        service.settings().await.plugins["test.fake"],
        PluginState::default()
    );
}
