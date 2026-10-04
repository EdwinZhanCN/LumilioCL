mod accounts;
mod activity;
mod content;
mod diagnostics;
mod discover;
mod java;
mod launch;
mod lifecycle;
mod packs;
mod portability;
mod runtime;
mod screenshots;
mod servers;
mod settings;
mod skins;
mod snapshots;
mod third_party;
mod worlds;

use super::LauncherService;
use super::error::ServiceError;
use crate::activity::CancellationToken;
use crate::history::{ChangeKind, HistoryEvent, HistoryLog, SessionOutcome};
use crate::instance::{InstanceRecord, Loader, NewInstance};
use crate::process::GameExit;
use crate::transfer::{FileTransport, Transport, TransportFuture, TransportResponse};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc;

/// Answers addresses containing a key from a table, and `file:` addresses
/// from disk; everything else fails like an unreachable host.
type Answer = (String, Vec<u8>);
type Replies = BTreeMap<String, std::collections::VecDeque<(u16, String)>>;

#[derive(Clone, Default)]
struct Scripted {
    answers: Arc<StdMutex<Vec<Answer>>>,
    cancel_on_file: Arc<StdMutex<Option<CancellationToken>>>,
    /// When set, answers that exist are held until this is notified.
    gate: Arc<StdMutex<Option<Arc<tokio::sync::Notify>>>>,
    /// Answers to `send`, by exact address; the last one repeats.
    replies: Arc<StdMutex<Replies>>,
    sent: Arc<StdMutex<Vec<crate::transfer::HttpRequest>>>,
}

impl Scripted {
    fn reply(&self, url: &str, status: u16, body: &str) {
        self.replies
            .lock()
            .unwrap()
            .entry(url.to_owned())
            .or_default()
            .push_back((status, body.to_owned()));
    }
    fn clear_replies(&self, url: &str) {
        self.replies.lock().unwrap().remove(url);
    }
    fn sent_to(&self, url: &str) -> usize {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.url == url)
            .count()
    }
    fn answer(&self, key: &str, body: impl Into<Vec<u8>>) {
        self.answers
            .lock()
            .unwrap()
            .push((key.to_owned(), body.into()));
    }
    fn forget_all(&self) {
        self.answers.lock().unwrap().clear();
    }
}

impl Transport for Scripted {
    fn send<'a>(&'a self, request: crate::transfer::HttpRequest) -> TransportFuture<'a> {
        self.sent.lock().unwrap().push(request.clone());
        let mut replies = self.replies.lock().unwrap();
        let answer = replies.get_mut(&request.url).map(|queue| {
            if queue.len() > 1 {
                queue.pop_front().unwrap()
            } else {
                queue.front().cloned().unwrap()
            }
        });
        Box::pin(async move {
            match answer {
                Some((status, body)) => {
                    Ok(TransportResponse::from_bytes(status, body.into_bytes()))
                }
                None => Err(crate::transfer::TransportError::transient("no route")),
            }
        })
    }

    /// A POST is answered like a GET of the same address.
    fn post_json<'a>(&'a self, source: &'a str, _body: Vec<u8>) -> TransportFuture<'a> {
        self.get(source)
    }

    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        if source.starts_with("file:")
            && let Some(cancel) = self.cancel_on_file.lock().unwrap().take()
        {
            cancel.cancel();
            return Box::pin(std::future::pending());
        }
        let found = self
            .answers
            .lock()
            .unwrap()
            .iter()
            .find(|(key, _)| source.contains(key.as_str()))
            .map(|(_, body)| body.clone());
        let gate = self.gate.lock().unwrap().clone();
        match found {
            Some(body) => Box::pin(async move {
                if let Some(gate) = gate {
                    gate.notified().await;
                }
                Ok(TransportResponse::from_bytes(200, body))
            }),
            None if source.starts_with("file:") => FileTransport.get(source),
            None => {
                Box::pin(async { Err(crate::transfer::TransportError::permanent("unreachable")) })
            }
        }
    }
}

fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn file_url(path: &std::path::Path) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}

struct World {
    _dir: tempfile::TempDir,
    service: LauncherService<Scripted>,
    net: Scripted,
    server: PathBuf,
    secrets: Arc<crate::credentials::MemoryCredentials>,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let server = dir.path().join("server");
    std::fs::create_dir_all(&server).unwrap();
    let net = Scripted::default();
    let root = dir.path().join("launcher");
    let secrets = Arc::new(crate::credentials::MemoryCredentials::default());
    let service = LauncherService::open(&root, net.clone(), Vec::new())
        .unwrap()
        .with_runtime_roots(vec![root.join("runtimes")])
        .with_credentials(secrets.clone());
    service
        .settings
        .try_lock()
        .unwrap()
        .add_offline_account("Steve", None)
        .unwrap();
    World {
        _dir: dir,
        service,
        net,
        server,
        secrets,
    }
}

/// Publishes release `1.0` (a one-file client) and a catalog listing it.
fn publish_release(world: &World) {
    publish_release_with(
        world,
        r#""minecraftArguments":"--username ${auth_player_name}""#,
    );
}

/// A release that passes the whole identity to the game.
fn publish_identity_release(world: &World) {
    publish_release_with(
        world,
        r#""minecraftArguments":"--username ${auth_player_name} --uuid ${auth_uuid} --accessToken ${auth_access_token} --userType ${user_type}""#,
    );
}

/// A release whose game arguments declare direct quick play, like 1.20+.
fn publish_modern_release(world: &World) {
    publish_release_with(
        world,
        r#""arguments":{"game":[
            "--username","${auth_player_name}",
            {"rules":[{"action":"allow","features":{"has_quick_plays_support":true}}],
             "value":["--quickPlayPath","${quickPlayPath}"]},
            {"rules":[{"action":"allow","features":{"is_quick_play_singleplayer":true}}],
             "value":["--quickPlaySingleplayer","${quickPlaySingleplayer}"]},
            {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],
             "value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}]}"#,
    );
}

fn publish_release_with(world: &World, arguments: &str) {
    let client = b"pretend client jar";
    std::fs::write(world.server.join("client.jar"), client).unwrap();
    let manifest = format!(
        r#"{{"id":"1.0","mainClass":"net.example.Main",
            {arguments},
            "javaVersion":{{"component":"x","majorVersion":21}},
            "downloads":{{"client":{{"url":"{}","sha1":"{}","size":{}}}}},
            "libraries":[]}}"#,
        file_url(&world.server.join("client.jar")),
        sha1_hex(client),
        client.len()
    );
    std::fs::write(world.server.join("1.0.json"), manifest).unwrap();
    world.net.answer(
        "piston-meta.mojang.com/mc/game/version_manifest",
        format!(
            r#"{{"latest":{{"release":"1.0","snapshot":"1.0"}},"versions":[
                {{"id":"1.0","type":"release","releaseTime":"2024-01-01T00:00:00+00:00",
                  "url":"{}"}}]}}"#,
            file_url(&world.server.join("1.0.json"))
        ),
    );
}

fn fake_java(world: &World, script: &str) {
    let jdk = world.service.layout().runtimes().join("jdk-21");
    std::fs::create_dir_all(jdk.join("bin")).unwrap();
    std::fs::write(jdk.join("release"), "JAVA_VERSION=\"21.0.1\"\n").unwrap();
    let java = jdk.join("bin/java");
    std::fs::write(&java, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn assert_send<T: Send>(_: &T) {}

/// An instance with a profile holding one world file.
async fn instance_with_profile(world: &World, name: &str) -> InstanceRecord {
    let record = world
        .service
        .create_instance(name, Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let saves = world.service.layout.game(&record.id).join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("level.dat"), b"world").unwrap();
    record
}

fn operation_dirs(world: &World) -> Vec<PathBuf> {
    std::fs::read_dir(world.service.layout.operations())
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default()
}

/// Restarts an already-owned service (same transport is not needed here).
fn reopen_service(
    service: LauncherService<Scripted>,
    root: &std::path::Path,
) -> (LauncherService<Scripted>, ()) {
    drop(service);
    (
        LauncherService::open(root, Scripted::default(), Vec::new()).unwrap(),
        (),
    )
}

/// Closes `world`'s service and opens the same root again, as a restart.
fn reopen(world: World, root: &std::path::Path) -> (LauncherService<Scripted>, tempfile::TempDir) {
    let World {
        service, net, _dir, ..
    } = world;
    drop(service);
    (LauncherService::open(root, net, Vec::new()).unwrap(), _dir)
}

async fn is_installed(world: &World, id: &str) -> bool {
    world.service.instance(id).await.unwrap().installed
}

async fn outcomes_of(world: &World, id: &str) -> Vec<SessionOutcome> {
    HistoryLog::for_instance(world.service.layout.root(), id)
        .sessions()
        .unwrap()
        .into_iter()
        .map(|event| match event {
            HistoryEvent::Session { outcome, .. } => outcome,
            HistoryEvent::Change { .. } => unreachable!("sessions() filters"),
        })
        .collect()
}

fn change_subjects(world: &World, id: &str) -> Vec<(ChangeKind, String)> {
    HistoryLog::for_instance(world.service.layout.root(), id)
        .changes()
        .unwrap()
        .into_iter()
        .map(|event| match event {
            HistoryEvent::Change { kind, subject, .. } => (kind, subject),
            HistoryEvent::Session { .. } => unreachable!("changes() filters"),
        })
        .collect()
}

/// Publishes a fake Java runtime the way Mojang's index does.
fn publish_java(world: &World, bad_hash: bool) {
    let release = b"JAVA_VERSION=\"21.0.1\"\nIMPLEMENTOR=\"Test\"\n";
    let launcher = b"#!/bin/sh\necho java\n";
    std::fs::write(world.server.join("jrelease"), release).unwrap();
    std::fs::write(world.server.join("jjava"), launcher).unwrap();
    let hash = |bytes: &[u8]| {
        if bad_hash {
            "0".repeat(40)
        } else {
            sha1_hex(bytes)
        }
    };
    let manifest = format!(
        r#"{{"files":{{
            "bin":{{"type":"directory"}},
            "bin/java":{{"type":"file","executable":true,"downloads":{{"raw":
                {{"url":"{}","sha1":"{}","size":{}}}}}}},
            "release":{{"type":"file","downloads":{{"raw":
                {{"url":"{}","sha1":"{}","size":{}}}}}}},
            "bin/jre":{{"type":"link","target":"java"}}}}}}"#,
        file_url(&world.server.join("jjava")),
        hash(launcher),
        launcher.len(),
        file_url(&world.server.join("jrelease")),
        hash(release),
        release.len(),
    );
    std::fs::write(world.server.join("jmanifest.json"), &manifest).unwrap();
    let platform = crate::java_runtime::platform_key(&crate::environment::HostProfile::current())
        .expect("tests run on a platform with downloadable Java");
    let index = format!(
        r#"{{"{platform}":{{"java-runtime-delta":[{{"manifest":
            {{"url":"{}","sha1":"x","size":1}},"version":{{"name":"21.0.1"}}}}]}}}}"#,
        file_url(&world.server.join("jmanifest.json")),
    );
    world.net.answer("java-runtime/", index);
}

fn write_mrpack(path: &std::path::Path, index: &str, entries: &[(&str, &str)]) {
    use std::io::Write as _;
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file("modrinth.index.json", options).unwrap();
    writer.write_all(index.as_bytes()).unwrap();
    for (name, body) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

fn pack_index(files: &str) -> String {
    format!(
        r#"{{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Cool Pack",
            "files":[{files}],"dependencies":{{"minecraft":"1.21.1"}}}}"#
    )
}

fn pack_file(path: &str, body: &str, extra: &str) -> String {
    format!(
        r#"{{"path":"{path}","hashes":{{"sha1":"{}"}},"fileSize":{},
            "downloads":["https://cdn.modrinth.com/data/{path}"]{extra}}}"#,
        sha1_hex(body.as_bytes()),
        body.len()
    )
}

fn no_leftovers(world: &World) {
    assert!(
        operation_dirs(world).is_empty(),
        "{:?}",
        operation_dirs(world)
    );
}

async fn launch_to_end(world: &World, id: &str) -> Result<GameExit, ServiceError> {
    let (tx, _rx) = mpsc::unbounded_channel();
    world.service.launch(id, tx, CancellationToken::new()).await
}

const ECHO_ARGS: &str = "echo \"Setting user: x\"\necho \"$*\" > \"$INST_DIR/args.txt\"";

/// Publishes releases `ids` (each a one-file client) and a catalog listing them.
fn publish_versions(world: &World, ids: &[&str]) {
    let mut entries = Vec::new();
    for id in ids {
        let client = format!("client of {id}").into_bytes();
        std::fs::write(world.server.join(format!("{id}.jar")), &client).unwrap();
        let manifest = format!(
            r#"{{"id":"{id}","mainClass":"net.example.Main",
                "minecraftArguments":"--username ${{auth_player_name}}",
                "javaVersion":{{"component":"x","majorVersion":21}},
                "downloads":{{"client":{{"url":"{}","sha1":"{}","size":{}}}}},
                "libraries":[]}}"#,
            file_url(&world.server.join(format!("{id}.jar"))),
            sha1_hex(&client),
            client.len()
        );
        std::fs::write(world.server.join(format!("{id}.json")), manifest).unwrap();
        entries.push(format!(
            r#"{{"id":"{id}","type":"release","releaseTime":"2024-01-01T00:00:00+00:00","url":"{}"}}"#,
            file_url(&world.server.join(format!("{id}.json")))
        ));
    }
    world.net.answer(
        "piston-meta.mojang.com/mc/game/version_manifest",
        format!(
            r#"{{"latest":{{"release":"{0}","snapshot":"{0}"}},"versions":[{1}]}}"#,
            ids[0],
            entries.join(",")
        ),
    );
}

async fn repair_to_end(world: &World, id: &str) -> Result<(), ServiceError> {
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .repair_instance(id, tx, CancellationToken::new())
        .await
}

const MS_ID: &str = "123e4567e89b12d3a456426614174000";

/// Scripts the whole Microsoft chain, ending at `name`'s profile.
fn microsoft_chain(world: &World, name: &str, mc_token: &str) {
    use crate::microsoft::{
        DEVICE_CODE_URL, MINECRAFT_LOGIN_URL, PROFILE_URL, TOKEN_URL, XBL_URL, XSTS_URL,
    };
    for url in [
        DEVICE_CODE_URL,
        TOKEN_URL,
        XBL_URL,
        XSTS_URL,
        MINECRAFT_LOGIN_URL,
        PROFILE_URL,
    ] {
        world.net.clear_replies(url);
    }
    world.net.reply(
        DEVICE_CODE_URL,
        200,
        r#"{"device_code":"dc","user_code":"AB12CD","verification_uri":"https://www.microsoft.com/link","expires_in":900,"interval":0}"#,
    );
    world.net.reply(
        TOKEN_URL,
        200,
        r#"{"access_token":"ms-access","refresh_token":"ms-refresh-next"}"#,
    );
    world.net.reply(
        XBL_URL,
        200,
        r#"{"Token":"xbl","DisplayClaims":{"xui":[{"uhs":"h"}]}}"#,
    );
    world.net.reply(
        XSTS_URL,
        200,
        r#"{"Token":"xsts","DisplayClaims":{"xui":[{"uhs":"h"}]}}"#,
    );
    world.net.reply(
        MINECRAFT_LOGIN_URL,
        200,
        &format!(r#"{{"access_token":"{mc_token}","expires_in":86400}}"#),
    );
    world.net.reply(
        PROFILE_URL,
        200,
        &format!(r#"{{"id":"{MS_ID}","name":"{name}"}}"#),
    );
}

async fn sign_in(world: &World) -> Result<(String, String), ServiceError> {
    world
        .service
        .microsoft_sign_in(|_| {}, CancellationToken::new())
        .await
}

fn mod_version(world: &World) -> Vec<u8> {
    let jar = b"a mod";
    std::fs::write(world.server.join("cool.jar"), jar).unwrap();
    format!(
        r#"[{{"id":"v1","project_id":"P","name":"Cool","version_number":"1",
            "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
            "date_published":"2024-01-01T00:00:00Z",
            "files":[{{"url":"{}","filename":"cool.jar","primary":true,
                       "size":{},"hashes":{{"sha1":"{}"}}}}],
            "dependencies":[]}}]"#,
        file_url(&world.server.join("cool.jar")),
        jar.len(),
        sha1_hex(jar)
    )
    .into_bytes()
}

/// A one-version project document whose version requires `requires`.
fn project_versions(id: &str, game: &str, requires: &[&str]) -> Vec<u8> {
    let dependencies: Vec<String> = requires
        .iter()
        .map(|project| format!(r#"{{"project_id":"{project}","dependency_type":"required"}}"#))
        .collect();
    format!(
        r#"[{{"id":"v-{id}","project_id":"{id}","name":"{id}","version_number":"1",
            "version_type":"release","game_versions":["{game}"],"loaders":["fabric"],
            "date_published":"2024-01-01T00:00:00Z",
            "files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                       "primary":true,"size":1,"hashes":{{"sha1":"{id}"}}}}],
            "dependencies":[{}]}}]"#,
        dependencies.join(",")
    )
    .into_bytes()
}

async fn fabric_instance(world: &World, game: &str) -> InstanceRecord {
    world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: format!("Pick {game}"),
                game_version: game.to_owned(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.0".to_owned()),
            },
            1,
        )
        .unwrap()
        .clone()
}

fn two_versions(world: &World) -> Vec<u8> {
    for (name, body) in [("v1.jar", b"one".as_slice()), ("v2.jar", b"two".as_slice())] {
        std::fs::write(world.server.join(name), body).unwrap();
    }
    let entry = |id: &str, file: &str, games: &str, body: &[u8]| {
        format!(
            r#"{{"id":"{id}","project_id":"P","name":"{id}","version_number":"{id}",
                "version_type":"release","game_versions":{games},"loaders":["fabric"],
                "date_published":"2024-01-0{}T00:00:00Z",
                "files":[{{"url":"{}","filename":"{file}","primary":true,"size":{},
                           "hashes":{{"sha1":"{}"}}}}]}}"#,
            if id == "v1" { 1 } else { 2 },
            file_url(&world.server.join(file)),
            body.len(),
            sha1_hex(body)
        )
    };
    format!(
        "[{},{}]",
        entry("v1", "v1.jar", r#"["1.0"]"#, b"one"),
        entry("v2", "v2.jar", r#"["2.0"]"#, b"two")
    )
    .into_bytes()
}
