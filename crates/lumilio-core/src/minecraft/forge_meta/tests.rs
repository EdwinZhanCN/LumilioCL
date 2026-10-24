use super::*;

#[test]
fn forge_lists_one_game_newest_first_and_marks_the_recommended_build() {
    let metadata = br#"{"1.7.10":["1.7.10-10.13.4.1614-1.7.10"],
            "26.2":["26.2-65.0.0","26.2-65.1.0","26.2-65.1.3"],
            "26.3":["26.3-66.0.0"]}"#;
    let promotions =
        br#"{"homepage":"x","promos":{"26.2-recommended":"65.1.0","26.2-latest":"65.1.3"}}"#;
    let versions = forge_versions(metadata, Some(promotions), "26.2").unwrap();
    let names: Vec<_> = versions.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(names, ["65.1.3", "65.1.0", "65.0.0"]);
    assert_eq!(
        versions.iter().map(|v| v.stable).collect::<Vec<_>>(),
        [false, true, false]
    );
    // The branch suffix stays part of the version; its number is what is promoted.
    let old = forge_versions(
        metadata,
        Some(br#"{"promos":{"1.7.10-recommended":"10.13.4.1614"}}"#),
        "1.7.10",
    )
    .unwrap();
    assert_eq!(old[0].version, "10.13.4.1614-1.7.10");
    assert!(old[0].stable);
    // No promotions: nothing is marked stable; unknown game: empty.
    assert!(
        forge_versions(metadata, None, "26.2")
            .unwrap()
            .iter()
            .all(|v| !v.stable)
    );
    assert!(forge_versions(metadata, None, "1.0").unwrap().is_empty());
    assert!(forge_versions(b"nope", None, "26.2").is_err());
}

#[test]
fn neoforge_numbers_map_to_their_game_versions() {
    for (version, game) in [
        ("21.1.77", Some("1.21.1")),
        ("21.0.10-beta", Some("1.21")),
        ("20.2.3-beta", Some("1.20.2")),
        ("20.4.237", Some("1.20.4")),
        ("26.3.0.31-beta", Some("26.3")),
        ("26.1.2.5", Some("26.1.2")),
        ("26.3.0.1-beta+pre-2", Some("26.3-pre-2")),
        ("0.25w14craftmine.3-beta", Some("25w14craftmine")),
        ("26.3.0", None),
        ("x.y.z", None),
        ("21", None),
    ] {
        assert_eq!(neoforge_game_version(version).as_deref(), game, "{version}");
    }
}

#[test]
fn neoforge_lists_one_game_newest_first_with_legacy_builds_for_1_20_1() {
    let modern = br#"{"isSnapshot":false,"versions":
            ["20.2.3-beta","21.1.1","21.1.2-beta","21.1.3","26.3.0.1-beta"]}"#;
    let versions = neoforge_versions(modern, None, "1.21.1").unwrap();
    let names: Vec<_> = versions.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(names, ["21.1.3", "21.1.2-beta", "21.1.1"]);
    assert_eq!(
        versions.iter().map(|v| v.stable).collect::<Vec<_>>(),
        [true, false, true]
    );
    let legacy = br#"{"versions":["1.20.1-47.1.5","1.20.1-forge-47.1.7"]}"#;
    let old = neoforge_versions(modern, Some(legacy), "1.20.1").unwrap();
    let names: Vec<_> = old.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(names, ["47.1.7", "47.1.5"]);
    // The legacy list is ignored for every other game version.
    assert_eq!(
        neoforge_versions(modern, Some(legacy), "26.3")
            .unwrap()
            .len(),
        1
    );
}
