//! The binding table (`data/entity_bindings.json`) reads, and its rows pick
//! the layer and texture Minecraft's renderers use for each block and entity.
//! LumilioCL local change (see `forks/README.md`, plan schematic-studio T7).

use schematic_mesher::mesher::entity::bindings::{
    capture, find, table, TransformRef, BINDING_FORMAT,
};
use schematic_mesher::InputBlock;

fn layer_of(block: InputBlock) -> Option<String> {
    let bound = find(&block)?;
    let first = bound.binding.layers.first()?;
    bound.expand(first.layer())
}

fn texture_of(block: InputBlock) -> Option<String> {
    let bound = find(&block)?;
    bound.expand(&bound.binding.texture.as_ref()?.items()[0])
}

#[test]
fn the_table_reads() {
    let table = table();
    assert_eq!(table.format, BINDING_FORMAT);
    assert!(table.bindings.len() > 40);
    for binding in &table.bindings {
        if let Some(TransformRef::Named(name)) = &binding.transform {
            assert!(table.transforms.contains_key(name), "no transform {name}");
        }
    }
}

#[test]
fn patterns_capture_the_shortest_part_that_fits() {
    assert_eq!(
        capture("{color}_bed", "light_blue_bed"),
        Some(vec![("color".into(), "light_blue".into())])
    );
    assert_eq!(
        capture("{wood}_wall_hanging_sign", "dark_oak_wall_hanging_sign"),
        Some(vec![("wood".into(), "dark_oak".into())])
    );
    assert_eq!(capture("{color}_bed", "_bed"), None);
    assert_eq!(capture("chest", "trapped_chest"), None);
    assert_eq!(capture("chest", "chest"), Some(vec![]));
}

#[test]
fn chests_pick_their_half() {
    let chest = |kind: &str| InputBlock::new("minecraft:chest").with_property("type", kind);
    assert_eq!(
        layer_of(chest("left")).as_deref(),
        Some("minecraft:double_chest_left#main")
    );
    assert_eq!(
        texture_of(chest("right")).as_deref(),
        Some("entity/chest/normal_right")
    );
    assert_eq!(
        layer_of(chest("single")).as_deref(),
        Some("minecraft:chest#main")
    );
    assert_eq!(
        texture_of(InputBlock::new("minecraft:trapped_chest")).as_deref(),
        Some("entity/chest/trapped")
    );
    assert_eq!(
        texture_of(
            InputBlock::new("minecraft:waxed_weathered_copper_chest").with_property("type", "left")
        )
        .as_deref(),
        Some("entity/chest/copper_weathered_left")
    );
}

#[test]
fn beds_signs_and_skulls_pick_their_layer() {
    let bed = |part: &str| InputBlock::new("minecraft:light_blue_bed").with_property("part", part);
    assert_eq!(
        layer_of(bed("head")).as_deref(),
        Some("minecraft:bed_head#main")
    );
    assert_eq!(
        layer_of(bed("foot")).as_deref(),
        Some("minecraft:bed_foot#main")
    );
    assert_eq!(
        texture_of(bed("foot")).as_deref(),
        Some("entity/bed/light_blue")
    );
    for (block, layer) in [
        ("dark_oak_sign", "minecraft:sign/standing/dark_oak#main"),
        ("dark_oak_wall_sign", "minecraft:sign/wall/dark_oak#main"),
        ("wall_birch_sign", "minecraft:sign/wall/birch#main"),
        (
            "cherry_wall_hanging_sign",
            "minecraft:hanging_sign/cherry/wall#main",
        ),
        (
            "cherry_hanging_sign",
            "minecraft:hanging_sign/cherry/ceiling#main",
        ),
        ("zombie_wall_head", "minecraft:zombie_head#main"),
        ("dragon_head", "minecraft:dragon_skull#main"),
        ("red_wall_banner", "minecraft:wall_banner#main"),
        ("red_banner", "minecraft:standing_banner#main"),
        ("lime_shulker_box", "minecraft:shulker_box#main"),
    ] {
        assert_eq!(
            layer_of(InputBlock::new(format!("minecraft:{block}"))).as_deref(),
            Some(layer),
            "{block}"
        );
    }
    assert_eq!(
        layer_of(
            InputBlock::new("minecraft:cherry_hanging_sign").with_property("attached", "true")
        )
        .as_deref(),
        Some("minecraft:hanging_sign/cherry/ceiling_middle#main")
    );
}

#[test]
fn rows_with_conditions_are_skipped_when_they_fail() {
    assert!(find(&InputBlock::new("minecraft:lectern")).is_none());
    assert!(
        find(&InputBlock::new("minecraft:lectern").with_property("has_book", "true")).is_some()
    );
    assert!(find(&InputBlock::new("minecraft:stone")).is_none());
    assert!(find(&InputBlock::new("minecraft:oak_planks")).is_none());
}

#[test]
fn entities_and_blocks_do_not_mix() {
    assert!(find(&InputBlock::new("entity:chest")).is_none());
    assert!(find(&InputBlock::new("minecraft:zombie")).is_none());
    let zombie = find(&InputBlock::new("entity:zombie")).unwrap();
    assert!(zombie.is_entity);
    assert_eq!(
        layer_of(InputBlock::new("entity:cow").with_property("variant", "warm")).as_deref(),
        Some("minecraft:warm_cow#main")
    );
    assert_eq!(
        layer_of(InputBlock::new("entity:pale_oak_chest_boat")).as_deref(),
        Some("minecraft:chest_boat/pale_oak#main")
    );
    assert_eq!(
        layer_of(InputBlock::new("entity:boat")).as_deref(),
        Some("minecraft:boat/oak#main")
    );
    assert_eq!(
        layer_of(InputBlock::new("entity:hopper_minecart")).as_deref(),
        Some("minecraft:minecart#main")
    );
}

#[test]
fn variables_come_from_captures_then_properties_then_defaults() {
    let horse = find(&InputBlock::new("entity:horse").with_property("variant", "white")).unwrap();
    assert_eq!(horse.var("variant"), Some("white"));
    assert_eq!(horse.var("saddle"), Some("false"));
    let horse = find(&InputBlock::new("entity:horse")).unwrap();
    assert_eq!(horse.var("variant"), Some("brown"));
    assert_eq!(horse.expand("x/{missing}"), None);
}
