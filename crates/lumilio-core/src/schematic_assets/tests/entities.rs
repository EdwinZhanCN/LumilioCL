use std::path::PathBuf;

use super::super::{
    BundleIndex, BundleReach, EntityGeometry, InstalledClient, InstanceAssets, resolve,
};

/// An index whose `entities.dat.z` covers 1.20.1 and 26.2, not 26.3.
fn index() -> BundleIndex {
    let zero = "0".repeat(64);
    BundleIndex::parse(
        format!(
            r#"{{"format":1,"file":"blocks.dat.z","size":1,"sha256":"{zero}",
            "versions":[{{"id":"1.20.1","data_version":3465}},{{"id":"26.2","data_version":4900}},
                        {{"id":"26.3","data_version":5023}}],
            "entities":{{"format":1,"file":"entities.dat.z","size":1,"sha256":"{zero}","layers":4,
              "versions":[{{"id":"1.20.1"}},{{"id":"26.2"}}]}}}}"#
        )
        .as_bytes(),
    )
    .unwrap()
}

fn with_client(version: &str) -> InstanceAssets {
    InstanceAssets {
        game_version: version.to_owned(),
        client: Some(InstalledClient {
            version: version.to_owned(),
            jar: PathBuf::from(format!("versions/{version}/{version}.jar")),
        }),
        ..InstanceAssets::default()
    }
}

/// D23: entity geometry is the bundle's or none; an installed 26.x client
/// the bake does not cover is never dumped here.
#[test]
fn entity_geometry_is_the_bundles_or_none() {
    let index = index();
    let online = BundleReach::Index(&index);
    let bundle = |version: &str| EntityGeometry::Bundle {
        version: version.to_owned(),
    };
    let cases = [
        (
            "26.2 is covered",
            with_client("26.2"),
            online,
            bundle("26.2"),
        ),
        (
            "26.3 is newer than the entity bake: none, and no dump",
            with_client("26.3"),
            online,
            EntityGeometry::None,
        ),
        (
            "26.3 offline: none",
            with_client("26.3"),
            BundleReach::Offline,
            EntityGeometry::None,
        ),
        (
            "download off: none",
            with_client("1.20.1"),
            BundleReach::Disabled,
            EntityGeometry::None,
        ),
        (
            "nothing installed: the bundle's version",
            InstanceAssets {
                game_version: "1.20.1".into(),
                ..InstanceAssets::default()
            },
            online,
            bundle("1.20.1"),
        ),
    ];
    for (case, instance, reach, expected) in cases {
        assert_eq!(resolve(&instance, None, reach).entities, expected, "{case}");
    }

    // A bake without entity geometry covers nothing.
    let mut bare = index.clone();
    bare.entities = None;
    let plan = resolve(&with_client("26.2"), None, BundleReach::Index(&bare));
    assert_eq!(plan.entities, EntityGeometry::None);
}

#[test]
fn an_entity_file_that_leaves_the_folder_is_refused() {
    let text = r#"{"format":1,"file":"blocks.dat.z","size":1,"sha256":"SHA",
        "versions":[{"id":"26.2","data_version":4900}],
        "entities":{"file":"../entities.dat.z","size":1,"sha256":"SHA","versions":[]}}"#
        .replace("SHA", &"0".repeat(64));
    assert!(BundleIndex::parse(text.as_bytes()).is_err());
}
