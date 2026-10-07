//! Regression test: signs and beds with Minecraft 26.2+ block models are drawn
//! from the model alone.
//!
//! Backstory: 26.2 turned signs and beds into ordinary block models and removed
//! their `entity/signs/*` and `entity/bed/*` textures. The mesher still added
//! its legacy entity geometry on top, which showed as a missing-texture copy.
//! Packs whose sign and bed models have no elements (older versions) keep the
//! entity path. This is a LumilioCL local change (see `forks/README.md`).

mod common;

use common::{add_block, add_texture, cube_model, Blocks};
use schematic_mesher::{BlockPosition, InputBlock, Mesher, ResourcePack};

const BLOCKS: [&str; 4] = [
    "minecraft:oak_sign",
    "minecraft:oak_wall_hanging_sign",
    "minecraft:red_bed",
    "minecraft:oak_hanging_sign",
];

fn vertices(pack: ResourcePack, block: &str) -> usize {
    let source = Blocks(vec![(BlockPosition::new(0, 0, 0), InputBlock::new(block))]);
    Mesher::new(pack).mesh(&source).unwrap().total_vertices()
}

#[test]
fn block_models_replace_legacy_sign_and_bed_entities() {
    for block in BLOCKS {
        let id = block.trim_start_matches("minecraft:");
        let mut pack = ResourcePack::new();
        add_block(&mut pack, id, &cube_model(&format!("minecraft:block/{id}")));
        add_texture(&mut pack, &format!("block/{id}"), false);
        // One cube: six faces of four vertices, nothing from the entity.
        assert_eq!(vertices(pack, block), 24, "{block}");
    }
}

#[test]
fn models_without_elements_keep_the_entity() {
    for block in BLOCKS {
        let id = block.trim_start_matches("minecraft:");
        let mut pack = ResourcePack::new();
        add_block(
            &mut pack,
            id,
            r#"{"textures":{"particle":"minecraft:block/oak_planks"}}"#,
        );
        assert!(vertices(pack, block) > 0, "{block}");
    }
}
