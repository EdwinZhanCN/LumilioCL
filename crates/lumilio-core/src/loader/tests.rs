use super::*;

#[test]
fn urls_are_built_per_loader_and_encode_odd_versions() {
    assert_eq!(
        profile_url(Loader::Fabric, "1.21.1", "0.16.0").unwrap(),
        "https://meta.fabricmc.net/v2/versions/loader/1.21.1/0.16.0/profile/json"
    );
    assert_eq!(
        profile_url(Loader::Quilt, "1.21.1", "0.26.0").unwrap(),
        "https://meta.quiltmc.org/v3/versions/loader/1.21.1/0.26.0/profile/json"
    );
    assert_eq!(
        versions_url(Loader::Fabric, "1.14 Pre-Release 1").unwrap(),
        "https://meta.fabricmc.net/v2/versions/loader/1.14%20Pre-Release%201"
    );
    assert_eq!(
        versions_url(Loader::Fabric, "../x/y").unwrap(),
        "https://meta.fabricmc.net/v2/versions/loader/..%2Fx%2Fy"
    );
    assert!(profile_url(Loader::Forge, "1.21.1", "1").is_none());
    assert!(versions_url(Loader::Vanilla, "1.21.1").is_none());
}

#[test]
fn decodes_versions_with_and_without_a_stable_flag() {
    let fabric = decode_versions(
        br#"[{"loader":{"version":"0.16.5","stable":true}},
                 {"loader":{"version":"0.17.0-beta.1","stable":false}},
                 {"loader":{}},{"other":1}]"#,
    )
    .unwrap();
    assert_eq!(fabric.len(), 2);
    assert!(fabric[0].stable && !fabric[1].stable);
    let quilt = decode_versions(
        br#"[{"loader":{"version":"0.27.0-beta.2"}},{"loader":{"version":"0.26.4"}}]"#,
    )
    .unwrap();
    assert!(!quilt[0].stable && quilt[1].stable);
    assert_eq!(recommended(&quilt).unwrap().version, "0.26.4");
    assert!(decode_versions(b"nope").is_err());
    assert!(recommended(&[]).is_none());
}

#[test]
fn with_no_stable_version_the_newest_is_recommended() {
    let only_beta =
        decode_versions(br#"[{"loader":{"version":"1.0-beta","stable":false}}]"#).unwrap();
    assert_eq!(recommended(&only_beta).unwrap().version, "1.0-beta");
}

#[test]
fn profiles_are_normalized_to_our_id_and_parent() {
    let out = normalize_profile(
        br#"{"id":"fabric-loader-0.16.0-1.21.1","inheritsFrom":"1.21.1","mainClass":"m","libraries":[]}"#,
        "custom-id",
        "1.21.1",
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["id"], "custom-id");
    assert_eq!(value["inheritsFrom"], "1.21.1");
    assert_eq!(value["mainClass"], "m");
    // A profile that named no parent gets one.
    let out = normalize_profile(br#"{"id":"x"}"#, "y", "1.20").unwrap();
    assert!(out.contains(r#""inheritsFrom":"1.20""#));
    assert!(normalize_profile(b"[]", "y", "1").is_err());
}

#[test]
fn every_loader_is_launchable() {
    for loader in [
        Loader::Vanilla,
        Loader::Fabric,
        Loader::Quilt,
        Loader::Forge,
        Loader::NeoForge,
    ] {
        assert!(LAUNCHABLE_LOADERS.contains(&loader), "{loader:?}");
    }
}
