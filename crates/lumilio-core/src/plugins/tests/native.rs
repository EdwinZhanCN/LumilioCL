use super::*;
use crate::plugins::{PluginHost, PluginStatus};
use lumilio_plugin_api::{LaunchEvent, LaunchOutcome, LaunchTarget, PluginState, SettingValue};
use tokio::net::UnixListener;

fn started() -> LaunchEvent {
    LaunchEvent::Started {
        instance_name: "Private".into(),
        game_version: "1.21".into(),
        loader: "Fabric".into(),
        target: Some(LaunchTarget::Server("private.test".into())),
    }
}
fn exited() -> LaunchEvent {
    LaunchEvent::Exited {
        outcome: LaunchOutcome::Clean,
        played_seconds: 5,
    }
}
fn host(socket: std::path::PathBuf, enabled: bool) -> Arc<PluginHost> {
    let mut host = PluginHost::new_core(
        vec![Arc::new(lumilio_plugin_discord::Discord::default())],
        BTreeMap::from([(
            lumilio_plugin_discord::ID.into(),
            PluginState {
                enabled: Some(enabled),
                values: BTreeMap::from([(
                    "application_id".into(),
                    SettingValue::Text("123456789012345678".into()),
                )]),
            },
        )]),
    );
    host.native = Arc::new(Native {
        workers: Mutex::default(),
        socket: Some(socket),
    });
    Arc::new(host)
}
async fn frame(stream: &mut Connection) -> (u32, serde_json::Value) {
    let opcode = stream.read_u32_le().await.unwrap();
    let length = stream.read_u32_le().await.unwrap() as usize;
    assert!(length < MAX_FRAME);
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await.unwrap();
    (opcode, serde_json::from_slice(&bytes).unwrap())
}
async fn accept(listener: &UnixListener) -> Connection {
    Box::new(
        tokio::time::timeout(Duration::from_secs(3), listener.accept())
            .await
            .unwrap()
            .unwrap()
            .0,
    )
}
async fn handshake(stream: &mut Connection) {
    let (opcode, body) = frame(stream).await;
    assert_eq!(opcode, 0);
    assert_eq!(
        body,
        serde_json::json!({"v":1,"client_id":"123456789012345678"})
    );
    send(
        stream,
        1,
        &serde_json::json!({"cmd":"DISPATCH","evt":"READY"}),
    )
    .await
    .unwrap();
}
async fn read_activity(stream: &mut Connection) -> serde_json::Value {
    let (opcode, body) = frame(stream).await;
    assert_eq!(opcode, 1);
    assert_eq!(body["cmd"], "SET_ACTIVITY");
    assert_eq!(body["args"]["pid"], std::process::id());
    send(
        stream,
        1,
        &serde_json::json!({"cmd":"SET_ACTIVITY","nonce":body["nonce"]}),
    )
    .await
    .unwrap();
    body["args"]["activity"].clone()
}

#[tokio::test]
async fn real_plugin_publishes_over_host_ipc_and_exit_disable_or_host_drop_withdraws_it() {
    for reason in ["exit", "disable", "panic", "abandon", "drop"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("discord-ipc-0");
        let listener = UnixListener::bind(&path).unwrap();
        let host = host(path, true);
        let events = host.launch_events();
        events.send(started()).unwrap();
        let mut stream = accept(&listener).await;
        handshake(&mut stream).await;
        let activity = tokio::time::timeout(Duration::from_secs(3), read_activity(&mut stream))
            .await
            .unwrap();
        assert_eq!(activity["details"], "Private · Minecraft 1.21 · Fabric");
        assert!(activity.get("state").is_none());
        assert!(!activity.to_string().contains("private.test"));
        // PING during idle must be answered without losing the activity.
        send(&mut stream, 3, &serde_json::json!({"probe": true}))
            .await
            .unwrap();
        let (opcode, body) = tokio::time::timeout(Duration::from_secs(3), frame(&mut stream))
            .await
            .unwrap();
        assert_eq!(opcode, 4);
        assert_eq!(body, serde_json::json!({"probe":true}));
        match reason {
            "exit" => {
                events.send(exited()).unwrap();
            }
            "disable" => host.set_state(
                lumilio_plugin_discord::ID.into(),
                PluginState {
                    enabled: Some(false),
                    ..Default::default()
                },
            ),
            "panic" => {
                host.call::<(), _>(lumilio_plugin_discord::ID, |_, _| {
                    panic!("native owner panicked")
                })
                .await;
                assert!(matches!(
                    host.list().await[0].status,
                    PluginStatus::Failed { .. }
                ));
            }
            _ => {}
        }
        drop(events);
        let retained = (reason == "abandon").then(|| host.clone());
        drop(host);
        let (_, clear) = tokio::time::timeout(Duration::from_secs(3), frame(&mut stream))
            .await
            .unwrap();
        assert!(clear["args"]["activity"].is_null());
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), stream.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        drop(retained);
    }
}

#[tokio::test]
async fn default_off_plugin_never_opens_ipc_and_absent_discord_keeps_enabled_plugin_healthy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("discord-ipc-0");
    let listener = UnixListener::bind(&path).unwrap();
    let disabled = host(path.clone(), false);
    let events = disabled.launch_events();
    events.send(started()).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
    assert_eq!(disabled.list().await[0].status, PluginStatus::Disabled);
    drop(listener);
    std::fs::remove_file(&path).unwrap();
    let enabled = host(path, true);
    let events = enabled.launch_events();
    events.send(started()).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(enabled.list().await[0].status, PluginStatus::Enabled);
    assert_eq!(
        enabled
            .last_transient_error(lumilio_plugin_discord::ID)
            .await,
        None
    );
}

#[tokio::test]
async fn disabling_during_a_stalled_handshake_cancels_without_publishing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("discord-ipc-0");
    let listener = UnixListener::bind(&path).unwrap();
    let host = host(path, true);
    let events = host.launch_events();
    events.send(started()).unwrap();
    let mut stream = accept(&listener).await;
    assert_eq!(frame(&mut stream).await.0, 0);
    host.set_state(
        lumilio_plugin_discord::ID.into(),
        PluginState {
            enabled: Some(false),
            ..Default::default()
        },
    );
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), stream.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert_eq!(host.list().await[0].status, PluginStatus::Disabled);
}

#[tokio::test]
async fn oversized_and_closed_ipc_frames_are_refused() {
    for opcode in [1, 2] {
        let (client, mut server) = tokio::io::duplex(1024);
        let mut client: Connection = Box::new(client);
        server.write_u32_le(opcode).await.unwrap();
        server
            .write_u32_le(if opcode == 1 { MAX_FRAME as u32 + 1 } else { 0 })
            .await
            .unwrap();
        assert!(receive(&mut client).await.is_err());
    }
}
