use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use super::chunked;
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

pub(super) fn failure(error: &keyring::Error) -> AuthError {
    AuthError::CredentialStore(error.to_string())
}

// Keep the raw keyring adapter separate so Windows can assemble bounded entries.
pub(super) struct RawCredentials;

impl CredentialStore for SystemCredentials {
    fn get(&self, key: &str) -> Result<Option<String>, AuthError> {
        #[cfg(target_os = "windows")]
        {
            chunked::get(&RawCredentials, key)
        }
        #[cfg(not(target_os = "windows"))]
        {
            RawCredentials.get(key)
        }
    }

    fn set(&self, key: &str, secret: &str) -> Result<(), AuthError> {
        #[cfg(target_os = "windows")]
        {
            chunked::set(&RawCredentials, key, secret)
        }
        #[cfg(not(target_os = "windows"))]
        {
            RawCredentials.set(key, secret)
        }
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        #[cfg(target_os = "windows")]
        {
            chunked::delete(&RawCredentials, key)
        }
        #[cfg(not(target_os = "windows"))]
        {
            RawCredentials.delete(key)
        }
    }
}

impl CredentialStore for RawCredentials {
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
    pub(super) secrets: Mutex<BTreeMap<String, String>>,
    pub(super) broken: std::sync::atomic::AtomicBool,
}

impl MemoryCredentials {
    /// Makes every later call fail like an unavailable store.
    pub fn break_it(&self) {
        self.broken.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(super) fn check(&self) -> Result<(), AuthError> {
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
            .expect("credential store lock was poisoned")
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
            .expect("credential store lock was poisoned")
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, secret: &str) -> Result<(), AuthError> {
        self.check()?;
        self.secrets
            .lock()
            .expect("credential store lock was poisoned")
            .insert(key.to_owned(), secret.to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        self.check()?;
        self.secrets
            .lock()
            .expect("credential store lock was poisoned")
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
