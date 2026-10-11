use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::credentials::{MemoryCredentials, StoredLogin, StoredYggdrasil};

#[derive(Default)]
struct WindowsLimit {
    raw: MemoryCredentials,
    writes: AtomicUsize,
    fail_at: AtomicUsize,
}

impl CredentialStore for WindowsLimit {
    fn get(&self, key: &str) -> Result<Option<String>, AuthError> {
        self.raw.get(key)
    }

    fn set(&self, key: &str, secret: &str) -> Result<(), AuthError> {
        let write = self.writes.fetch_add(1, Ordering::SeqCst) + 1;
        if secret.encode_utf16().count() * 2 > 2560 || self.fail_at.load(Ordering::SeqCst) == write
        {
            return Err(AuthError::CredentialStore(
                "credential write rejected".to_owned(),
            ));
        }
        self.raw.set(key, secret)
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        self.raw.delete(key)
    }
}

struct Chunked(WindowsLimit);

impl CredentialStore for Chunked {
    fn get(&self, key: &str) -> Result<Option<String>, AuthError> {
        get(&self.0, key)
    }
    fn set(&self, key: &str, secret: &str) -> Result<(), AuthError> {
        set(&self.0, key, secret)
    }
    fn delete(&self, key: &str) -> Result<(), AuthError> {
        delete(&self.0, key)
    }
}

#[test]
fn windows_long_microsoft_login_round_trips_and_removes_all_secrets() {
    let store = Chunked(WindowsLimit::default());
    let login = StoredLogin {
        refresh_token: "refresh".repeat(400),
        access_token: "jwt".repeat(3000),
        expires_at: 99,
    };
    assert!(
        store
            .0
            .set("msa:1", &serde_json::to_string(&login).unwrap())
            .is_err()
    );
    login.save(&store, "msa:1").unwrap();
    assert_eq!(StoredLogin::load(&store, "msa:1").unwrap(), Some(login));
    store.delete("msa:1").unwrap();
    store.delete("msa:1").unwrap();
    assert_eq!(StoredLogin::load(&store, "msa:1").unwrap(), None);
    assert!(store.0.raw.values().is_empty());
}

#[test]
fn windows_long_third_party_session_preserves_unicode() {
    let store = Chunked(WindowsLimit::default());
    let session = StoredYggdrasil {
        client_token: "client".into(),
        access_token: "jwt".repeat(2000),
        user_properties: std::collections::BTreeMap::from([("text".into(), "中文🔥".repeat(900))]),
    };
    session.save(&store, "ali:1").unwrap();
    assert_eq!(
        StoredYggdrasil::load(&store, "ali:1").unwrap(),
        Some(session)
    );
    assert!(
        store
            .0
            .raw
            .values()
            .iter()
            .all(|value| value.encode_utf16().count() <= 1200)
    );
}

#[test]
fn windows_legacy_entries_migrate_and_replacements_clean_old_chunks() {
    let store = Chunked(WindowsLimit::default());
    store.0.set("k", "legacy").unwrap();
    assert_eq!(store.get("k").unwrap().as_deref(), Some("legacy"));
    store.set("k", &"a".repeat(8000)).unwrap();
    store.set("k", &"b".repeat(4000)).unwrap();
    assert_eq!(store.get("k").unwrap(), Some("b".repeat(4000)));
    assert!(
        !store
            .0
            .raw
            .values()
            .iter()
            .any(|text| text.contains("aaaa"))
    );
    store.set("k", "short").unwrap();
    assert_eq!(store.0.raw.values(), vec!["short"]);
}

#[test]
fn windows_failed_chunk_or_manifest_write_keeps_old_login() {
    for failed_offset in [2, 4] {
        let store = Chunked(WindowsLimit::default());
        let old = "old".repeat(1400);
        store.set("k", &old).unwrap();
        let before = store.0.raw.values();
        store.0.fail_at.store(
            store.0.writes.load(Ordering::SeqCst) + failed_offset,
            Ordering::SeqCst,
        );
        // Three chunks then the root: test failure halfway and during the commit.
        assert!(store.set("k", &"n".repeat(3500)).is_err());
        assert_eq!(store.get("k").unwrap(), Some(old));
        assert_eq!(store.0.raw.values(), before);
    }
}

#[test]
fn windows_missing_chunks_and_bad_manifests_are_errors() {
    let store = Chunked(WindowsLimit::default());
    store.set("k", &"x".repeat(3000)).unwrap();
    let text = store.0.get("k").unwrap().unwrap();
    let value = manifest(&text).unwrap().unwrap();
    store.0.delete(&value.key("k", 1)).unwrap();
    assert!(store.get("k").is_err());
    store.delete("k").unwrap();
    assert!(store.0.raw.values().is_empty());
    store
        .0
        .set(
            "k",
            &format!("{PREFIX}{{\"generation\":\"../other\",\"parts\":9999}}"),
        )
        .unwrap();
    assert!(store.get("k").is_err());
}

#[test]
fn windows_reserved_prefix_is_a_round_trip_secret() {
    let store = Chunked(WindowsLimit::default());
    let secret = format!("{PREFIX}literal secret");
    store.set("k", &secret).unwrap();
    assert_eq!(store.get("k").unwrap(), Some(secret));
}
