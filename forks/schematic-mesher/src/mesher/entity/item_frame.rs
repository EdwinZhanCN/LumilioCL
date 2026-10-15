//! Item frames: the game draws the frame with its own block model
//! (`minecraft:item_frame` with `map=false`), turned to the frame's facing.
//!
//! The face layout follows schematic-mesher's original `item_frame.rs`
//! (AGPL-3.0-only, Schem-at), which copied the model's numbers by hand; they
//! now come from the pack (LumilioCL local change, plan schematic-studio T7).
//! The turn is [`hanging_pose`], the steps Minecraft's renderers take for
//! entities hung on a block (LumilioCL local change, plan schematic-studio
//! T33, D32): the original turned the model to the opposite side of its
//! cell.

use std::collections::HashMap;

use glam::{Mat4, Vec3, Vec4};

use super::model::{EntityFaceTexture, Geometry};
use crate::mesher::geometry::Vertex;
use crate::resolver::{resolve_block, ModelResolver};
use crate::resource_pack::ResourcePack;
use crate::types::{Direction, InputBlock};

/// Where an entity hung on a block is drawn from, block-local (the block's
/// centre at 0.5), as Minecraft's item frame renderer places it: at the
/// centre of the block it hangs in, turned so that its model's +z side, the
/// side the game's models put against the wall, lies against the block it
/// hangs on. A horizontal facing turns about Y by 180° less the facing's yaw
/// (south 0, west 90, north 180, east 270); up and down turn about X by ∓90°
/// and then about Y by 180°. The model is drawn from its centre: its own
/// 0..1 space goes through `translate(-0.5)` first.
pub(crate) fn hanging_pose(facing: &str) -> Mat4 {
    let (about_x, about_y): (f32, f32) = match facing {
        "up" => (-90.0, 180.0),
        "down" => (90.0, 180.0),
        "west" => (0.0, 90.0),
        "north" => (0.0, 0.0),
        "east" => (0.0, -90.0),
        // South, and a facing the save does not give.
        _ => (0.0, 180.0),
    };
    Mat4::from_translation(Vec3::splat(0.5))
        * Mat4::from_rotation_x(about_x.to_radians())
        * Mat4::from_rotation_y(about_y.to_radians())
}

/// The frame of an item frame facing `facing`, block-local (0..1); empty
/// when the pack has no frame model.
pub(crate) fn frame_geometry(pack: &ResourcePack, glow: bool, facing: &str) -> Geometry {
    let mut out: Geometry = (Vec::new(), Vec::new(), Vec::new());
    let block = if glow {
        "minecraft:glow_item_frame"
    } else {
        "minecraft:item_frame"
    };
    let Ok(resolved) = resolve_block(pack, &InputBlock::new(block).with_property("map", "false"))
    else {
        return out;
    };
    let resolver = ModelResolver::new(pack);
    for model in &resolved {
        let textures = resolver.resolve_textures(&model.model);
        for element in &model.model.elements {
            for (&direction, face) in &element.faces {
                let texture = texture_of(&face.texture, &textures);
                let is_transparent = pack
                    .get_texture(&texture)
                    .is_some_and(|data| data.has_transparency());
                add_face(
                    element.from,
                    element.to,
                    direction,
                    face.uv
                        .unwrap_or_else(|| default_uv(element.from, element.to, direction)),
                    texture,
                    is_transparent,
                    &mut out,
                );
            }
        }
    }

    let place = hanging_pose(facing) * Mat4::from_translation(Vec3::splat(-0.5));
    for vertex in &mut out.0 {
        let [x, y, z] = vertex.position;
        vertex.position = (place * Vec4::new(x, y, z, 1.0)).truncate().into();
        let [x, y, z] = vertex.normal;
        vertex.normal = (place * Vec4::new(x, y, z, 0.0))
            .truncate()
            .normalize_or_zero()
            .into();
    }
    out
}

/// A face's texture id without the default namespace, as the atlas keys it.
fn texture_of(reference: &str, textures: &HashMap<String, String>) -> String {
    let id = match reference.strip_prefix('#') {
        Some(name) => textures
            .get(name)
            .cloned()
            .unwrap_or_else(|| "block/missing".to_owned()),
        None => reference.to_owned(),
    };
    id.strip_prefix("minecraft:").unwrap_or(&id).to_owned()
}

/// Minecraft's default face UVs: the element's extent seen from that side.
fn default_uv(from: [f32; 3], to: [f32; 3], direction: Direction) -> [f32; 4] {
    match direction {
        Direction::Down | Direction::Up => [from[0], from[2], to[0], to[2]],
        Direction::North | Direction::South => [from[0], 16.0 - to[1], to[0], 16.0 - from[1]],
        Direction::West | Direction::East => [from[2], 16.0 - to[1], to[2], 16.0 - from[1]],
    }
}

fn add_face(
    from: [f32; 3],
    to: [f32; 3],
    direction: Direction,
    uv: [f32; 4],
    texture: String,
    is_transparent: bool,
    (vertices, indices, faces): &mut Geometry,
) {
    let [x0, y0, z0] = from.map(|v| v / 16.0);
    let [x1, y1, z1] = to.map(|v| v / 16.0);
    let corners: [[f32; 3]; 8] = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
    ];
    let order = match direction {
        Direction::Down => [4, 5, 1, 0],
        Direction::Up => [3, 2, 6, 7],
        Direction::North => [1, 0, 3, 2],
        Direction::South => [4, 5, 6, 7],
        Direction::West => [0, 4, 7, 3],
        Direction::East => [5, 1, 2, 6],
    };
    let [u0, v0, u1, v1] = uv.map(|v| v / 16.0);
    // The block model convention: corners right-top, left-top, left-bottom, right-bottom.
    let uvs = [[u1, v0], [u0, v0], [u0, v1], [u1, v1]];
    let start = vertices.len() as u32;
    for (corner, uv) in order.into_iter().zip(uvs) {
        let normal = direction.normal();
        vertices.push(Vertex::new(corners[corner], normal, uv));
    }
    if matches!(direction, Direction::Down | Direction::Up) {
        indices.extend_from_slice(&[start, start + 2, start + 1, start, start + 3, start + 2]);
    } else {
        indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    faces.push(EntityFaceTexture {
        texture,
        is_transparent,
    });
}
