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
