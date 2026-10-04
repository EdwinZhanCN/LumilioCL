use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use sha1::{Digest, Sha1};

use super::*;
use crate::launch_session::{LaunchSession, LaunchSignal};
use crate::transfer::{FileTransport, TransportFuture, TransportResponse};

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
    let service = LauncherService::open(&root, net.clone())
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

#[tokio::test]
async fn explicit_cancel_unblocks_reads_and_records_cancelled_for_both_install_kinds() {
    let world = world();
    let record = world
        .service
        .create_instance("Target", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let defaults = world.service.settings.lock().await;
    let cancel = CancellationToken::new();
    let mut content = Box::pin(world.service.install_content(
        &record.id,
        ProjectKind::Mod,
        "project",
        cancel.clone(),
    ));
    assert!(futures_util::poll!(&mut content).is_pending());
    cancel.cancel();
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), content)
            .await
            .unwrap(),
        Err(ServiceError::Cancelled)
    ));
    let cancel = CancellationToken::new();
    let mut pack = Box::pin(world.service.install_modpack("pack", cancel.clone()));
    assert!(futures_util::poll!(&mut pack).is_pending());
    cancel.cancel();
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), pack)
            .await
            .unwrap(),
        Err(ServiceError::Cancelled)
    ));
    let activity = world.service.activity(10);
    assert!(activity.active.is_empty());
    assert_eq!(activity.finished.len(), 2);
    assert!(
        activity
            .finished
            .iter()
            .all(|task| task.outcome == TaskOutcome::Cancelled)
    );
    assert!(!world.service.layout.game(&record.id).exists());
    world.service.delete_instance(&record.id).await.unwrap();
    drop(defaults);
}

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

#[tokio::test]
async fn delete_removes_only_that_profile_and_leaves_no_journal() {
    let world = world();
    publish_release(&world);
    let target = instance_with_profile(&world, "Target").await;
    let other = instance_with_profile(&world, "Other").await;
    let meta = world.service.layout.meta();
    std::fs::create_dir_all(&meta).unwrap();
    std::fs::write(meta.join("shared.jar"), b"shared").unwrap();
    world.service.delete_instance(&target.id).await.unwrap();
    assert!(world.service.instance(&target.id).await.is_err());
    assert!(!world.service.layout.profile(&target.id).exists());
    assert!(
        world
            .service
            .layout
            .game(&other.id)
            .join("saves/level.dat")
            .exists()
    );
    assert!(meta.join("shared.jar").exists());
    assert!(operation_dirs(&world).is_empty());
}

#[tokio::test]
async fn failed_isolation_keeps_record_and_profile_and_allows_retry() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    let profiles = world.service.layout.profiles();
    std::fs::set_permissions(&profiles, std::fs::Permissions::from_mode(0o555)).unwrap();
    let failed = world.service.delete_instance(&record.id).await;
    std::fs::set_permissions(&profiles, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(failed, Err(ServiceError::Io(_))));
    assert!(world.service.instance(&record.id).await.is_ok());
    assert!(
        world
            .service
            .layout
            .game(&record.id)
            .join("saves/level.dat")
            .exists()
    );
    assert!(operation_dirs(&world).is_empty());
    world.service.delete_instance(&record.id).await.unwrap();
    assert!(!world.service.layout.profile(&record.id).exists());
}

#[tokio::test]
async fn failed_library_commit_restores_the_profile() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    world.service.store.lock().await.set_read_only(true);
    let failed = world.service.delete_instance(&record.id).await;
    world.service.store.lock().await.set_read_only(false);
    assert!(matches!(failed, Err(ServiceError::Store(_))));
    assert!(world.service.instance(&record.id).await.is_ok());
    assert_eq!(
        std::fs::read(
            world
                .service
                .layout
                .game(&record.id)
                .join("saves/level.dat")
        )
        .unwrap(),
        b"world"
    );
    assert!(operation_dirs(&world).is_empty());
    world.service.delete_instance(&record.id).await.unwrap();
}

#[tokio::test]
async fn failed_cleanup_still_deletes_and_the_next_start_finishes_it() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    // A read-only folder inside the profile makes the final removal fail.
    let locked = world.service.layout.game(&record.id).join("saves");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    world.service.delete_instance(&record.id).await.unwrap();
    assert!(world.service.instance(&record.id).await.is_err());
    let leftovers = operation_dirs(&world);
    assert_eq!(leftovers.len(), 1, "journal and files stay for recovery");
    std::fs::set_permissions(
        leftovers[0].join("profile/game/saves"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert_eq!(
        service.startup_notes(),
        [RecoveryNote::DeleteCompleted {
            instance_id: record.id.clone()
        }]
    );
    assert!(
        service
            .layout
            .operations()
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn crash_after_isolation_before_commit_rolls_back_on_next_start() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Target").await;
    // Rehearse the crash: journal and quarantine happened, the library did not commit.
    let mut crashed = Deletion::begin(&world.service.layout, &record, 7).unwrap();
    crashed.quarantine().unwrap();
    drop(crashed);
    assert!(!world.service.layout.profile(&record.id).exists());
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert_eq!(
        service.startup_notes(),
        [RecoveryNote::DeleteRolledBack {
            instance_id: record.id.clone()
        }]
    );
    assert!(service.instance(&record.id).await.is_ok());
    assert_eq!(
        std::fs::read(service.layout.game(&record.id).join("saves/level.dat")).unwrap(),
        b"world"
    );
    assert!(
        service
            .layout
            .operations()
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn crash_after_commit_completes_and_conflicts_or_bad_journals_are_kept() {
    let world = world();
    publish_release(&world);
    let committed = instance_with_profile(&world, "Committed").await;
    let conflict = instance_with_profile(&world, "Conflict").await;
    let broken = instance_with_profile(&world, "Broken").await;
    let layout = world.service.layout.clone();
    let mut crashed = Deletion::begin(&layout, &committed, 1).unwrap();
    crashed.quarantine().unwrap();
    world
        .service
        .store
        .lock()
        .await
        .remove(&committed.id)
        .unwrap();
    drop(crashed);
    // Both copies exist for the conflicting one.
    let mut both = Deletion::begin(&layout, &conflict, 2).unwrap();
    both.quarantine().unwrap();
    std::fs::create_dir_all(layout.profile(&conflict.id)).unwrap();
    drop(both);
    // An unreadable journal beside real files must not be removed.
    let mut bad = Deletion::begin(&layout, &broken, 3).unwrap();
    bad.quarantine().unwrap();
    drop(bad);
    let bad_dir = layout.operations().join(format!("delete-{}-3", broken.id));
    std::fs::write(bad_dir.join("delete.json"), b"{ not json").unwrap();
    let root = layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    let notes = service.startup_notes();
    assert_eq!(notes.len(), 3, "{notes:?}");
    assert!(notes.contains(&RecoveryNote::DeleteCompleted {
        instance_id: committed.id.clone()
    }));
    assert!(notes.iter().any(|note| matches!(
        note,
        RecoveryNote::DeleteConflict { instance_id, .. } if *instance_id == conflict.id
    )));
    assert!(notes.iter().any(|note| matches!(
        note,
        RecoveryNote::JournalUnusable { path, .. } if *path == bad_dir
    )));
    assert!(
        layout
            .operations()
            .join(format!("delete-{}-2", conflict.id))
            .join("profile")
            .exists()
    );
    assert!(bad_dir.join("profile/game/saves/level.dat").exists());
    assert!(
        !layout
            .operations()
            .join(format!("delete-{}-1", committed.id))
            .exists()
    );
}

#[tokio::test]
async fn damaged_library_is_kept_reported_and_never_overwritten_by_a_second_failure() {
    let world = world();
    publish_release(&world);
    let record = instance_with_profile(&world, "Lost").await;
    let root = world.service.layout.root().to_path_buf();
    let (service, dir) = reopen(world, &root);
    drop(service);
    let database = root.join("launcher.db");
    std::fs::write(&database, b"first damage").unwrap();
    let service = LauncherService::open(&root, Scripted::default()).unwrap();
    let [
        RecoveryNote::LibraryRecovered {
            preserved,
            candidates,
        },
    ] = service.startup_notes()
    else {
        panic!("{:?}", service.startup_notes());
    };
    assert_eq!(std::fs::read(preserved).unwrap(), b"first damage");
    assert_eq!(candidates, std::slice::from_ref(&record.id));
    let preserved = preserved.clone();
    assert!(service.library().await.instances.is_empty());
    assert!(root.join("profiles").join(&record.id).exists());
    drop(service);
    std::fs::write(&database, b"second damage").unwrap();
    let service = LauncherService::open(&root, Scripted::default()).unwrap();
    let [
        RecoveryNote::LibraryRecovered {
            preserved: second, ..
        },
    ] = service.startup_notes()
    else {
        panic!("{:?}", service.startup_notes());
    };
    assert_ne!(*second, preserved);
    assert_eq!(std::fs::read(second).unwrap(), b"second damage");
    assert_eq!(std::fs::read(&preserved).unwrap(), b"first damage");
    drop(dir);
}

#[tokio::test]
async fn newer_schema_refuses_to_open_and_leaves_the_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    drop(LauncherService::open(&root, Scripted::default()).unwrap());
    {
        let db = rusqlite::Connection::open(root.join("launcher.db")).unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
    }
    let before = std::fs::read(root.join("launcher.db")).unwrap();
    let refused = LauncherService::open(&root, Scripted::default());
    assert!(matches!(
        refused,
        Err(ServiceError::Store(StoreError::NewerSchema(99)))
    ));
    assert_eq!(std::fs::read(root.join("launcher.db")).unwrap(), before);
    assert!(!root.join("launcher.db.broken").exists());
    // The refused open must not keep the root locked.
    assert!(matches!(
        LauncherService::open(&root, Scripted::default()),
        Err(ServiceError::Store(StoreError::NewerSchema(99)))
    ));
}

#[tokio::test]
async fn damaged_settings_and_torn_activity_lines_are_reported_with_counts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    drop(LauncherService::open(&root, Scripted::default()).unwrap());
    std::fs::write(root.join("settings.json"), b"{ nope").unwrap();
    let good = FinishedTask {
        category: TaskCategory::Install,
        label: "ok".into(),
        instance_id: None,
        started: 1,
        finished: 2,
        outcome: TaskOutcome::Succeeded,
        retry: None,
    };
    let mut lines = serde_json::to_string(&good).unwrap();
    lines.push_str("\n{torn\nnot even json\n");
    std::fs::write(root.join("activity.jsonl"), lines).unwrap();
    let service = LauncherService::open(&root, Scripted::default()).unwrap();
    let notes = service.startup_notes();
    assert!(notes.iter().any(|note| matches!(
        note,
        RecoveryNote::SettingsRecovered { preserved } if std::fs::read(preserved).unwrap() == b"{ nope"
    )));
    assert!(notes.contains(&RecoveryNote::ActivityLogSkipped { count: 2 }));
    assert_eq!(service.activity(10).finished.len(), 1);
}

/// Restarts an already-owned service (same transport is not needed here).
fn reopen_service(
    service: LauncherService<Scripted>,
    root: &std::path::Path,
) -> (LauncherService<Scripted>, ()) {
    drop(service);
    (
        LauncherService::open(root, Scripted::default()).unwrap(),
        (),
    )
}

/// Closes `world`'s service and opens the same root again, as a restart.
fn reopen(world: World, root: &std::path::Path) -> (LauncherService<Scripted>, tempfile::TempDir) {
    let World {
        service, net, _dir, ..
    } = world;
    drop(service);
    (LauncherService::open(root, net).unwrap(), _dir)
}

#[tokio::test]
async fn clearing_finished_work_empties_the_history_and_keeps_running_tasks() {
    let world = world();
    let finished = |label: &str| FinishedTask {
        category: TaskCategory::Download,
        label: label.to_owned(),
        instance_id: None,
        started: 1,
        finished: 2,
        outcome: TaskOutcome::Succeeded,
        retry: None,
    };
    world.service.log.append(&finished("one")).unwrap();
    world.service.log.append(&finished("two")).unwrap();
    let running =
        world
            .service
            .board
            .lock()
            .unwrap()
            .start(TaskCategory::Install, "running", None, 5);
    assert_eq!(world.service.activity(10).finished.len(), 2);

    world.service.clear_finished().unwrap();

    let view = world.service.activity(10);
    assert!(view.finished.is_empty());
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].id, running);
}

#[tokio::test]
async fn a_failed_download_remembers_its_input_and_runs_again_from_it() {
    let world = world();
    let record = world
        .service
        .create_instance("Retry", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    assert!(
        world
            .service
            .install_content(
                &record.id,
                ProjectKind::Mod,
                "cool",
                CancellationToken::new()
            )
            .await
            .is_err()
    );
    let failed = world.service.activity(10).finished.remove(0);
    assert!(matches!(failed.outcome, TaskOutcome::Failed(_)));
    let action = failed.retry.expect("a failed install can be retried");
    assert_eq!(
        action,
        RetryAction::InstallContent {
            instance: record.id.clone(),
            kind: ProjectKind::Mod,
            project: "cool".into(),
            version: None,
        }
    );

    // The network is back: the same input now works.
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let (updates, _seen) = mpsc::unbounded_channel();
    world
        .service
        .retry_task(action, updates, CancellationToken::new())
        .await
        .unwrap();
    assert!(
        world
            .service
            .layout
            .game(&record.id)
            .join("mods/cool.jar")
            .is_file()
    );
    let view = world.service.activity(10);
    assert_eq!(
        view.finished.len(),
        2,
        "the first failure stays in the history"
    );
    assert_eq!(view.finished[0].outcome, TaskOutcome::Succeeded);
}

#[tokio::test]
async fn the_bytes_of_all_transfers_add_up_to_one_task_and_an_unknown_length_hides_the_total() {
    use crate::transfer::TransferEvent;
    let board = StdMutex::new(TaskBoard::default());
    let task = board
        .lock()
        .unwrap()
        .start(TaskCategory::Download, "x", None, 1);
    let (events, receiver) = tokio::sync::broadcast::channel(16);
    let progress = |id: &str, completed, total| TransferEvent::Progress {
        id: id.to_owned(),
        completed,
        total,
    };
    events.send(progress("a", 10, Some(100))).unwrap();
    events.send(progress("b", 5, Some(50))).unwrap();
    events.send(progress("a", 40, Some(100))).unwrap();
    drop(events);
    meter_bytes(&board, task, receiver).await;
    let shown = board.lock().unwrap().active()[0].clone();
    assert_eq!(shown.unit, crate::ProgressUnit::Bytes);
    assert_eq!(shown.progress, Some((45, 150)));

    let task = board
        .lock()
        .unwrap()
        .start(TaskCategory::Download, "y", None, 1);
    let (events, receiver) = tokio::sync::broadcast::channel(16);
    events.send(progress("c", 7, None)).unwrap();
    events.send(progress("d", 3, Some(30))).unwrap();
    drop(events);
    meter_bytes(&board, task, receiver).await;
    let shown = board
        .lock()
        .unwrap()
        .active()
        .iter()
        .find(|t| t.id == task)
        .cloned()
        .unwrap();
    assert_eq!(shown.progress, Some((10, 0)), "total 0 means unknown");
}

#[tokio::test]
async fn cancel_task_by_id_stops_only_that_running_task_and_expires_with_it() {
    let world = world();
    let record = world
        .service
        .create_instance("Target", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let defaults = world.service.settings.lock().await;
    let mut first = Box::pin(world.service.install_content(
        &record.id,
        ProjectKind::Mod,
        "one",
        CancellationToken::new(),
    ));
    assert!(futures_util::poll!(&mut first).is_pending());
    let mut second = Box::pin(
        world
            .service
            .install_modpack("pack", CancellationToken::new()),
    );
    assert!(futures_util::poll!(&mut second).is_pending());
    let view = world.service.activity(10);
    assert_eq!(view.active.len(), 2);
    assert_eq!(view.cancellable.len(), 2);
    let install_id = view
        .active
        .iter()
        .find(|task| task.category == TaskCategory::Download)
        .unwrap()
        .id;
    assert!(!world.service.cancel_task(install_id + 100));
    assert!(world.service.cancel_task(install_id));
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), first)
            .await
            .unwrap(),
        Err(ServiceError::Cancelled)
    ));
    // The other task is untouched and still cancellable; the ended one is not.
    assert!(futures_util::poll!(&mut second).is_pending());
    let view = world.service.activity(10);
    assert_eq!(view.active.len(), 1);
    assert!(!view.cancellable.contains(&install_id));
    assert!(!world.service.cancel_task(install_id));
    drop(second);
    let view = world.service.activity(10);
    assert!(view.active.is_empty() && view.cancellable.is_empty());
    drop(defaults);
}

#[tokio::test]
async fn cancelled_transfer_leaves_nothing_behind_and_allows_retry() {
    let world = world();
    let record = world
        .service
        .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let target = world.service.layout.game(&record.id).join("mods/cool.jar");
    let cancel = CancellationToken::new();
    *world.net.cancel_on_file.lock().unwrap() = Some(cancel.clone());
    assert!(matches!(
        world
            .service
            .install_content(&record.id, ProjectKind::Mod, "cool", cancel)
            .await,
        Err(ServiceError::Cancelled)
    ));
    assert!(!target.exists());
    let leftovers: Vec<_> = std::fs::read_dir(target.parent().unwrap())
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    assert!(change_subjects(&world, &record.id).is_empty());
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Cancelled
    );
    world
        .service
        .install_content(
            &record.id,
            ProjectKind::Mod,
            "cool",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
    let activity = world.service.activity(10);
    assert!(activity.active.is_empty());
    assert_eq!(activity.finished.len(), 2);
    assert!(
        activity
            .finished
            .iter()
            .any(|task| task.outcome == TaskOutcome::Succeeded)
    );
}

#[tokio::test]
async fn switching_versions_replaces_the_file_keeps_it_disabled_and_never_clobbers() {
    let world = world();
    let record = world
        .service
        .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("cool-0.9.jar.disabled"), b"old build").unwrap();
    // Another file already called like the new version is the user's.
    std::fs::write(mods.join("cool.jar"), b"hand made").unwrap();
    let switch = || {
        world.service.switch_content_version(
            &record.id,
            ProjectKind::Mod,
            "cool-0.9.jar.disabled",
            "cool",
            "v1",
            CancellationToken::new(),
        )
    };
    assert!(matches!(
        switch().await,
        Err(ServiceError::Content(ContentError::Conflict(_)))
    ));
    assert_eq!(
        std::fs::read(mods.join("cool-0.9.jar.disabled")).unwrap(),
        b"old build"
    );
    assert_eq!(std::fs::read(mods.join("cool.jar")).unwrap(), b"hand made");

    std::fs::remove_file(mods.join("cool.jar")).unwrap();
    switch().await.unwrap();
    assert!(
        !mods.join("cool-0.9.jar.disabled").exists(),
        "the old file went"
    );
    assert_eq!(
        std::fs::read(mods.join("cool.jar.disabled")).unwrap(),
        b"a mod",
        "the new version, still switched off"
    );
    assert!(
        change_subjects(&world, &record.id)
            .contains(&(ChangeKind::ContentUpdated, "cool.jar".to_owned()))
    );
    // An unknown version is refused.
    assert!(matches!(
        world
            .service
            .switch_content_version(
                &record.id,
                ProjectKind::Mod,
                "cool.jar.disabled",
                "cool",
                "nope",
                CancellationToken::new(),
            )
            .await,
        Err(ServiceError::NoCompatibleVersion)
    ));
}

#[tokio::test]
async fn install_never_replaces_a_same_named_file_with_other_content() {
    let world = world();
    let record = world
        .service
        .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let target = world.service.layout.game(&record.id).join("mods/cool.jar");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"my own build").unwrap();
    let install = || {
        world.service.install_content(
            &record.id,
            ProjectKind::Mod,
            "cool",
            CancellationToken::new(),
        )
    };
    assert!(matches!(
        install().await,
        Err(ServiceError::Content(ContentError::Conflict(_)))
    ));
    assert_eq!(std::fs::read(&target).unwrap(), b"my own build");
    assert!(change_subjects(&world, &record.id).is_empty());
    // Removing it on purpose clears the way.
    world
        .service
        .delete_content(&record.id, ProjectKind::Mod, &["cool.jar".to_owned()])
        .await
        .unwrap();
    install().await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
    // The same content again is simply there already.
    install().await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
}

#[tokio::test]
async fn dropped_install_finishes_activity_and_releases_its_instance() {
    let world = world();
    let record = world
        .service
        .create_instance("Target", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let defaults = world.service.settings.lock().await;
    let mut install = Box::pin(world.service.install_content(
        &record.id,
        ProjectKind::Mod,
        "first",
        CancellationToken::new(),
    ));
    assert!(futures_util::poll!(&mut install).is_pending());
    assert_eq!(world.service.activity(10).active.len(), 1);
    drop(install);
    let activity = world.service.activity(10);
    assert!(activity.active.is_empty());
    assert_eq!(activity.finished.len(), 1);
    assert!(matches!(
        activity.finished[0].outcome,
        TaskOutcome::Failed(_)
    ));
    assert_eq!(
        activity.finished[0].instance_id.as_deref(),
        Some(record.id.as_str())
    );
    let mut retry = Box::pin(world.service.install_content(
        &record.id,
        ProjectKind::Mod,
        "second",
        CancellationToken::new(),
    ));
    assert!(futures_util::poll!(&mut retry).is_pending());
    drop(retry);
    drop(defaults);
    assert_eq!(world.service.activity(10).finished.len(), 2);
    assert!(world.service.activity(10).active.is_empty());
}

#[tokio::test]
async fn log_failure_keeps_terminal_results_visible_without_resurrecting_tasks() {
    let world = world();
    let log_path = world.service.layout.root().join("activity.jsonl");
    std::fs::create_dir(&log_path).unwrap();
    let result: Result<(), ServiceError> = Ok(());
    for _ in 0..2 {
        let task = world
            .service
            .begin(TaskCategory::Install, "same operation".into(), None, None);
        world.service.end(task, &result);
    }
    let activity = world.service.activity(10);
    assert!(activity.active.is_empty());
    assert_eq!(activity.finished.len(), 2);
    assert!(
        activity
            .finished
            .iter()
            .all(|task| task.outcome == TaskOutcome::Succeeded)
    );
    assert!(world.service.activity(0).finished.is_empty());
    std::fs::remove_dir(log_path).unwrap();
    assert_eq!(world.service.activity(10).finished.len(), 2);
    assert!(world.service.activity(10).active.is_empty());
}

#[tokio::test]
async fn pending_launch_rejects_conflicting_writes_and_drop_releases_target() {
    let world = world();
    let mut records = Vec::new();
    for name in ["Target", "Independent"] {
        records.push(
            world
                .service
                .create_instance(name, Some("1.0"), Loader::Vanilla, None)
                .await
                .unwrap(),
        );
    }
    let target = &records[0].id;
    // Pause launch at the settings boundary after it reserves its target.
    let defaults = world.service.settings.lock().await;
    let (updates, _receiver) = mpsc::unbounded_channel();
    let mut launch = Box::pin(
        world
            .service
            .launch(target, updates, CancellationToken::new()),
    );
    assert!(futures_util::poll!(&mut launch).is_pending());
    assert!(
        matches!(world.service.delete_instance(target).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
    );
    assert!(
        matches!(world.service.install_content(target, ProjectKind::Mod, "project", CancellationToken::new()).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
    );
    let (updates, _receiver) = mpsc::unbounded_channel();
    assert!(
        matches!(world.service.launch(target, updates, CancellationToken::new()).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
    );
    assert_eq!(world.service.instance(target).await.unwrap().name, "Target");
    world.service.rename(target, "Renamed").await.unwrap();
    world.service.delete_instance(&records[1].id).await.unwrap();
    assert!(world.service.instance(target).await.is_ok());
    drop(launch);
    drop(defaults);
    world.service.delete_instance(target).await.unwrap();
    assert!(matches!(
        world.service.instance(target).await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn failed_launch_does_not_leave_an_instance_reserved() {
    let world = world();
    let (updates, _receiver) = mpsc::unbounded_channel();
    assert!(matches!(
        world
            .service
            .launch("missing", updates, CancellationToken::new())
            .await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    assert!(matches!(
        world.service.delete_instance("missing").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn registered_instance_details_survive_rename_and_missing_profile() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Details".into(),
                game_version: "1.21.1".into(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            1,
        )
        .unwrap()
        .clone();
    world.service.set_favorite(&record.id, true).await.unwrap();
    {
        let mut store = world.service.store.lock().await;
        store.create_collection("Keep").unwrap();
        store.set_membership("Keep", &record.id, true).unwrap();
    }
    assert!(!world.service.layout().profile(&record.id).exists());
    world.service.rename(&record.id, "生存游戏").await.unwrap();
    let detail = world.service.instance(&record.id).await.unwrap();
    assert_eq!(detail.name, "生存游戏");
    assert_eq!(detail.id, record.id);
    assert!(detail.favorite);
    assert_eq!(
        world.service.library().await.collections[0].members,
        std::slice::from_ref(&record.id)
    );
    let root = world.service.layout().root().to_path_buf();
    drop(world.service);
    let reopened = LauncherService::open(root, world.net.clone()).unwrap();
    assert_eq!(reopened.instance(&record.id).await.unwrap(), detail);
    assert!(matches!(
        reopened.instance("missing").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn instance_memory_overrides_are_validated_and_can_resume_inheritance() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Memory".into(),
                game_version: "1.0".into(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            1,
        )
        .unwrap()
        .clone();
    world
        .service
        .set_default_memory(Some(512), Some(2048))
        .await
        .unwrap();
    let settings = InstanceSettings {
        min_memory_mb: Some(4096),
        ..InstanceSettings::default()
    };
    assert!(matches!(
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await,
        Err(ServiceError::Settings(SettingsError::InvalidMemory))
    ));
    assert_eq!(world.service.library().await.instances[0], record);
    let settings = InstanceSettings {
        max_memory_mb: Some(4096),
        ..InstanceSettings::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings.clone())
        .await
        .unwrap();
    assert_eq!(
        world.service.library().await.instances[0].settings,
        settings
    );
    world
        .service
        .update_instance_settings(&record.id, InstanceSettings::default())
        .await
        .unwrap();
    assert_eq!(
        world.service.library().await.instances[0].settings,
        InstanceSettings::default()
    );
    let settings = InstanceSettings {
        min_memory_mb: Some(1024),
        ..InstanceSettings::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    world
        .service
        .set_default_memory(Some(128), Some(512))
        .await
        .unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    // There is no catalog answer: memory validation must win over network failure.
    assert!(matches!(
        world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await,
        Err(ServiceError::Settings(SettingsError::InvalidMemory))
    ));
    assert!(!world.service.layout().game(&record.id).exists());
    let expected_library = world.service.library().await;
    let expected_settings = world.service.settings().await;
    let root = world.service.layout().root().to_path_buf();
    drop(world.service);
    let reopened = LauncherService::open(root, world.net.clone()).unwrap();
    assert_eq!(reopened.library().await, expected_library);
    assert_eq!(reopened.settings().await, expected_settings);
}

#[tokio::test]
async fn service_settings_expose_only_saved_defaults_after_a_write_failure() {
    let world = world();
    world
        .service
        .set_default_memory(Some(512), Some(2048))
        .await
        .unwrap();
    let before = world.service.settings().await;
    std::fs::create_dir(world.service.layout().root().join("settings.json.tmp")).unwrap();
    assert!(
        world
            .service
            .set_default_memory(Some(1024), Some(4096))
            .await
            .is_err()
    );
    assert_eq!(world.service.settings().await, before);
}

#[tokio::test]
async fn a_new_instance_defaults_to_the_newest_release_and_recommended_loader() {
    let world = world();
    publish_release(&world);
    world.net.answer(
        "meta.fabricmc.net/v2/versions/loader/1.0",
        r#"[{"loader":{"version":"0.17.0-beta","stable":false}},
            {"loader":{"version":"0.16.0","stable":true}}]"#,
    );
    let vanilla = world
        .service
        .create_instance("Plain", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert_eq!(vanilla.game_version, "1.0");
    assert_eq!(vanilla.loader_version, None);
    let fabric = world
        .service
        .create_instance("Modded", None, Loader::Fabric, None)
        .await
        .unwrap();
    assert_eq!(fabric.loader_version.as_deref(), Some("0.16.0"));
    assert_eq!(world.service.library().await.instances.len(), 2);

    // A chosen Forge build is recorded as chosen; installing it is the
    // launcher's job (its installer runs then).
    let forge = world
        .service
        .create_instance("Forge", Some("1.0"), Loader::Forge, Some("1.0.0"))
        .await
        .unwrap();
    assert_eq!(forge.loader, Loader::Forge);
    assert_eq!(forge.loader_version.as_deref(), Some("1.0.0"));
    assert_eq!(world.service.library().await.instances.len(), 3);
}

#[tokio::test]
async fn an_unreachable_catalog_creates_nothing() {
    let world = world();
    let result = world
        .service
        .create_instance("X", None, Loader::Vanilla, None)
        .await;
    assert!(matches!(result, Err(ServiceError::Remote(_))));
    assert!(world.service.library().await.instances.is_empty());
}

#[tokio::test]
async fn launching_installs_runs_records_and_then_works_offline() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let launch = world
        .service
        .launch(&record.id, tx, CancellationToken::new());
    assert_send(&launch);
    let exit = launch.await.unwrap();
    assert_eq!(exit.code, Some(0));
    let mut session = LaunchSession::new();
    while let Ok(update) = rx.try_recv() {
        if let LaunchUpdate::Signal(signal) = update {
            session.apply(signal);
        }
    }
    assert!(session.is_finished());

    let library = world.service.library().await;
    let stored = &library.instances[0];
    assert!(stored.installed);
    assert!(stored.last_played.is_some());
    let history = HistoryLog::for_instance(world.service.layout().root(), &record.id)
        .sessions()
        .unwrap();
    assert_eq!(history.len(), 1);
    assert!(
        world
            .service
            .layout()
            .versions()
            .join("1.0/1.0.jar")
            .is_file()
    );

    // The network is gone; the installed release still launches.
    world.net.forget_all();
    let (tx, _rx) = mpsc::unbounded_channel();
    let again = world
        .service
        .launch(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(again.code, Some(0));
}

#[tokio::test]
async fn creating_does_not_install_and_bad_requests_publish_nothing() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Only", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert!(!record.installed);
    assert!(!world.service.layout.versions().exists());
    assert!(world.service.activity(10).finished.is_empty());
    // With no catalog, "newest" cannot be resolved: no guessed version.
    world.net.forget_all();
    assert!(matches!(
        world
            .service
            .create_instance("Guess", None, Loader::Vanilla, None)
            .await,
        Err(ServiceError::Remote(_))
    ));
    assert_eq!(world.service.library().await.instances.len(), 1);
    // The same name again gets its own id and never touches the first profile.
    let again = world
        .service
        .create_instance("Only", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    assert_ne!(again.id, record.id);
}

async fn is_installed(world: &World, id: &str) -> bool {
    world.service.instance(id).await.unwrap().installed
}

#[tokio::test]
async fn installing_is_separate_cancellable_and_only_success_marks_installed() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Pack", None, Loader::Vanilla, None)
        .await
        .unwrap();

    // Cancelled mid-download: not installed, recorded as cancelled.
    let cancel = CancellationToken::new();
    *world.net.cancel_on_file.lock().unwrap() = Some(cancel.clone());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let cancelled = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        world.service.install_instance(&record.id, tx, cancel),
    )
    .await
    .expect("cancelling must not wait for a stalled download");
    assert!(matches!(
        cancelled,
        Err(ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled))
    ));
    assert!(!is_installed(&world, &record.id).await);
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Cancelled
    );
    let mut session = LaunchSession::new();
    while let Ok(LaunchUpdate::Signal(signal)) = rx.try_recv() {
        session.apply(signal);
    }
    assert!(session.is_finished());

    // A failing download: not installed, recorded as failed with a reason.
    let client = world.server.join("client.jar");
    let kept = std::fs::read(&client).unwrap();
    std::fs::remove_file(&client).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .install_instance(&record.id, tx, CancellationToken::new())
            .await
            .is_err()
    );
    assert!(!is_installed(&world, &record.id).await);
    assert!(matches!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Failed(_)
    ));

    // Retry succeeds; no game process or play session came with it.
    std::fs::write(&client, kept).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .install_instance(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert!(is_installed(&world, &record.id).await);
    let stored = world.service.instance(&record.id).await.unwrap();
    assert!(stored.last_played.is_none());
    assert!(
        HistoryLog::for_instance(world.service.layout.root(), &record.id)
            .sessions()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Succeeded
    );

    // Installed once, it launches with the network gone (AC-OFFLINE-01).
    fake_java(&world, "echo \"Setting user: x\"");
    world.net.forget_all();
    let (tx, _rx) = mpsc::unbounded_channel();
    let exit = world
        .service
        .launch(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(exit.code, Some(0));
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

#[tokio::test]
async fn every_way_a_launch_can_end_leaves_its_own_record() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Ends", None, Loader::Vanilla, None)
        .await
        .unwrap();

    // No Java: preparation fails before any process exists.
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await
            .is_err()
    );
    // Cancelled before it starts: no process, its own outcome.
    let cancel = CancellationToken::new();
    cancel.cancel();
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(world.service.launch(&record.id, tx, cancel).await.is_err());
    let stored = world.service.instance(&record.id).await.unwrap();
    assert!(stored.last_played.is_none());
    assert_eq!(stored.play_seconds, 0);

    // Dies at once, crashes after starting, exits cleanly.
    for script in [
        "exit 1",
        "echo \"Setting user: x\"; exit 3",
        "echo \"Setting user: x\"",
    ] {
        fake_java(&world, script);
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await
            .unwrap();
    }

    // Running, then the user stops it: the process really ends.
    fake_java(&world, "echo \"Setting user: x\"; sleep 30");
    let stop = CancellationToken::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let launch = world.service.launch(&record.id, tx, stop.clone());
    let stopper = async {
        while let Some(update) = rx.recv().await {
            if matches!(update, LaunchUpdate::Signal(LaunchSignal::Running)) {
                stop.cancel();
                break;
            }
        }
        // Keep the receiver open so later updates do not fail the send.
        while rx.recv().await.is_some() {}
    };
    let (exit, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        tokio::join!(launch, stopper)
    })
    .await
    .expect("stopping must end the process promptly");
    let exit = exit.unwrap();
    assert!(exit.killed && exit.was_running);

    assert_eq!(
        outcomes_of(&world, &record.id).await,
        [
            SessionOutcome::Stopped,
            SessionOutcome::Clean,
            SessionOutcome::Crashed,
            SessionOutcome::FailedToStart,
            SessionOutcome::Cancelled,
            SessionOutcome::FailedToPrepare,
        ]
    );
    // Nothing is left holding the instance.
    assert!(world.service.reserve_instance(&record.id).is_ok());
}

#[tokio::test]
async fn a_launcher_that_died_mid_launch_reports_the_session_and_touches_no_process() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Died", None, Loader::Vanilla, None)
        .await
        .unwrap();
    // While a launch is in progress the marker exists; a normal return removes it.
    fake_java(&world, "echo \"Setting user: x\"");
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .launch(&record.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert!(!world.service.layout.sessions().join("died.json").exists());
    // A launcher killed mid-launch cannot clean up: rehearse by leaving the marker.
    let marker = SessionMarker::place(&world.service.layout, &record.id, 42).unwrap();
    std::mem::forget(marker);
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert!(
        service
            .startup_notes()
            .contains(&RecoveryNote::SessionInterrupted {
                instance_id: record.id.clone(),
                started: 42
            })
    );
    let history = HistoryLog::for_instance(&root, &record.id)
        .sessions()
        .unwrap();
    assert!(matches!(
        history[..],
        [
            HistoryEvent::Session {
                started: 42,
                outcome: SessionOutcome::Interrupted,
                ..
            },
            ..
        ]
    ));
    assert!(!service.layout.sessions().join("died.json").exists());
    // Reported once; nothing was relaunched.
    let (service, _dir) = reopen_service(service, &root);
    assert!(service.startup_notes().is_empty());
    assert_eq!(
        service.instance(&record.id).await.unwrap().play_seconds,
        0,
        "an interrupted session counts no play time"
    );
    // An id that is not plain is refused before any file is named after it.
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(matches!(
        service
            .launch("../escape", tx, CancellationToken::new())
            .await,
        Err(ServiceError::NoSuchInstance(_))
    ));
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

#[tokio::test]
async fn content_changes_report_each_file_and_record_only_real_changes() {
    let world = world();
    let record = instance_with_profile(&world, "Mods").await;
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"a").unwrap();
    std::fs::write(mods.join("b.jar"), b"b").unwrap();
    std::fs::write(mods.join("b.jar.disabled"), b"twin").unwrap();
    let names = |list: &[&str]| list.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();

    let results = world
        .service
        .set_content_state(
            &record.id,
            ProjectKind::Mod,
            &names(&["a.jar", "b.jar.disabled", "ghost.jar", "../x.jar"]),
            true,
        )
        .await
        .unwrap();
    // a is already enabled; b.jar.disabled collides with b.jar; two are refused.
    assert_eq!(
        results[0].outcome.as_ref().unwrap(),
        &ContentEffect::Unchanged
    );
    assert!(matches!(results[1].outcome, Err(ContentError::Conflict(_))));
    assert!(matches!(results[2].outcome, Err(ContentError::NotFound(_))));
    assert!(matches!(
        results[3].outcome,
        Err(ContentError::UnsafeFileName(_))
    ));
    assert!(change_subjects(&world, &record.id).is_empty());

    let results = world
        .service
        .set_content_state(&record.id, ProjectKind::Mod, &names(&["a.jar"]), false)
        .await
        .unwrap();
    assert_eq!(
        results[0].outcome.as_ref().unwrap(),
        &ContentEffect::Renamed("a.jar.disabled".into())
    );
    assert!(results[0].recorded);
    let listed = world
        .service
        .content(&record.id, ProjectKind::Mod)
        .await
        .unwrap();
    assert!(
        listed
            .iter()
            .any(|item| item.display_name == "a.jar" && !item.enabled)
    );

    let results = world
        .service
        .delete_content(
            &record.id,
            ProjectKind::Mod,
            &names(&["b.jar.disabled", "b.jar"]),
        )
        .await
        .unwrap();
    assert!(results.iter().all(|result| result.outcome.is_ok()));
    assert!(!mods.join("b.jar").exists() && !mods.join("b.jar.disabled").exists());
    assert_eq!(
        change_subjects(&world, &record.id),
        [
            (ChangeKind::ContentRemoved, "b.jar".to_owned()),
            (ChangeKind::ContentRemoved, "b.jar.disabled".to_owned()),
            (ChangeKind::ContentDisabled, "a.jar".to_owned()),
        ]
    );
    // Other instances and unknown ones are not reachable through this call.
    assert!(matches!(
        world.service.content("ghost", ProjectKind::Mod).await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    assert!(matches!(
        world
            .service
            .set_content_state(&record.id, ProjectKind::Modpack, &names(&["a"]), true)
            .await
            .unwrap()[0]
            .outcome,
        Err(ContentError::NotAFileKind)
    ));
}

#[tokio::test]
async fn content_writes_wait_for_a_launching_instance_but_reads_do_not() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"; sleep 30");
    let record = world
        .service
        .create_instance("Busy", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"a").unwrap();
    let stop = CancellationToken::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let launch = world.service.launch(&record.id, tx, stop.clone());
    let probe = async {
        while let Some(update) = rx.recv().await {
            if matches!(update, LaunchUpdate::Signal(LaunchSignal::Running)) {
                break;
            }
        }
        let refused = world
            .service
            .set_content_state(&record.id, ProjectKind::Mod, &["a.jar".to_owned()], false)
            .await;
        assert!(matches!(refused, Err(ServiceError::InstanceBusy(_))));
        assert!(matches!(
            world
                .service
                .delete_content(&record.id, ProjectKind::Mod, &["a.jar".to_owned()])
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert_eq!(
            world
                .service
                .content(&record.id, ProjectKind::Mod)
                .await
                .unwrap()
                .len(),
            1
        );
        stop.cancel();
        while rx.recv().await.is_some() {}
    };
    let (exit, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        tokio::join!(launch, probe)
    })
    .await
    .unwrap();
    exit.unwrap();
    assert!(mods.join("a.jar").exists());
    // Once the game is gone the same change goes through.
    world
        .service
        .set_content_state(&record.id, ProjectKind::Mod, &["a.jar".to_owned()], false)
        .await
        .unwrap();
    assert!(mods.join("a.jar.disabled").exists());
}

#[tokio::test]
async fn history_and_problems_are_readable_without_the_lease() {
    let world = world();
    let record = world
        .service
        .create_instance("Read", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let saves = world.service.layout.game(&record.id).join("saves");
    std::fs::create_dir_all(saves.join("w")).unwrap();
    world
        .service
        .copy_world(&record.id, "w", None)
        .await
        .unwrap();
    let _busy = world.service.reserve_instance(&record.id).unwrap();
    let read = world.service.history(&record.id).await.unwrap();
    assert!(matches!(
        read.events[..],
        [HistoryEvent::Change {
            kind: ChangeKind::WorldCopied,
            ..
        }]
    ));
    let problems = world.service.problems(&record.id).await.unwrap();
    assert!(
        problems
            .iter()
            .any(|problem| problem.kind == crate::diagnostics::ProblemKind::NotInstalled)
    );
    assert!(matches!(
        world.service.history("nobody").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    assert!(matches!(
        world.service.problems("nobody").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn logs_and_crash_reports_are_read_safely_without_the_lease() {
    let world = world();
    let record = world
        .service
        .create_instance("Logs", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    let none = world.service.logs(&record.id).await.unwrap();
    assert_eq!((none.latest, none.crashes.len()), (None, 0));
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::create_dir_all(game.join("crash-reports")).unwrap();
    let big = "x\n".repeat(LOG_TAIL_BYTES as usize);
    std::fs::write(game.join("logs/latest.log"), &big).unwrap();
    std::fs::write(
        game.join("crash-reports/crash-1.txt"),
        "java.lang.OutOfMemoryError: heap",
    )
    .unwrap();
    let _busy = world.service.reserve_instance(&record.id).unwrap();
    let logs = world.service.logs(&record.id).await.unwrap();
    assert!(logs.latest.unwrap().len() as u64 <= LOG_TAIL_BYTES);
    assert_eq!(logs.crashes[0].file_name, "crash-1.txt");
    let (text, hints) = world
        .service
        .crash_report(&record.id, "crash-1.txt")
        .await
        .unwrap();
    assert!(text.contains("OutOfMemory"));
    assert_eq!(hints, vec![crate::diagnostics::CrashHint::OutOfMemory]);
    assert!(
        world
            .service
            .crash_report(&record.id, "../../launcher.db")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn worlds_are_copied_and_deleted_under_the_lease_and_recorded() {
    let world = world();
    let record = world
        .service
        .create_instance("Worlds", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let saves = world.service.layout.game(&record.id).join("saves");
    std::fs::create_dir_all(saves.join("w/region")).unwrap();
    std::fs::write(saves.join("w/level.dat"), b"not nbt").unwrap();
    std::fs::write(saves.join("w/region/r.0.0.mca"), b"chunks").unwrap();

    let listed = world.service.worlds(&record.id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].damaged, "a damaged world is still listed");

    // While the instance is busy, writes are refused and reads still work.
    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world.service.copy_world(&record.id, "w", None).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert!(matches!(
            world.service.delete_world(&record.id, "w").await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert_eq!(world.service.worlds(&record.id).await.unwrap().len(), 1);
    }
    assert!(saves.join("w").is_dir());

    // A leftover half-copy is swept, the copy is whole, and nothing is overwritten.
    std::fs::create_dir_all(saves.join(".w copy.copying")).unwrap();
    let first = world
        .service
        .copy_world(&record.id, "w", None)
        .await
        .unwrap();
    let second = world
        .service
        .copy_world(&record.id, "w", None)
        .await
        .unwrap();
    assert_eq!((first.as_str(), second.as_str()), ("w copy", "w copy 2"));
    assert!(!saves.join(".w copy.copying").exists());
    assert_eq!(
        std::fs::read(saves.join("w copy/region/r.0.0.mca")).unwrap(),
        b"chunks"
    );
    assert!(matches!(
        world
            .service
            .copy_world(&record.id, "w", Some("w copy"))
            .await,
        Err(ServiceError::World(WorldError::AlreadyExists(_)))
    ));
    assert!(matches!(
        world.service.copy_world(&record.id, "ghost", None).await,
        Err(ServiceError::World(WorldError::NotFound(_)))
    ));

    world
        .service
        .delete_world(&record.id, "w copy")
        .await
        .unwrap();
    assert!(!saves.join("w copy").exists());
    assert!(saves.join("w").is_dir() && saves.join("w copy 2").is_dir());
    assert_eq!(
        change_subjects(&world, &record.id),
        [
            (ChangeKind::WorldDeleted, "w copy".to_owned()),
            (ChangeKind::WorldCopied, "w copy 2".to_owned()),
            (ChangeKind::WorldCopied, "w copy".to_owned()),
        ]
    );
    assert!(matches!(
        world.service.worlds("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

#[tokio::test]
async fn a_world_is_exported_and_imported_under_the_lease_and_the_import_is_recorded() {
    let world = world();
    let record = world
        .service
        .create_instance("Zip", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let saves = world.service.layout.game(&record.id).join("saves");
    std::fs::create_dir_all(saves.join("w")).unwrap();
    std::fs::write(saves.join("w/level.dat"), b"not nbt").unwrap();
    let archive = world._dir.path().join("w.zip");

    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world.service.export_world(&record.id, "w", &archive).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert!(matches!(
            world.service.import_world(&record.id, &archive).await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(!archive.exists());
    assert!(
        world
            .service
            .export_world(&record.id, "w", &archive)
            .await
            .unwrap()
            > 0
    );
    assert!(matches!(
        world
            .service
            .export_world(&record.id, "ghost", &archive)
            .await,
        Err(ServiceError::World(WorldError::NotFound(_)))
    ));

    let imported = world
        .service
        .import_world(&record.id, &archive)
        .await
        .unwrap();
    assert_eq!(imported, "w 2");
    assert!(saves.join("w 2/level.dat").is_file());
    assert_eq!(
        change_subjects(&world, &record.id),
        [(ChangeKind::WorldImported, "w 2".to_owned())]
    );
}

#[tokio::test]
async fn an_exported_pack_carries_everything_when_modrinth_is_out_of_reach_and_needs_the_lease() {
    let world = world();
    let record = world
        .service
        .create_instance("Pack", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:70").unwrap();
    let spec = crate::pack_export::ExportSpec {
        format: crate::pack_export::PackFormat::Modrinth,
        name: "Pack".into(),
        version: "1".into(),
        summary: None,
        include: vec!["mods".into(), "options.txt".into()],
    };
    let pack = world._dir.path().join("pack.mrpack");

    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world
                .service
                .export_modpack(&record.id, spec.clone(), &pack, CancellationToken::new())
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(!pack.exists());
    let report = world
        .service
        .export_modpack(&record.id, spec.clone(), &pack, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!((report.linked, report.bundled), (0, 2));
    assert!(report.lookup_failed && report.size > 0);
    let index = crate::modpack::read_index(&pack).unwrap();
    assert_eq!(
        (index.name.as_str(), index.minecraft.as_str()),
        ("Pack", "1.0")
    );
    assert!(index.files.is_empty());

    let mut nothing = spec;
    nothing.include = vec!["saves".into()];
    assert!(matches!(
        world
            .service
            .export_modpack(&record.id, nothing, &pack, CancellationToken::new())
            .await,
        Err(ServiceError::Export(
            crate::pack_export::ExportError::NothingChosen
        ))
    ));
}

#[tokio::test]
async fn a_games_size_counts_its_own_folder_only() {
    let world = world();
    let record = world
        .service
        .create_instance("Size", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    let before = world.service.instance_size(&record.id).await.unwrap();
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), vec![0_u8; 1000]).unwrap();
    std::fs::write(game.join("options.txt"), vec![0_u8; 24]).unwrap();
    assert_eq!(
        world.service.instance_size(&record.id).await.unwrap(),
        before + 1024
    );
    assert!(matches!(
        world.service.instance_size("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
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

#[tokio::test]
async fn a_whole_game_is_backed_up_and_restored_as_a_new_one_with_nothing_touched() {
    let world = world();
    let record = world
        .service
        .create_instance("生存", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .service
        .store
        .lock()
        .await
        .update_settings(
            &record.id,
            InstanceSettings {
                max_memory_mb: Some(4096),
                java_path: Some("/somewhere/java".into()),
                ..InstanceSettings::default()
            },
        )
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::create_dir_all(game.join("saves/W")).unwrap();
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
    std::fs::write(game.join("saves/W/level.dat"), b"world").unwrap();
    std::fs::write(game.join("logs/latest.log"), b"log").unwrap();
    let archive = world._dir.path().join("b.zip");

    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world
                .service
                .backup_instance(&record.id, &archive, CancellationToken::new())
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(
        world
            .service
            .backup_instance(&record.id, &archive, CancellationToken::new())
            .await
            .unwrap()
            > 0
    );

    let restored = world
        .service
        .restore_backup(&archive, CancellationToken::new())
        .await
        .unwrap();
    assert_ne!(restored.id, record.id);
    assert_eq!(restored.name, "生存（恢复）");
    assert_eq!(
        (restored.game_version.as_str(), restored.loader),
        ("1.0", Loader::Fabric)
    );
    assert_eq!(restored.settings.max_memory_mb, Some(4096));
    assert_eq!(
        restored.settings.java_path, None,
        "a Java path is per machine"
    );
    assert!(!restored.installed);
    let there = world.service.layout.game(&restored.id);
    assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(
        std::fs::read(there.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(!there.join("logs").exists());
    // The original is as it was.
    assert_eq!(
        std::fs::read(game.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert_eq!(world.service.library().await.instances.len(), 2);

    // A file that is not a backup changes nothing.
    let junk = world._dir.path().join("junk.zip");
    std::fs::write(&junk, b"nope").unwrap();
    assert!(
        world
            .service
            .restore_backup(&junk, CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(world.service.library().await.instances.len(), 2);
    let failed = world.service.activity(5).finished.remove(0);
    assert_eq!(
        failed.retry,
        Some(RetryAction::RestoreBackup {
            path: junk.display().to_string()
        })
    );
}

#[tokio::test]
async fn a_game_from_another_launcher_becomes_a_new_one_and_the_source_stays_as_it_was() {
    let world = world();
    let source = world._dir.path().join("prism-inst");
    let write = |path: &str, body: &str| {
        let full = source.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, body).unwrap();
    };
    write("instance.cfg", "name=我的生存\n");
    write(
        "mmc-pack.json",
        r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},
            {"uid":"net.fabricmc.fabric-loader","version":"0.15.7"}]}"#,
    );
    write(".minecraft/mods/a.jar", "mod");
    write(".minecraft/saves/W/level.dat", "world");
    write(".minecraft/logs/latest.log", "log");

    let found = world.service.detect_games(&source).await.unwrap();
    assert_eq!(found.len(), 1);
    let record = world
        .service
        .import_game(found[0].clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(record.name, "我的生存");
    assert_eq!(
        (
            record.game_version.as_str(),
            record.loader,
            record.loader_version.as_deref()
        ),
        ("1.20.1", Loader::Fabric, Some("0.15.7"))
    );
    assert!(!record.installed);
    let there = world.service.layout.game(&record.id);
    assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(
        std::fs::read(there.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(!there.join("logs").exists());
    assert!(
        source.join(".minecraft/mods/a.jar").is_file(),
        "the source is untouched"
    );

    // Not a launcher's folder; a cancelled import; a modded game with no loader version.
    let empty = world._dir.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(world.service.detect_games(&empty).await.is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        world.service.import_game(found[0].clone(), cancel).await,
        Err(ServiceError::Cancelled)
    ));
    let mut nameless = found[0].clone();
    nameless.loader_version = None;
    assert!(
        world
            .service
            .import_game(nameless, CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(
        world.service.library().await.instances.len(),
        1,
        "failures add no game"
    );
    let leftovers = std::fs::read_dir(world.service.layout.operations())
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "no staging folder is left behind");
}

#[tokio::test]
async fn unused_shared_files_are_measured_then_removed_only_when_nothing_is_running() {
    let world = world();
    let record = world
        .service
        .create_instance("Keep", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let meta = world.service.layout.meta();
    let put = |path: &str, body: &str| {
        let full = meta.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, body).unwrap();
    };
    put(
        "versions/1.0/1.0.json",
        r#"{"id":"1.0","assets":"9","assetIndex":{"id":"9"},"libraries":[]}"#,
    );
    put(
        "assets/indexes/9.json",
        r#"{"objects":{"a":{"hash":"aa11"}}}"#,
    );
    put("assets/objects/aa/aa11", "used");
    put("assets/objects/bb/bb22", "unused-bytes");
    put("versions/0.9/0.9.json", "{}");

    let found = world.service.reclaimable().await.unwrap();
    let bytes = found.bytes();
    assert!(bytes > 0);
    assert!(found.unused.iter().any(|(p, _)| p.ends_with("bb22")));
    assert!(
        found
            .unused
            .iter()
            .any(|(p, _)| p.ends_with("versions/0.9"))
    );
    assert!(!found.unused.iter().any(|(p, _)| p.ends_with("aa11")));

    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world.service.reclaim().await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    assert!(
        meta.join("assets/objects/bb/bb22").is_file(),
        "nothing removed while busy"
    );
    assert_eq!(world.service.reclaim().await.unwrap(), bytes);
    assert!(!meta.join("assets/objects/bb/bb22").exists());
    assert!(!meta.join("versions/0.9").exists());
    assert!(meta.join("assets/objects/aa/aa11").is_file());
    assert!(meta.join("versions/1.0/1.0.json").is_file());
    assert!(world.service.reclaimable().await.unwrap().unused.is_empty());
}

#[tokio::test]
async fn a_game_exported_as_a_prism_zip_comes_back_in_through_the_pack_importer() {
    let world = world();
    let record = world
        .service
        .create_instance("Round", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:70").unwrap();
    let spec = crate::pack_export::ExportSpec {
        format: crate::pack_export::PackFormat::Prism,
        name: "Round".into(),
        version: "1".into(),
        summary: None,
        include: vec!["mods".into(), "options.txt".into()],
    };
    let zip = world._dir.path().join("round.zip");
    let report = world
        .service
        .export_modpack(&record.id, spec, &zip, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!((report.linked, report.bundled), (0, 2));

    let back = world
        .service
        .import_modpack_file(&zip, CancellationToken::new())
        .await
        .unwrap();
    assert_ne!(back.id, record.id);
    assert_eq!(
        (
            back.game_version.as_str(),
            back.loader,
            back.loader_version.as_deref()
        ),
        ("1.0", Loader::Fabric, Some("0.16.0"))
    );
    let there = world.service.layout.game(&back.id);
    assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(std::fs::read(there.join("options.txt")).unwrap(), b"fov:70");
    // The scratch folder is gone, and the finished task knows the new game.
    let scratch = world.service.layout.root().join("cache");
    let leftovers = std::fs::read_dir(&scratch).map(|e| e.count()).unwrap_or(0);
    assert_eq!(leftovers, 0);
    assert_eq!(
        world.service.activity(5).finished[0].instance_id.as_deref(),
        Some(back.id.as_str())
    );
}

#[tokio::test]
async fn java_is_installed_whole_found_by_discovery_and_a_bad_file_leaves_nothing() {
    let world = world();
    publish_java(&world, true);
    let runtimes = world.service.layout.runtimes();
    assert!(
        world
            .service
            .install_java(Some(21), CancellationToken::new())
            .await
            .is_err()
    );
    assert!(!runtimes.join("java-runtime-delta").exists());
    let staging = runtimes.join(".java-runtime-delta.installing");
    assert!(!staging.exists(), "a failed install leaves no half runtime");
    let failed = world.service.activity(5).finished.remove(0);
    assert!(matches!(failed.outcome, TaskOutcome::Failed(_)));
    assert_eq!(
        failed.retry,
        Some(RetryAction::InstallJava { major: Some(21) })
    );

    publish_java(&world, false);
    let java = world
        .service
        .install_java(Some(21), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(java.major(), 21);
    assert!(java.executable().is_file());
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(java.executable())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "the launcher is executable");
    }
    assert!(runtimes.join("java-runtime-delta/bin/jre").is_symlink());
    // The ordinary discovery finds it, and asking again changes nothing.
    let found = world.service.java_installations().await;
    assert!(
        found
            .iter()
            .any(|(runtime, _)| runtime.home().ends_with("java-runtime-delta"))
    );
    let again = world
        .service
        .install_java(Some(21), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(again.home(), java.home());
    assert!(matches!(
        world
            .service
            .install_java(Some(99), CancellationToken::new())
            .await,
        Err(ServiceError::Install(_))
    ));
}

#[tokio::test]
async fn a_version_can_be_saved_where_the_person_chooses_and_can_be_retried() {
    let world = world();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let target = world._dir.path().join("elsewhere.jar");

    let name = world
        .service
        .save_version_as("cool", "v1", &target, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(name, "cool.jar");
    assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
    assert!(world.service.activity(10).finished[0].retry.is_some());

    // An unknown version is an error.
    assert!(matches!(
        world
            .service
            .save_version_as("cool", "nope", &target, CancellationToken::new())
            .await,
        Err(ServiceError::NoCompatibleVersion)
    ));
}

#[tokio::test]
async fn installed_projects_list_the_file_and_the_newer_version_if_any() {
    let world = world();
    let record = world
        .service
        .create_instance("Have", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("libx.jar"), b"libx").unwrap();
    std::fs::write(mods.join("mine.jar"), b"mine").unwrap();
    let version = |id: &str, hash: &str| {
        format!(
            r#"{{"id":"{id}","project_id":"LIBX","name":"x","version_number":"1",
                "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",
                "files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                           "primary":true,"size":4,"hashes":{{"sha1":"{hash}"}}}}],
                "dependencies":[]}}"#
        )
    };
    let known = sha1_hex(b"libx");
    // More specific address first: answers are matched by containment.
    world.net.answer(
        "/v2/version_files/update",
        format!(r#"{{"{known}":{}}}"#, version("v-new", "newhash")),
    );
    world.net.answer(
        "/v2/version_files",
        format!(r#"{{"{known}":{}}}"#, version("v-old", &known)),
    );

    let have = world
        .service
        .installed_projects(&record.id, ProjectKind::Mod)
        .await
        .unwrap();
    assert_eq!(have.len(), 1, "a file Modrinth does not know is not listed");
    let libx = &have["LIBX"];
    assert_eq!(libx.file_name, "libx.jar");
    assert_eq!(libx.version_id, "v-old");
    assert_eq!(libx.update.as_deref(), Some("v-new"));

    // Modrinth out of reach is an error, not "nothing installed".
    world.net.forget_all();
    assert!(
        world
            .service
            .installed_projects(&record.id, ProjectKind::Mod)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn an_exported_log_hides_the_player_and_the_folders_and_leaves_nothing_half_written() {
    let world = world();
    let record = world
        .service
        .create_instance("Log", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::create_dir_all(game.join("crash-reports")).unwrap();
    std::fs::write(
        game.join("logs/latest.log"),
        format!(
            "[1] [main/INFO]: Setting user: Steve\n[2] [main/INFO]: dir {}\n",
            game.display()
        ),
    )
    .unwrap();
    std::fs::write(
        game.join("crash-reports/crash-1.txt"),
        "Player Steve crashed",
    )
    .unwrap();
    let out = world._dir.path().join("out.txt");

    world
        .service
        .export_log(&record.id, None, &out)
        .await
        .unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("Setting user: <player>") && text.contains("dir <game>"));
    assert!(!text.contains("Steve"));
    assert!(!world._dir.path().join("out.txt.part").exists());

    world
        .service
        .export_log(&record.id, Some("crash-1.txt"), &out)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "Player <player> crashed"
    );

    // No log, an unsafe name, or an unwritable place: an error and no file.
    let empty = world
        .service
        .create_instance("None", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let none = world._dir.path().join("none.txt");
    assert!(
        world
            .service
            .export_log(&empty.id, None, &none)
            .await
            .is_err()
    );
    assert!(
        world
            .service
            .export_log(&record.id, Some("../x"), &none)
            .await
            .is_err()
    );
    let blocked = world._dir.path().join("no-such-folder/out.txt");
    assert!(
        world
            .service
            .export_log(&record.id, None, &blocked)
            .await
            .is_err()
    );
    assert!(!none.exists());
}

#[tokio::test]
async fn a_cancelled_export_is_recorded_as_cancelled_and_leaves_no_pack() {
    let world = world();
    let record = world
        .service
        .create_instance("Stop", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
    let spec = crate::pack_export::ExportSpec {
        format: crate::pack_export::PackFormat::Modrinth,
        name: "Stop".into(),
        version: "1".into(),
        summary: None,
        include: vec!["mods".into()],
    };
    let pack = world._dir.path().join("stop.mrpack");
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        world
            .service
            .export_modpack(&record.id, spec, &pack, cancel)
            .await,
        Err(ServiceError::Cancelled)
    ));
    assert!(!pack.exists() && !world._dir.path().join("stop.mrpack.part").exists());
    assert_eq!(
        world.service.activity(5).finished[0].outcome,
        TaskOutcome::Cancelled
    );
}

#[tokio::test]
async fn snapshots_back_up_restore_and_undo_a_failed_restore_under_the_lease() {
    use std::os::unix::fs::PermissionsExt;
    let world = world();
    let record = world
        .service
        .create_instance("Backup", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("saves/W")).unwrap();
    std::fs::write(game.join("saves/W/level.dat"), b"v1").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:70").unwrap();

    // Nothing to back up is an error, not an empty snapshot.
    let empty = world
        .service
        .create_instance("Empty", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    assert!(matches!(
        world
            .service
            .create_snapshot(&empty.id, SnapshotScope::Full, "")
            .await,
        Err(ServiceError::Snapshot(SnapshotError::Empty))
    ));

    let snapshot = world
        .service
        .create_snapshot(&record.id, SnapshotScope::Full, "before")
        .await
        .unwrap();
    assert_eq!(world.service.snapshots(&record.id).await.unwrap().len(), 1);
    std::fs::write(game.join("saves/W/level.dat"), b"v2").unwrap();
    std::fs::write(game.join("options.txt"), b"fov:110").unwrap();

    // Busy instance: writes refused, listing still works.
    {
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        assert!(matches!(
            world.service.restore_snapshot(&record.id, &snapshot).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert!(matches!(
            world.service.delete_snapshot(&record.id, &snapshot).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert_eq!(world.service.snapshots(&record.id).await.unwrap().len(), 1);
    }

    // A restore that cannot finish leaves the game exactly as it was.
    std::fs::set_permissions(&game, std::fs::Permissions::from_mode(0o555)).unwrap();
    let failed = world.service.restore_snapshot(&record.id, &snapshot).await;
    std::fs::set_permissions(&game, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(failed, Err(ServiceError::Snapshot(_))));
    assert_eq!(
        std::fs::read(game.join("saves/W/level.dat")).unwrap(),
        b"v2"
    );
    assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"fov:110");
    assert!(operation_dirs(&world).is_empty());
    assert!(
        !change_subjects(&world, &record.id)
            .iter()
            .any(|(kind, _)| *kind == ChangeKind::SnapshotRestored)
    );

    let units = world
        .service
        .restore_snapshot(&record.id, &snapshot)
        .await
        .unwrap();
    assert!(units.contains(&std::path::PathBuf::from("saves/W")));
    assert_eq!(
        std::fs::read(game.join("saves/W/level.dat")).unwrap(),
        b"v1"
    );
    assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"fov:70");
    world
        .service
        .delete_snapshot(&record.id, &snapshot)
        .await
        .unwrap();
    assert!(
        world
            .service
            .snapshots(&record.id)
            .await
            .unwrap()
            .is_empty()
    );
    let kinds: Vec<_> = change_subjects(&world, &record.id)
        .into_iter()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(
        kinds,
        [
            ChangeKind::SnapshotDeleted,
            ChangeKind::SnapshotRestored,
            ChangeKind::SnapshotCreated
        ]
    );
}

#[tokio::test]
async fn a_restore_the_launcher_died_in_is_rolled_back_when_the_service_opens() {
    let world = world();
    let record = world
        .service
        .create_instance("Crash", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("saves/W")).unwrap();
    std::fs::write(game.join("saves/W/level.dat"), b"original").unwrap();
    // Rehearse: the original was moved aside, the launcher died before the
    // staged world was placed.
    let operation = world
        .service
        .layout
        .operations()
        .join(format!("restore-{}-snap", record.id));
    std::fs::create_dir_all(operation.join("new/saves/W")).unwrap();
    std::fs::write(operation.join("new/saves/W/level.dat"), b"restored").unwrap();
    std::fs::create_dir_all(operation.join("old/saves")).unwrap();
    std::fs::rename(game.join("saves/W"), operation.join("old/saves/W")).unwrap();
    std::fs::write(
        operation.join("restore.json"),
        format!(
            r#"{{"schema":1,"instance_id":"{}","snapshot":"snap","units":["saves/W"]}}"#,
            record.id
        ),
    )
    .unwrap();
    let root = world.service.layout.root().to_path_buf();
    let (service, _dir) = reopen(world, &root);
    assert_eq!(
        service.startup_notes(),
        [RecoveryNote::RestoreRolledBack {
            instance_id: record.id.clone(),
            snapshot: "snap".into(),
            units: vec![std::path::PathBuf::from("saves/W")],
        }]
    );
    assert_eq!(
        std::fs::read(service.layout.game(&record.id).join("saves/W/level.dat")).unwrap(),
        b"original"
    );
    assert!(!operation.exists());
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

#[tokio::test]
async fn importing_a_local_pack_builds_a_whole_instance_and_keeps_the_file() {
    let world = world();
    let other = instance_with_profile(&world, "Other").await;
    world
        .net
        .answer("cdn.modrinth.com/data/mods/a.jar", "mod-a");
    world
        .net
        .answer("cdn.modrinth.com/data/mods/server-only.jar", "never");
    let index = pack_index(&format!(
        "{},{}",
        pack_file("mods/a.jar", "mod-a", ""),
        pack_file(
            "mods/server-only.jar",
            "never",
            r#","env":{"client":"unsupported","server":"required"}"#
        ),
    ));
    let pack = world._dir.path().join("cool.mrpack");
    write_mrpack(
        &pack,
        &index,
        &[
            ("overrides/config/x.toml", "from-overrides"),
            ("overrides/options.txt", "base"),
            ("client-overrides/options.txt", "client-wins"),
            ("overrides/../escape.txt", "no"),
        ],
    );
    let record = world
        .service
        .import_modpack_file(&pack, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(record.name, "Cool Pack");
    let game = world.service.layout.game(&record.id);
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"mod-a");
    assert!(!game.join("mods/server-only.jar").exists());
    assert_eq!(
        std::fs::read(game.join("config/x.toml")).unwrap(),
        b"from-overrides"
    );
    assert_eq!(
        std::fs::read(game.join("options.txt")).unwrap(),
        b"client-wins"
    );
    assert!(!world.service.layout.profiles().join("escape.txt").exists());
    assert!(pack.is_file(), "a local pack is the user's file");
    no_leftovers(&world);
    // The other instance is exactly as it was.
    assert_eq!(
        std::fs::read(world.service.layout.game(&other.id).join("saves/level.dat")).unwrap(),
        b"world"
    );
    assert_eq!(world.service.library().await.instances.len(), 2);
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Succeeded
    );
}

#[tokio::test]
async fn the_library_stays_usable_while_a_pack_downloads() {
    let world = world();
    world
        .net
        .answer("cdn.modrinth.com/data/mods/a.jar", "mod-a");
    let pack = world._dir.path().join("slow.mrpack");
    write_mrpack(
        &pack,
        &pack_index(&pack_file("mods/a.jar", "mod-a", "")),
        &[],
    );
    let gate = Arc::new(tokio::sync::Notify::new());
    *world.net.gate.lock().unwrap() = Some(gate.clone());
    let import = world
        .service
        .import_modpack_file(&pack, CancellationToken::new());
    let probe = async {
        // Give the download time to start, then use the library.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let library =
            tokio::time::timeout(std::time::Duration::from_secs(2), world.service.library())
                .await
                .expect("the library must not wait for the download");
        assert!(library.instances.is_empty(), "nothing is published yet");
        assert_eq!(world.service.activity(10).active.len(), 1);
        gate.notify_waiters();
    };
    let (record, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(import, probe)
    })
    .await
    .unwrap();
    record.unwrap();
    assert_eq!(world.service.library().await.instances.len(), 1);
}

#[tokio::test]
async fn bad_packs_and_failed_downloads_leave_nothing_and_touch_no_instance() {
    let world = world();
    let other = instance_with_profile(&world, "Keep").await;
    let dir = world._dir.path().to_path_buf();
    let assert_untouched = |world: &World| {
        let library = std::fs::read_dir(world.service.layout.profiles())
            .unwrap()
            .count();
        assert_eq!(library, 1, "only the existing profile is on disk");
        no_leftovers(world);
    };

    // Not an archive at all.
    let junk = dir.join("junk.mrpack");
    std::fs::write(&junk, b"not a zip").unwrap();
    assert!(
        world
            .service
            .import_modpack_file(&junk, CancellationToken::new())
            .await
            .is_err()
    );
    // A path that climbs out of the game folder.
    let climbing = dir.join("climb.mrpack");
    write_mrpack(
        &climbing,
        &pack_index(&pack_file("../evil.jar", "x", "")),
        &[],
    );
    assert!(
        world
            .service
            .import_modpack_file(&climbing, CancellationToken::new())
            .await
            .is_err()
    );
    // Only an untrusted host to download from.
    let untrusted = dir.join("untrusted.mrpack");
    let index = pack_index(&format!(
        r#"{{"path":"mods/a.jar","hashes":{{"sha1":"{}"}},"fileSize":1,"downloads":["https://evil.example/a.jar"]}}"#,
        sha1_hex(b"a")
    ));
    write_mrpack(&untrusted, &index, &[("overrides/config/x.toml", "x")]);
    assert!(matches!(
        world
            .service
            .import_modpack_file(&untrusted, CancellationToken::new())
            .await,
        Err(ServiceError::Install(_))
    ));
    // A download whose content does not match the pack's hash.
    world
        .net
        .answer("cdn.modrinth.com/data/mods/a.jar", "TAMPERED");
    let tampered = dir.join("tampered.mrpack");
    write_mrpack(
        &tampered,
        &pack_index(&pack_file("mods/a.jar", "mod-a", "")),
        &[("overrides/config/x.toml", "x")],
    );
    assert!(
        world
            .service
            .import_modpack_file(&tampered, CancellationToken::new())
            .await
            .is_err()
    );
    // Not a file (a folder).
    assert!(
        world
            .service
            .import_modpack_file(&dir, CancellationToken::new())
            .await
            .is_err()
    );
    assert_untouched(&world);

    // Cancelled while the download is in flight.
    world
        .net
        .answer("cdn.modrinth.com/data/mods/b.jar", "mod-b");
    let slow = dir.join("slow.mrpack");
    write_mrpack(
        &slow,
        &pack_index(&pack_file("mods/b.jar", "mod-b", "")),
        &[("overrides/config/x.toml", "x")],
    );
    *world.net.gate.lock().unwrap() = Some(Arc::new(tokio::sync::Notify::new()));
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(world.service.import_modpack_file(&slow, cancel), async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            stop.cancel();
        })
    })
    .await
    .expect("cancelling must not wait for the download");
    assert!(matches!(result, Err(ServiceError::Cancelled)));
    *world.net.gate.lock().unwrap() = None;
    assert_untouched(&world);
    assert_eq!(world.service.library().await.instances.len(), 1);
    assert_eq!(
        std::fs::read(world.service.layout.game(&other.id).join("saves/level.dat")).unwrap(),
        b"world"
    );
    assert!(
        world
            .service
            .activity(20)
            .finished
            .iter()
            .any(|task| task.outcome == TaskOutcome::Cancelled)
    );
}

#[tokio::test]
async fn a_copy_is_independent_inherits_settings_and_leaves_the_source_alone() {
    let world = world();
    let source = instance_with_profile(&world, "Source").await;
    world.service.toggle_favorite(&source.id).await.unwrap();
    world
        .service
        .update_instance_settings(
            &source.id,
            InstanceSettings {
                max_memory_mb: Some(4096),
                ..InstanceSettings::default()
            },
        )
        .await
        .unwrap();
    world
        .service
        .store
        .lock()
        .await
        .mark_installed(&source.id, true)
        .unwrap();
    let game = world.service.layout.game(&source.id);
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(game.join("logs/latest.log"), b"log").unwrap();
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
    let history = HistoryLog::for_instance(world.service.layout.root(), &source.id);
    history
        .append(&HistoryEvent::Change {
            at: 1,
            kind: ChangeKind::ContentAdded,
            subject: "a.jar".into(),
        })
        .unwrap();
    let before = std::fs::read(game.join("saves/level.dat")).unwrap();

    let with = world
        .service
        .copy_instance(&source.id, "Source", true, CancellationToken::new())
        .await
        .unwrap();
    let without = world
        .service
        .copy_instance(&source.id, "Source", false, CancellationToken::new())
        .await
        .unwrap();
    assert_ne!(with.id, source.id);
    assert_ne!(with.id, without.id, "the same name never reuses an id");
    // Inherits what makes it the same game; starts clean otherwise.
    assert!(with.installed && with.loader == source.loader);
    assert_eq!(with.settings.max_memory_mb, Some(4096));
    assert!(!with.favorite && with.last_played.is_none() && with.play_seconds == 0);
    // Files: worlds only when asked, volatile output never, history not copied.
    let copy = world.service.layout.game(&with.id);
    assert_eq!(std::fs::read(copy.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(std::fs::read(copy.join("saves/level.dat")).unwrap(), before);
    assert!(!copy.join("logs").exists());
    let bare = world.service.layout.game(&without.id);
    assert!(bare.join("mods/a.jar").is_file() && !bare.join("saves").exists());
    assert!(
        HistoryLog::for_instance(world.service.layout.root(), &with.id)
            .read()
            .unwrap()
            .events
            .is_empty()
    );
    // Independent: changing the copy leaves the source as it was.
    std::fs::write(copy.join("mods/a.jar"), b"changed").unwrap();
    std::fs::write(copy.join("saves/level.dat"), b"changed").unwrap();
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(std::fs::read(game.join("saves/level.dat")).unwrap(), before);
    no_leftovers(&world);
    assert_eq!(world.service.library().await.instances.len(), 3);
    assert_eq!(
        world.service.activity(10).finished[0].outcome,
        TaskOutcome::Succeeded
    );
}

#[tokio::test]
async fn a_copy_that_is_refused_cancelled_or_fails_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let world = world();
    let source = instance_with_profile(&world, "Source").await;
    let game = world.service.layout.game(&source.id);
    std::fs::write(game.join("secret.txt"), b"s").unwrap();
    let tree_before = std::fs::read_dir(&game).unwrap().count();

    // Blank name: refused up front.
    assert!(matches!(
        world
            .service
            .copy_instance(&source.id, "  ", true, CancellationToken::new())
            .await,
        Err(ServiceError::Store(StoreError::InvalidName))
    ));
    // Unknown source.
    assert!(matches!(
        world
            .service
            .copy_instance("ghost", "X", true, CancellationToken::new())
            .await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    // Busy source (launching or running): no copy of a moving target.
    {
        let _busy = world.service.reserve_instance(&source.id).unwrap();
        assert!(matches!(
            world
                .service
                .copy_instance(&source.id, "X", true, CancellationToken::new())
                .await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    // Cancelled before it starts.
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        world
            .service
            .copy_instance(&source.id, "X", true, cancel)
            .await,
        Err(ServiceError::Cancelled)
    ));
    // An unreadable file stops the copy part-way.
    std::fs::set_permissions(
        game.join("secret.txt"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let failed = world
        .service
        .copy_instance(&source.id, "X", true, CancellationToken::new())
        .await;
    std::fs::set_permissions(
        game.join("secret.txt"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(matches!(failed, Err(ServiceError::Io(_))));

    // Nothing was published, nothing is left behind, the source is as it was.
    assert_eq!(world.service.library().await.instances.len(), 1);
    assert_eq!(
        std::fs::read_dir(world.service.layout.profiles())
            .unwrap()
            .count(),
        1
    );
    no_leftovers(&world);
    assert_eq!(std::fs::read_dir(&game).unwrap().count(), tree_before);
    assert_eq!(std::fs::read(game.join("secret.txt")).unwrap(), b"s");
    // And a plain retry works.
    world
        .service
        .copy_instance(&source.id, "X", true, CancellationToken::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn favorites_toggle_and_persist() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Fav", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert!(world.service.toggle_favorite(&record.id).await.unwrap());
    assert!(world.service.library().await.instances[0].favorite);
    assert!(!world.service.toggle_favorite(&record.id).await.unwrap());
    assert!(matches!(
        world.service.toggle_favorite("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
}

async fn launch_to_end(world: &World, id: &str) -> Result<GameExit, ServiceError> {
    let (tx, _rx) = mpsc::unbounded_channel();
    world.service.launch(id, tx, CancellationToken::new()).await
}

#[tokio::test]
async fn launch_defaults_reach_the_real_process_and_its_commands() {
    use crate::tuning::{EnvVar, LaunchTuning};
    let world = world();
    publish_release(&world);
    fake_java(
        &world,
        concat!(
            "echo \"Setting user: x\"\n",
            "echo \"$*\" > \"$INST_DIR/args.txt\"\n",
            "echo \"FOO=$FOO WRAPPED=$WRAPPED NAME=$INST_NAME\" > \"$INST_DIR/env.txt\"",
        ),
    );
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    world
        .service
        .set_launch_defaults(LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            fullscreen: Some(true),
            jvm_arguments: vec!["-Dtuned=1".into()],
            game_arguments: vec!["--demo".into()],
            environment: vec![EnvVar {
                name: "FOO".into(),
                value: "bar".into(),
            }],
            wrapper: Some("env WRAPPED=yes".into()),
            pre_launch: Some("echo before > \"$INST_DIR/pre.txt\"".into()),
            post_exit: Some("echo after > \"$INST_DIR/post.txt\"".into()),
        })
        .await
        .unwrap();

    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(args.contains("-Dtuned=1"), "{args}");
    assert!(args.contains("--width 1280 --height 720"), "{args}");
    assert!(args.contains("--fullscreen"), "{args}");
    assert!(args.trim_end().ends_with("--demo"), "{args}");
    let env = std::fs::read_to_string(game.join("env.txt")).unwrap();
    assert_eq!(env.trim(), "FOO=bar WRAPPED=yes NAME=Run");
    assert_eq!(
        std::fs::read_to_string(game.join("pre.txt"))
            .unwrap()
            .trim(),
        "before"
    );
    assert_eq!(
        std::fs::read_to_string(game.join("post.txt"))
            .unwrap()
            .trim(),
        "after"
    );
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

#[tokio::test]
async fn changing_the_version_prepares_the_new_files_then_commits_and_records_it() {
    let world = world();
    publish_versions(&world, &["1.0", "2.0"]);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let versions = world.service.layout().versions();
    assert!(versions.join("1.0/1.0.jar").is_file());
    assert!(!versions.join("2.0/2.0.jar").exists());

    let (tx, _rx) = mpsc::unbounded_channel();
    let changed = world
        .service
        .change_runtime(
            &record.id,
            "2.0",
            Loader::Vanilla,
            None,
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(changed.game_version, "2.0");
    assert!(changed.installed);
    assert!(
        versions.join("2.0/2.0.jar").is_file(),
        "the new files are fetched"
    );
    assert!(
        versions.join("1.0/1.0.jar").is_file(),
        "the old shared files stay"
    );
    let stored = world.service.instance(&record.id).await.unwrap();
    assert_eq!(stored.game_version, "2.0");

    // It starts on the new version, offline, and the history says what changed.
    world.net.forget_all();
    launch_to_end(&world, &record.id).await.unwrap();
    let history = world.service.history(&record.id).await.unwrap();
    assert!(
        history.events.iter().any(|event| matches!(
            event,
            HistoryEvent::Change { kind: ChangeKind::GameVersionChanged, subject, .. }
                if subject.contains("1.0") && subject.contains("2.0")
        )),
        "{:?}",
        history.events
    );

    // The same combination again is refused, not repeated.
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .change_runtime(
                &record.id,
                "2.0",
                Loader::Vanilla,
                None,
                tx,
                CancellationToken::new()
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_version_change_that_cannot_finish_leaves_the_old_game_launchable() {
    let world = world();
    publish_versions(&world, &["1.0", "2.0"]);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let ask = |version: &'static str, cancel: CancellationToken| {
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .change_runtime(&record.id, version, Loader::Vanilla, None, tx, cancel)
    };

    // The new client cannot be downloaded: nothing changes.
    std::fs::remove_file(world.server.join("2.0.jar")).unwrap();
    assert!(ask("2.0", CancellationToken::new()).await.is_err());
    assert_eq!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .game_version,
        "1.0"
    );

    // Cancelled before it starts: nothing changes.
    std::fs::write(world.server.join("2.0.jar"), b"client of 2.0").unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(ask("2.0", cancel).await.is_err());
    assert_eq!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .game_version,
        "1.0"
    );

    // The library cannot be written: the files were fetched but the
    // record stays on the old version.
    world.service.store.lock().await.set_read_only(true);
    assert!(ask("2.0", CancellationToken::new()).await.is_err());
    world.service.store.lock().await.set_read_only(false);
    assert_eq!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .game_version,
        "1.0"
    );

    // Through all of it, the old version still starts without the network.
    world.net.forget_all();
    launch_to_end(&world, &record.id).await.unwrap();
    assert!(
        world
            .service
            .history(&record.id)
            .await
            .unwrap()
            .events
            .iter()
            .all(|event| {
                !matches!(
                    event,
                    HistoryEvent::Change {
                        kind: ChangeKind::GameVersionChanged,
                        ..
                    }
                )
            }),
        "no change was recorded"
    );
}

#[tokio::test]
async fn a_loader_needs_its_version_and_an_unsupported_one_is_refused() {
    let world = world();
    publish_versions(&world, &["1.0"]);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let error = world
        .service
        .change_runtime(
            &record.id,
            "1.0",
            Loader::Fabric,
            None,
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("loader version"), "{error}");
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(
        world
            .service
            .change_runtime(
                "ghost",
                "1.0",
                Loader::Vanilla,
                None,
                tx,
                CancellationToken::new()
            )
            .await
            .is_err()
    );
}

async fn repair_to_end(world: &World, id: &str) -> Result<(), ServiceError> {
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .repair_instance(id, tx, CancellationToken::new())
        .await
}

#[tokio::test]
async fn repairing_refetches_only_what_is_damaged_and_needs_no_network_when_intact() {
    let world = world();
    publish_versions(&world, &["1.0"]);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let jar = world.service.layout().versions().join("1.0/1.0.jar");
    let good = std::fs::read(&jar).unwrap();

    // Damaged: put back, and a worlds/mods folder stays as it was.
    let saves = world.service.layout().game(&record.id).join("saves/keep");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(&jar, b"broken").unwrap();
    repair_to_end(&world, &record.id).await.unwrap();
    assert_eq!(std::fs::read(&jar).unwrap(), good);
    assert!(saves.is_dir());

    // Intact and offline: nothing to fetch, nothing to fail.
    world.net.forget_all();
    repair_to_end(&world, &record.id).await.unwrap();
    assert_eq!(std::fs::read(&jar).unwrap(), good);

    // Missing: restored too, and both repairs are in the history.
    std::fs::remove_file(&jar).unwrap();
    world.net.answer(
        "piston-meta.mojang.com/mc/game/version_manifest",
        r#"{"latest":{"release":"1.0","snapshot":"1.0"},"versions":[]}"#.to_owned(),
    );
    repair_to_end(&world, &record.id).await.unwrap();
    assert_eq!(std::fs::read(&jar).unwrap(), good);
    let repaired = world
        .service
        .history(&record.id)
        .await
        .unwrap()
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                HistoryEvent::Change {
                    kind: ChangeKind::Repaired,
                    ..
                }
            )
        })
        .count();
    assert_eq!(repaired, 3);
    let activity = world.service.activity(10);
    assert!(
        activity
            .finished
            .iter()
            .any(|task| task.category == TaskCategory::Repair)
    );
}

#[tokio::test]
async fn launching_into_a_world_adds_the_release_s_quick_play_arguments_only_where_declared() {
    let world = world();
    publish_modern_release(&world);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    std::fs::create_dir_all(game.join("saves/My World")).unwrap();
    let go = |name: &'static str| {
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .launch_world(&record.id, name, tx, CancellationToken::new())
    };

    go("My World").await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(args.contains("--quickPlaySingleplayer My World"), "{args}");
    assert!(
        args.contains("--quickPlayPath quickPlay/lumilio.json"),
        "{args}"
    );
    assert!(!args.contains("--quickPlayMultiplayer"), "{args}");

    // A world that is not there never starts the game.
    std::fs::remove_file(game.join("args.txt")).unwrap();
    let error = go("Gone").await.unwrap_err();
    assert!(error.to_string().contains("no saved world"), "{error}");
    assert!(go("../escape").await.is_err());
    assert!(!game.join("args.txt").exists());

    // The ordinary launch goes to the menu: no quick-play arguments.
    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(!args.contains("quickPlay"), "{args}");
}

#[tokio::test]
async fn a_version_without_quick_play_says_so_instead_of_starting_at_the_menu() {
    let world = world();
    publish_release(&world);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    std::fs::create_dir_all(game.join("saves/My World")).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let error = world
        .service
        .launch_world(&record.id, "My World", tx, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot start directly in singleplayer"),
        "{error}"
    );
    assert!(!game.join("args.txt").exists(), "the game must not start");
}

#[tokio::test]
async fn a_chosen_server_uses_quick_play_where_declared_and_the_old_arguments_elsewhere() {
    use crate::tuning::{InstanceLaunch, QuickPlay};
    for (modern, expected) in [
        (true, "--quickPlayMultiplayer mc.example.com:25565"),
        (false, "--server mc.example.com --port 25565"),
    ] {
        let world = world();
        if modern {
            publish_modern_release(&world);
        } else {
            publish_release(&world);
        }
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let mut settings = record.settings.clone();
        settings.launch = InstanceLaunch {
            quick_play: Some(QuickPlay::Server("mc.example.com:25565".into())),
            ..InstanceLaunch::default()
        };
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let args =
            std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt"))
                .unwrap();
        assert!(args.contains(expected), "modern={modern}: {args}");
    }
}

#[tokio::test]
async fn an_instance_overrides_some_defaults_and_follows_the_rest() {
    use crate::tuning::{InstanceLaunch, LaunchTuning};
    let world = world();
    publish_release(&world);
    fake_java(&world, ECHO_ARGS);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    world
        .service
        .set_launch_defaults(LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            game_arguments: vec!["--demo".into()],
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    let mut settings = record.settings.clone();
    settings.launch = InstanceLaunch {
        fullscreen: Some(true),
        ..InstanceLaunch::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings.clone())
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let game = world.service.layout().game(&record.id);
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(
        args.contains("--width 1280 --height 720"),
        "follows the defaults: {args}"
    );
    assert!(args.contains("--fullscreen"), "its own override: {args}");
    assert!(args.contains("--demo"), "{args}");

    // Overriding the arguments with "none" drops them; clearing the
    // override follows the defaults again.
    settings.launch.game_arguments = Some(Vec::new());
    world
        .service
        .update_instance_settings(&record.id, settings.clone())
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(!args.contains("--demo"), "{args}");
    settings.launch = InstanceLaunch::default();
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
    assert!(
        args.contains("--demo") && !args.contains("--fullscreen"),
        "{args}"
    );
}

#[tokio::test]
async fn a_failing_command_before_launch_stops_the_game_but_one_after_exit_does_not() {
    use crate::tuning::LaunchTuning;
    let world = world();
    publish_release(&world);
    fake_java(
        &world,
        "echo \"Setting user: x\"\necho ran > \"$INST_DIR/ran.txt\"",
    );
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    world
        .service
        .set_launch_defaults(LaunchTuning {
            pre_launch: Some("exit 3".into()),
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    let error = launch_to_end(&world, &record.id).await.unwrap_err();
    assert!(error.to_string().contains("before launch"), "{error}");
    assert!(error.to_string().contains('3'), "{error}");
    assert!(!game.join("ran.txt").exists(), "the game must not start");

    world
        .service
        .set_launch_defaults(LaunchTuning {
            post_exit: Some("exit 9".into()),
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    let exit = launch_to_end(&world, &record.id).await.unwrap();
    assert_eq!(
        exit.code,
        Some(0),
        "a failed command after exit changes nothing"
    );
    assert!(game.join("ran.txt").exists());
}

#[tokio::test]
async fn adding_a_java_by_its_executable_or_folder_remembers_the_folder() {
    let world = world();
    let elsewhere = world._dir.path().join("jdks/zulu-17");
    std::fs::create_dir_all(elsewhere.join("bin")).unwrap();
    std::fs::write(elsewhere.join("release"), "JAVA_VERSION=\"17.0.9\"\n").unwrap();
    std::fs::write(elsewhere.join("bin/java"), "#!/bin/sh\n").unwrap();
    assert!(world.service.java_installations().await.is_empty());

    let added = world
        .service
        .add_java(&elsewhere.join("bin/java"))
        .await
        .unwrap();
    assert_eq!(added.home(), elsewhere);
    assert_eq!(world.service.java_installations().await.len(), 1);
    assert_eq!(
        world.service.settings().await.extra_java_roots,
        std::slice::from_ref(&elsewhere)
    );
    // The same one again changes nothing; a folder of installations works too.
    world.service.add_java(&elsewhere).await.unwrap();
    assert_eq!(world.service.settings().await.extra_java_roots.len(), 1);
    world
        .service
        .add_java(elsewhere.parent().unwrap())
        .await
        .unwrap();

    let nothing = world._dir.path().join("empty");
    std::fs::create_dir_all(&nothing).unwrap();
    assert!(matches!(
        world.service.add_java(&nothing).await,
        Err(ServiceError::NoJavaAt(_))
    ));
}

#[tokio::test]
async fn a_java_the_user_turned_off_is_listed_but_never_chosen() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let installed = world.service.java_installations().await;
    assert_eq!(installed.len(), 1);
    assert!(!installed[0].1);
    let home = installed[0].0.home().to_owned();

    world.service.set_java_disabled(&home, true).await.unwrap();
    assert!(
        world.service.java_installations().await[0].1,
        "still listed, marked off"
    );
    let error = launch_to_end(&world, &record.id).await.unwrap_err();
    assert!(error.to_string().contains("Java"), "{error}");

    world.service.set_java_disabled(&home, false).await.unwrap();
    assert!(!world.service.java_installations().await[0].1);
    launch_to_end(&world, &record.id).await.unwrap();
}

#[tokio::test]
async fn clearing_the_cache_frees_space_and_games_still_start() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let natives = world.service.layout().natives("1.0");
    std::fs::create_dir_all(&natives).unwrap();
    std::fs::write(natives.join("lib.so"), vec![1u8; 512]).unwrap();
    let before = world.service.storage_usage().await;
    assert!(before.cache >= 512 && before.shared > 0 && before.runtimes > 0);
    let freed = world.service.clear_cache().await.unwrap();
    assert!(freed >= 512);
    let after = world.service.storage_usage().await;
    assert_eq!(after.cache, 0);
    assert_eq!(after.shared, before.shared, "shared game files stay");
    launch_to_end(&world, &record.id).await.unwrap();
}

#[tokio::test]
async fn the_diagnostics_bundle_hides_the_player_the_folders_and_commands() {
    use crate::tuning::LaunchTuning;
    use std::io::Read;
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: Steve\"");
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    world
        .service
        .set_launch_defaults(LaunchTuning {
            pre_launch: Some("echo secret-command".into()),
            ..LaunchTuning::default()
        })
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let game = world.service.layout().game(&record.id);
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(
        game.join("logs/latest.log"),
        format!(
            "Steve joined from {}\n",
            world.service.layout().root().display()
        ),
    )
    .unwrap();
    let path = world._dir.path().join("diagnostics.zip");
    world.service.export_diagnostics(&path).await.unwrap();

    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut all = String::new();
    for index in 0..archive.len() {
        archive
            .by_index(index)
            .unwrap()
            .read_to_string(&mut all)
            .unwrap();
    }
    assert!(all.contains("LumilioCL"), "{all}");
    assert!(all.contains("<player> joined from <launcher>"), "{all}");
    assert!(!all.contains("Steve"), "{all}");
    assert!(!all.contains(&world.service.layout().root().display().to_string()));
    assert!(!all.contains("secret-command"), "commands are only counted");
    assert!(all.contains("pre-launch true"));
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

#[tokio::test]
async fn signing_in_adds_the_account_keeps_secrets_in_the_store_and_shows_the_code() {
    let world = world();
    microsoft_chain(&world, "Edwin_Zhan", "mc-1");
    let shown: Arc<StdMutex<Option<DeviceCode>>> = Arc::default();
    let sink = shown.clone();
    let (key, name) = world
        .service
        .microsoft_sign_in(
            move |code| *sink.lock().unwrap() = Some(code.clone()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        (key.as_str(), name.as_str()),
        ("msa:123e4567e89b12d3a456426614174000", "Edwin_Zhan")
    );
    assert_eq!(shown.lock().unwrap().as_ref().unwrap().user_code, "AB12CD");

    // A second account does not take over the selection; the offline one stays.
    let settings = world.service.settings().await;
    assert_eq!(settings.accounts.len(), 2);
    assert_eq!(settings.selected_account.as_deref(), Some("Steve"));
    // Public facts in settings; every secret only in the store.
    let file =
        std::fs::read_to_string(world.service.layout().root().join("settings.json")).unwrap();
    for secret in ["ms-refresh-next", "mc-1", "ms-access"] {
        assert!(!file.contains(secret), "{secret} leaked into settings.json");
    }
    let stored = StoredLogin::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap();
    assert_eq!(stored.refresh_token, "ms-refresh-next");
    assert_eq!(stored.access_token, "mc-1");

    // Signing in again updates the one account and its name.
    microsoft_chain(&world, "Renamed", "mc-2");
    sign_in(&world).await.unwrap();
    let accounts = world.service.settings().await.accounts;
    assert_eq!(accounts.len(), 2);
    assert!(accounts.iter().any(|entry| entry.name == "Renamed"));
}

#[tokio::test]
async fn a_store_that_does_not_work_stops_the_sign_in_before_any_code_is_shown() {
    let world = world();
    microsoft_chain(&world, "Edwin_Zhan", "mc-1");
    world.secrets.break_it();
    let shown = Arc::new(StdMutex::new(false));
    let sink = shown.clone();
    let error = world
        .service
        .microsoft_sign_in(
            move |_| *sink.lock().unwrap() = true,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, ServiceError::Auth(AuthError::CredentialStore(_))),
        "{error}"
    );
    assert!(
        !*shown.lock().unwrap(),
        "no code for a sign-in that cannot be kept"
    );
    assert_eq!(world.service.settings().await.accounts.len(), 1);
}

#[tokio::test]
async fn a_refusal_or_a_cancel_leaves_no_account_and_no_secret_behind() {
    use crate::microsoft::PROFILE_URL;
    let world = world();
    microsoft_chain(&world, "Edwin_Zhan", "mc-1");
    world.net.clear_replies(PROFILE_URL);
    world
        .net
        .reply(PROFILE_URL, 404, r#"{"error":"NOT_FOUND"}"#);
    let error = sign_in(&world).await.unwrap_err();
    assert!(
        matches!(error, ServiceError::Auth(AuthError::NoGameOwnership)),
        "{error}"
    );
    assert_eq!(world.service.settings().await.accounts.len(), 1);
    assert!(world.secrets.values().is_empty());

    microsoft_chain(&world, "Edwin_Zhan", "mc-1");
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = world
        .service
        .microsoft_sign_in(|_| {}, cancel)
        .await
        .unwrap_err();
    assert!(
        matches!(error, ServiceError::Auth(AuthError::Cancelled)),
        "{error}"
    );
    assert_eq!(world.service.settings().await.accounts.len(), 1);
}

#[tokio::test]
async fn a_launch_uses_the_real_profile_and_token_and_reuses_a_fresh_token_without_the_network() {
    use crate::microsoft::TOKEN_URL;
    let world = world();
    publish_identity_release(&world);
    fake_java(&world, ECHO_ARGS);
    microsoft_chain(&world, "Edwin_Zhan", "mc-1");
    let (key, _) = sign_in(&world).await.unwrap();
    world.service.select_account(&key).await.unwrap();
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let before = world.net.sent_to(TOKEN_URL);
    launch_to_end(&world, &record.id).await.unwrap();
    let args =
        std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt")).unwrap();
    assert!(args.contains("--username Edwin_Zhan"), "{args}");
    assert!(
        args.contains(&format!("--uuid {MS_ID}")) || args.contains(MS_ID),
        "{args}"
    );
    assert!(args.contains("--accessToken mc-1"), "{args}");
    assert_eq!(
        world.net.sent_to(TOKEN_URL),
        before,
        "a fresh token needs no refresh"
    );
}

#[tokio::test]
async fn an_expiring_token_is_refreshed_and_the_rotated_secret_is_kept() {
    use crate::microsoft::TOKEN_URL;
    let world = world();
    publish_identity_release(&world);
    fake_java(&world, ECHO_ARGS);
    microsoft_chain(&world, "Edwin_Zhan", "mc-1");
    let (key, _) = sign_in(&world).await.unwrap();
    world.service.select_account(&key).await.unwrap();
    // The cached token has run out.
    let mut stored = StoredLogin::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap();
    stored.expires_at = now() + 10;
    stored.save(world.secrets.as_ref(), &key).unwrap();
    microsoft_chain(&world, "Edwin_Zhan", "mc-fresh");
    world.net.clear_replies(TOKEN_URL);
    world.net.reply(
        TOKEN_URL,
        200,
        r#"{"access_token":"ms-access-2","refresh_token":"ms-refresh-3"}"#,
    );
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    launch_to_end(&world, &record.id).await.unwrap();
    let args =
        std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt")).unwrap();
    assert!(args.contains("--accessToken mc-fresh"), "{args}");
    let kept = StoredLogin::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap();
    assert_eq!(
        kept.refresh_token, "ms-refresh-3",
        "the rotated token replaces the old"
    );
    assert_eq!(kept.access_token, "mc-fresh");
    assert!(kept.expires_at > now() + 3600);
}

#[tokio::test]
async fn a_rejected_sign_in_stops_the_launch_and_marks_the_account() {
    use crate::microsoft::TOKEN_URL;
    let world = world();
    publish_identity_release(&world);
    fake_java(&world, ECHO_ARGS);
    microsoft_chain(&world, "Edwin_Zhan", "mc-1");
    let (key, _) = sign_in(&world).await.unwrap();
    world.service.select_account(&key).await.unwrap();
    let mut stored = StoredLogin::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap();
    stored.expires_at = 0;
    stored.save(world.secrets.as_ref(), &key).unwrap();
    world.net.clear_replies(TOKEN_URL);
    world
        .net
        .reply(TOKEN_URL, 400, r#"{"error":"invalid_grant"}"#);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let error = launch_to_end(&world, &record.id).await.unwrap_err();
    assert!(
        matches!(error, ServiceError::SignInRequired(ref name) if name == "Edwin_Zhan"),
        "{error}"
    );
    let accounts = world.service.settings().await.accounts;
    assert!(
        accounts
            .iter()
            .any(|entry| entry.kind == AccountKind::Microsoft && entry.needs_sign_in)
    );
    assert!(
        !world
            .service
            .layout()
            .game(&record.id)
            .join("args.txt")
            .exists(),
        "no game, and never as someone else"
    );

    // A network failure is a different error and does not mark the account.
    world.net.clear_replies(TOKEN_URL);
    world
        .service
        .settings
        .lock()
        .await
        .set_needs_sign_in(&key, false)
        .unwrap();
    let error = launch_to_end(&world, &record.id).await.unwrap_err();
    assert!(
        matches!(error, ServiceError::Auth(AuthError::Network(_))),
        "{error}"
    );
    assert!(
        world
            .service
            .settings()
            .await
            .accounts
            .iter()
            .all(|entry| !entry.needs_sign_in)
    );

    // Refreshing by hand after the account recovers clears the mark.
    microsoft_chain(&world, "Edwin_Zhan", "mc-9");
    world.service.refresh_account(&key).await.unwrap();
    assert!(
        world
            .service
            .settings()
            .await
            .accounts
            .iter()
            .all(|entry| !entry.needs_sign_in)
    );
}

#[tokio::test]
async fn removing_a_microsoft_account_deletes_its_secret_and_tokens_stay_out_of_diagnostics() {
    use std::io::Read;
    let world = world();
    microsoft_chain(&world, "Edwin_Zhan", "mc-secret-token");
    let (key, _) = sign_in(&world).await.unwrap();
    let path = world._dir.path().join("diagnostics.zip");
    world.service.export_diagnostics(&path).await.unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut all = String::new();
    for index in 0..archive.len() {
        archive
            .by_index(index)
            .unwrap()
            .read_to_string(&mut all)
            .unwrap();
    }
    for hidden in ["mc-secret-token", "ms-refresh-next", "Edwin_Zhan", MS_ID] {
        assert!(!all.contains(hidden), "{hidden} reached the bundle: {all}");
    }

    world.service.remove_account(&key).await.unwrap();
    assert!(
        world.secrets.values().is_empty(),
        "the secret goes with the account"
    );
    assert_eq!(world.service.settings().await.accounts.len(), 1);
    // An unknown key is an error and touches nothing.
    assert!(world.service.remove_account("msa:nope").await.is_err());
}

#[tokio::test]
async fn launching_without_an_account_is_refused_instead_of_using_a_stand_in() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    world.service.remove_account("Steve").await.unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let result = world
        .service
        .launch(&record.id, tx, CancellationToken::new())
        .await;
    assert!(matches!(result, Err(ServiceError::NoAccount)));
    // The refusal leaves the instance free for the retry after adding one.
    world.service.add_account("Alex", None).await.unwrap();
    assert_eq!(
        world.service.settings().await.selected_account.as_deref(),
        Some("Alex")
    );
    assert!(world.service.delete_instance(&record.id).await.is_ok());
}

#[tokio::test]
async fn accounts_selection_and_custom_ids_persist_across_reopen() {
    let world = world();
    world
        .service
        .add_account("Alex", Some("123e4567-e89b-12d3-a456-426614174000"))
        .await
        .unwrap();
    world.service.select_account("Alex").await.unwrap();
    let settings = world.service.settings().await;
    assert_eq!(settings.selected_account.as_deref(), Some("Alex"));
    let alex = settings.accounts.iter().find(|a| a.name == "Alex").unwrap();
    assert_eq!(
        alex.profile().unwrap().id().compact(),
        "123e4567e89b12d3a456426614174000"
    );
    // Same id again, or a malformed one, is refused and changes nothing.
    assert!(
        world
            .service
            .add_account("Bob", Some("123E4567E89B12D3A456426614174000"))
            .await
            .is_err()
    );
    assert!(
        world
            .service
            .add_account("Bob", Some("nope"))
            .await
            .is_err()
    );
    assert_eq!(world.service.settings().await.accounts.len(), 2);
}

#[tokio::test]
async fn an_instance_java_must_exist_and_a_bad_override_changes_nothing() {
    use crate::tuning::InstanceLaunch;
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let mut settings = record.settings.clone();
    settings.java_path = Some(world._dir.path().join("no-such-java"));
    assert!(matches!(
        world
            .service
            .update_instance_settings(&record.id, settings.clone())
            .await,
        Err(ServiceError::NoJavaAt(_))
    ));
    settings.java_path = None;
    settings.launch = InstanceLaunch {
        window_width: Some(800),
        ..InstanceLaunch::default()
    };
    assert!(
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await
            .is_err()
    );
    assert!(
        world
            .service
            .instance(&record.id)
            .await
            .unwrap()
            .settings
            .launch
            .is_default()
    );
}

#[tokio::test]
async fn the_window_follows_the_instance_choice_then_the_preference() {
    use crate::tuning::{AfterLaunch, InstanceLaunch, Preferences};
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert_eq!(
        world.service.after_launch_for(&record.id).await,
        AfterLaunch::Keep
    );
    world
        .service
        .set_preferences(Preferences {
            after_launch: AfterLaunch::Hide,
            ..Preferences::default()
        })
        .await
        .unwrap();
    assert_eq!(
        world.service.after_launch_for(&record.id).await,
        AfterLaunch::Hide
    );
    let mut settings = record.settings.clone();
    settings.launch = InstanceLaunch {
        after_launch: Some(AfterLaunch::Keep),
        ..InstanceLaunch::default()
    };
    world
        .service
        .update_instance_settings(&record.id, settings)
        .await
        .unwrap();
    assert_eq!(
        world.service.after_launch_for(&record.id).await,
        AfterLaunch::Keep
    );
    assert_eq!(
        world.service.after_launch_for("ghost").await,
        AfterLaunch::Hide
    );
}

#[tokio::test]
async fn the_current_instance_persists_and_reads_as_none_once_deleted() {
    let world = world();
    publish_release(&world);
    let a = world
        .service
        .create_instance("A", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let b = world
        .service
        .create_instance("B", None, Loader::Vanilla, None)
        .await
        .unwrap();
    assert_eq!(world.service.current_instance().await, None);
    assert!(matches!(
        world.service.set_current_instance("ghost").await,
        Err(ServiceError::NoSuchInstance(_))
    ));
    world.service.set_current_instance(&b.id).await.unwrap();
    assert_eq!(world.service.current_instance().await, Some(b.id.clone()));
    world.service.delete_instance(&b.id).await.unwrap();
    assert_eq!(world.service.current_instance().await, None);
    world.service.set_current_instance(&a.id).await.unwrap();
    assert_eq!(world.service.current_instance().await, Some(a.id.clone()));
}

#[tokio::test]
async fn launching_an_unknown_instance_is_an_error() {
    let world = world();
    let (tx, _rx) = mpsc::unbounded_channel();
    let result = world
        .service
        .launch("ghost", tx, CancellationToken::new())
        .await;
    assert!(matches!(result, Err(ServiceError::NoSuchInstance(_))));
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

#[tokio::test]
async fn required_dependencies_are_listed_nearest_first_once_and_without_what_is_there() {
    let world = world();
    let record = world
        .service
        .create_instance("Deps", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    // cool needs LIBX and LIBY; LIBX needs LIBY and cool back (a cycle).
    world.net.answer(
        "/v2/project/cool/version",
        project_versions("P", "1.0", &["LIBX", "LIBY"]),
    );
    world.net.answer(
        "/v2/project/LIBX/version",
        project_versions("LIBX", "1.0", &["LIBY", "P"]),
    );
    world.net.answer(
        "/v2/project/LIBY/version",
        project_versions("LIBY", "1.0", &[]),
    );

    let need = world
        .service
        .missing_dependencies(&record.id, ProjectKind::Mod, "cool", None)
        .await
        .unwrap();
    let ids: Vec<_> = need.iter().map(|n| n.project_id.as_str()).collect();
    assert_eq!(ids, ["LIBX", "LIBY"]);
    assert!(need.iter().all(|n| n.version.is_some()));

    // Only mods have dependencies to offer.
    assert!(
        world
            .service
            .missing_dependencies(&record.id, ProjectKind::ResourcePack, "cool", None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn optional_mods_are_suggested_and_installed_ones_it_cannot_live_with_are_named() {
    let world = world();
    let record = world
        .service
        .create_instance("Rel", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    let file = |id: &str| {
        format!(
            r#""files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                "primary":true,"size":4,"hashes":{{"sha1":"{id}"}}}}]"#
        )
    };
    let version = |id: &str, deps: &str| {
        format!(
            r#"[{{"id":"v-{id}","project_id":"{id}","name":"{id}","version_number":"1",
                "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",{},"dependencies":[{deps}]}}]"#,
            file(id)
        )
        .into_bytes()
    };
    world.net.answer(
        "/v2/project/cool/version",
        version(
            "P",
            r#"{"project_id":"OPT","dependency_type":"optional"},
               {"project_id":"BAD","dependency_type":"incompatible"},
               {"project_id":"FINE","dependency_type":"incompatible"}"#,
        ),
    );
    world
        .net
        .answer("/v2/project/OPT/version", version("OPT", ""));
    // BAD is installed (Modrinth recognises it by hash); FINE is not.
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("bad.jar"), b"bad").unwrap();
    world.net.answer(
        "/v2/version_files",
        format!(
            r#"{{"{}":{}}}"#,
            sha1_hex(b"bad"),
            String::from_utf8(version("BAD", ""))
                .unwrap()
                .trim_start_matches('[')
                .trim_end_matches(']')
        ),
    );

    let report = world
        .service
        .dependency_report(&record.id, ProjectKind::Mod, "cool", None)
        .await
        .unwrap();
    assert!(report.needs.is_empty());
    let optional: Vec<_> = report
        .optional
        .iter()
        .map(|n| n.project_id.as_str())
        .collect();
    assert_eq!(optional, ["OPT"]);
    assert!(report.optional[0].version.is_some());
    assert_eq!(
        report.conflicts,
        ["BAD"],
        "only what is installed conflicts"
    );
}

#[tokio::test]
async fn a_dependency_already_in_the_game_is_not_offered_and_one_without_a_fit_is_flagged() {
    let world = world();
    let record = world
        .service
        .create_instance("Deps", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world.net.answer(
        "/v2/project/cool/version",
        project_versions("P", "1.0", &["LIBX", "OLD"]),
    );
    world.net.answer(
        "/v2/project/LIBX/version",
        project_versions("LIBX", "1.0", &[]),
    );
    world.net.answer(
        "/v2/project/OLD/version",
        project_versions("OLD", "0.9", &[]),
    );
    // The game already holds LIBX, which Modrinth recognises by hash.
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("libx.jar"), b"libx").unwrap();
    world.net.answer(
        "/v2/version_files",
        format!(
            r#"{{"{}":{{"id":"v-LIBX","project_id":"LIBX","name":"x","version_number":"1",
                "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",
                "files":[{{"url":"https://cdn.modrinth.com/l.jar","filename":"l.jar",
                           "primary":true,"size":4,"hashes":{{"sha1":"{}"}}}}],
                "dependencies":[]}}}}"#,
            sha1_hex(b"libx"),
            sha1_hex(b"libx")
        ),
    );

    let need = world
        .service
        .missing_dependencies(&record.id, ProjectKind::Mod, "cool", None)
        .await
        .unwrap();
    assert_eq!(need.len(), 1, "{need:?}");
    assert_eq!(need[0].project_id, "OLD");
    assert!(need[0].version.is_none(), "no version fits this game");
}

#[tokio::test]
async fn installed_content_lands_in_the_profile_and_is_logged() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Mods".to_owned(),
                game_version: "1.0".to_owned(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.0".to_owned()),
            },
            1,
        )
        .unwrap()
        .clone();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));

    let name = world
        .service
        .install_content(
            &record.id,
            ProjectKind::Mod,
            "cool",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(name, "cool.jar");
    let installed = world
        .service
        .layout()
        .game(&record.id)
        .join("mods/cool.jar");
    assert_eq!(std::fs::read(installed).unwrap(), b"a mod");
    let changes = HistoryLog::for_instance(world.service.layout().root(), &record.id)
        .changes()
        .unwrap();
    assert_eq!(changes.len(), 1);

    let activity = world.service.activity(10);
    assert!(activity.active.is_empty());
    assert_eq!(activity.finished.len(), 1);
    assert_eq!(activity.finished[0].outcome, TaskOutcome::Succeeded);
}

#[tokio::test]
async fn content_for_another_game_version_is_refused_and_the_failure_is_logged() {
    let world = world();
    let record = world
        .service
        .store
        .lock()
        .await
        .create(
            NewInstance {
                name: "Old".to_owned(),
                game_version: "1.7".to_owned(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.0".to_owned()),
            },
            1,
        )
        .unwrap()
        .clone();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let result = world
        .service
        .install_content(
            &record.id,
            ProjectKind::Mod,
            "cool",
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(result, Err(ServiceError::NoCompatibleVersion)));
    assert!(
        !world
            .service
            .layout()
            .game(&record.id)
            .join("mods")
            .exists()
    );
    let activity = world.service.activity(10);
    assert!(matches!(
        activity.finished[0].outcome,
        TaskOutcome::Failed(_)
    ));
}

#[tokio::test]
async fn deleting_an_instance_removes_its_profile_and_keeps_shared_files() {
    let world = world();
    publish_release(&world);
    fake_java(&world, "echo \"Setting user: x\"");
    let a = world
        .service
        .create_instance("A", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let b = world
        .service
        .create_instance("B", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    world
        .service
        .launch(&a.id, tx, CancellationToken::new())
        .await
        .unwrap();
    assert!(world.service.layout().game(&a.id).is_dir());

    world.service.delete_instance(&a.id).await.unwrap();
    assert!(!world.service.layout().profile(&a.id).exists());
    assert!(
        world
            .service
            .layout()
            .versions()
            .join("1.0/1.0.jar")
            .is_file()
    );
    assert_eq!(world.service.library().await.instances.len(), 1);

    // B never installed anything itself, and launches from the shared files.
    world.net.forget_all();
    let (tx, mut rx) = mpsc::unbounded_channel();
    world
        .service
        .launch(&b.id, tx, CancellationToken::new())
        .await
        .unwrap();
    let mut downloaded = false;
    while let Ok(update) = rx.try_recv() {
        downloaded |= matches!(
            update,
            LaunchUpdate::Signal(LaunchSignal::Phase(
                crate::launch_session::LaunchPhase::Libraries
            ))
        );
    }
    assert!(!downloaded);
}

#[tokio::test]
async fn filters_are_fetched_once_and_failures_are_not_remembered() {
    let world = world();
    assert!(matches!(
        world.service.discover_filters().await,
        Err(ServiceError::Remote(_))
    ));
    world.net.answer(
        "/v2/tag/category",
        r#"[{"name":"adventure","project_type":"mod","header":"categories"}]"#,
    );
    world.net.answer(
        "/v2/tag/game_version",
        r#"[{"version":"1.21.1","version_type":"release","date":"2024-08-08T00:00:00Z"}]"#,
    );
    let filters = world.service.discover_filters().await.unwrap();
    assert_eq!(filters.categories.len(), 1);
    assert_eq!(filters.game_versions[0].version, "1.21.1");
    // Served from memory: the network can vanish.
    world.net.forget_all();
    assert_eq!(world.service.discover_filters().await.unwrap(), filters);
}

#[tokio::test]
async fn detail_bundles_project_versions_newest_first_and_survives_a_missing_owner() {
    let world = world();
    world.net.answer(
        "/v2/project/cool/version",
        r#"[{"id":"old","project_id":"P","name":"Old","version_number":"1","version_type":"release",
             "game_versions":["1.0"],"loaders":["fabric"],"date_published":"2023-01-01T00:00:00Z",
             "files":[{"url":"https://x/o.jar","filename":"o.jar","primary":true,"size":1,"hashes":{}}]},
            {"id":"new","project_id":"P","name":"New","version_number":"2","version_type":"release",
             "game_versions":["1.0"],"loaders":["fabric"],"date_published":"2024-01-01T00:00:00Z",
             "files":[{"url":"https://x/n.jar","filename":"n.jar","primary":true,"size":1,"hashes":{}}]}]"#,
    );
    world
        .net
        .answer("/v2/project/cool/members", "not json at all");
    world.net.answer(
        "/v2/project/cool",
        r##"{"id":"P","slug":"cool","title":"Cool","project_type":"mod","body":"# Hello"}"##,
    );
    let detail = world.service.project_detail("cool").await.unwrap();
    assert_eq!(detail.project.title, "Cool");
    assert_eq!(detail.project.body, "# Hello");
    let order: Vec<_> = detail.versions.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(order, ["new", "old"]);
    assert_eq!(detail.owner, None);
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

#[tokio::test]
async fn a_chosen_older_version_is_installed_instead_of_the_newest() {
    let world = world();
    let record = fabric_instance(&world, "1.0").await;
    world
        .net
        .answer("/v2/project/cool/version", two_versions(&world));
    let file = world
        .service
        .install_version(
            &record.id,
            ProjectKind::Mod,
            "cool",
            "v1",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(file, "v1.jar");
    let path = world.service.layout().game(&record.id).join("mods/v1.jar");
    assert_eq!(std::fs::read(path).unwrap(), b"one");
}

#[tokio::test]
async fn a_version_for_another_game_or_an_unknown_id_is_refused_before_any_download() {
    let world = world();
    let record = fabric_instance(&world, "1.0").await;
    world
        .net
        .answer("/v2/project/cool/version", two_versions(&world));
    for id in ["v2", "ghost"] {
        let result = world
            .service
            .install_version(
                &record.id,
                ProjectKind::Mod,
                "cool",
                id,
                CancellationToken::new(),
            )
            .await;
        assert!(
            matches!(result, Err(ServiceError::NoCompatibleVersion)),
            "{id}"
        );
    }
    assert!(
        !world
            .service
            .layout()
            .game(&record.id)
            .join("mods")
            .exists()
    );
}
