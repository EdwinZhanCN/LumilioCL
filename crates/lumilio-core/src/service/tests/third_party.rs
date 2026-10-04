use std::io::Write as _;

use super::super::error::ServiceError;
use super::super::third_party::ThirdPartySignIn;
use super::{ECHO_ARGS, World, fake_java, launch_to_end, publish_identity_release, world};
use crate::account::ProfileId;
use crate::credentials::StoredYggdrasil;
use crate::instance::Loader;
use crate::settings::AccountKind;
use crate::yggdrasil::{LITTLE_SKIN_URL, YggdrasilError};
use sha2::{Digest, Sha256};

const ROOT: &str = "https://skins.example/api/yggdrasil/";
const ID: &str = "123e4567e89b12d3a456426614174000";
const OTHER: &str = "223e4567e89b12d3a456426614174000";
const METADATA: &str = r#"{"meta":{"serverName":"Example Skins","feature.non_email_login":true}}"#;

fn url(path: &str) -> String {
    format!("{ROOT}{path}")
}

fn session(
    client: &str,
    token: &str,
    selected: Option<(&str, &str)>,
    available: &[(&str, &str)],
) -> String {
    serde_json::json!({
        "accessToken": token,
        "clientToken": client,
        "selectedProfile": selected.map(|(id, name)| serde_json::json!({"id": id, "name": name})),
        "availableProfiles": available.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "user": {"id": "u", "properties": [{"name": "lang", "value": "zh"}]},
    })
    .to_string()
}

const CLIENT: &str = "client-1";

/// A world whose sign-ins use a known client token, so the scripted server
/// can answer with it.
fn third_party_world() -> World {
    let mut world = world();
    world.service.client_token_override = Some(CLIENT.to_owned());
    world
}

async fn sign_in_as(
    world: &World,
    selected: Option<(&str, &str)>,
    available: &[(&str, &str)],
) -> Result<ThirdPartySignIn, ServiceError> {
    world
        .service
        .add_auth_server(&crate::yggdrasil::AuthServer {
            url: ROOT.to_owned(),
            name: Some("Example Skins".into()),
            non_email_login: true,
            links: Default::default(),
        })
        .await
        .unwrap();
    world.net.clear_replies(&url("authserver/authenticate"));
    world.net.reply(
        &url("authserver/authenticate"),
        200,
        &session(CLIENT, "access-1", selected, available),
    );
    world
        .service
        .third_party_sign_in(ROOT, "me@example.com", "pw")
        .await
}

fn jar_with_manifest(build: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut out);
        writer
            .start_file::<_, ()>(
                "META-INF/MANIFEST.MF",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        write!(
            writer,
            "Manifest-Version: 1.0\r\nImplementation-Title: authlib-injector\r\nImplementation-Version: 1.2.{build}\r\nBuild-Number: {build}\r\n"
        )
        .unwrap();
        writer.finish().unwrap();
    }
    out.into_inner()
}

/// The authlib-injector index and jar, and the server's metadata.
fn publish_injector(world: &World) {
    let jar = jar_with_manifest(54);
    let sha: String = Sha256::digest(&jar)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    world.net.answer(
        "authlib-injector.yushi.moe/artifact/latest.json",
        serde_json::json!({
            "build_number": 54, "version": "1.2.54",
            "download_url": "https://download.example/authlib-injector-1.2.54.jar",
            "checksums": {"sha256": sha},
        })
        .to_string(),
    );
    world
        .net
        .answer("download.example/authlib-injector-1.2.54.jar", jar);
    world.net.answer("skins.example/api/yggdrasil", METADATA);
}

#[tokio::test]
async fn littleskin_is_built_in_and_a_found_server_is_remembered_and_forgotten_with_its_accounts() {
    let world = third_party_world();
    let listed = world.service.auth_servers().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].url, LITTLE_SKIN_URL);

    world.net.answer("skins.example/api/yggdrasil", METADATA);
    let found = world
        .service
        .locate_auth_server("skins.example/api/yggdrasil")
        .await
        .unwrap();
    assert_eq!(found.display_name(), "Example Skins");
    assert_eq!(
        world.service.auth_servers().await.len(),
        1,
        "finding is not adding"
    );
    world.service.add_auth_server(&found).await.unwrap();
    world.service.add_auth_server(&found).await.unwrap();
    // LittleSkin needs no entry of its own.
    world
        .service
        .add_auth_server(&crate::yggdrasil::little_skin())
        .await
        .unwrap();
    assert_eq!(world.service.auth_servers().await.len(), 2);

    let signed = sign_in_as(&world, Some((ID, "Edwin")), &[]).await.unwrap();
    let ThirdPartySignIn::Done { key, .. } = signed else {
        panic!("expected a sign-in")
    };
    assert!(
        world
            .secrets
            .values()
            .iter()
            .any(|v| v.contains("access-1"))
    );
    world.service.select_account(&key).await.unwrap();

    // Forgetting the server takes its accounts and their secrets with it.
    world.net.reply(&url("authserver/invalidate"), 204, "");
    world.service.remove_auth_server(ROOT).await.unwrap();
    let settings = world.service.settings().await;
    assert!(
        settings
            .accounts
            .iter()
            .all(|entry| entry.kind != AccountKind::ThirdParty)
    );
    assert_eq!(settings.selected_account.as_deref(), Some("Steve"));
    assert!(world.secrets.values().is_empty());
    assert!(
        world
            .service
            .remove_auth_server(LITTLE_SKIN_URL)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn signing_in_adds_the_account_and_keeps_every_secret_in_the_store() {
    let world = third_party_world();
    let signed = sign_in_as(&world, Some((ID, "Edwin")), &[]).await.unwrap();
    let ThirdPartySignIn::Done { key, name } = signed else {
        panic!("expected a sign-in")
    };
    assert_eq!(
        (key.as_str(), name.as_str()),
        (format!("ali:{ID}@{ROOT}").as_str(), "Edwin")
    );

    let settings = world.service.settings().await;
    let entry = settings
        .accounts
        .iter()
        .find(|e| e.kind == AccountKind::ThirdParty)
        .unwrap();
    assert_eq!(entry.server.as_deref(), Some(ROOT));
    assert_eq!(entry.login.as_deref(), Some("me@example.com"));
    assert_eq!(
        settings.selected_account.as_deref(),
        Some("Steve"),
        "the selection stays"
    );
    let file =
        std::fs::read_to_string(world.service.layout().root().join("settings.json")).unwrap();
    for secret in ["access-1", "pw"] {
        assert!(!file.contains(secret), "{secret} leaked into settings.json");
    }
    let stored = StoredYggdrasil::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap();
    assert_eq!(stored.access_token, "access-1");
    assert_eq!(stored.user_properties["lang"], "zh");

    // Signing in again updates the one account.
    sign_in_as(&world, Some((ID, "Renamed")), &[])
        .await
        .unwrap();
    let accounts = world.service.settings().await.accounts;
    assert_eq!(
        accounts
            .iter()
            .filter(|e| e.kind == AccountKind::ThirdParty)
            .count(),
        1
    );
    assert!(accounts.iter().any(|entry| entry.name == "Renamed"));
}

#[tokio::test]
async fn several_characters_ask_for_a_choice_that_is_then_made_once() {
    let world = third_party_world();
    let asked = sign_in_as(&world, None, &[(ID, "A"), (OTHER, "B")])
        .await
        .unwrap();
    let ThirdPartySignIn::Choose {
        pending,
        characters,
    } = asked
    else {
        panic!("expected a choice")
    };
    assert_eq!(characters.len(), 2);
    assert_eq!(
        world.service.settings().await.accounts.len(),
        1,
        "nothing is added yet"
    );
    assert!(world.secrets.values().is_empty(), "nothing is kept yet");

    world.net.reply(
        &url("authserver/refresh"),
        200,
        &session(CLIENT, "access-2", Some((OTHER, "B")), &[]),
    );
    let (key, name) = world
        .service
        .third_party_choose(pending, ProfileId::parse(OTHER).unwrap())
        .await
        .unwrap();
    assert_eq!(name, "B");
    assert!(key.starts_with(&format!("ali:{OTHER}@")));
    let refresh: serde_json::Value = {
        let sent = world.net.sent.lock().unwrap();
        let request = sent
            .iter()
            .find(|r| r.url == url("authserver/refresh"))
            .unwrap();
        serde_json::from_slice(request.body.as_ref().unwrap()).unwrap()
    };
    assert_eq!(refresh["selectedProfile"]["id"], OTHER);
    assert!(matches!(
        world
            .service
            .third_party_choose(pending, ProfileId::parse(OTHER).unwrap())
            .await,
        Err(ServiceError::NoPendingSignIn)
    ));
}

#[tokio::test]
async fn one_character_is_chosen_for_you_and_none_is_an_error() {
    let world = third_party_world();
    let none = sign_in_as(&world, None, &[]).await;
    assert!(matches!(
        none,
        Err(ServiceError::Yggdrasil(YggdrasilError::NoCharacter))
    ));
    assert!(world.secrets.values().is_empty());

    world.net.clear_replies(&url("authserver/authenticate"));
    world.net.reply(
        &url("authserver/authenticate"),
        200,
        &session(CLIENT, "access-1", None, &[(ID, "Only")]),
    );
    world.net.reply(
        &url("authserver/refresh"),
        200,
        &session(CLIENT, "access-2", Some((ID, "Only")), &[]),
    );
    let ThirdPartySignIn::Done { name, .. } = world
        .service
        .third_party_sign_in(ROOT, "me", "pw")
        .await
        .unwrap()
    else {
        panic!("a single character needs no choice")
    };
    assert_eq!(name, "Only");
    let refresh: serde_json::Value = {
        let sent = world.net.sent.lock().unwrap();
        let request = sent
            .iter()
            .find(|r| r.url == url("authserver/refresh"))
            .unwrap();
        serde_json::from_slice(request.body.as_ref().unwrap()).unwrap()
    };
    assert_eq!(refresh["selectedProfile"]["id"], ID);
    assert!(
        world
            .secrets
            .values()
            .iter()
            .any(|v| v.contains("access-2"))
    );
}

#[tokio::test]
async fn a_wrong_password_adds_nothing() {
    let world = third_party_world();
    world
        .service
        .add_auth_server(&crate::yggdrasil::AuthServer {
            url: ROOT.to_owned(),
            name: None,
            non_email_login: false,
            links: Default::default(),
        })
        .await
        .unwrap();
    world.net.reply(
        &url("authserver/authenticate"),
        403,
        r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid credentials. Invalid username or password."}"#,
    );
    let error = world
        .service
        .third_party_sign_in(ROOT, "me", "bad")
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            ServiceError::Yggdrasil(YggdrasilError::InvalidCredentials)
        ),
        "{error}"
    );
    assert_eq!(world.service.settings().await.accounts.len(), 1);
    assert!(world.secrets.values().is_empty());
    // An unknown server is refused before anything is sent.
    assert!(
        world
            .service
            .third_party_sign_in("https://nobody.example/", "me", "pw")
            .await
            .is_err()
    );
}

/// A launch-ready world with a signed-in third-party account selected.
async fn signed_in_world() -> (World, String, String) {
    signed_in_world_with(true).await
}

async fn signed_in_world_with(injector: bool) -> (World, String, String) {
    let world = third_party_world();
    publish_identity_release(&world);
    fake_java(&world, ECHO_ARGS);
    if injector {
        publish_injector(&world);
    } else {
        world.net.answer("skins.example/api/yggdrasil", METADATA);
    }
    let ThirdPartySignIn::Done { key, .. } =
        sign_in_as(&world, Some((ID, "Edwin")), &[]).await.unwrap()
    else {
        panic!("expected a sign-in")
    };
    world.service.select_account(&key).await.unwrap();
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    (world, key, record.id)
}

async fn flagged(world: &World) -> bool {
    world
        .service
        .settings()
        .await
        .accounts
        .iter()
        .any(|e| e.kind == AccountKind::ThirdParty && e.needs_sign_in)
}

async fn args_of(world: &World, id: &str) -> String {
    std::fs::read_to_string(world.service.layout().game(id).join("args.txt")).unwrap()
}

#[tokio::test]
async fn a_launch_presents_the_character_with_the_agent_and_needs_no_refresh_while_the_token_holds()
{
    let (world, _, id) = signed_in_world().await;
    world.net.reply(&url("authserver/validate"), 204, "");
    launch_to_end(&world, &id).await.unwrap();
    let args = args_of(&world, &id).await;
    assert!(args.contains("--username Edwin"), "{args}");
    assert!(args.contains(&format!("--uuid {ID}")), "{args}");
    assert!(args.contains("--accessToken access-1"), "{args}");
    let agent = args
        .split_whitespace()
        .find(|a| a.starts_with("-javaagent:"))
        .expect("the agent is loaded");
    assert!(
        agent.ends_with(&format!("authlib-injector.jar={ROOT}")),
        "{agent}"
    );
    assert!(args.contains("-Dauthlibinjector.side=client"));
    assert!(args.contains("-Dauthlibinjector.yggdrasil.prefetched="));
    assert_eq!(world.net.sent_to(&url("authserver/refresh")), 0);
    assert!(
        world
            .service
            .layout()
            .injector()
            .join("authlib-injector.jar")
            .is_file()
    );
}

#[tokio::test]
async fn a_token_the_server_no_longer_accepts_is_renewed_and_the_new_one_kept() {
    let (world, key, id) = signed_in_world().await;
    let client = StoredYggdrasil::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap()
        .client_token;
    world.net.reply(
        &url("authserver/validate"),
        403,
        r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid token."}"#,
    );
    world.net.reply(
        &url("authserver/refresh"),
        200,
        &session(&client, "access-9", Some((ID, "Edwin2")), &[]),
    );
    launch_to_end(&world, &id).await.unwrap();
    let args = args_of(&world, &id).await;
    assert!(args.contains("--accessToken access-9"), "{args}");
    assert!(
        args.contains("--username Edwin2"),
        "a renamed character shows its new name: {args}"
    );
    assert_eq!(
        StoredYggdrasil::load(world.secrets.as_ref(), &key)
            .unwrap()
            .unwrap()
            .access_token,
        "access-9"
    );
    assert!(
        world
            .service
            .settings()
            .await
            .accounts
            .iter()
            .any(|e| e.name == "Edwin2")
    );
}

#[tokio::test]
async fn an_expired_sign_in_or_another_character_stops_the_launch_and_marks_the_account() {
    let (world, key, id) = signed_in_world().await;
    let client = StoredYggdrasil::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap()
        .client_token;
    let forbidden = r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid token."}"#;
    world.net.reply(&url("authserver/validate"), 403, forbidden);
    world.net.reply(&url("authserver/refresh"), 403, forbidden);
    let error = launch_to_end(&world, &id).await.unwrap_err();
    assert!(matches!(error, ServiceError::SignInRequired(_)), "{error}");
    assert!(flagged(&world).await);

    // A refresh that answers with another character is not played as.
    world
        .service
        .settings
        .lock()
        .await
        .set_needs_sign_in(&key, false)
        .unwrap();
    world.net.clear_replies(&url("authserver/refresh"));
    world.net.reply(
        &url("authserver/refresh"),
        200,
        &session(&client, "x", Some((OTHER, "Else")), &[]),
    );
    assert!(matches!(
        launch_to_end(&world, &id).await.unwrap_err(),
        ServiceError::SignInRequired(_)
    ));
}

#[tokio::test]
async fn an_unreachable_server_stops_the_launch_without_blaming_the_account() {
    let (world, _, id) = signed_in_world().await;
    // No route for validate at all: the network is down.
    let error = launch_to_end(&world, &id).await.unwrap_err();
    assert!(
        matches!(error, ServiceError::Yggdrasil(YggdrasilError::Network(_))),
        "{error}"
    );
    assert!(
        world
            .service
            .settings()
            .await
            .accounts
            .iter()
            .all(|e| !e.needs_sign_in),
        "a network failure is not a reason to sign in again"
    );
}

#[tokio::test]
async fn an_injector_that_cannot_be_fetched_stops_the_launch_with_its_own_error() {
    let (world, _, id) = signed_in_world_with(false).await;
    world.net.reply(&url("authserver/validate"), 204, "");
    let error = launch_to_end(&world, &id).await.unwrap_err();
    assert!(matches!(error, ServiceError::Injector(_)), "{error}");
}

#[tokio::test]
async fn removing_an_account_tells_the_server_to_forget_its_token_and_survives_a_dead_server() {
    let world = third_party_world();
    let ThirdPartySignIn::Done { key, .. } =
        sign_in_as(&world, Some((ID, "Edwin")), &[]).await.unwrap()
    else {
        panic!("expected a sign-in")
    };
    world.net.reply(&url("authserver/invalidate"), 204, "");
    world.service.remove_account(&key).await.unwrap();
    assert_eq!(world.net.sent_to(&url("authserver/invalidate")), 1);
    assert!(world.secrets.values().is_empty());

    // With no route for the server the account is still removed.
    let ThirdPartySignIn::Done { key, .. } =
        sign_in_as(&world, Some((ID, "Edwin")), &[]).await.unwrap()
    else {
        panic!("expected a sign-in")
    };
    world.net.clear_replies(&url("authserver/invalidate"));
    world.service.remove_account(&key).await.unwrap();
    assert!(world.secrets.values().is_empty());
    assert_eq!(world.service.settings().await.accounts.len(), 1);
}
