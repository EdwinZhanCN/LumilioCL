//! An MCP client over real loopback HTTP: the path an external agent takes.

use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::sync::Arc;

use lumilio_core::{FileTransport, InstanceStore, LauncherService, Layout, Loader, NewInstance};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};

use crate::{Running, access, serve};

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

fn open(home: &Path) -> Arc<LauncherService<FileTransport>> {
    Arc::new(
        LauncherService::open(
            home,
            FileTransport,
            vec![Arc::new(lumilio_plugin_litematica::Litematica)],
        )
        .unwrap(),
    )
}

async fn connect(running: &Running, token: &str) -> RunningService<RoleClient, ()> {
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::new(),
        StreamableHttpClientTransportConfig::with_uri(running.url()).auth_header(token),
    );
    ().serve(transport).await.unwrap()
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
    let running = serve(open(home.path()), 0).await.unwrap();
    let client = connect(&running, running.token()).await;

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
    let running = serve(open(home.path()), 0).await.unwrap();
    let client = connect(&running, running.token()).await;

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
    let running = serve(open(home.path()), 0).await.unwrap();
    let client = connect(&running, running.token()).await;

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
}

/// The handshake a client would send, as a bare HTTP request.
fn initialize() -> Value {
    json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "probe", "version": "0" }
        }
    })
}

#[tokio::test]
async fn refuses_requests_without_the_token_and_from_browsers() {
    let home = tempfile::tempdir().unwrap();
    let running = serve(open(home.path()), 0).await.unwrap();
    let http = reqwest::Client::new();
    let post = |builder: reqwest::RequestBuilder| {
        builder
            .header("accept", "application/json, text/event-stream")
            .json(&initialize())
            .send()
    };

    let anonymous = post(http.post(running.url())).await.unwrap();
    assert_eq!(anonymous.status(), 401);
    assert!(anonymous.headers().contains_key("www-authenticate"));

    let wrong = post(http.post(running.url()).bearer_auth("not-it"))
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);

    let from_a_page = post(
        http.post(running.url())
            .bearer_auth(running.token())
            .header("origin", "http://evil.example"),
    )
    .await
    .unwrap();
    assert_eq!(from_a_page.status(), 403);

    let elsewhere = post(
        http.post(running.url().replace("/mcp", "/other"))
            .bearer_auth(running.token()),
    )
    .await
    .unwrap();
    assert_eq!(elsewhere.status(), 404);

    let proper = post(http.post(running.url()).bearer_auth(running.token()))
        .await
        .unwrap();
    assert_eq!(proper.status(), 200);
}

#[tokio::test]
async fn keeps_one_token_per_data_folder_and_moves_off_a_busy_port() {
    let home = tempfile::tempdir().unwrap();
    let busy = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let wanted = busy.local_addr().unwrap().port();

    let first = serve(open(home.path()), wanted).await.unwrap();
    assert!(
        (wanted + 1..=wanted + 20).contains(&first.port()),
        "{} is not above the busy port {wanted}",
        first.port()
    );
    let token = first.token().to_owned();
    assert_eq!(token.len(), 64);
    let file = access::path(home.path());
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(written["url"], first.url());
    assert_eq!(written["token"], token);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    first.shutdown().await;

    // The same folder, a later run: the token stays, the address is current.
    let again = serve(open(home.path()), 0).await.unwrap();
    assert_eq!(again.token(), token);
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(written["url"], again.url());
    let client = connect(&again, &token).await;
    assert!(!client.list_all_tools().await.unwrap().is_empty());
}
