use std::path::PathBuf;

use crate::{BakedPack, Bundle, icon_item, icon_tint, table_items};

#[test]
fn a_block_is_its_own_item_unless_the_table_says_otherwise() {
    assert_eq!(icon_item("minecraft:stone"), "minecraft:item/stone");
    assert_eq!(
        icon_item("minecraft:oak_stairs[facing=east,half=top]"),
        "minecraft:item/oak_stairs"
    );
    assert_eq!(icon_item("minecraft:wall_torch"), "minecraft:item/torch");
    assert_eq!(icon_item("redstone_wire"), "minecraft:item/redstone");
    assert_eq!(
        icon_item("minecraft:spruce_wall_hanging_sign"),
        "minecraft:item/spruce_hanging_sign"
    );
    assert_eq!(
        icon_item("minecraft:dead_brain_coral_wall_fan"),
        "minecraft:item/dead_brain_coral_fan"
    );
    assert_eq!(
        icon_item("minecraft:potted_red_tulip"),
        "minecraft:item/flower_pot"
    );
    assert_eq!(
        icon_item("farmersdelight:stove"),
        "farmersdelight:item/stove"
    );
    assert_eq!(icon_tint("minecraft:vine[up=true]"), Some("foliage"));
    assert_eq!(icon_tint("minecraft:stone"), None);
}

/// Table lint against the newest real version in the baked bundle (built by
/// `cargo xtask bake-blocks`; without it the test says so and passes).
#[test]
fn every_item_the_table_names_has_a_model_in_the_newest_bundle() {
    let path = std::env::var_os("LUMILIO_BLOCK_BUNDLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/block-assets/blocks.dat.z")
        });
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!(
            "skipped: no baked bundle at {} (cargo xtask bake-blocks)",
            path.display()
        );
        return;
    };
    let pack: BakedPack = Bundle::read(&bytes).unwrap().pack("26.3").unwrap();
    let missing: Vec<String> = table_items()
        .into_iter()
        .filter(|item| {
            !pack
                .assets
                .models
                .contains_key(&format!("minecraft:item/{item}"))
        })
        .collect();
    assert!(missing.is_empty(), "no item model for {missing:?}");
}
