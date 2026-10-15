use std::path::PathBuf;

use super::super::{
    AssetNotice, BundleIndex, BundleReach, InstalledClient, InstanceAssets, PlannedSource, resolve,
};

pub(super) fn index_json(sha256: &str, size: u64) -> String {
    format!(
        r#"{{"format":1,"file":"blocks.dat.z","size":{size},"sha256":"{sha256}","tool_commit":"abc",
        "versions":[{{"id":"1.13.2","data_version":1631,"jar_sha1":"a"}},
                    {{"id":"1.20.1","data_version":3465,"jar_sha1":"b"}},
                    {{"id":"26.3","data_version":5023,"jar_sha1":"c"}}]}}"#
    )
}

fn index() -> BundleIndex {
    BundleIndex::parse(index_json(&"0".repeat(64), 10).as_bytes()).unwrap()
}

fn installed(version: &str) -> InstalledClient {
    InstalledClient {
        version: version.to_owned(),
        jar: PathBuf::from(format!("versions/{version}/{version}.jar")),
    }
}

/// An instance of `version` (its client installed when `client`) offering a
/// mod and a resource pack, both switched on.
fn instance(version: &str, client: bool) -> InstanceAssets {
    let overlays = vec![
        PathBuf::from("mods/create.jar"),
        PathBuf::from("resourcepacks/faithful.zip"),
    ];
    InstanceAssets {
        game_version: version.to_owned(),
        client: client.then(|| installed(version)),
        other_clients: Vec::new(),
        switched_on: overlays.clone(),
        overlays,
        added: Vec::new(),
    }
}

fn bundle(version: &str) -> PlannedSource {
    PlannedSource::Bundle {
        version: version.to_owned(),
    }
}

fn overlays() -> [PlannedSource; 2] {
    [
        PlannedSource::Overlay("mods/create.jar".into()),
        PlannedSource::Overlay("resourcepacks/faithful.zip".into()),
    ]
}

/// What a case is, the instance, the schematic's data version and the
/// expected sources.
type Case = (
    &'static str,
    InstanceAssets,
    Option<u32>,
    Vec<PlannedSource>,
);

#[test]
fn the_rule_table_lists_the_sources_d3_and_d23_name() {
    let index = index();
    let mut plain_1_20 = instance("1.20.1", true);
    plain_1_20.overlays.clear();
    let cases: Vec<Case> = vec![
        (
            "1.20.1 installed without overlays takes the bundle's 1.20.1",
            plain_1_20,
            Some(3465),
            vec![bundle("1.20.1")],
        ),
        (
            "1.20.1 installed with mods still takes the bundle; the mods go over it",
            instance("1.20.1", true),
            Some(3465),
            [vec![bundle("1.20.1")], overlays().to_vec()].concat(),
        ),
        (
            "1.20.1 installed, schematic from 26.3: the bundle's 26.3",
            instance("1.20.1", true),
            Some(5023),
            [vec![bundle("26.3")], overlays().to_vec()].concat(),
        ),
        (
            "not installed: the bundle by data version (between versions: the next newer)",
            instance("1.20.1", false),
            Some(3000),
            [vec![bundle("1.20.1")], overlays().to_vec()].concat(),
        ),
        (
            "not installed, no data version: the instance's version",
            instance("1.13.2", false),
            None,
            [vec![bundle("1.13.2")], overlays().to_vec()].concat(),
        ),
    ];
    for (case, facts, data_version, expected) in cases {
        let plan = resolve(&facts, data_version, BundleReach::Index(&index));
        assert_eq!(plan.sources, expected, "{case}");
        assert_eq!(plan.notice, None, "{case}");
    }
}

/// D23: an installed 26.x client is never baked here; its schematics are
/// drawn with the bundle's 26.x pack like any other.
#[test]
fn an_installed_twenty_six_client_still_takes_the_bundle() {
    let index = index();
    let mut facts = instance("26.3", true);
    facts.other_clients = vec![installed("1.20.1")];
    for data_version in [Some(5023), Some(3465), None] {
        let plan = resolve(&facts, data_version, BundleReach::Index(&index));
        assert!(
            matches!(plan.base(), Some(PlannedSource::Bundle { .. })),
            "{data_version:?}: {:?}",
            plan.sources
        );
        assert_eq!(plan.notice, None);
    }
    let plan = resolve(&facts, Some(5023), BundleReach::Index(&index));
    assert_eq!(plan.base(), Some(&bundle("26.3")));
}

/// D23: the bundle out of reach is a notice, never a bake of an installed
/// game, its own or another.
#[test]
fn out_of_reach_is_one_calm_notice_and_no_sources_even_with_games_installed() {
    let mut facts = instance("26.3", true);
    facts.other_clients = vec![installed("1.20.1"), installed("1.19.4")];
    let offline = resolve(&facts, Some(5023), BundleReach::Offline);
    assert!(offline.sources.is_empty(), "{:?}", offline.sources);
    assert_eq!(offline.notice, Some(AssetNotice::Offline));
    let disabled = resolve(&facts, Some(3465), BundleReach::Disabled);
    assert!(disabled.sources.is_empty(), "{:?}", disabled.sources);
    assert_eq!(disabled.notice, Some(AssetNotice::DownloadDisabled));
}

/// T21: an overlay on offer but not switched on is not in the plan; the
/// rest keep their order.
#[test]
fn an_overlay_switched_off_is_left_out_of_the_plan() {
    let index = index();
    let mut facts = instance("1.20.1", false);
    facts.overlays.push("resourcepacks/stay-true.zip".into());
    facts.switched_on = vec![
        "resourcepacks/stay-true.zip".into(),
        "mods/create.jar".into(),
        // Switched on, but the instance no longer offers it.
        "mods/gone.jar".into(),
    ];
    let plan = resolve(&facts, Some(3465), BundleReach::Index(&index));
    assert_eq!(
        plan.overlays(),
        [
            std::path::Path::new("mods/create.jar"),
            std::path::Path::new("resourcepacks/stay-true.zip"),
        ]
    );
    facts.switched_on.clear();
    let plan = resolve(&facts, Some(3465), BundleReach::Index(&index));
    assert_eq!(plan.sources, [bundle("1.20.1")], "all off by default");
}

#[test]
fn an_index_of_a_newer_format_or_with_odd_file_names_is_refused() {
    use super::super::SchematicAssetsError;
    let newer = index_json(&"0".repeat(64), 1).replace("\"format\":1", "\"format\":9");
    assert!(matches!(
        BundleIndex::parse(newer.as_bytes()),
        Err(SchematicAssetsError::UnsupportedIndex(9))
    ));
    let escaping = index_json(&"0".repeat(64), 1).replace("blocks.dat.z", "../blocks.dat.z");
    assert!(matches!(
        BundleIndex::parse(escaping.as_bytes()),
        Err(SchematicAssetsError::Index(_))
    ));
}
