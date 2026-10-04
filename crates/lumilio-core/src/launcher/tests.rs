use super::Launcher;
use super::progress::runtimes_root;
use super::types::{LaunchRequest, LaunchServiceError, LaunchUpdate};
use crate::activity::CancellationToken;
use crate::instance::Loader;
use crate::launch_session::{LaunchFailure, LaunchPhase, LaunchSignal};
use crate::process::GameExit;
use crate::transfer::SourceChain;
use crate::tuning::LaunchTuning;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::instance::{InstanceStore, NewInstance};
use crate::java::JavaLocator;
use crate::launch_session::{LaunchSession, LaunchStatus};
use crate::transfer::{FileTransport, OfficialSource};
use sha1::{Digest, Sha1};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
    request: LaunchRequest,
}

fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A launcher root with a fake Java, a local "server" holding a one-file
/// release, and an instance pointing at it.
fn world(java_script: &str) -> World {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    let server = dir.path().join("server");
    std::fs::create_dir_all(&server).unwrap();

    let client = b"pretend client jar";
    std::fs::write(server.join("client.jar"), client).unwrap();
    let client_url = url::Url::from_file_path(server.join("client.jar")).unwrap();
    let manifest = format!(
        r#"{{
            "id": "1.0",
            "mainClass": "net.example.Main",
            "minecraftArguments": "--username ${{auth_player_name}} --version ${{version_name}}",
            "javaVersion": {{"component": "java-runtime-gamma", "majorVersion": 21}},
            "downloads": {{"client": {{"url": "{client_url}", "sha1": "{}", "size": {}}}}},
            "libraries": []
        }}"#,
        sha1_hex(client),
        client.len()
    );
    std::fs::write(server.join("1.0.json"), &manifest).unwrap();
    let manifest_url = url::Url::from_file_path(server.join("1.0.json")).unwrap();

    // A fake JDK 21 whose "java" is a script.
    let jdk = root.join("runtimes/jdk-21");
    std::fs::create_dir_all(jdk.join("bin")).unwrap();
    std::fs::write(jdk.join("release"), "JAVA_VERSION=\"21.0.1\"\n").unwrap();
    let java = jdk.join("bin/java");
    std::fs::write(&java, format!("#!/bin/sh\n{java_script}\n")).unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut store = InstanceStore::open(&root).unwrap();
    let instance = store
        .create(
            NewInstance {
                name: "Test".to_owned(),
                game_version: "1.0".to_owned(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            1,
        )
        .unwrap()
        .clone();
    let directories = store.directories(&instance);
    let runtimes = JavaLocator::new([runtimes_root(&root)]).discover();
    assert_eq!(runtimes.len(), 1);
    World {
        request: LaunchRequest {
            instance,
            directories,
            session: crate::account::OfflineProfile::new("Steve")
                .unwrap()
                .session(),
            manifest_url: Some(manifest_url.to_string()),
            loader_profile_url: None,
            runtimes,
            default_max_memory_mb: Some(1024),
            default_min_memory_mb: None,
            tuning: LaunchTuning::default(),
            download_concurrency: None,
            quick_play: None,
        },
        root,
        _dir: dir,
    }
}

fn launcher() -> Launcher<FileTransport> {
    let chain =
        SourceChain::new([Arc::new(OfficialSource) as Arc<dyn crate::transfer::SourceProvider>])
            .unwrap();
    Launcher::new(FileTransport, chain)
}

async fn run_to_end(
    launcher: &Launcher<FileTransport>,
    request: LaunchRequest,
) -> (Result<GameExit, LaunchServiceError>, Vec<LaunchUpdate>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let result = launcher.launch(request, tx, CancellationToken::new()).await;
    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    (result, updates)
}

#[tokio::test]
async fn invalid_effective_memory_fails_before_installing_or_starting() {
    let mut world = world("echo 'Setting user: Steve'");
    world.request.instance.settings.min_memory_mb = Some(4096);
    let (result, updates) = run_to_end(&launcher(), world.request).await;
    assert!(matches!(result, Err(LaunchServiceError::InvalidMemory)));
    assert!(
        updates
            .iter()
            .any(|update| matches!(update, LaunchUpdate::Signal(LaunchSignal::Failed(_))))
    );
    assert!(!world.root.join("meta/versions").exists());
    assert!(!world.root.join("profiles/test/game").exists());
}

fn session_of(updates: &[LaunchUpdate]) -> LaunchSession {
    let mut session = LaunchSession::new();
    for update in updates {
        if let LaunchUpdate::Signal(signal) = update {
            session.apply(signal.clone());
        }
    }
    session
}

#[tokio::test]
async fn installs_then_runs_a_release_end_to_end() {
    let world = world("echo \"Setting user: $*\"\nexit 0");
    let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
    let exit = result.unwrap();
    assert_eq!(exit.code, Some(0));

    // The client and manifest were published.
    assert!(world.root.join("meta/versions/1.0/1.0.jar").is_file());
    assert!(world.root.join("meta/versions/1.0/1.0.json").is_file());

    // The session walked the phases and ended cleanly.
    let phases: Vec<_> = updates
        .iter()
        .filter_map(|u| match u {
            LaunchUpdate::Signal(LaunchSignal::Phase(phase)) => Some(*phase),
            _ => None,
        })
        .collect();
    assert_eq!(phases.first(), Some(&LaunchPhase::Verifying));
    assert!(phases.contains(&LaunchPhase::Libraries));
    assert_eq!(phases.last(), Some(&LaunchPhase::Starting));
    assert_eq!(
        session_of(&updates).status(),
        &LaunchStatus::Exited { code: Some(0) }
    );

    // The fake Java saw the substituted arguments and printed its marker.
    let logged: Vec<_> = updates
        .iter()
        .filter_map(|u| match u {
            LaunchUpdate::Log { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        logged
            .iter()
            .any(|line| line.contains("--username Steve --version 1.0")),
        "game arguments reach the process: {logged:?}"
    );
}

#[tokio::test]
async fn a_second_launch_skips_the_install() {
    let world = world("echo \"Setting user: x\"");
    let launcher = launcher();
    run_to_end(&launcher, world.request.clone())
        .await
        .0
        .unwrap();
    let (result, updates) = run_to_end(&launcher, world.request.clone()).await;
    result.unwrap();
    assert!(!updates.iter().any(|u| matches!(
        u,
        LaunchUpdate::Signal(LaunchSignal::Phase(LaunchPhase::Libraries))
    )));
}

#[tokio::test]
async fn a_second_instance_of_the_same_version_installs_nothing() {
    let world = world("echo \"Setting user: x\"");
    let launcher = launcher();
    run_to_end(&launcher, world.request.clone())
        .await
        .0
        .unwrap();

    let mut store = InstanceStore::open(&world.root).unwrap();
    let other = store
        .create(
            NewInstance {
                name: "Other".to_owned(),
                game_version: "1.0".to_owned(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            2,
        )
        .unwrap()
        .clone();
    let mut request = world.request.clone();
    request.directories = store.directories(&other);
    request.instance = other;
    assert_ne!(request.directories.game(), world.request.directories.game());

    let (result, updates) = run_to_end(&launcher, request).await;
    result.unwrap();
    assert!(!updates.iter().any(|u| matches!(
        u,
        LaunchUpdate::Signal(LaunchSignal::Phase(LaunchPhase::Libraries))
    )));
    assert!(world.root.join("profiles/other/game").is_dir());
}

#[tokio::test]
async fn a_damaged_client_is_repaired_on_the_next_launch() {
    let world = world("echo \"Setting user: x\"");
    let launcher = launcher();
    run_to_end(&launcher, world.request.clone())
        .await
        .0
        .unwrap();
    let jar = world.root.join("meta/versions/1.0/1.0.jar");
    std::fs::write(&jar, "corrupt").unwrap();
    run_to_end(&launcher, world.request.clone())
        .await
        .0
        .unwrap();
    assert_eq!(std::fs::read(&jar).unwrap(), b"pretend client jar");
}

#[tokio::test]
async fn progress_reports_item_counts_for_the_download_phase() {
    let world = world("echo \"Setting user: x\"");
    let (_, updates) = run_to_end(&launcher(), world.request.clone()).await;
    let counts: Vec<_> = updates
        .iter()
        .filter_map(|u| match u {
            LaunchUpdate::Signal(LaunchSignal::Progress { done, total }) => Some((*done, *total)),
            _ => None,
        })
        .collect();
    assert!(
        counts.contains(&(0, 1)) && counts.contains(&(1, 1)),
        "{counts:?}"
    );
}

#[tokio::test]
async fn a_missing_java_fails_with_a_clear_reason_and_a_terminal_signal() {
    let mut world = world("exit 0");
    world.request.runtimes.clear();
    let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
    assert!(matches!(
        result,
        Err(LaunchServiceError::NoJava { required: Some(21) })
    ));
    assert!(matches!(
        session_of(&updates).status(),
        LaunchStatus::Failed { failure: LaunchFailure::Step { message }, .. }
            if message.contains("Java 21")
    ));
}

#[tokio::test]
async fn a_game_that_dies_at_once_fails_the_session() {
    let world = world("echo crash >&2\nexit 1");
    let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
    assert_eq!(result.unwrap().code, Some(1));
    assert!(matches!(
        session_of(&updates).status(),
        LaunchStatus::Failed {
            failure: LaunchFailure::ExitedEarly { code: Some(1) },
            ..
        }
    ));
}

#[tokio::test]
async fn a_forge_instance_without_a_build_is_refused_before_any_work() {
    let mut world = world("exit 0");
    world.request.instance.loader = Loader::Forge;
    world.request.instance.loader_version = None;
    let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
    assert!(matches!(
        result,
        Err(LaunchServiceError::LoaderVersionMissing)
    ));
    assert!(session_of(&updates).is_finished());
    assert!(!world.root.join("meta/versions").exists());
}

#[tokio::test]
async fn an_unknown_version_without_an_address_is_reported() {
    let mut world = world("exit 0");
    world.request.manifest_url = None;
    let (result, _) = run_to_end(&launcher(), world.request.clone()).await;
    assert!(matches!(
        result,
        Err(LaunchServiceError::ManifestUnavailable(_))
    ));
}

/// Turns the world's instance into a Fabric one served from the local
/// "server": a profile that inherits the vanilla release and adds a
/// library from a maven-style folder.
fn make_fabric(world: &mut World) {
    let server = world._dir.path().join("server");
    let lib_dir = server.join("maven/net/example/loader-lib/1.0");
    std::fs::create_dir_all(&lib_dir).unwrap();
    std::fs::write(lib_dir.join("loader-lib-1.0.jar"), "loader library").unwrap();
    let base = url::Url::from_directory_path(server.join("maven")).unwrap();
    let profile = format!(
        r#"{{"id":"whatever-upstream-calls-it","inheritsFrom":"1.0",
            "mainClass":"net.example.LoaderMain",
            "libraries":[{{"name":"net.example:loader-lib:1.0","url":"{base}"}}]}}"#
    );
    std::fs::write(server.join("profile.json"), profile).unwrap();
    world.request.instance.loader = Loader::Fabric;
    world.request.instance.loader_version = Some("0.16.0".to_owned());
    world.request.loader_profile_url = Some(
        url::Url::from_file_path(server.join("profile.json"))
            .unwrap()
            .to_string(),
    );
}

#[tokio::test]
async fn a_fabric_instance_installs_its_profile_and_starts_the_loader() {
    let mut world = world("echo \"Setting user: $*\"\nexit 0");
    make_fabric(&mut world);
    assert_eq!(
        world.request.instance.release_id().as_deref(),
        Some("fabric-loader-0.16.0-1.0")
    );
    let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
    assert_eq!(result.unwrap().code, Some(0));
    let logged: String = updates
        .iter()
        .filter_map(|u| match u {
            LaunchUpdate::Log { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(logged.contains("net.example.LoaderMain"), "{logged}");
    assert!(logged.contains("loader-lib-1.0.jar"), "{logged}");
    assert!(
        logged.contains("fabric-loader-0.16.0-1.0.jar"),
        "the client jar is on the classpath: {logged}"
    );
    assert!(
        world
            .root
            .join("meta/libraries/net/example/loader-lib/1.0/loader-lib-1.0.jar")
            .is_file()
    );
    assert!(
        world
            .root
            .join("meta/versions/fabric-loader-0.16.0-1.0/fabric-loader-0.16.0-1.0.json")
            .is_file()
    );
    assert!(
        world
            .root
            .join("meta/versions/fabric-loader-0.16.0-1.0/fabric-loader-0.16.0-1.0.jar")
            .is_file()
    );
    assert_eq!(
        session_of(&updates).status(),
        &LaunchStatus::Exited { code: Some(0) }
    );
}

#[tokio::test]
async fn an_installed_loader_instance_launches_without_any_network_address() {
    let mut world = world("echo \"Setting user: $*\"");
    make_fabric(&mut world);
    let launcher = launcher();
    run_to_end(&launcher, world.request.clone())
        .await
        .0
        .unwrap();
    world.request.manifest_url = None;
    world.request.loader_profile_url = None;
    let (result, updates) = run_to_end(&launcher, world.request.clone()).await;
    result.unwrap();
    assert!(!updates.iter().any(|u| matches!(
        u,
        LaunchUpdate::Signal(LaunchSignal::Phase(LaunchPhase::Libraries))
    )));
}

#[tokio::test]
async fn a_loader_instance_without_a_version_or_address_reports_why() {
    let mut world = world("exit 0");
    make_fabric(&mut world);
    world.request.instance.loader_version = None;
    let (result, _) = run_to_end(&launcher(), world.request.clone()).await;
    assert!(matches!(
        result,
        Err(LaunchServiceError::LoaderVersionMissing)
    ));

    make_fabric(&mut world);
    world.request.loader_profile_url = None;
    let (result, _) = run_to_end(&launcher(), world.request.clone()).await;
    assert!(matches!(
        result,
        Err(LaunchServiceError::ManifestUnavailable(_))
    ));
}
