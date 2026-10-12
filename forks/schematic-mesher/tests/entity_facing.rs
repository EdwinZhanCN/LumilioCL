//! Regression test: block entities and mobs facing east or west turn to that side.
//!
//! Backstory: the entity facing angle used Minecraft's yaw sign with glam's
//! counter-clockwise rotation, so east and west were swapped. A chest facing
//! east showed its latch on the west, the two halves of an east- or west-facing
//! double chest sat back to front with their black insides open between them,
//! and wall banners hung on the wrong side of their block. This is a LumilioCL
//! local change (see `forks/README.md`).

mod common;

use common::Blocks;
use schematic_mesher::{BlockPosition, InputBlock, Mesher, ResourcePack};

/// Mean vertex position of one block's geometry, in block-local units.
fn centroid(block: InputBlock) -> [f32; 3] {
    let source = Blocks(vec![(BlockPosition::new(0, 0, 0), block)]);
    let out = Mesher::new(ResourcePack::new()).mesh(&source).unwrap();
    let mut sum = [0.0f32; 3];
    let mut n = 0.0;
    for layer in [&out.opaque_mesh, &out.cutout_mesh, &out.transparent_mesh] {
        for p in &layer.positions {
            for axis in 0..3 {
                sum[axis] += p[axis];
            }
            n += 1.0;
        }
    }
    assert!(n > 0.0, "no geometry");
    sum.map(|s| s / n)
}

fn facing(name: &str, facing: &str) -> InputBlock {
    InputBlock::new(name).with_property("facing", facing)
}

/// The latch is the only part off the chest's centre line, so it pulls the
/// centroid toward the side the chest faces.
#[test]
fn chests_face_their_facing() {
    for (dir, axis, sign) in [
        ("north", 2, -1.0),
        ("south", 2, 1.0),
        ("east", 0, 1.0),
        ("west", 0, -1.0),
    ] {
        let c = centroid(facing("minecraft:chest", dir));
        assert!(c[axis] * sign > 0.1, "{dir}: centroid {c:?}");
    }
}

/// A cow's head sticks out in front of its body.
#[test]
fn mobs_face_their_facing() {
    let east = centroid(facing("entity:cow", "east"));
    let west = centroid(facing("entity:cow", "west"));
    assert!(east[0] > 0.1, "east: {east:?}");
    assert!(west[0] < -0.1, "west: {west:?}");
}

/// A wall banner faces away from the wall it hangs on, so it sits on the
/// opposite side of its block.
#[test]
fn wall_banners_hang_on_the_wall_behind_them() {
    let east = centroid(facing("minecraft:white_wall_banner", "east"));
    let south = centroid(facing("minecraft:white_wall_banner", "south"));
    assert!(east[0] < -0.3, "east: {east:?}");
    assert!(south[2] < -0.3, "south: {south:?}");
}
