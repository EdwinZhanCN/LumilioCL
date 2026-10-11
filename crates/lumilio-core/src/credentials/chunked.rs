//! Windows Credential Manager limits each blob. Keep every secret in that store,
//! using a small manifest and independently named chunks for long login payloads.

use serde::{Deserialize, Serialize};

use super::CredentialStore;
use crate::microsoft::AuthError;

// A UTF-16 password at this size uses at most 2400 bytes, below the Windows
// CRED_MAX_CREDENTIAL_BLOB_SIZE (5 * 512 bytes), including surrogate pairs.
const CHUNK_UNITS: usize = 1200;
const MAX_PARTS: usize = 256;
const PREFIX: &str = "lumilio:credential-chunks:v1:";

#[derive(Deserialize, Serialize)]
struct Manifest {
    generation: String,
    parts: usize,
}

fn unreadable() -> AuthError {
    AuthError::CredentialStore("the stored sign-in is incomplete or unreadable".to_owned())
}

fn manifest(text: &str) -> Result<Option<Manifest>, AuthError> {
    let Some(json) = text.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    let value: Manifest = serde_json::from_str(json).map_err(|_| unreadable())?;
    if !(1..=MAX_PARTS).contains(&value.parts)
        || value.generation.len() != 32
        || !value
            .generation
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(unreadable());
    }
    Ok(Some(value))
}

impl Manifest {
    fn key(&self, key: &str, part: usize) -> String {
        format!("{key}:lumilio-chunk:{}:{part}", self.generation)
    }

    fn delete_parts(&self, store: &dyn CredentialStore, key: &str) -> Result<(), AuthError> {
        let mut error = None;
        for part in 0..self.parts {
            if let Err(failure) = store.delete(&self.key(key, part)) {
                error.get_or_insert(failure);
            }
        }
        error.map_or(Ok(()), Err)
    }
}

pub(super) fn get(store: &dyn CredentialStore, key: &str) -> Result<Option<String>, AuthError> {
    let Some(text) = store.get(key)? else {
        return Ok(None);
    };
    let Some(value) = manifest(&text)? else {
        return Ok(Some(text));
    };
    let mut secret = String::new();
    for part in 0..value.parts {
        let chunk = store.get(&value.key(key, part))?.ok_or_else(unreadable)?;
        secret.push_str(&chunk);
    }
    Ok(Some(secret))
}

pub(super) fn set(store: &dyn CredentialStore, key: &str, secret: &str) -> Result<(), AuthError> {
    let old = store
        .get(key)?
        .map(|text| manifest(&text))
        .transpose()?
        .flatten();
    if secret.encode_utf16().count() <= CHUNK_UNITS && !secret.starts_with(PREFIX) {
        store.set(key, secret)?;
    } else {
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut units = 0;
        for (offset, character) in secret.char_indices() {
            if units + character.len_utf16() > CHUNK_UNITS {
                chunks.push(&secret[start..offset]);
                start = offset;
                units = 0;
            }
            units += character.len_utf16();
        }
        chunks.push(&secret[start..]);
        if chunks.len() > MAX_PARTS {
            return Err(AuthError::CredentialStore(
                "the sign-in data is too large".to_owned(),
            ));
        }
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)
            .map_err(|error| AuthError::CredentialStore(error.to_string()))?;
        let value = Manifest {
            generation: random.iter().map(|byte| format!("{byte:02x}")).collect(),
            parts: chunks.len(),
        };
        let text = format!(
            "{PREFIX}{}",
            serde_json::to_string(&value).map_err(|_| unreadable())?
        );
        // Commit the root last: failed refreshes leave the old login readable.
        let written = (|| {
            for (part, chunk) in chunks.into_iter().enumerate() {
                store.set(&value.key(key, part), chunk)?;
            }
            store.set(key, &text)
        })();
        if let Err(error) = written {
            let _ = value.delete_parts(store, key);
            return Err(error);
        }
    }
    if let Some(old) = old {
        // The new login is committed; cleanup must not turn a successful refresh into a failure.
        let _ = old.delete_parts(store, key);
    }
    Ok(())
}

pub(super) fn delete(store: &dyn CredentialStore, key: &str) -> Result<(), AuthError> {
    if let Some(text) = store.get(key)?
        && let Some(value) = manifest(&text)?
    {
        // Keep the manifest on failure so deletion can be retried.
        value.delete_parts(store, key)?;
    }
    store.delete(key)
}

#[cfg(test)]
mod tests;
