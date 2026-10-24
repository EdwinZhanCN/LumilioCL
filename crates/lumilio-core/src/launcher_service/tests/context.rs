//! Plan global-context T2: every save publishes the kind of state it changed,
//! and the current game is always one that exists.

use crate::context::Revisions;
use crate::instances::Loader;
use crate::testing::{reopen, world};

fn revisions(world: &crate::testing::World) -> Revisions {
    world.service.context().revisions
}

/// Each write moves only its own revision; a change nothing shows moves none.
#[tokio::test]
async fn each_kind_of_write_publishes_only_its_own_change() {
    let world = world();
    let service = &world.service;
    let step = |before: Revisions, after: Revisions| {
        (
            after.library - before.library,
            after.accounts - before.accounts,
            after.java - before.java,
            after.plugins - before.plugins,
        )
    };

    let before = revisions(&world);
    let game = service
        .create_instance("A", Some("1.20.1"), Loader::Vanilla, None)
        .await
        .unwrap();
    assert_eq!(step(before, revisions(&world)), (1, 0, 0, 0), "a new game");

    let before = revisions(&world);
    service.set_favorite(&game.id, true).await.unwrap();
    assert_eq!(step(before, revisions(&world)), (1, 0, 0, 0), "a favorite");

    let before = revisions(&world);
    service.add_account("Alex", None).await.unwrap();
    service.select_account("Alex").await.unwrap();
    service.remove_account("Steve").await.unwrap();
    assert_eq!(step(before, revisions(&world)), (0, 3, 0, 0), "accounts");
    assert_eq!(service.context().current_account.as_deref(), Some("Alex"));

    let before = revisions(&world);
    service.set_auto_install_java(false).await.unwrap();
    service
        .set_java_roots(vec!["/nowhere".into()])
        .await
        .unwrap();
    assert_eq!(step(before, revisions(&world)), (0, 0, 2, 0), "Java");

    let before = revisions(&world);
    service
        .set_plugin_enabled(lumilio_plugin_modrinth::ID, false)
        .await
        .unwrap();
    assert_eq!(step(before, revisions(&world)), (0, 0, 0, 1), "a plugin");

    let before = revisions(&world);
    let mut preferences = service.settings().await.preferences;
    preferences.library_sort = preferences.library_sort.wrapping_add(1);
    service.set_preferences(preferences).await.unwrap();
    assert_eq!(
        step(before, revisions(&world)),
        (0, 0, 0, 0),
        "a preference"
    );
}

/// The first game becomes current; choosing another is published; deleting
/// the current one saves its replacement, and a restart reads the same.
#[tokio::test]
async fn the_current_game_always_exists_and_is_saved() {
    let world = world();
    let root = world._dir.path().join("launcher");
    let service = &world.service;
    let a = service
        .create_instance("A", Some("1.20.1"), Loader::Vanilla, None)
        .await
        .unwrap()
        .id;
    assert_eq!(
        service.context().current_instance.as_deref(),
        Some(a.as_str())
    );
    let b = service
        .create_instance("B", Some("1.20.1"), Loader::Vanilla, None)
        .await
        .unwrap()
        .id;
    assert_eq!(
        service.context().current_instance.as_deref(),
        Some(a.as_str()),
        "a second game does not take over"
    );

    let mut changes = service.subscribe_context();
    service.set_current_instance(&b).await.unwrap();
    assert!(changes.has_changed().unwrap());
    assert_eq!(
        changes.borrow_and_update().current_instance.as_deref(),
        Some(b.as_str())
    );

    service.delete_instance(&b).await.unwrap();
    assert_eq!(
        service.context().current_instance.as_deref(),
        Some(a.as_str())
    );
    assert_eq!(
        service.settings().await.current_instance.as_deref(),
        Some(a.as_str()),
        "the replacement is saved"
    );

    let (service, _dir) = reopen(world, &root);
    assert_eq!(
        service.context().current_instance.as_deref(),
        Some(a.as_str())
    );
    assert_eq!(
        service.current_instance().await.as_deref(),
        Some(a.as_str())
    );
}

/// A subscriber that falls behind sees the newest state, never an older one.
#[tokio::test]
async fn a_late_subscriber_reads_only_the_newest_state() {
    let world = world();
    let service = &world.service;
    let mut changes = service.subscribe_context();
    let mut last = changes.borrow_and_update().revisions.library;
    for name in ["A", "B", "C"] {
        service
            .create_instance(name, Some("1.20.1"), Loader::Vanilla, None)
            .await
            .unwrap();
    }
    changes.changed().await.unwrap();
    let seen = changes.borrow_and_update().revisions.library;
    assert!(seen >= last + 3, "{seen} after {last}");
    last = seen;
    assert!(
        !changes.has_changed().unwrap(),
        "nothing older is still queued"
    );
    assert_eq!(service.context().revisions.library, last);
}

/// D7: Home's checks hold no account problem; the account is applied when
/// Home is summarized, so a change of account changes Home from the checks
/// already made.
#[tokio::test]
async fn home_applies_the_account_to_checks_already_made() {
    use crate::diagnostics::ProblemKind;
    let world = world();
    let service = &world.service;
    let id = service
        .create_instance("A", Some("1.20.1"), Loader::Vanilla, None)
        .await
        .unwrap()
        .id;
    let (records, checks) = service.home_checks().await;
    assert!(
        checks[&id]
            .iter()
            .all(|problem| problem.kind != ProblemKind::NoAccount),
        "{:?}",
        checks[&id]
    );
    let needs_account = |summary: &crate::attention::HomeSummary| {
        summary
            .needs_attention
            .iter()
            .any(|item| item.instance_id == id && item.headline.kind == ProblemKind::NoAccount)
    };
    assert!(!needs_account(&service.home_from(&records, &checks).await));

    service.remove_account("Steve").await.unwrap();
    let summary = service.home_from(&records, &checks).await;
    assert!(needs_account(&summary), "{summary:?}");
    assert_eq!(
        service.home().await.1,
        summary,
        "the same as checking again"
    );
}

/// Plan global-context T6: a launch publishes its session while it is on its
/// way; a launch that fails stays published as the one to recover, and one
/// that is abandoned, or whose game is deleted, leaves none.
#[tokio::test]
async fn a_launch_publishes_its_session_until_it_is_over() {
    use crate::context::{Session, SessionPhase};
    let world = world();
    let service = &world.service;
    let game = service
        .create_instance("A", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let session = |phase| {
        Some(Session {
            instance: game.id.clone(),
            phase,
        })
    };

    // Paused where it reads the settings: on its way.
    let defaults = service.settings.lock().await;
    let (updates, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut launch = Box::pin(service.launch(&game.id, updates, crate::CancellationToken::new()));
    assert!(futures_util::poll!(&mut launch).is_pending());
    assert_eq!(service.context().session, session(SessionPhase::Preparing));
    drop(launch);
    drop(defaults);
    assert_eq!(service.context().session, None, "an abandoned launch");

    // There is no catalog to install from: the launch fails.
    let (updates, _receiver) = tokio::sync::mpsc::unbounded_channel();
    assert!(
        service
            .launch(&game.id, updates, crate::CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(service.context().session, session(SessionPhase::Failed));
    assert!(!service.context().session.unwrap().is_active());

    service.delete_instance(&game.id).await.unwrap();
    assert_eq!(service.context().session, None, "nothing left to recover");
}
