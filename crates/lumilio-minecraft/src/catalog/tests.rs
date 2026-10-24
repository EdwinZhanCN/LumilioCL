use super::*;

const DOC: &str = r#"{
        "latest": {"release": "1.21.1", "snapshot": "24w40a"},
        "versions": [
            {"id": "1.20.4", "type": "release", "url": "https://x/1.20.4.json",
             "releaseTime": "2023-12-07T12:56:20+00:00", "sha1": "aa"},
            {"id": "24w40a", "type": "snapshot", "url": "https://x/24w40a.json",
             "releaseTime": "2024-10-02T13:00:00+00:00"},
            {"id": "1.21.1", "type": "release", "url": "https://x/1.21.1.json",
             "releaseTime": "2024-08-08T12:24:45+00:00"},
            {"id": "b1.7.3", "type": "old_beta", "url": "https://x/b1.7.3.json",
             "releaseTime": "2011-07-08T00:00:00+00:00"},
            {"id": "", "type": "release", "url": "https://x/blank.json"},
            {"id": "no-url", "type": "release"},
            {"id": "1.21.1", "type": "release", "url": "https://x/dup.json",
             "releaseTime": "2000-01-01T00:00:00+00:00"},
            {"id": "weird", "type": "future_kind", "url": "https://x/w.json",
             "time": "2025-01-01T00:00:00+00:00"}
        ]
    }"#;

#[test]
fn orders_newest_first_and_drops_unusable_entries() {
    let catalog = VersionCatalog::decode_json(DOC).unwrap();
    let ids: Vec<_> = catalog.entries().iter().map(CatalogEntry::id).collect();
    assert_eq!(ids, ["weird", "24w40a", "1.21.1", "1.20.4", "b1.7.3"]);
}

#[test]
fn duplicate_ids_keep_the_first_occurrence() {
    let catalog = VersionCatalog::decode_json(DOC).unwrap();
    assert_eq!(
        catalog.get("1.21.1").unwrap().manifest_url(),
        "https://x/1.21.1.json"
    );
}

#[test]
fn classifies_kinds_and_falls_back_to_time() {
    let catalog = VersionCatalog::decode_json(DOC).unwrap();
    assert_eq!(catalog.get("b1.7.3").unwrap().kind(), VersionKind::Old);
    let weird = catalog.get("weird").unwrap();
    assert_eq!(weird.kind(), VersionKind::Other);
    assert_eq!(weird.release_time(), "2025-01-01T00:00:00+00:00");
}

#[test]
fn channels_tell_snapshots_pre_releases_and_candidates_apart() {
    use VersionChannel::*;
    for (id, channel) in [
        ("26.4-snapshot-2", Snapshot),
        ("25w14a", Snapshot),
        ("26.3-pre-3", PreRelease),
        ("1.14 Pre-Release 1", PreRelease),
        ("26.3-rc-1", Candidate),
        ("1.16-rc1", Candidate),
        ("1.17 Release Candidate 2", Candidate),
    ] {
        assert_eq!(VersionChannel::of_unreleased(id), channel, "{id}");
    }
    let catalog = VersionCatalog::decode_json(DOC).unwrap();
    assert_eq!(catalog.get("1.21.1").unwrap().channel(), Release);
    assert_eq!(catalog.get("24w40a").unwrap().channel(), Snapshot);
    assert_eq!(catalog.get("b1.7.3").unwrap().channel(), Old);
}

#[test]
fn filters_by_kind() {
    let catalog = VersionCatalog::decode_json(DOC).unwrap();
    let releases: Vec<_> = catalog
        .of_kinds(&[VersionKind::Release])
        .map(CatalogEntry::id)
        .collect();
    assert_eq!(releases, ["1.21.1", "1.20.4"]);
}

#[test]
fn resolves_latest_pointers() {
    let catalog = VersionCatalog::decode_json(DOC).unwrap();
    assert_eq!(catalog.latest_release().unwrap().id(), "1.21.1");
    assert_eq!(catalog.latest_snapshot().unwrap().id(), "24w40a");
}

#[test]
fn rejects_a_document_without_versions() {
    assert!(matches!(
        VersionCatalog::decode_json(r#"{"latest":{}}"#),
        Err(CatalogError::Decode(_))
    ));
    assert!(matches!(
        VersionCatalog::decode_json("not json"),
        Err(CatalogError::Decode(_))
    ));
}
