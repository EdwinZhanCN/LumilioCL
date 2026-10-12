use super::*;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn release_paths_cannot_escape_the_work_directory() {
    for invalid in [
        "",
        ".",
        "..",
        "1..2",
        "../26.3",
        "26.3-snapshot-1",
        "/tmp/26.3",
    ] {
        assert!(
            prepare::path(Path::new("/repo"), invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(
        prepare::path(Path::new("/repo"), "26.1.2").unwrap(),
        Path::new("/repo/target/worldgen/26.1.2")
    );
}

fn words(text: &str) -> Vec<u64> {
    text.split("0x")
        .skip(1)
        .filter_map(|v| {
            v.get(..16)
                .and_then(|hex| u64::from_str_radix(hex, 16).ok())
        })
        .collect()
}

#[test]
fn original_1_21_4_export_reconstructs_every_upstream_node() {
    let root = root();
    let input = read_tree(&root.join("forks/cubiomes/data/1.21.4/trees.json.gz")).unwrap();
    let ids = serde_json::from_str(include_str!("biome-ids.json")).unwrap();
    let encoded = btree::encode(&input["trees"]["overworld"], &ids, "btree21wd").unwrap();
    let baseline = std::fs::read_to_string(root.join("forks/cubiomes/tables/btree21wd.h")).unwrap();
    assert_eq!(words(&encoded), words(&baseline));
    assert_eq!(words(&encoded).len(), 9112);
    assert!(encoded.contains("{ 1555, 259, 43, 7, 1, 0 }"));
}

#[test]
fn checked_in_decoration_table_is_reproducible() {
    let root = root();
    assert_eq!(
        decorations::render(&root).unwrap(),
        std::fs::read_to_string(root.join("forks/cubiomes/tables/decorations.h")).unwrap()
    );
}

#[test]
fn decoration_modifiers_the_fork_cannot_evaluate_are_refused() {
    let ids = serde_json::from_str(include_str!("biome-ids.json")).unwrap();
    let release = |placement: Value| {
        vec![(
            "26.3".to_owned(),
            serde_json::json!({
                "dimensions": {"overworld": {"min_y": -64, "height": 384}},
                "features": [{"name": "fossil_upper", "dimension": "overworld",
                    "step": 3, "index": 0, "placement": placement, "biomes": ["desert"]}],
            }),
        )]
    };
    let table = decorations::table(
        &release(
            serde_json::json!([{"type": "minecraft:rarity_filter", "chance": 64},
            {"type": "minecraft:in_square"}, {"type": "minecraft:biome"}]),
        ),
        &ids,
    )
    .unwrap();
    assert!(table.contains("{ MC_26_3, DIM_OVERWORLD, Deco_Fossil_Upper, 30000, -64, 384,"));
    assert!(table.contains("{DP_RARITY, 64, 0, 0, 0}, {DP_IN_SQUARE, 0, 0, 0, 0}"));
    // A modifier that draws random numbers unknown to the fork would shift
    // every later draw: the generator stops instead of guessing.
    let unknown = decorations::table(
        &release(serde_json::json!([{"type": "minecraft:noise_based_count"}])),
        &ids,
    );
    assert!(unknown.unwrap_err().contains("noise_based_count"));
}

#[test]
fn checked_in_new_trees_are_reproducible_and_shared_only_when_equal() {
    let root = root();
    let ids = serde_json::from_str(include_str!("biome-ids.json")).unwrap();
    for (version, table) in [
        ("1.21.5", "btree215"),
        ("26.2", "btree26_2"),
        ("26.3", "btree26_3"),
    ] {
        let input =
            read_tree(&root.join(format!("forks/cubiomes/data/{version}/trees.json.gz"))).unwrap();
        let output = btree::encode(&input["trees"]["overworld"], &ids, table).unwrap();
        assert_eq!(
            output,
            std::fs::read_to_string(root.join(format!("forks/cubiomes/tables/{table}.h"))).unwrap()
        );
    }
    let first = read_tree(&root.join("forks/cubiomes/data/1.21.5/trees.json.gz")).unwrap();
    for version in [
        "1.21.6", "1.21.7", "1.21.8", "1.21.9", "1.21.10", "1.21.11", "26.1", "26.1.1", "26.1.2",
    ] {
        let next =
            read_tree(&root.join(format!("forks/cubiomes/data/{version}/trees.json.gz"))).unwrap();
        assert_eq!(
            first["trees"]["overworld"], next["trees"]["overworld"],
            "{version}"
        );
    }
}
