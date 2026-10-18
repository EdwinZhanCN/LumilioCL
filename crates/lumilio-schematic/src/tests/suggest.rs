use crate::suggest::suggest;

/// A cut-down 1.18 block table: no cherry, no mud, no copper bulbs.
const KNOWN: &[&str] = &[
    "minecraft:stone",
    "minecraft:iron_ore",
    "minecraft:coal_ore",
    "minecraft:oak_stairs",
    "minecraft:stone_stairs",
    "minecraft:birch_stairs",
    "minecraft:brick_stairs",
    "minecraft:stone_brick_stairs",
    "minecraft:oak_slab",
    "minecraft:stone_slab",
    "minecraft:cobblestone_wall",
    "minecraft:brick_wall",
    "minecraft:oak_sign",
    "minecraft:oak_wall_sign",
    "minecraft:oak_leaves",
    "minecraft:oak_log",
    "minecraft:copper_block",
    "minecraft:oxidized_copper",
    "minecraft:redstone_lamp",
    "create:brick_stairs",
];

fn known() -> impl Iterator<Item = &'static str> {
    KNOWN.iter().copied()
}

#[test]
fn a_variant_prefix_falls_back_to_the_plain_block() {
    assert_eq!(
        suggest("minecraft:deepslate_iron_ore", known()).as_deref(),
        Some("minecraft:iron_ore")
    );
    // Prefixes come off one at a time until a block is known.
    assert_eq!(
        suggest("minecraft:waxed_oxidized_copper", known()).as_deref(),
        Some("minecraft:oxidized_copper")
    );
    assert_eq!(
        suggest("minecraft:deepslate_brick_stairs", known()).as_deref(),
        Some("minecraft:brick_stairs"),
        "the vanilla block, not the mod's of the same path"
    );
}

#[test]
fn a_shape_keeps_its_suffix_and_the_closest_material() {
    assert_eq!(
        suggest("minecraft:mud_brick_stairs", known()).as_deref(),
        Some("minecraft:brick_stairs"),
        "shares `brick`, the shorter of two"
    );
    assert_eq!(
        suggest("minecraft:mud_brick_wall", known()).as_deref(),
        Some("minecraft:brick_wall")
    );
    // No word in common: the plain material of that shape.
    assert_eq!(
        suggest("minecraft:cherry_stairs", known()).as_deref(),
        Some("minecraft:oak_stairs")
    );
    assert_eq!(
        suggest("minecraft:cherry_slab", known()).as_deref(),
        Some("minecraft:oak_slab")
    );
    // A wall sign is not a sign, nor a wall.
    assert_eq!(
        suggest("minecraft:cherry_wall_sign", known()).as_deref(),
        Some("minecraft:oak_wall_sign")
    );
    assert_eq!(
        suggest("minecraft:cherry_leaves", known()).as_deref(),
        Some("minecraft:oak_leaves")
    );
}

#[test]
fn without_a_shape_the_most_shared_words_win() {
    assert_eq!(
        suggest("minecraft:copper_bulb", known()).as_deref(),
        Some("minecraft:copper_block")
    );
    assert_eq!(suggest("minecraft:sculk_sensor", known()), None);
}

#[test]
fn vanilla_is_preferred_and_a_mod_block_maps_to_vanilla() {
    assert_eq!(
        suggest("othermod:redstone_lamp", known()).as_deref(),
        Some("minecraft:redstone_lamp")
    );
    let only_mod = ["create:brick_stairs", "minecraft:brick_stairs"];
    assert_eq!(
        suggest("minecraft:mud_brick_stairs", only_mod).as_deref(),
        Some("minecraft:brick_stairs")
    );
}

#[test]
fn the_missing_block_itself_is_never_proposed() {
    assert_eq!(
        suggest("minecraft:cherry_log", ["minecraft:cherry_log"]),
        None
    );
}
