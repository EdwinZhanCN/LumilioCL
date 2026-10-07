//! Regression test: Minecraft 26.x element rotations parse and transform like
//! the game does.
//!
//! Backstory: 26.x writes rotations beyond ±45° as `{"x", "y", "z"}` Euler
//! angles (e.g. `template_hanging_sign_rot_3`). The mesher required `axis` and
//! `angle`, so the model failed to parse and every hanging sign using it went
//! missing. Once parsed, its faces also need normals that turn with them, or
//! they are lit from the wrong side. This is a LumilioCL local change (see
//! `forks/README.md`).

mod common;

use common::{add_block, add_texture, Blocks};
use glam::Vec3;
use schematic_mesher::{BlockPosition, InputBlock, Mesher, ModelElement, ResourcePack};

/// Every triangle's winding (the side the GPU draws) agrees with its normal.
#[test]
fn rotated_faces_keep_normals_on_their_front_side() {
    let mut pack = ResourcePack::new();
    add_texture(&mut pack, "block/board", false);
    let face = r##"{"texture":"#t"}"##;
    add_block(
        &mut pack,
        "board",
        &format!(
            r#"{{"textures":{{"t":"minecraft:block/board"}},"elements":[{{"from":[2,0,7],"to":[14,8,9],"rotation":{{"x":180,"y":-67.5,"z":-180,"origin":[8,0,8]}},"faces":{{"north":{face},"south":{face},"east":{face},"west":{face},"up":{face},"down":{face}}}}}]}}"#
        ),
    );
    let source = Blocks(vec![(
        BlockPosition::new(0, 0, 0),
        InputBlock::new("minecraft:board"),
    )]);
    let output = Mesher::new(pack).mesh(&source).unwrap();
    let layer = &output.opaque_mesh;
    assert!(!layer.indices.is_empty());
    for t in layer.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(layer.positions[i as usize]));
        let normal = Vec3::from(layer.normals[t[0] as usize]);
        assert!((b - a).cross(c - a).dot(normal) > 0.0, "triangle {t:?}");
    }
}

fn rotation(json: &str) -> glam::Mat3 {
    let element: ModelElement = serde_json::from_str(&format!(
        r#"{{"from":[0,0,0],"to":[16,16,16],"rotation":{json},"faces":{{}}}}"#
    ))
    .unwrap();
    element.rotation.unwrap().matrix()
}

#[test]
fn euler_rotation_parses_and_applies_x_then_y_then_z() {
    // From template_hanging_sign_rot_3 in the 26.3 client.
    let sign = rotation(r#"{"x": 180, "y": -67.5, "z": -180, "origin": [8, 0, 8]}"#);
    let expected = glam::Mat3::from_rotation_y(247.5f32.to_radians());
    assert!(sign.abs_diff_eq(expected, 1e-5), "{sign:?}");

    // Minecraft's rotationZYX(z, y, x) turns X first: +Y goes to +Z, then to +X.
    let order = rotation(r#"{"x": 90, "y": 90, "z": 0}"#);
    assert!((order * Vec3::Y).abs_diff_eq(Vec3::X, 1e-5), "{order:?}");
}

#[test]
fn single_axis_rotation_keeps_its_rescale() {
    let m = rotation(r#"{"origin": [8, 8, 8], "axis": "y", "angle": 45, "rescale": true}"#);
    // A rescaled 45° element reaches the block edge on both rotated axes.
    assert!(
        (m * Vec3::X).abs_diff_eq(Vec3::new(1.0, 0.0, -1.0), 1e-5),
        "{m:?}"
    );
    assert!((m * Vec3::Y).abs_diff_eq(Vec3::Y, 1e-5));

    let plain = rotation(r#"{"axis": "x", "angle": 22.5}"#);
    let expected = glam::Mat3::from_rotation_x(22.5f32.to_radians());
    assert!(plain.abs_diff_eq(expected, 1e-6));
}
