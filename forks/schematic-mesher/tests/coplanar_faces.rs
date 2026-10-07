//! Regression test: overlapping faces of one block that share a plane and a
//! facing are separated in the order the model draws them.
//!
//! Backstory: grass side overlays, redstone line and overlay, and multipart
//! pieces put several faces in one plane. Minecraft relies on draw order; the
//! native renderer saw them at equal depth and they flickered (Z-fighting).
//! This is a LumilioCL local change (see `forks/README.md`).

mod common;

use common::{add_block, add_texture, Blocks};
use schematic_mesher::{BlockPosition, InputBlock, Mesher, ResourcePack};

/// Distinct Z of every north-facing vertex for a block made of `elements`.
fn north_planes(elements: &str) -> Vec<f32> {
    let mut pack = ResourcePack::new();
    add_texture(&mut pack, "block/base", false);
    add_texture(&mut pack, "block/overlay", false);
    add_block(
        &mut pack,
        "layered",
        &format!(
            r#"{{"textures":{{"base":"minecraft:block/base","overlay":"minecraft:block/overlay"}},"elements":{elements}}}"#
        ),
    );
    let source = Blocks(vec![(
        BlockPosition::new(0, 0, 0),
        InputBlock::new("minecraft:layered"),
    )]);
    let output = Mesher::new(pack).mesh(&source).unwrap();
    let layer = &output.opaque_mesh;
    let mut planes: Vec<f32> = layer
        .positions
        .iter()
        .zip(&layer.normals)
        .filter(|(_, n)| n[2] < -0.9)
        .map(|(p, _)| p[2])
        .collect();
    planes.sort_by(f32::total_cmp);
    planes.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    planes
}

#[test]
fn a_later_overlapping_face_moves_out_by_one_step() {
    let planes = north_planes(
        r##"[
            {"from":[0,0,0],"to":[16,16,16],"faces":{"north":{"texture":"#base"}}},
            {"from":[0,8,0],"to":[16,16,16],"faces":{"north":{"texture":"#overlay"}}}
        ]"##,
    );
    // The base stays on the block face; the overlay, drawn second, moves out
    // in front of it (north is -Z).
    assert_eq!(planes.len(), 2, "{planes:?}");
    assert!((planes[1] + 0.5).abs() < 1e-6, "{planes:?}");
    assert!((planes[0] + 0.5 + 1.0 / 1024.0).abs() < 1e-6, "{planes:?}");
}

#[test]
fn faces_that_only_touch_stay_in_place() {
    let planes = north_planes(
        r##"[
            {"from":[0,0,0],"to":[8,16,16],"faces":{"north":{"texture":"#base"}}},
            {"from":[8,0,0],"to":[16,16,16],"faces":{"north":{"texture":"#overlay"}}}
        ]"##,
    );
    assert_eq!(planes.len(), 1, "{planes:?}");
}
