use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use super::*;
use crate::transfer::{TransportError, TransportFuture, TransportResponse};

/// Answers requests from a script, keyed by address, and remembers them.
#[derive(Default)]
pub(crate) struct Script {
    answers: Mutex<BTreeMap<String, VecDeque<(u16, String)>>>,
    pub(crate) seen: Mutex<Vec<HttpRequest>>,
}

impl Script {
    pub(crate) fn on(&self, url: &str, status: u16, body: &str) {
        self.answers
            .lock()
            .unwrap()
            .entry(url.to_owned())
            .or_default()
            .push_back((status, body.to_owned()));
    }

    pub(crate) fn requests_to(&self, url: &str) -> Vec<HttpRequest> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.url == url)
            .cloned()
            .collect()
    }
}

impl Transport for Script {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        Box::pin(async { Err(TransportError::permanent("GET is not scripted")) })
    }

    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        Box::pin(async move {
            self.seen.lock().unwrap().push(request.clone());
            let mut answers = self.answers.lock().unwrap();
            let queue = answers
                .get_mut(&request.url)
                .ok_or_else(|| TransportError::transient("no route"))?;
            // The last answer repeats, so a steady state needs one line.
            let (status, body) = if queue.len() > 1 {
                queue.pop_front().unwrap()
            } else {
                queue.front().cloned().unwrap()
            };
            Ok(TransportResponse::from_bytes(status, body.into_bytes()))
        })
    }
}

pub(crate) fn happy_chain(script: &Script) {
    script.on(
        XBL_URL,
        200,
        r#"{"Token":"xbl-token","DisplayClaims":{"xui":[{"uhs":"hash-1"}]}}"#,
    );
    script.on(
        XSTS_URL,
        200,
        r#"{"Token":"xsts-token","DisplayClaims":{"xui":[{"uhs":"hash-1"}]}}"#,
    );
    script.on(
        MINECRAFT_LOGIN_URL,
        200,
        r#"{"access_token":"mc-token","expires_in":86400,"token_type":"Bearer"}"#,
    );
    script.on(
        PROFILE_URL,
        200,
        r#"{"id":"123e4567e89b12d3a456426614174000","name":"Edwin_Zhan","skins":[]}"#,
    );
}

fn client(script: &Script) -> MicrosoftClient<'_, Script> {
    MicrosoftClient::new(script)
        .with_client_id("client-1")
        .with_slow_down_step(Duration::ZERO)
        .with_clock(|| 1_000)
}

fn oauth() -> OAuthTokens {
    OAuthTokens {
        access_token: Secret::new("ms-access"),
        refresh_token: Secret::new("ms-refresh"),
    }
}

#[tokio::test]
async fn the_device_code_is_requested_with_the_client_and_scope() {
    let script = Script::default();
    script.on(
        DEVICE_CODE_URL,
        200,
        r#"{"device_code":"dc","user_code":"AB12CD","verification_uri":"https://www.microsoft.com/link","expires_in":900,"interval":5}"#,
    );
    let code = client(&script).request_device_code().await.unwrap();
    assert_eq!(code.user_code, "AB12CD");
    assert_eq!(code.verification_uri, "https://www.microsoft.com/link");
    assert_eq!((code.expires_in, code.interval), (900, 5));
    let sent = &script.requests_to(DEVICE_CODE_URL)[0];
    let body = String::from_utf8(sent.body.clone().unwrap()).unwrap();
    assert!(body.contains("client_id=client-1"), "{body}");
    assert!(
        body.contains("scope=XboxLive.signin+offline_access"),
        "{body}"
    );
    // The secret device code never shows in debug output.
    assert!(!format!("{code:?}").contains("\"dc\""));
}

#[tokio::test]
async fn polling_waits_through_pending_and_slow_down_then_returns_the_tokens() {
    let script = Script::default();
    script.on(
        DEVICE_CODE_URL,
        200,
        r#"{"device_code":"dc","user_code":"U","verification_uri":"v","expires_in":900,"interval":0}"#,
    );
    for body in [
        r#"{"error":"authorization_pending"}"#,
        r#"{"error":"slow_down"}"#,
        r#"{"error":"authorization_pending"}"#,
    ] {
        script.on(TOKEN_URL, 400, body);
    }
    script.on(
        TOKEN_URL,
        200,
        r#"{"access_token":"a","refresh_token":"r"}"#,
    );
    let client = client(&script);
    let code = client.request_device_code().await.unwrap();
    let tokens = client
        .wait_for_sign_in(&code, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(tokens.access_token.expose(), "a");
    assert_eq!(tokens.refresh_token.expose(), "r");
    let polls = script.requests_to(TOKEN_URL);
    assert_eq!(polls.len(), 4);
    let body = String::from_utf8(polls[0].body.clone().unwrap()).unwrap();
    assert!(body.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
    assert!(body.contains("device_code=dc"), "{body}");
}

#[tokio::test]
async fn declined_expired_and_cancelled_end_the_wait_in_their_own_way() {
    let wait = |body: &'static str, cancel: bool| async move {
        let script = Script::default();
        script.on(TOKEN_URL, 400, body);
        let client = client(&script);
        let code = DeviceCode {
            user_code: "U".into(),
            verification_uri: "v".into(),
            expires_in: 900,
            interval: 0,
            device_code: Secret::new("dc"),
        };
        let token = CancellationToken::new();
        if cancel {
            token.cancel();
        }
        client.wait_for_sign_in(&code, &token).await.unwrap_err()
    };
    assert_eq!(
        wait(r#"{"error":"authorization_declined"}"#, false).await,
        AuthError::Declined
    );
    assert_eq!(
        wait(r#"{"error":"expired_token"}"#, false).await,
        AuthError::Expired
    );
    assert_eq!(
        wait(r#"{"error":"authorization_pending"}"#, true).await,
        AuthError::Cancelled
    );
    // An error the protocol does not define is told, not retried forever.
    assert!(matches!(
        wait(r#"{"error":"server_error"}"#, false).await,
        AuthError::Protocol(_)
    ));
}

#[tokio::test]
async fn a_code_that_runs_out_is_expired_even_while_pending() {
    let script = Script::default();
    script.on(TOKEN_URL, 400, r#"{"error":"authorization_pending"}"#);
    let code = DeviceCode {
        user_code: "U".into(),
        verification_uri: "v".into(),
        expires_in: 0,
        interval: 0,
        device_code: Secret::new("dc"),
    };
    let error = client(&script)
        .wait_for_sign_in(&code, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, AuthError::Expired);
}

#[tokio::test]
async fn the_token_chain_ends_in_a_session_with_the_real_profile() {
    let script = Script::default();
    happy_chain(&script);
    let login = client(&script).minecraft_login(&oauth()).await.unwrap();
    assert_eq!(login.access_token.expose(), "mc-token");
    assert_eq!(login.expires_at, 1_000 + 86_400);
    assert_eq!(login.profile_name, "Edwin_Zhan");
    assert_eq!(
        login.profile_id.compact(),
        "123e4567e89b12d3a456426614174000"
    );

    // Each step carried what the next one needs.
    let xbl = String::from_utf8(script.requests_to(XBL_URL)[0].body.clone().unwrap()).unwrap();
    assert!(xbl.contains("d=ms-access"), "{xbl}");
    let xsts = String::from_utf8(script.requests_to(XSTS_URL)[0].body.clone().unwrap()).unwrap();
    assert!(xsts.contains("xbl-token") && xsts.contains("rp://api.minecraftservices.com/"));
    let mc = String::from_utf8(
        script.requests_to(MINECRAFT_LOGIN_URL)[0]
            .body
            .clone()
            .unwrap(),
    )
    .unwrap();
    assert!(mc.contains("XBL3.0 x=hash-1;xsts-token"), "{mc}");
    let profile = &script.requests_to(PROFILE_URL)[0];
    assert!(
        profile
            .headers
            .iter()
            .any(|(name, value)| name == "authorization" && value == "Bearer mc-token")
    );
}

#[tokio::test]
async fn each_refusal_along_the_chain_has_its_own_error() {
    let fail = |step: &'static str, status: u16, body: &'static str| async move {
        let script = Script::default();
        happy_chain(&script);
        let url = match step {
            "xsts" => XSTS_URL,
            "login" => MINECRAFT_LOGIN_URL,
            _ => PROFILE_URL,
        };
        // Replace the happy answer with the failure.
        script.answers.lock().unwrap().get_mut(url).unwrap().clear();
        script.on(url, status, body);
        client(&script).minecraft_login(&oauth()).await.unwrap_err()
    };
    for (code, expected) in [
        (2_148_916_233_u64, AuthError::NoXboxAccount),
        (2_148_916_235, AuthError::XboxUnavailable),
        (2_148_916_236, AuthError::AdultVerificationRequired),
        (2_148_916_237, AuthError::AdultVerificationRequired),
        (2_148_916_238, AuthError::ChildAccount),
    ] {
        let body: &'static str = Box::leak(format!(r#"{{"XErr":{code}}}"#).into_boxed_str());
        assert_eq!(fail("xsts", 401, body).await, expected, "{code}");
    }
    assert!(matches!(
        fail("xsts", 401, r#"{"XErr":1}"#).await,
        AuthError::Protocol(_)
    ));
    assert!(matches!(
        fail("login", 403, r#"{"error":"Forbidden"}"#).await,
        AuthError::ServicesRefused(_)
    ));
    assert_eq!(
        fail("profile", 404, r#"{"error":"NOT_FOUND"}"#).await,
        AuthError::NoGameOwnership
    );
    assert!(matches!(
        fail("profile", 500, "oops").await,
        AuthError::ServicesRefused(_)
    ));
    assert!(matches!(
        fail("login", 200, "not json").await,
        AuthError::Protocol(_)
    ));
}

#[tokio::test]
async fn refreshing_rotates_the_token_and_a_rejected_one_needs_a_new_sign_in() {
    let script = Script::default();
    script.on(
        TOKEN_URL,
        200,
        r#"{"access_token":"a2","refresh_token":"r2"}"#,
    );
    let fresh = client(&script).refresh(&Secret::new("r1")).await.unwrap();
    assert_eq!(fresh.refresh_token.expose(), "r2");
    let body = String::from_utf8(script.requests_to(TOKEN_URL)[0].body.clone().unwrap()).unwrap();
    assert!(body.contains("grant_type=refresh_token") && body.contains("refresh_token=r1"));

    let rejected = Script::default();
    rejected.on(TOKEN_URL, 400, r#"{"error":"invalid_grant"}"#);
    assert_eq!(
        client(&rejected)
            .refresh(&Secret::new("old"))
            .await
            .unwrap_err(),
        AuthError::SignInRequired
    );

    // No route at all is the network failing, not a rejected sign-in.
    let offline = Script::default();
    assert!(matches!(
        client(&offline)
            .refresh(&Secret::new("r"))
            .await
            .unwrap_err(),
        AuthError::Network(_)
    ));
}

#[test]
fn the_client_id_can_be_overridden_for_development() {
    assert_eq!(MICROSOFT_CLIENT_ID, "966805e9-75e4-4193-b75e-0e247a312cfd");
    assert_eq!(client_id(), MICROSOFT_CLIENT_ID);
}

/// Asks Microsoft for a code with the registered client id; nothing is
/// signed in. Run on purpose with
/// `cargo nextest run -p lumilio-core real_device_code --ignored --no-capture`.
#[tokio::test]
#[ignore = "talks to the real Microsoft sign-in service"]
async fn real_device_code_is_offered_for_the_registered_client() {
    let transport = crate::transfer::DefaultTransport::new().unwrap();
    let code = MicrosoftClient::new(&transport)
        .request_device_code()
        .await
        .unwrap();
    println!("{} at {}", code.user_code, code.verification_uri);
    assert!(!code.user_code.is_empty());
}
