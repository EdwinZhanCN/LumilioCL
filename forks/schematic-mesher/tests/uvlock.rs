//! Regression test: a `uvlock` variant keeps its textures aligned to the world.
//!
//! Backstory: the mesher parsed `uvlock` but never applied it, so the textures
//! of rotated stairs, walls, fences, fence gates and buttons turned with the
//! block — rotated plank grain and brick courses on every stair that does not
//! face north. This is a LumilioCL local change (see `forks/README.md`).

mod common;

use std::collections::HashMap;

use common::{add_texture, cube_model, Blocks};
use schematic_mesher::{BlockPosition, InputBlock, Mesher, ResourcePack};

fn pack(x: i32, y: i32, uvlock: bool) -> ResourcePack {
    let mut pack = ResourcePack::new();
    let blockstate = serde_json::from_str(&format!(
        r#"{{"variants":{{"":{{"model":"minecraft:block/cube","x":{x},"y":{y},"uvlock":{uvlock}}}}}}}"#
    ))
    .unwrap();
    pack.add_blockstate("minecraft", "cube", blockstate);
    let model = serde_json::from_str(&cube_model("minecraft:block/cube")).unwrap();
    pack.add_model("minecraft", "block/cube", model);
    add_texture(&mut pack, "block/cube", false);
    pack
}

/// UV of every vertex, keyed by its face normal and position (rounded).
fn uv_by_corner(pack: ResourcePack) -> HashMap<[i32; 6], [f32; 2]> {
    let source = Blocks(vec![(BlockPosition::new(0, 0, 0), InputBlock::new("minecraft:cube"))]);
    let out = Mesher::new(pack).mesh(&source).unwrap();
    let layer = &out.opaque_mesh;
    let round = |v: f32| (v * 1000.0).round() as i32;
    (0..layer.positions.len())
        .map(|i| {
            let (n, p) = (layer.normals[i], layer.positions[i]);
            (
                [round(n[0]), round(n[1]), round(n[2]), round(p[0]), round(p[1]), round(p[2])],
                layer.uvs[i],
            )
        })
        .collect()
}

fn same(a: &HashMap<[i32; 6], [f32; 2]>, b: &HashMap<[i32; 6], [f32; 2]>) -> bool {
    a.len() == b.len()
        && a.iter().all(|(key, uv)| {
            b.get(key)
                .is_some_and(|other| (uv[0] - other[0]).abs() < 1e-4 && (uv[1] - other[1]).abs() < 1e-4)
        })
}

/// A locked cube looks exactly like an unrotated one, whatever the rotation.
#[test]
fn locked_cubes_keep_world_aligned_textures() {
    let unrotated = uv_by_corner(pack(0, 0, false));
    assert_eq!(unrotated.len(), 24);
    for (x, y) in [(0, 90), (0, 180), (0, 270), (180, 0), (90, 270), (270, 90), (180, 90)] {
        assert!(same(&unrotated, &uv_by_corner(pack(x, y, true))), "x={x} y={y}");
    }
}

/// Without the lock the texture turns with the block (and the test can tell).
#[test]
fn unlocked_cubes_turn_their_textures() {
    let unrotated = uv_by_corner(pack(0, 0, false));
    assert!(!same(&unrotated, &uv_by_corner(pack(0, 90, false))));
    assert!(!same(&unrotated, &uv_by_corner(pack(180, 0, false))));
}
