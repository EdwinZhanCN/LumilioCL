use super::super::error::ServiceError;
use super::super::types::now;
use super::{
    ECHO_ARGS, MS_ID, fake_java, launch_to_end, microsoft_chain, publish_identity_release,
    publish_release, sign_in, world,
};
use crate::activity::CancellationToken;
use crate::credentials::StoredLogin;
use crate::instance::Loader;
use crate::microsoft::{AuthError, DeviceCode};
use crate::settings::AccountKind;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc;

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
