//! Talking to a Yggdrasil authentication server (what authlib-injector servers
//! such as LittleSkin speak): sign in with a name and password, keep the
//! session alive, and tell the character list.
//!
//! The rules follow HMCL (`HMCLCore/.../auth/yggdrasil/YggdrasilService.java`,
//! `YggdrasilAccount.java`, AGPL-3.0; ADR 0011). This module only speaks the
//! protocol; where secrets are kept is [`crate::credentials`] (ADR 0020).

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use futures_util::StreamExt as _;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::account::ProfileId;
use crate::microsoft::Secret;
use crate::transfer::{HttpMethod, HttpRequest, Transport};

mod server;

pub use self::server::{AuthServer, LITTLE_SKIN_URL, fetch_metadata, little_skin, locate_server};

/// Answers are small JSON documents; anything bigger is not one.
const BODY_LIMIT: usize = 1024 * 1024;
/// How much of an unexpected answer is kept for the technical details.
const SNIPPET: usize = 300;

/// A character on the server. The name can be empty on a session read from
/// storage, so it is only trusted after a fresh answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
}

/// What the server handed out for one sign-in or refresh.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub client_token: String,
    pub access_token: Secret,
    /// The character the token belongs to; `None` until one is chosen.
    pub selected: Option<Profile>,
    /// The characters to choose from, when none is selected yet.
    pub available: Vec<Profile>,
    /// `user.properties` of the answer, passed on to the game.
    pub user_properties: BTreeMap<String, String>,
}

/// Why talking to the server did not work, in terms a person can act on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum YggdrasilError {
    /// The server could not be reached (network, TLS, DNS).
    Network(String),
    /// The server answered something that is not what the protocol says.
    Malformed(String),
    /// The server refused with its own error (`error` and `errorMessage`).
    Remote {
        kind: String,
        message: Option<String>,
    },
    /// The name or password was refused.
    InvalidCredentials,
    /// The stored session was refused and cannot be renewed: sign in again.
    SessionExpired,
    /// The account has no character on this server.
    NoCharacter,
    /// The character this account was added for is gone.
    CharacterDeleted,
}

impl Display for YggdrasilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(detail) => {
                write!(f, "could not reach the authentication server: {detail}")
            }
            Self::Malformed(detail) => {
                write!(f, "the authentication server answered oddly: {detail}")
            }
            Self::Remote { kind, message } => match message {
                Some(message) => write!(f, "{kind}: {message}"),
                None => f.write_str(kind),
            },
            Self::InvalidCredentials => f.write_str("the name or password was refused"),
            Self::SessionExpired => f.write_str("the sign-in has expired; sign in again"),
            Self::NoCharacter => f.write_str("the account has no character on this server"),
            Self::CharacterDeleted => f.write_str("the character of this account no longer exists"),
        }
    }
}

impl Error for YggdrasilError {}

impl YggdrasilError {
    /// `ForbiddenOperationException`: the server says no to these credentials.
    #[must_use]
    pub fn is_forbidden(&self) -> bool {
        matches!(self, Self::Remote { kind, .. } if kind == "ForbiddenOperationException")
    }
}

#[derive(Deserialize)]
struct ErrorWire {
    #[serde(default)]
    error: Option<String>,
    #[serde(default, rename = "errorMessage")]
    error_message: Option<String>,
}

#[derive(Deserialize)]
struct ProfileWire {
    id: String,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct PropertyWire {
    name: String,
    #[serde(default)]
    value: String,
}

#[derive(Deserialize)]
struct UserWire {
    #[serde(default)]
    properties: Vec<PropertyWire>,
}

#[derive(Deserialize)]
struct AuthenticationWire {
    #[serde(default, rename = "accessToken")]
    access_token: Option<String>,
    #[serde(default, rename = "clientToken")]
    client_token: Option<String>,
    #[serde(default, rename = "selectedProfile")]
    selected_profile: Option<ProfileWire>,
    #[serde(default, rename = "availableProfiles")]
    available_profiles: Option<Vec<ProfileWire>>,
    #[serde(default)]
    user: Option<UserWire>,
}

fn snippet(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    text.chars().take(SNIPPET).collect()
}

fn profile(wire: ProfileWire) -> Result<Profile, YggdrasilError> {
    let id = ProfileId::parse(&wire.id)
        .map_err(|_| YggdrasilError::Malformed("a character id is not valid".to_owned()))?;
    Ok(Profile {
        id,
        name: wire.name,
    })
}

pub struct YggdrasilClient<'a, T: Transport + ?Sized> {
    transport: &'a T,
    /// The API root, ending with `/`.
    root: String,
}

impl<'a, T: Transport + ?Sized> YggdrasilClient<'a, T> {
    #[must_use]
    pub fn new(transport: &'a T, api_root: &str) -> Self {
        let mut root = api_root.to_owned();
        if !root.ends_with('/') {
            root.push('/');
        }
        Self { transport, root }
    }

    async fn post(&self, path: &str, body: &Value) -> Result<(u16, Vec<u8>), YggdrasilError> {
        let response = self
            .transport
            .send(HttpRequest {
                method: HttpMethod::Post,
                url: format!("{}{path}", self.root),
                headers: vec![
                    ("content-type".to_owned(), "application/json".to_owned()),
                    ("accept".to_owned(), "application/json".to_owned()),
                ],
                body: Some(body.to_string().into_bytes()),
            })
            .await
            .map_err(|error| YggdrasilError::Network(error.to_string()))?;
        let status = response.status();
        let mut stream = response.into_body();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend(chunk.map_err(|error| YggdrasilError::Network(error.to_string()))?);
            if bytes.len() > BODY_LIMIT {
                return Err(YggdrasilError::Malformed(
                    "the answer is too large".to_owned(),
                ));
            }
        }
        Ok((status, bytes))
    }

    /// The server's own error in an answer, if it carries one.
    fn refusal(body: &[u8]) -> Result<(), YggdrasilError> {
        if body.iter().all(u8::is_ascii_whitespace) {
            return Ok(());
        }
        let wire: ErrorWire =
            serde_json::from_slice(body).map_err(|_| YggdrasilError::Malformed(snippet(body)))?;
        match wire.error.filter(|kind| !kind.trim().is_empty()) {
            Some(kind) => Err(YggdrasilError::Remote {
                kind,
                message: wire
                    .error_message
                    .filter(|message| !message.trim().is_empty()),
            }),
            None => Ok(()),
        }
    }

    fn session(body: &[u8], client_token: &str) -> Result<Session, YggdrasilError> {
        Self::refusal(body)?;
        let wire: AuthenticationWire =
            serde_json::from_slice(body).map_err(|_| YggdrasilError::Malformed(snippet(body)))?;
        if wire.client_token.as_deref() != Some(client_token) {
            return Err(YggdrasilError::Malformed(
                "the client token changed".to_owned(),
            ));
        }
        let access_token = wire
            .access_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| YggdrasilError::Malformed("no access token".to_owned()))?;
        Ok(Session {
            client_token: client_token.to_owned(),
            access_token: Secret::new(access_token),
            selected: wire.selected_profile.map(profile).transpose()?,
            available: wire
                .available_profiles
                .unwrap_or_default()
                .into_iter()
                .map(profile)
                .collect::<Result<_, _>>()?,
            user_properties: wire
                .user
                .map(|user| {
                    user.properties
                        .into_iter()
                        .map(|property| (property.name, property.value))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Signs in with a name (or email) and password.
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
        client_token: &str,
    ) -> Result<Session, YggdrasilError> {
        let (_, body) = self
            .post(
                "authserver/authenticate",
                &json!({
                    "agent": { "name": "Minecraft", "version": 1 },
                    "username": username,
                    "password": password,
                    "clientToken": client_token,
                    "requestUser": true,
                }),
            )
            .await?;
        Self::session(&body, client_token).map_err(|error| {
            if error.is_forbidden() {
                YggdrasilError::InvalidCredentials
            } else {
                error
            }
        })
    }

    /// Renews a session, optionally choosing the character it belongs to.
    pub async fn refresh(
        &self,
        access_token: &Secret,
        client_token: &str,
        select: Option<&Profile>,
    ) -> Result<Session, YggdrasilError> {
        let mut request = json!({
            "accessToken": access_token.expose(),
            "clientToken": client_token,
            "requestUser": true,
        });
        if let Some(choice) = select {
            request["selectedProfile"] = json!({
                "id": choice.id.compact(),
                "name": choice.name,
            });
        }
        let (_, body) = self.post("authserver/refresh", &request).await?;
        let session = Self::session(&body, client_token)?;
        if let Some(choice) = select
            && session.selected.as_ref().map(|selected| selected.id) != Some(choice.id)
        {
            return Err(YggdrasilError::Malformed(
                "the character could not be chosen".to_owned(),
            ));
        }
        Ok(session)
    }

    /// Whether the server still accepts this token.
    pub async fn validate(
        &self,
        access_token: &Secret,
        client_token: &str,
    ) -> Result<bool, YggdrasilError> {
        let (_, body) = self
            .post(
                "authserver/validate",
                &json!({
                    "accessToken": access_token.expose(),
                    "clientToken": client_token,
                }),
            )
            .await?;
        match Self::refusal(&body) {
            Ok(()) => Ok(true),
            Err(error) if error.is_forbidden() => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Tells the server to forget this token (signing out).
    pub async fn invalidate(
        &self,
        access_token: &Secret,
        client_token: &str,
    ) -> Result<(), YggdrasilError> {
        let (_, body) = self
            .post(
                "authserver/invalidate",
                &json!({
                    "accessToken": access_token.expose(),
                    "clientToken": client_token,
                }),
            )
            .await?;
        Self::refusal(&body)
    }
}

#[cfg(test)]
mod tests;
