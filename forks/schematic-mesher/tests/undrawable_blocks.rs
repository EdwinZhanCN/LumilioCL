//! Regression test: blocks the pack cannot draw are reported, not only logged.
//!
//! Backstory: a block whose state or model failed to resolve (a renamed ID such
//! as `minecraft:chain`, a model in a newer format) was skipped with one stderr
//! line, so the preview silently lost blocks. This is a LumilioCL local change
//! (see `forks/README.md`).

mod common;

use common::{cube_pack, entity_layers};
use schematic_mesher::{undrawable_blocks, InputBlock};

#[test]
fn only_blocks_nothing_can_draw_are_reported() {
    let pack = cube_pack(&[("stone", false)])
        .bake()
        .with_entity_layers(entity_layers());
    let blocks = [
        InputBlock::new("minecraft:stone"),
        InputBlock::new("minecraft:chain"),
        InputBlock::new("minecraft:chain"),
        InputBlock::new("minecraft:air"),
        // Drawn by fluid and block-entity geometry without a block model.
        InputBlock::new("minecraft:water"),
        InputBlock::new("minecraft:chest"),
        InputBlock::new("entity:zombie"),
        // The handed-in geometry has no spider layer.
        InputBlock::new("entity:spider"),
    ];
    assert_eq!(
        undrawable_blocks(&pack, &blocks),
        ["entity:spider", "minecraft:chain"]
    );
}

/// Without entity geometry handed in, bound entities cannot be drawn and say so.
#[test]
fn entities_without_geometry_are_reported() {
    let pack = cube_pack(&[("stone", false)]).bake();
    let blocks = [
        InputBlock::new("entity:zombie"),
        InputBlock::new("minecraft:chest"),
    ];
    assert_eq!(
        undrawable_blocks(&pack, &blocks),
        ["entity:zombie", "minecraft:chest"]
    );
}

/// A block nothing can draw is still meshed: one missing-texture cube, so the
/// shape stays whole and the block can be picked (plan schematic-studio D24).
#[test]
fn an_undrawable_block_is_drawn_as_a_missing_texture_cube() {
    use common::Blocks;
    use schematic_mesher::{BlockPosition, Mesher};
    let pack = cube_pack(&[("stone", false)]).bake();
    let source = Blocks(vec![
        (
            BlockPosition::new(0, 0, 0),
            InputBlock::new("minecraft:stone"),
        ),
        (
            BlockPosition::new(0, 0, 2),
            InputBlock::new("minecraft:chain"),
        ),
    ]);
    let output = Mesher::new(pack).mesh(&source).unwrap();
    // Two cubes apart: six faces of four vertices each.
    assert_eq!(output.total_vertices(), 48);
}
