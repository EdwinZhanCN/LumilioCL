//! `lumiliocl mcp` as an external agent uses it: a child process speaking
//! MCP over its standard streams (plan agent-mcp, T4).

use std::path::Path;

use lumilio_core::{InstanceStore, Layout, Loader, NewInstance};
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::json;
use tokio::process::Command;

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

#[tokio::test]
async fn serves_the_library_over_stdio() {
    let home = tempfile::tempdir().unwrap();
    let id = fixture(home.path());

    let launcher = Command::new(env!("CARGO_BIN_EXE_lumiliocl")).configure(|command| {
        command.arg("mcp").env("LUMILIO_HOME", home.path());
    });
    let client = ().serve(TokioChildProcess::new(launcher).unwrap()).await.unwrap();

    let info = client.peer_info().expect("server info");
    assert_eq!(info.server_info.as_ref().unwrap().name, "LumilioCL");
    let names: Vec<_> = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert!(names.contains(&"read_schematic".to_owned()), "{names:?}");

    let files = client
        .call_tool(
            CallToolRequestParams::new("list_schematics")
                .with_arguments(json!({ "instance": id }).as_object().cloned().unwrap()),
        )
        .await
        .unwrap();
    assert_ne!(files.is_error, Some(true), "{:?}", files.content);
    assert_eq!(
        files.structured_content.unwrap()["files"],
        json!(["schematics/3x3.litematic"])
    );

    // A clean close: the launcher exits when its client goes away.
    client.cancel().await.unwrap();
}
