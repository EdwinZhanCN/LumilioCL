//! Regression test: a full cube whose texture has see-through pixels must not
//! hide the faces of its neighbours.
//!
//! Backstory: the culler classified any single full-cube model as opaque from
//! its geometry alone, so copper grates hid the stone behind them and all
//! leaves culled each other. Through the holes the viewer saw into hollow
//! blocks. This is a LumilioCL local change (see `forks/README.md`).
//!
//! It lives here rather than beside the culler because the crate's unit tests
//! do not compile at the adopted baseline.

mod common;

use common::cube_pack;
use schematic_mesher::mesher::face_culler::FaceCuller;
use schematic_mesher::{BlockPosition, Direction, InputBlock, ResourcePack};

fn culler_test(pack: &ResourcePack, blocks: &[(BlockPosition, &str)], check: impl Fn(&FaceCuller)) {
    let inputs: Vec<InputBlock> = blocks
        .iter()
        .map(|(_, name)| InputBlock::new(*name))
        .collect();
    let placed: Vec<(BlockPosition, &InputBlock)> = blocks
        .iter()
        .zip(&inputs)
        .map(|((pos, _), block)| (*pos, block))
        .collect();
    check(&FaceCuller::new(pack, &placed));
}

fn at(x: i32, y: i32, z: i32) -> BlockPosition {
    BlockPosition::new(x, y, z)
}

/// A full cube that no transparent group names is judged by its texture.
#[test]
fn see_through_cubes_keep_neighbour_faces() {
    let pack = cube_pack(&[("stone", false), ("mesh", true)]);
    let blocks = [
        (at(0, 0, 0), "minecraft:stone"),
        (at(1, 0, 0), "minecraft:stone"),
        (at(0, 0, -1), "minecraft:mesh"),
        (at(0, 0, -2), "minecraft:mesh"),
    ];
    culler_test(&pack, &blocks, |culler| {
        // Solid cubes still hide the faces between them.
        assert!(culler.should_cull(at(0, 0, 0), Direction::East));
        // The stone face behind the mesh shows through its holes.
        assert!(!culler.should_cull(at(0, 0, 0), Direction::North));
        // The mesh face pressed against stone is still hidden.
        assert!(culler.should_cull(at(0, 0, -1), Direction::South));
        // Two see-through cubes keep the faces between them.
        assert!(!culler.should_cull(at(0, 0, -1), Direction::North));
        // A see-through cube still darkens ambient occlusion but never occludes.
        assert!(culler.is_opaque_at(at(0, 0, -1)));
        assert!(!culler.is_fully_opaque_at(at(0, 0, -1)));
    });
}

/// Minecraft's fancy leaves draw the faces between neighbouring leaves.
#[test]
fn leaves_keep_the_faces_between_them() {
    let pack = cube_pack(&[("oak_leaves", true)]);
    let blocks = [
        (at(0, 0, 0), "minecraft:oak_leaves"),
        (at(1, 0, 0), "minecraft:oak_leaves"),
    ];
    culler_test(&pack, &blocks, |culler| {
        assert!(!culler.should_cull(at(0, 0, 0), Direction::East));
        assert!(!culler.should_cull(at(1, 0, 0), Direction::West));
    });
}

/// Copper grates behave like glass: they hide faces only against the same block.
#[test]
fn copper_grates_cull_only_their_own_block() {
    let pack = cube_pack(&[
        ("stone", false),
        ("copper_grate", true),
        ("exposed_copper_grate", true),
    ]);
    let blocks = [
        (at(0, 0, 0), "minecraft:stone"),
        (at(0, 0, -1), "minecraft:copper_grate"),
        (at(1, 0, -1), "minecraft:copper_grate"),
        (at(2, 0, -1), "minecraft:exposed_copper_grate"),
    ];
    culler_test(&pack, &blocks, |culler| {
        // The stone face behind a grate shows through its holes.
        assert!(!culler.should_cull(at(0, 0, 0), Direction::North));
        assert!(culler.should_cull(at(0, 0, -1), Direction::South));
        // The same grate hides the face it shares; another oxidation stage does not.
        assert!(culler.should_cull(at(0, 0, -1), Direction::East));
        assert!(!culler.should_cull(at(1, 0, -1), Direction::East));
    });
}
