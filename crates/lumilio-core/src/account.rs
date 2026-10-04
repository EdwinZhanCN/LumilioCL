//! Accounts and the identity a launch presents to the game.
//!
//! Only offline profiles exist for now; online sign-in needs its own decision
//! record.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use md5::{Digest, Md5};

use crate::launch::LaunchContext;

/// Longest profile name the game accepts.
pub const MAX_PROFILE_NAME: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileError {
    Empty,
    TooLong,
    /// Names are limited to ASCII letters, digits and underscore.
    InvalidCharacter(char),
    /// The id is not 32 hex digits (dashes optional) or is the nil id.
    InvalidId,
}

impl Display for ProfileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("profile name is empty"),
            Self::TooLong => write!(f, "profile name is longer than {MAX_PROFILE_NAME}"),
            Self::InvalidCharacter(ch) => write!(f, "profile name contains {ch:?}"),
            Self::InvalidId => f.write_str("profile id must be 32 hex digits"),
        }
    }
}

impl Error for ProfileError {}

/// A 128-bit profile id, shown as 32 lowercase hex digits without dashes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProfileId([u8; 16]);

impl ProfileId {
    /// The id a server derives for a name it cannot verify: an MD5 name-based
    /// (version 3) UUID over `OfflinePlayer:<name>`.
    #[must_use]
    pub fn offline(name: &str) -> Self {
        let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
        bytes[6] = (bytes[6] & 0x0f) | 0x30;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(bytes)
    }

    /// Reads 32 hex digits, with or without the 8-4-4-4-12 dashes. The nil id
    /// is rejected because the game treats it as "no profile".
    pub fn parse(text: &str) -> Result<Self, ProfileError> {
        let text = text.trim();
        let hex: String = if text.contains('-') {
            let parts: Vec<&str> = text.split('-').collect();
            let lengths: Vec<usize> = parts.iter().map(|part| part.len()).collect();
            if lengths != [8, 4, 4, 4, 12] {
                return Err(ProfileError::InvalidId);
            }
            parts.concat()
        } else {
            text.to_owned()
        };
        if hex.len() != 32 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ProfileError::InvalidId);
        }
        let mut bytes = [0u8; 16];
        for (at, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16)
                .map_err(|_| ProfileError::InvalidId)?;
        }
        if bytes == [0u8; 16] {
            return Err(ProfileError::InvalidId);
        }
        Ok(Self(bytes))
    }

    /// The game's argument form: 32 hex digits, no dashes.
    #[must_use]
    pub fn compact(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl Display for ProfileId {
    /// The dashed 8-4-4-4-12 form.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let hex = self.compact();
        write!(
            f,
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        )
    }
}

/// A local profile that plays without signing in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineProfile {
    name: String,
    id: ProfileId,
}

impl OfflineProfile {
    pub fn new(name: &str) -> Result<Self, ProfileError> {
        Self::with_id(name, None)
    }

    /// A profile with an explicit id; `None` derives it from the name.
    pub fn with_id(name: &str, id: Option<ProfileId>) -> Result<Self, ProfileError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProfileError::Empty);
        }
        if name.chars().count() > MAX_PROFILE_NAME {
            return Err(ProfileError::TooLong);
        }
        if let Some(bad) = name
            .chars()
            .find(|ch| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        {
            return Err(ProfileError::InvalidCharacter(bad));
        }
        Ok(Self {
            id: id.unwrap_or_else(|| ProfileId::offline(name)),
            name: name.to_owned(),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn id(&self) -> ProfileId {
        self.id
    }

    /// The values a launch needs to present this profile.
    #[must_use]
    pub fn session(&self) -> AuthSession {
        AuthSession {
            player_name: self.name.clone(),
            profile_id: self.id.compact(),
            // Nothing verifies an offline token; the game only needs one.
            access_token: self.id.compact(),
            user_type: "msa".to_owned(),
            user_properties: "{}".to_owned(),
            injection: None,
            keepalive: None,
            notes: Vec::new(),
        }
    }
}

/// The authlib-injector agent a session needs in the game's JVM: the jar and
/// the server the game should talk to instead of Mojang's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Injection {
    pub jar: std::path::PathBuf,
    /// The API root of the authentication (or skin) server.
    pub api_root: String,
    /// The server's metadata document, handed on so the game need not ask.
    pub prefetched: Option<String>,
}

/// Something that must stay alive as long as the game runs (the launcher's own
/// skin server): dropped with the last copy of the session.
#[derive(Clone)]
pub struct Keepalive(std::sync::Arc<dyn std::any::Any + Send + Sync>);

impl Keepalive {
    #[must_use]
    pub fn new(held: impl std::any::Any + Send + Sync) -> Self {
        Self(std::sync::Arc::new(held))
    }
}

impl PartialEq for Keepalive {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Keepalive {}

impl std::fmt::Debug for Keepalive {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("Keepalive")
    }
}

/// The identity values substituted into game arguments.
#[derive(Clone, Eq, PartialEq)]
pub struct AuthSession {
    player_name: String,
    profile_id: String,
    access_token: String,
    user_type: String,
    /// The `--userProperties` JSON the server sent with the sign-in.
    user_properties: String,
    injection: Option<Injection>,
    keepalive: Option<Keepalive>,
    /// Things worth telling when the game starts (a skin that did not load).
    notes: Vec<String>,
}

impl std::fmt::Debug for AuthSession {
    /// The access token never shows in debug output.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthSession")
            .field("player_name", &self.player_name)
            .field("profile_id", &self.profile_id)
            .field("user_type", &self.user_type)
            .field("injection", &self.injection)
            .finish_non_exhaustive()
    }
}

impl AuthSession {
    /// A signed-in Microsoft session: the real profile and the Minecraft
    /// access token the game presents to servers.
    #[must_use]
    pub fn online(player_name: &str, profile_id: ProfileId, access_token: &str) -> Self {
        Self {
            player_name: player_name.to_owned(),
            profile_id: profile_id.compact(),
            access_token: access_token.to_owned(),
            user_type: "msa".to_owned(),
            user_properties: "{}".to_owned(),
            injection: None,
            keepalive: None,
            notes: Vec::new(),
        }
    }

    /// A session from an authlib-injector server: the character the server
    /// signed in, the server's token, and the agent that points the game at it.
    /// `user_properties` are the server's `user.properties`.
    #[must_use]
    pub fn third_party(
        player_name: &str,
        profile_id: ProfileId,
        access_token: &str,
        user_properties: &std::collections::BTreeMap<String, String>,
        injection: Injection,
    ) -> Self {
        // `{"name": ["value"]}`: the shape the game's `--userProperties` takes.
        let properties: std::collections::BTreeMap<_, _> = user_properties
            .iter()
            .map(|(name, value)| (name.as_str(), [value.as_str()]))
            .collect();
        Self {
            player_name: player_name.to_owned(),
            profile_id: profile_id.compact(),
            access_token: access_token.to_owned(),
            user_type: "msa".to_owned(),
            user_properties: serde_json::to_string(&properties).unwrap_or_else(|_| "{}".to_owned()),
            injection: Some(injection),
            keepalive: None,
            notes: Vec::new(),
        }
    }

    /// The same identity with the agent pointed at another server (the
    /// launcher's own skin server for an offline player).
    #[must_use]
    pub fn with_injection(mut self, injection: Injection) -> Self {
        self.injection = Some(injection);
        self
    }

    /// Keeps `held` alive for as long as this session (or a copy) is.
    #[must_use]
    pub fn with_keepalive(mut self, held: Keepalive) -> Self {
        self.keepalive = Some(held);
        self
    }

    #[must_use]
    pub fn with_notes(mut self, notes: Vec<String>) -> Self {
        self.notes = notes;
        self
    }

    /// What to tell when the game starts.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    #[must_use]
    pub const fn injection(&self) -> Option<&Injection> {
        self.injection.as_ref()
    }

    /// The JVM arguments this session adds: the agent, when it has one.
    #[must_use]
    pub fn jvm_arguments(&self) -> Vec<String> {
        self.injection
            .as_ref()
            .map(|injection| {
                crate::injector::jvm_arguments(
                    &injection.jar,
                    &injection.api_root,
                    injection.prefetched.as_deref(),
                )
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn player_name(&self) -> &str {
        &self.player_name
    }

    /// Adds this session's placeholders to a launch context.
    #[must_use]
    pub fn apply(&self, context: LaunchContext) -> LaunchContext {
        context
            .with_value("auth_player_name", &self.player_name)
            .with_value("auth_uuid", &self.profile_id)
            .with_value("auth_access_token", &self.access_token)
            .with_value("auth_session", &self.access_token)
            .with_value("user_type", &self.user_type)
            .with_value("clientid", "")
            .with_value("auth_xuid", "")
            .with_value("user_properties", &self.user_properties)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_id_matches_the_name_based_uuid() {
        // MD5("OfflinePlayer:Steve") with version 3 / variant bits applied.
        let id = ProfileId::offline("Steve");
        assert_eq!(id.compact(), "5627dd98e6be3c21b8a8e92344183641");
        assert_eq!(id.to_string().len(), 36);
        assert_eq!(id.to_string().as_bytes()[14], b'3');
    }

    #[test]
    fn different_names_get_different_ids() {
        assert_ne!(ProfileId::offline("Steve"), ProfileId::offline("Alex"));
    }

    #[test]
    fn validates_names() {
        assert_eq!(OfflineProfile::new("  "), Err(ProfileError::Empty));
        assert_eq!(
            OfflineProfile::new("a_name_that_is_too_long"),
            Err(ProfileError::TooLong)
        );
        assert_eq!(
            OfflineProfile::new("bad name"),
            Err(ProfileError::InvalidCharacter(' '))
        );
        assert_eq!(
            OfflineProfile::new("玩家"),
            Err(ProfileError::InvalidCharacter('玩'))
        );
        assert_eq!(OfflineProfile::new(" Steve_1 ").unwrap().name(), "Steve_1");
    }

    #[test]
    fn a_third_party_session_carries_the_agent_and_the_servers_user_properties() {
        let properties = std::collections::BTreeMap::from([("lang".to_owned(), "zh".to_owned())]);
        let session = AuthSession::third_party(
            "Edwin",
            ProfileId::parse("123e4567e89b12d3a456426614174000").unwrap(),
            "tok",
            &properties,
            Injection {
                jar: "/data/injector.jar".into(),
                api_root: "https://skin.example/api/".to_owned(),
                prefetched: None,
            },
        );
        assert_eq!(
            session.jvm_arguments()[0],
            "-javaagent:/data/injector.jar=https://skin.example/api/"
        );
        assert_eq!(session.user_properties, r#"{"lang":["zh"]}"#);
        assert!(!format!("{session:?}").contains("tok"));
        assert!(
            OfflineProfile::new("Steve")
                .unwrap()
                .session()
                .jvm_arguments()
                .is_empty()
        );
    }

    #[test]
    fn session_fills_the_auth_placeholders() {
        use crate::environment::HostProfile;
        use crate::launch::{LaunchContext, LaunchDirectories};
        let session = OfflineProfile::new("Steve").unwrap().session();
        let context = session.apply(LaunchContext::new(
            HostProfile::current(),
            LaunchDirectories::under("/tmp/x"),
        ));
        assert_eq!(context.values()["auth_player_name"], "Steve");
        assert_eq!(context.values()["auth_uuid"].len(), 32);
    }
}
