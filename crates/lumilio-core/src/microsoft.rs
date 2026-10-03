//! Signing in with a Microsoft account: the device code grant and the chain
//! of tokens from it to a Minecraft session. Decisions: ADR 0013; behavior
//! notes: `docs/behavior/accounts.md`.
//!
//! This module only speaks the protocol. Where tokens are kept is
//! [`crate::credentials`], and what an account is lives in `settings`.

use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::time::Duration;

use futures_util::StreamExt as _;
use serde::Deserialize;
use serde_json::json;

use crate::account::ProfileId;
use crate::activity::CancellationToken;
use crate::transfer::{HttpMethod, HttpRequest, Transport};

/// The application registered for this launcher (a public client: no secret).
pub const MICROSOFT_CLIENT_ID: &str = "966805e9-75e4-4193-b75e-0e247a312cfd";

/// The client id in use: the registered one, or the development override.
#[must_use]
pub fn client_id() -> String {
    std::env::var("LUMILIO_MS_CLIENT_ID")
        .ok()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| MICROSOFT_CLIENT_ID.to_owned())
}

pub(crate) const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
pub(crate) const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
pub(crate) const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
pub(crate) const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
pub(crate) const MINECRAFT_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
pub(crate) const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";
/// Answers are small JSON documents; anything bigger is not one.
const BODY_LIMIT: usize = 1024 * 1024;
/// How much of an unexpected answer is kept for the technical details.
const SNIPPET: usize = 300;

/// Why a sign-in step did not work, in terms a person can act on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthError {
    /// The person refused the request in the browser.
    Declined,
    /// The code was not used in time.
    Expired,
    Cancelled,
    /// The stored sign-in can no longer be refreshed: sign in again.
    SignInRequired,
    /// The Microsoft account has no Xbox profile yet.
    NoXboxAccount,
    /// A child account that an adult has not allowed to play online.
    ChildAccount,
    /// Xbox Live is not offered where the account is.
    XboxUnavailable,
    /// Korea requires an adult verification first.
    AdultVerificationRequired,
    /// The account does not own Minecraft: Java Edition.
    NoGameOwnership,
    /// Minecraft's services refused this application's tokens.
    ServicesRefused(String),
    /// The system credential store is not usable.
    CredentialStore(String),
    /// The network failed or answered nonsense; try again.
    Network(String),
    /// An answer did not have the expected shape.
    Protocol(String),
}

impl Display for AuthError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declined => f.write_str("sign-in was declined in the browser"),
            Self::Expired => f.write_str("the sign-in code expired before it was used"),
            Self::Cancelled => f.write_str("sign-in was cancelled"),
            Self::SignInRequired => f.write_str("the account must sign in again"),
            Self::NoXboxAccount => f.write_str("this Microsoft account has no Xbox profile"),
            Self::ChildAccount => {
                f.write_str("this is a child account; an adult must allow it to play online")
            }
            Self::XboxUnavailable => f.write_str("Xbox Live is not available in this region"),
            Self::AdultVerificationRequired => {
                f.write_str("this account needs adult verification on the Xbox site first")
            }
            Self::NoGameOwnership => {
                f.write_str("this account does not own Minecraft: Java Edition")
            }
            Self::ServicesRefused(detail) => {
                write!(f, "Minecraft services refused this launcher: {detail}")
            }
            Self::CredentialStore(detail) => {
                write!(f, "the system credential store is not usable: {detail}")
            }
            Self::Network(detail) => write!(f, "could not reach the sign-in service: {detail}"),
            Self::Protocol(detail) => write!(f, "unexpected sign-in answer: {detail}"),
        }
    }
}

impl Error for AuthError {}

/// A secret kept out of debug output.
#[derive(Clone, Eq, PartialEq)]
pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The secret itself. Call only where it is sent or stored.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Debug for Secret {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

/// What the person does in their browser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCode {
    /// The short code to type.
    pub user_code: String,
    /// Where to type it.
    pub verification_uri: String,
    /// Seconds the code stays valid.
    pub expires_in: u64,
    /// Seconds between polls.
    pub interval: u64,
    device_code: Secret,
}

/// The Microsoft tokens that start the chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthTokens {
    pub access_token: Secret,
    pub refresh_token: Secret,
}

/// A Minecraft session: what the game needs to start online.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinecraftLogin {
    pub access_token: Secret,
    /// When the access token stops working, in Unix seconds.
    pub expires_at: u64,
    pub profile_id: ProfileId,
    pub profile_name: String,
}

pub struct MicrosoftClient<'a, T: Transport + ?Sized> {
    transport: &'a T,
    client_id: String,
    slow_down: Duration,
    clock: fn() -> u64,
}

type Answer = (u16, Vec<u8>);

impl<'a, T: Transport + ?Sized> MicrosoftClient<'a, T> {
    #[must_use]
    pub fn new(transport: &'a T) -> Self {
        Self {
            transport,
            client_id: client_id(),
            slow_down: Duration::from_secs(5),
            clock: crate::service::now,
        }
    }

    #[must_use]
    pub fn with_client_id(mut self, id: impl Into<String>) -> Self {
        self.client_id = id.into();
        self
    }

    /// How much longer to wait after the service says "slow down".
    #[must_use]
    pub const fn with_slow_down_step(mut self, step: Duration) -> Self {
        self.slow_down = step;
        self
    }

    #[must_use]
    pub const fn with_clock(mut self, clock: fn() -> u64) -> Self {
        self.clock = clock;
        self
    }

    async fn send(&self, request: HttpRequest) -> Result<Answer, AuthError> {
        let response = self
            .transport
            .send(request)
            .await
            .map_err(|error| AuthError::Network(error.to_string()))?;
        let status = response.status();
        let mut body = response.into_body();
        let mut bytes = Vec::new();
        while let Some(chunk) = body.next().await {
            bytes.extend(chunk.map_err(|error| AuthError::Network(error.to_string()))?);
            if bytes.len() > BODY_LIMIT {
                return Err(AuthError::Protocol("the answer is too large".to_owned()));
            }
        }
        Ok((status, bytes))
    }

    async fn form(&self, url: &str, fields: &[(&str, &str)]) -> Result<Answer, AuthError> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields)
            .finish();
        self.send(HttpRequest {
            method: HttpMethod::Post,
            url: url.to_owned(),
            headers: vec![
                (
                    "content-type".to_owned(),
                    "application/x-www-form-urlencoded".to_owned(),
                ),
                ("accept".to_owned(), "application/json".to_owned()),
            ],
            body: Some(body.into_bytes()),
        })
        .await
    }

    async fn json(&self, url: &str, body: &serde_json::Value) -> Result<Answer, AuthError> {
        self.send(HttpRequest {
            method: HttpMethod::Post,
            url: url.to_owned(),
            headers: vec![
                ("content-type".to_owned(), "application/json".to_owned()),
                ("accept".to_owned(), "application/json".to_owned()),
            ],
            body: Some(body.to_string().into_bytes()),
        })
        .await
    }

    async fn bearer(&self, url: &str, token: &Secret) -> Result<Answer, AuthError> {
        self.send(HttpRequest {
            method: HttpMethod::Get,
            url: url.to_owned(),
            headers: vec![
                (
                    "authorization".to_owned(),
                    format!("Bearer {}", token.expose()),
                ),
                ("accept".to_owned(), "application/json".to_owned()),
            ],
            body: None,
        })
        .await
    }

    /// Asks for a code the person types at the verification address.
    pub async fn request_device_code(&self) -> Result<DeviceCode, AuthError> {
        let (status, body) = self
            .form(
                DEVICE_CODE_URL,
                &[("client_id", &self.client_id), ("scope", SCOPE)],
            )
            .await?;
        if status != 200 {
            return Err(oauth_failure(status, &body));
        }
        let wire: DeviceCodeWire = decode(&body, "device code")?;
        Ok(DeviceCode {
            user_code: wire.user_code,
            verification_uri: wire.verification_uri,
            expires_in: wire.expires_in,
            interval: wire.interval.unwrap_or(5),
            device_code: Secret::new(wire.device_code),
        })
    }

    /// Waits until the person finished in the browser (or declined, or the
    /// code ran out), polling at the service's pace.
    pub async fn wait_for_sign_in(
        &self,
        code: &DeviceCode,
        cancel: &CancellationToken,
    ) -> Result<OAuthTokens, AuthError> {
        let mut interval = Duration::from_secs(code.interval);
        let give_up = tokio::time::Instant::now() + Duration::from_secs(code.expires_in);
        loop {
            tokio::select! {
                () = cancel.cancelled() => return Err(AuthError::Cancelled),
                () = tokio::time::sleep(interval) => {}
            }
            if tokio::time::Instant::now() >= give_up {
                return Err(AuthError::Expired);
            }
            let fields = [
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", self.client_id.as_str()),
                ("device_code", code.device_code.expose()),
            ];
            let (status, body) = tokio::select! {
                () = cancel.cancelled() => return Err(AuthError::Cancelled),
                answer = self.form(TOKEN_URL, &fields) => answer?,
            };
            if status == 200 {
                return tokens(&body);
            }
            match oauth_error(&body).as_deref() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval += self.slow_down,
                Some("authorization_declined") => return Err(AuthError::Declined),
                Some("expired_token" | "bad_verification_code") => return Err(AuthError::Expired),
                _ => return Err(oauth_failure(status, &body)),
            }
        }
    }

    /// Exchanges a stored refresh token for fresh Microsoft tokens (the
    /// refresh token is replaced each time).
    pub async fn refresh(&self, refresh_token: &Secret) -> Result<OAuthTokens, AuthError> {
        let (status, body) = self
            .form(
                TOKEN_URL,
                &[
                    ("grant_type", "refresh_token"),
                    ("client_id", &self.client_id),
                    ("refresh_token", refresh_token.expose()),
                    ("scope", SCOPE),
                ],
            )
            .await?;
        if status == 200 {
            return tokens(&body);
        }
        match oauth_error(&body).as_deref() {
            Some("invalid_grant" | "interaction_required" | "invalid_request") => {
                Err(AuthError::SignInRequired)
            }
            _ => Err(oauth_failure(status, &body)),
        }
    }

    /// Turns Microsoft tokens into a Minecraft session: the Xbox Live user
    /// token, the XSTS token for Minecraft's services, the services login and
    /// the profile.
    pub async fn minecraft_login(&self, oauth: &OAuthTokens) -> Result<MinecraftLogin, AuthError> {
        let (status, body) = self
            .json(
                XBL_URL,
                &json!({
                    "Properties": {
                        "AuthMethod": "RPS",
                        "SiteName": "user.auth.xboxlive.com",
                        "RpsTicket": format!("d={}", oauth.access_token.expose()),
                    },
                    "RelyingParty": "http://auth.xboxlive.com",
                    "TokenType": "JWT",
                }),
            )
            .await?;
        if status != 200 {
            return Err(xbox_failure(status, &body));
        }
        let user: XboxToken = decode(&body, "Xbox Live token")?;

        let (status, body) = self
            .json(
                XSTS_URL,
                &json!({
                    "Properties": {
                        "SandboxId": "RETAIL",
                        "UserTokens": [user.token],
                    },
                    "RelyingParty": "rp://api.minecraftservices.com/",
                    "TokenType": "JWT",
                }),
            )
            .await?;
        if status != 200 {
            return Err(xbox_failure(status, &body));
        }
        let xsts: XboxToken = decode(&body, "XSTS token")?;
        let hash = xsts
            .display_claims
            .xui
            .first()
            .map(|claim| claim.uhs.clone())
            .ok_or_else(|| AuthError::Protocol("the XSTS token has no user hash".to_owned()))?;

        let (status, body) = self
            .json(
                MINECRAFT_LOGIN_URL,
                &json!({ "identityToken": format!("XBL3.0 x={hash};{}", xsts.token) }),
            )
            .await?;
        if status != 200 {
            return Err(AuthError::ServicesRefused(describe(status, &body)));
        }
        let login: MinecraftLoginWire = decode(&body, "Minecraft login")?;
        let access_token = Secret::new(login.access_token);
        let expires_at = (self.clock)() + login.expires_in;

        let (status, body) = self.bearer(PROFILE_URL, &access_token).await?;
        match status {
            200 => {}
            404 => return Err(AuthError::NoGameOwnership),
            _ => return Err(AuthError::ServicesRefused(describe(status, &body))),
        }
        let profile: ProfileWire = decode(&body, "Minecraft profile")?;
        let profile_id = ProfileId::parse(&profile.id)
            .map_err(|_| AuthError::Protocol("the profile id is not valid".to_owned()))?;
        Ok(MinecraftLogin {
            access_token,
            expires_at,
            profile_id,
            profile_name: profile.name,
        })
    }
}

// ── wire shapes ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DeviceCodeWire {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct TokenWire {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct XboxToken {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<UserHash>,
}

#[derive(Deserialize)]
struct UserHash {
    uhs: String,
}

#[derive(Deserialize)]
struct MinecraftLoginWire {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct ProfileWire {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct OAuthErrorWire {
    error: String,
}

#[derive(Deserialize)]
struct XErrWire {
    #[serde(rename = "XErr")]
    code: Option<u64>,
}

fn decode<V: for<'de> Deserialize<'de>>(body: &[u8], what: &str) -> Result<V, AuthError> {
    serde_json::from_slice(body).map_err(|error| AuthError::Protocol(format!("{what}: {error}")))
}

fn tokens(body: &[u8]) -> Result<OAuthTokens, AuthError> {
    let wire: TokenWire = decode(body, "token")?;
    let refresh = wire
        .refresh_token
        .ok_or_else(|| AuthError::Protocol("the answer has no refresh token".to_owned()))?;
    Ok(OAuthTokens {
        access_token: Secret::new(wire.access_token),
        refresh_token: Secret::new(refresh),
    })
}

fn oauth_error(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<OAuthErrorWire>(body)
        .ok()
        .map(|wire| wire.error)
}

/// A short, secret-free description of an unexpected answer.
fn describe(status: u16, body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let text: String = text.chars().take(SNIPPET).collect();
    format!("status {status}: {text}")
}

fn oauth_failure(status: u16, body: &[u8]) -> AuthError {
    AuthError::Protocol(describe(status, body))
}

/// The Xbox services explain a refusal with an `XErr` number.
fn xbox_failure(status: u16, body: &[u8]) -> AuthError {
    let code = serde_json::from_slice::<XErrWire>(body)
        .ok()
        .and_then(|wire| wire.code);
    match code {
        Some(2_148_916_233) => AuthError::NoXboxAccount,
        Some(2_148_916_235) => AuthError::XboxUnavailable,
        Some(2_148_916_236 | 2_148_916_237) => AuthError::AdultVerificationRequired,
        Some(2_148_916_238) => AuthError::ChildAccount,
        _ => AuthError::Protocol(describe(status, body)),
    }
}

#[cfg(test)]
mod tests {
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
        let xsts =
            String::from_utf8(script.requests_to(XSTS_URL)[0].body.clone().unwrap()).unwrap();
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
        let body =
            String::from_utf8(script.requests_to(TOKEN_URL)[0].body.clone().unwrap()).unwrap();
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
    /// `cargo test -p lumilio-core real_device_code -- --ignored --nocapture`.
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
}
