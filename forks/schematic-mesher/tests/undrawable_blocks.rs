//! Regression test: blocks the pack cannot draw are reported, not only logged.
//!
//! Backstory: a block whose state or model failed to resolve (a renamed ID such
//! as `minecraft:chain`, a model in a newer format) was skipped with one stderr
//! line, so the preview silently lost blocks. This is a LumilioCL local change
//! (see `forks/README.md`).

mod common;

use common::cube_pack;
use schematic_mesher::{undrawable_blocks, InputBlock};

#[test]
fn only_blocks_nothing_can_draw_are_reported() {
    let pack = cube_pack(&[("stone", false)]);
    let blocks = [
        InputBlock::new("minecraft:stone"),
        InputBlock::new("minecraft:chain"),
        InputBlock::new("minecraft:chain"),
        InputBlock::new("minecraft:air"),
        // Drawn by fluid and block-entity geometry without a block model.
        InputBlock::new("minecraft:water"),
        InputBlock::new("minecraft:chest"),
    ];
    assert_eq!(undrawable_blocks(&pack, &blocks), ["minecraft:chain"]);
}
