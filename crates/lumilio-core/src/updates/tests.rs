use super::*;
use crate::discover::{ReleaseChannel, VersionFile};
use crate::plugins::PluginHost;
use crate::transfer::{
    FileTransport, HttpRequest, OfficialSource, SourceChain, SourceProvider, TransportFuture,
    TransportResponse,
};
use lumilio_plugin_modrinth::Modrinth;
use std::fs;
use std::sync::{Arc, Mutex};

/// Answers the two update endpoints from canned JSON and records bodies.
#[derive(Clone)]
struct Api {
    identify: String,
    update: String,
    bodies: Arc<Mutex<Vec<(String, String)>>>,
}

impl Api {
    fn new(identify: impl Into<String>, update: impl Into<String>) -> Self {
        Self {
            identify: identify.into(),
            update: update.into(),
            bodies: Arc::default(),
        }
    }

    /// The Modrinth source plugin, asking through this transport.
    fn host(&self) -> PluginHost {
        PluginHost::new(vec![Arc::new(Modrinth)], Default::default()).with_network(
            self.clone(),
            SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>]).unwrap(),
        )
    }

    fn bodies(&self) -> Vec<(String, String)> {
        self.bodies.lock().unwrap().clone()
    }
}

impl Transport for Api {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        Box::pin(async { Ok(TransportResponse::from_bytes(404, Vec::new())) })
    }
    fn send_no_redirect<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        self.bodies.lock().unwrap().push((
            request.url.clone(),
            String::from_utf8(request.body.unwrap_or_default()).unwrap(),
        ));
        let answer = if request.url.ends_with("/v2/version_files/update") {
            self.update.clone()
        } else {
            self.identify.clone()
        };
        Box::pin(async move { Ok(TransportResponse::from_bytes(200, answer.into_bytes())) })
    }
}

fn version_json(id: &str, file: &str, url: &str, sha1: &str) -> String {
    format!(
        r#"{{"id":"{id}","project_id":"P","version_number":"{id}","game_versions":["1.21.1"],
            "loaders":["fabric"],"date_published":"2024-01-01T00:00:00Z",
            "files":[{{"url":"{url}","filename":"{file}","primary":true,"size":3,"hashes":{{"sha1":"{sha1}"}}}}]}}"#
    )
}

fn sha1_of(text: &str) -> String {
    use sha1::{Digest, Sha1};
    Sha1::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

struct Setup {
    _dir: tempfile::TempDir,
    game: std::path::PathBuf,
    server: std::path::PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(game.join("mods")).unwrap();
    let server = dir.path().join("server");
    fs::create_dir_all(&server).unwrap();
    Setup {
        game,
        server,
        _dir: dir,
    }
}

#[tokio::test]
async fn classifies_outdated_current_and_unknown_files() {
    let s = setup();
    fs::write(s.game.join("mods/outdated.jar"), "old").unwrap();
    fs::write(s.game.join("mods/current.jar"), "cur").unwrap();
    fs::write(s.game.join("mods/mystery.jar"), "???").unwrap();
    fs::write(s.game.join("mods/off.jar.disabled"), "off").unwrap();
    let (old, cur) = (sha1_of("old"), sha1_of("cur"));
    let identify = format!(
        "{{\"{old}\":{},\"{cur}\":{}}}",
        version_json("v1", "outdated.jar", "u1", &old),
        version_json("v9", "current.jar", "u2", &cur)
    );
    let update = format!(
        "{{\"{old}\":{},\"{cur}\":{}}}",
        version_json("v2", "outdated-2.jar", "u3", &sha1_of("new")),
        version_json("v9", "current.jar", "u2", &cur)
    );
    let host = Api::new(identify, update).host();
    let client = host.content_client().await.unwrap();
    let report = check(&client, &s.game, ProjectKind::Mod, "1.21.1", Loader::Fabric)
        .await
        .unwrap();
    assert_eq!(report.updates.len(), 1);
    assert_eq!(report.updates[0].file_name, "outdated.jar");
    assert_eq!(report.updates[0].latest.id, "v2");
    assert_eq!(report.up_to_date, 1);
    assert_eq!(report.unknown, ["mystery.jar"]);
}

#[tokio::test]
async fn requests_carry_hashes_loader_and_game_version_but_never_disabled_files() {
    let s = setup();
    fs::write(s.game.join("mods/a.jar"), "a").unwrap();
    fs::write(s.game.join("mods/b.jar.disabled"), "b").unwrap();
    let api = Api::new("{}", "{}");
    let host = api.host();
    let client = host.content_client().await.unwrap();
    check(&client, &s.game, ProjectKind::Mod, "1.21.1", Loader::Fabric)
        .await
        .unwrap();
    let bodies = api.bodies();
    let update_body: serde_json::Value = serde_json::from_str(
        &bodies
            .iter()
            .find(|(u, _)| u.ends_with("/update"))
            .unwrap()
            .1,
    )
    .unwrap();
    assert_eq!(update_body["hashes"], serde_json::json!([sha1_of("a")]));
    assert_eq!(update_body["loaders"], serde_json::json!(["fabric"]));
    assert_eq!(update_body["game_versions"], serde_json::json!(["1.21.1"]));
    assert_eq!(update_body["algorithm"], "sha1");
}

#[tokio::test]
async fn an_empty_folder_makes_no_requests() {
    let s = setup();
    let api = Api::new("{}", "{}");
    let host = api.host();
    let client = host.content_client().await.unwrap();
    let report = check(&client, &s.game, ProjectKind::Mod, "1.21.1", Loader::Fabric)
        .await
        .unwrap();
    assert_eq!(report, UpdateReport::default());
    assert!(api.bodies().is_empty());
}

#[tokio::test]
async fn a_stopped_source_fails_the_check_instead_of_reporting_everything_current() {
    use lumilio_plugin_api::PluginState;
    let s = setup();
    fs::write(s.game.join("mods/a.jar"), "a").unwrap();
    let api = Api::new("{}", "{}");
    let host = api.host();
    let client = host.content_client().await.unwrap();
    host.set_state(
        lumilio_plugin_modrinth::ID.into(),
        PluginState {
            enabled: Some(false),
            ..Default::default()
        },
    );
    let result = check(&client, &s.game, ProjectKind::Mod, "1.21.1", Loader::Fabric).await;
    assert!(matches!(
        result,
        Err(UpdateError::Discover(DiscoverError::NoSource))
    ));
    assert!(
        api.bodies().is_empty(),
        "a disabled source gets no requests"
    );
    assert!(matches!(
        host.content_client().await,
        Err(DiscoverError::NoSource)
    ));
}

fn update_for(server: &Path, new_name: &str, new_body: &str, old: &str) -> ContentUpdate {
    fs::write(server.join(new_name), new_body).unwrap();
    let url = url::Url::from_file_path(server.join(new_name))
        .unwrap()
        .to_string();
    let latest = Version {
        id: "v2".to_owned(),
        project_id: "P".to_owned(),
        name: String::new(),
        number: "v2".to_owned(),
        channel: ReleaseChannel::Release,
        game_versions: vec!["1.21.1".to_owned()],
        loaders: vec!["fabric".to_owned()],
        published: "2024-01-01T00:00:00Z".to_owned(),
        files: vec![VersionFile {
            url,
            filename: new_name.to_owned(),
            primary: true,
            size: 3,
            sha1: Some(sha1_of(new_body)),
        }],
        dependencies: Vec::new(),
        downloads: 0,
        changelog: String::new(),
    };
    ContentUpdate {
        kind: ProjectKind::Mod,
        file_name: old.to_owned(),
        current_sha1: sha1_of("old"),
        latest,
    }
}

#[tokio::test]
async fn applying_downloads_and_verifies_then_removes_the_old_file() {
    let s = setup();
    fs::write(s.game.join("mods/old.jar"), "old").unwrap();
    let update = update_for(&s.server, "new.jar", "new", "old.jar");
    let engine = TransferEngine::new(FileTransport, 2).unwrap();
    let sources = vec![update.latest.install_file().unwrap().url.clone()];
    let name = apply(&engine, &update, &s.game, sources, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(name, "new.jar");
    assert_eq!(
        fs::read_to_string(s.game.join("mods/new.jar")).unwrap(),
        "new"
    );
    assert!(!s.game.join("mods/old.jar").exists());
}

#[tokio::test]
async fn a_failed_download_keeps_the_old_file() {
    let s = setup();
    fs::write(s.game.join("mods/old.jar"), "old").unwrap();
    let mut update = update_for(&s.server, "new.jar", "new", "old.jar");
    // Corrupt what the "server" holds: the verified hash no longer matches.
    fs::write(s.server.join("new.jar"), "tampered").unwrap();
    update.latest.files[0].size = 0;
    let engine = TransferEngine::new(FileTransport, 2).unwrap();
    let sources = vec![update.latest.install_file().unwrap().url.clone()];
    let result = apply(&engine, &update, &s.game, sources, CancellationToken::new()).await;
    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(s.game.join("mods/old.jar")).unwrap(),
        "old"
    );
    assert!(!s.game.join("mods/new.jar").exists());
}

#[tokio::test]
async fn the_same_file_name_is_replaced_in_place() {
    let s = setup();
    fs::write(s.game.join("mods/same.jar"), "old").unwrap();
    let update = update_for(&s.server, "same.jar", "new", "same.jar");
    let engine = TransferEngine::new(FileTransport, 2).unwrap();
    let sources = vec![update.latest.install_file().unwrap().url.clone()];
    apply(&engine, &update, &s.game, sources, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(s.game.join("mods/same.jar")).unwrap(),
        "new"
    );
}
