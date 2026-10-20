//! An MCP client in the same process, over an in-memory pipe: the protocol
//! path an external agent takes, without a child process.

use std::path::Path;
use std::sync::Arc;

use lumilio_core::{FileTransport, InstanceStore, LauncherService, Layout, Loader, NewInstance};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};

use crate::LauncherMcp;

/// A small schematic from Nucleation's samples (MIT).
const SAMPLE: &[u8] = include_bytes!("../../../forks/nucleation/tests/samples/3x3.litematic");

/// A library with one game that holds one schematic; returns the game's id.
fn fixture(home: &Path) -> String {
    let id = {
        let mut store = InstanceStore::open(home).unwrap();
        let record = store
            .create(
                NewInstance {
                    name: "Redstone lab".into(),
                    game_version: "1.21.4".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                },
                1_700_000_000,
            )
            .unwrap();
        record.id.clone()
    };
    let folder = Layout::new(home).game(&id).join("schematics");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("3x3.litematic"), SAMPLE).unwrap();
    id
}

async fn connect(home: &Path) -> RunningService<RoleClient, ()> {
    let service = LauncherService::open(
        home,
        FileTransport,
        vec![Arc::new(lumilio_plugin_litematica::Litematica)],
    )
    .unwrap();
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    let server = LauncherMcp::new(Arc::new(service));
    tokio::spawn(async move {
        let running = server.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    ().serve(client_io).await.unwrap()
}

async fn call(client: &RunningService<RoleClient, ()>, tool: &str, arguments: Value) -> Value {
    let arguments = arguments.as_object().cloned().unwrap();
    let result = client
        .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(arguments))
        .await
        .unwrap();
    assert_ne!(
        result.is_error,
        Some(true),
        "{tool} failed: {:?}",
        result.content
    );
    result.structured_content.expect("structured result")
}

#[tokio::test]
async fn announces_the_tools_and_instructions() {
    let home = tempfile::tempdir().unwrap();
    fixture(home.path());
    let client = connect(home.path()).await;

    let info = client.peer_info().expect("server info");
    assert!(info.instructions.as_deref().unwrap().contains("Minecraft"));
    let names: Vec<_> = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(
        names,
        [
            "get_instance",
            "list_instances",
            "list_schematics",
            "read_schematic"
        ]
    );
}

#[tokio::test]
async fn reads_a_schematic_through_the_library() {
    let home = tempfile::tempdir().unwrap();
    let id = fixture(home.path());
    let client = connect(home.path()).await;

    let instances = call(&client, "list_instances", json!({})).await;
    let listed = &instances["instances"][0];
    assert_eq!(listed["id"], id);
    assert_eq!(listed["name"], "Redstone lab");
    assert_eq!(listed["loader"], "vanilla");
    assert!(
        listed["game_directory"]
            .as_str()
            .unwrap()
            .ends_with(&format!("{id}/game"))
    );

    let one = call(&client, "get_instance", json!({ "instance": id })).await;
    assert_eq!(one["game_version"], "1.21.4");

    let files = call(&client, "list_schematics", json!({ "instance": id })).await;
    assert_eq!(files["files"], json!(["schematics/3x3.litematic"]));

    let schematic = call(
        &client,
        "read_schematic",
        json!({ "instance": id, "file": "schematics/3x3.litematic" }),
    )
    .await;
    assert_eq!(schematic["file"], "schematics/3x3.litematic");
    assert!(schematic["block_count"].as_u64().unwrap() > 0);
    assert_eq!(schematic["regions"].as_array().unwrap().len(), 1);
    assert!(
        schematic["blocks"][0]["id"]
            .as_str()
            .unwrap()
            .starts_with("minecraft:")
    );
}

#[tokio::test]
async fn a_missing_instance_is_a_tool_error_the_agent_can_read() {
    let home = tempfile::tempdir().unwrap();
    fixture(home.path());
    let client = connect(home.path()).await;

    let result = client
        .call_tool(
            CallToolRequestParams::new("get_instance")
                .with_arguments(json!({ "instance": "nope" }).as_object().cloned().unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    let text = result.content[0].as_text().unwrap().text.clone();
    assert!(text.contains("nope"), "{text}");

    let result = client
        .call_tool(
            CallToolRequestParams::new("read_schematic").with_arguments(
                json!({ "instance": "nope", "file": "schematics/3x3.litematic" })
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
}
