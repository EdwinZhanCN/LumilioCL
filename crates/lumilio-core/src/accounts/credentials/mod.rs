//! Where sign-in secrets live: the operating system credential store, and
//! nowhere else (ADR 0020). There is no file fallback: when the store is not
//! usable, signing in fails and says why.

#[cfg(test)]
use std::collections::BTreeMap;

pub use credential_store::{
    CredentialStore, MemoryCredentials, StoredLogin, StoredYggdrasil, SystemCredentials,
};

#[cfg(test)]
use crate::accounts::microsoft::AuthError;

#[cfg(any(target_os = "windows", test))]
mod chunked;

#[cfg(test)]
mod tests;

mod credential_store;
