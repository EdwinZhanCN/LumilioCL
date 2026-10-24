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
