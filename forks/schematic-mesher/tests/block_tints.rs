//! Regression test: blocks take Minecraft's block colors.
//!
//! Backstory: the tint table guessed from block names. Every `*_leaves` took the
//! plains foliage green, so pink cherry and grey pale oak leaves were dyed green
//! while spruce and birch lost their own colors; the bush, leaf litter and the
//! stems of pink petals and wildflowers were left untinted and drawn grey; the
//! redstone and stem colors were approximations. The table now follows the color
//! providers Minecraft registers. This is a LumilioCL local change (see
//! `forks/README.md`).

use schematic_mesher::{InputBlock, TintProvider};

fn tint(name: &str, tint_index: i32) -> [u8; 3] {
    let c = TintProvider::new().get_tint(&InputBlock::new(name), tint_index);
    [0, 1, 2].map(|i| (c[i] * 255.0).round() as u8)
}

fn hex(rgb: u32) -> [u8; 3] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

#[test]
fn leaves_take_their_own_colors() {
    assert_eq!(tint("minecraft:oak_leaves", 0), hex(0x77AB2F));
    assert_eq!(tint("minecraft:spruce_leaves", 0), hex(0x619961));
    assert_eq!(tint("minecraft:birch_leaves", 0), hex(0x80A755));
    assert_eq!(tint("minecraft:mangrove_leaves", 0), hex(0x92C648));
    for untinted in [
        "cherry_leaves",
        "pale_oak_leaves",
        "azalea_leaves",
        "bamboo",
    ] {
        assert_eq!(
            tint(&format!("minecraft:{untinted}"), 0),
            [255; 3],
            "{untinted}"
        );
    }
}

#[test]
fn grass_colored_plants_include_bushes_and_flower_stems() {
    let grass = hex(0x91BD59);
    for name in ["grass_block", "short_grass", "bush", "sugar_cane"] {
        assert_eq!(tint(&format!("minecraft:{name}"), 0), grass, "{name}");
    }
    // Pink petals and wildflowers tint only their stems (tint index 1).
    assert_eq!(tint("minecraft:pink_petals", 1), grass);
    assert_eq!(tint("minecraft:wildflowers", 1), grass);
    assert_eq!(tint("minecraft:wildflowers", 0), [255; 3]);
    assert_eq!(tint("minecraft:leaf_litter", 0), hex(0xA37546));
}

#[test]
fn redstone_and_stems_use_minecraft_formulas() {
    let wire = |power: &str| {
        let block = InputBlock::new("minecraft:redstone_wire").with_property("power", power);
        let c = TintProvider::new().get_tint(&block, 0);
        [0, 1, 2].map(|i| (c[i] * 255.0).round() as u8)
    };
    assert_eq!(wire("0"), [77, 0, 0]);
    assert_eq!(wire("15"), [255, 51, 0]);
    let stem = |age: &str| {
        let block = InputBlock::new("minecraft:melon_stem").with_property("age", age);
        let c = TintProvider::new().get_tint(&block, 0);
        [0, 1, 2].map(|i| (c[i] * 255.0).round() as u8)
    };
    assert_eq!(stem("7"), [224, 199, 28]);
    assert_eq!(tint("minecraft:attached_pumpkin_stem", 0), hex(0xE0C71C));
    assert_eq!(tint("minecraft:lily_pad", 0), hex(0x208030));
}
