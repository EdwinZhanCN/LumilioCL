use tokio::sync::mpsc;

use crate::activity::{CancellationToken, RetryAction, TaskCategory, TaskOutcome};
use crate::discover::ProjectKind;
use crate::error::ServiceError;
use crate::instances::Loader;
use crate::testing::{change_subjects, mod_version, world};

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
        crate::discover::DEFAULT_SOURCE,
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
    let mut pack = Box::pin(world.service.install_modpack(
        crate::discover::DEFAULT_SOURCE,
        "pack",
        None,
        cancel.clone(),
    ));
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
    let game = world.service.layout.game(&record.id);
    assert!(std::fs::read_dir(game).unwrap().next().is_none());
    // Deleting the current game saves its replacement, so settings are free again.
    drop(defaults);
    world.service.delete_instance(&record.id).await.unwrap();
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
                crate::discover::DEFAULT_SOURCE,
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
            source: crate::discover::DEFAULT_SOURCE.into(),
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

/// Drives `work` until `done` holds, for at most two seconds.
async fn drive_until<F: std::future::Future + Unpin>(work: &mut F, done: impl Fn() -> bool) {
    let wait = async {
        for _ in 0..200 {
            if done() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("the condition never held");
    };
    tokio::select! {
        _ = work => panic!("the work ended first"),
        () = wait => {}
    }
}

#[tokio::test]
async fn a_paused_task_lets_its_connections_go_and_finishes_once_resumed() {
    let world = world();
    let record = world
        .service
        .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    *world.net.stall_next_file.lock().unwrap() = true;
    let mut install = Box::pin(world.service.install_content(
        &record.id,
        ProjectKind::Mod,
        crate::discover::DEFAULT_SOURCE,
        "cool",
        CancellationToken::new(),
    ));
    let downloading = || {
        let view = world.service.activity(10);
        view.traffic.open == 1
            && view.active.iter().any(|task| {
                task.downloads.as_ref().is_some_and(|downloads| {
                    downloads.files.iter().any(|file| file.source.is_some())
                })
            })
    };
    drive_until(&mut install, downloading).await;
    let id = world.service.activity(10).active[0].id;

    assert!(!world.service.pause_task(id + 100));
    assert!(world.service.pause_task(id));
    drive_until(&mut install, || {
        let view = world.service.activity(10);
        view.traffic.open == 0 && view.paused.contains(&id)
    })
    .await;

    assert!(world.service.resume_task(id));
    install.await.unwrap();
    let target = world.service.layout.game(&record.id).join("mods/cool.jar");
    assert_eq!(std::fs::read(target).unwrap(), b"a mod");
    let view = world.service.activity(10);
    assert!(view.active.is_empty() && view.paused.is_empty());
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
        crate::discover::DEFAULT_SOURCE,
        "one",
        CancellationToken::new(),
    ));
    assert!(futures_util::poll!(&mut first).is_pending());
    let mut second = Box::pin(world.service.install_modpack(
        crate::discover::DEFAULT_SOURCE,
        "pack",
        None,
        CancellationToken::new(),
    ));
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
            .install_content(
                &record.id,
                ProjectKind::Mod,
                crate::discover::DEFAULT_SOURCE,
                "cool",
                cancel
            )
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
            crate::discover::DEFAULT_SOURCE,
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
        crate::discover::DEFAULT_SOURCE,
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
        crate::discover::DEFAULT_SOURCE,
        "second",
        CancellationToken::new(),
    ));
    assert!(futures_util::poll!(&mut retry).is_pending());
    drop(retry);
    drop(defaults);
    assert_eq!(world.service.activity(10).finished.len(), 2);
    assert!(world.service.activity(10).active.is_empty());
}
