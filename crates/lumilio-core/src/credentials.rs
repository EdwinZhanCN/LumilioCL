//! Where sign-in secrets live: the operating system credential store, and
//! nowhere else (ADR 0020). There is no file fallback: when the store is not
//! usable, signing in fails and says why.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::microsoft::AuthError;

/// The service name every entry is kept under.
pub const SERVICE: &str = "LumilioCL";

/// A place that keeps small secrets by key.
pub trait CredentialStore: Send + Sync + 'static {
    /// The secret under `key`, or `None` when there is none.
    fn get(&self, key: &str) -> Result<Option<String>, AuthError>;
    fn set(&self, key: &str, secret: &str) -> Result<(), AuthError>;
    /// Removes the secret; one that is not there is not an error.
    fn delete(&self, key: &str) -> Result<(), AuthError>;
}

/// Keychain, Credential Manager or Secret Service, whichever the system has.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCredentials;

fn failure(error: &keyring::Error) -> AuthError {
    AuthError::CredentialStore(error.to_string())
}

impl CredentialStore for SystemCredentials {
    fn get(&self, key: &str) -> Result<Option<String>, AuthError> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(|error| failure(&error))?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(failure(&error)),
        }
    }

    fn set(&self, key: &str, secret: &str) -> Result<(), AuthError> {
        keyring::Entry::new(SERVICE, key)
            .and_then(|entry| entry.set_password(secret))
            .map_err(|error| failure(&error))
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(|error| failure(&error))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(failure(&error)),
        }
    }
}

/// A store in memory, for tests and for rehearsing an unusable store.
#[derive(Debug, Default)]
pub struct MemoryCredentials {
    secrets: Mutex<BTreeMap<String, String>>,
    broken: std::sync::atomic::AtomicBool,
}

impl MemoryCredentials {
    /// Makes every later call fail like an unavailable store.
    pub fn break_it(&self) {
        self.broken.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn check(&self) -> Result<(), AuthError> {
        if self.broken.load(std::sync::atomic::Ordering::SeqCst) {
            Err(AuthError::CredentialStore("store unavailable".to_owned()))
        } else {
            Ok(())
        }
    }

    /// Every stored value, for checking that secrets stay out of other places.
    #[must_use]
    pub fn values(&self) -> Vec<String> {
        self.secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }
}

impl CredentialStore for MemoryCredentials {
    fn get(&self, key: &str) -> Result<Option<String>, AuthError> {
        self.check()?;
        Ok(self
            .secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, secret: &str) -> Result<(), AuthError> {
        self.check()?;
        self.secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key.to_owned(), secret.to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        self.check()?;
        self.secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
        Ok(())
    }
}

/// What is kept for one signed-in account.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoredLogin {
    pub refresh_token: String,
    pub access_token: String,
    /// When the Minecraft access token stops working, in Unix seconds.
    pub expires_at: u64,
}

impl std::fmt::Debug for StoredLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredLogin")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

impl StoredLogin {
    /// Reads the login kept under `key`.
    pub fn load(store: &dyn CredentialStore, key: &str) -> Result<Option<Self>, AuthError> {
        match store.get(key)? {
            None => Ok(None),
            Some(text) => serde_json::from_str(&text).map(Some).map_err(|_| {
                AuthError::CredentialStore("the stored sign-in is unreadable".to_owned())
            }),
        }
    }

    pub fn save(&self, store: &dyn CredentialStore, key: &str) -> Result<(), AuthError> {
        let text = serde_json::to_string(self)
            .map_err(|error| AuthError::CredentialStore(error.to_string()))?;
        store.set(key, &text)
    }
}

/// What is kept for one account signed in on an authlib-injector server.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoredYggdrasil {
    pub client_token: String,
    pub access_token: String,
    /// The server's `user.properties`, passed on to the game.
    #[serde(default)]
    pub user_properties: BTreeMap<String, String>,
}

impl std::fmt::Debug for StoredYggdrasil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredYggdrasil").finish_non_exhaustive()
    }
}

impl StoredYggdrasil {
    /// Reads the session kept under `key`.
    pub fn load(store: &dyn CredentialStore, key: &str) -> Result<Option<Self>, AuthError> {
        match store.get(key)? {
            None => Ok(None),
            Some(text) => serde_json::from_str(&text).map(Some).map_err(|_| {
                AuthError::CredentialStore("the stored sign-in is unreadable".to_owned())
            }),
        }
    }

    pub fn save(&self, store: &dyn CredentialStore, key: &str) -> Result<(), AuthError> {
        let text = serde_json::to_string(self)
            .map_err(|error| AuthError::CredentialStore(error.to_string()))?;
        store.set(key, &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_login_round_trips_and_deleting_what_is_not_there_is_fine() {
        let store = MemoryCredentials::default();
        assert_eq!(StoredLogin::load(&store, "msa:1").unwrap(), None);
        let login = StoredLogin {
            refresh_token: "r".into(),
            access_token: "a".into(),
            expires_at: 99,
        };
        login.save(&store, "msa:1").unwrap();
        assert_eq!(StoredLogin::load(&store, "msa:1").unwrap(), Some(login));
        store.delete("msa:1").unwrap();
        store.delete("msa:1").unwrap();
        assert_eq!(StoredLogin::load(&store, "msa:1").unwrap(), None);
    }

    #[test]
    fn a_third_party_session_round_trips_and_hides_its_tokens() {
        let store = MemoryCredentials::default();
        let session = StoredYggdrasil {
            client_token: "client-secret".into(),
            access_token: "access-secret".into(),
            user_properties: BTreeMap::from([("lang".into(), "zh".into())]),
        };
        session.save(&store, "ali:1@https://s/").unwrap();
        assert_eq!(
            StoredYggdrasil::load(&store, "ali:1@https://s/").unwrap(),
            Some(session.clone())
        );
        assert!(!format!("{session:?}").contains("secret"));
    }

    #[test]
    fn an_unusable_store_is_an_error_not_an_empty_answer() {
        let store = MemoryCredentials::default();
        store.break_it();
        assert!(matches!(store.get("k"), Err(AuthError::CredentialStore(_))));
        assert!(matches!(
            store.set("k", "v"),
            Err(AuthError::CredentialStore(_))
        ));
        assert!(store.values().is_empty());
    }

    #[test]
    fn secrets_stay_out_of_debug_output_and_garbage_is_reported() {
        let login = StoredLogin {
            refresh_token: "super-secret".into(),
            access_token: "also-secret".into(),
            expires_at: 5,
        };
        let text = format!("{login:?}");
        assert!(!text.contains("secret"), "{text}");
        let store = MemoryCredentials::default();
        store.set("k", "not json").unwrap();
        assert!(matches!(
            StoredLogin::load(&store, "k"),
            Err(AuthError::CredentialStore(_))
        ));
    }

    /// Touches the real credential store of this machine; run on purpose with
    /// `cargo nextest run -p lumilio-core system_store --ignored`.
    #[test]
    #[ignore = "touches the real operating system credential store"]
    fn the_system_store_round_trips_a_secret() {
        let store = SystemCredentials;
        let key = "lumilio-test-entry";
        store.delete(key).unwrap();
        assert_eq!(store.get(key).unwrap(), None);
        store.set(key, "value").unwrap();
        assert_eq!(store.get(key).unwrap().as_deref(), Some("value"));
        store.delete(key).unwrap();
        assert_eq!(store.get(key).unwrap(), None);
    }
}
