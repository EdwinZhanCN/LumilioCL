//! Paintings (LumilioCL local change, plan schematic-studio T35): the faces
//! Minecraft's `PaintingRenderer` draws, sized by the variant. The picture
//! is the front, the `painting/back` texture the back, and a one-pixel strip
//! of it each edge, all 1/16 block thick about the entity's centre. The row
//! turns them by the facing.

use glam::{Mat4, Vec3, Vec4};

use super::bindings::Bound;
use super::model::{EntityFaceTexture, Geometry};
use crate::mesher::geometry::Vertex;
use crate::resource_pack::ResourcePack;

const BACK: &str = "minecraft:painting/back";
/// Half the thickness: the faces lie at ±1/32 block.
const HALF_DEPTH: f32 = 1.0 / 32.0;

/// The faces of the `variant` painting, in model space (centred on the
/// entity, the picture toward -Z), placed by `place`. `None` for a variant
/// the version does not have, which the game draws nothing for.
pub fn geometry(pack: &ResourcePack, bound: &Bound, place: Mat4) -> Option<Geometry> {
    let variant = bound.var("variant")?;
    let id = if variant.contains(':') {
        variant.to_owned()
    } else {
        format!("minecraft:{variant}")
    };
    let painting = pack.paintings.get(&id)?;
    let (w, h) = (painting.width as f32, painting.height as f32);
    let mut out: Geometry = (Vec::new(), Vec::new(), Vec::new());
    let (left, bottom) = (-w / 2.0, -h / 2.0);
    // The picture, seen from -Z: +X is its left, so U runs against X.
    quad(
        &mut out,
        place,
        [
            Vec3::new(-left, bottom, -HALF_DEPTH),
            Vec3::new(left, bottom, -HALF_DEPTH),
            Vec3::new(left, -bottom, -HALF_DEPTH),
            Vec3::new(-left, -bottom, -HALF_DEPTH),
        ],
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        Vec3::NEG_Z,
        &painting.texture,
    );
    let strip = 1.0 / 16.0;
    for i in 0..painting.width {
        for j in 0..painting.height {
            let x0 = left + i as f32;
            let (x1, y0) = (x0 + 1.0, bottom + j as f32);
            let y1 = y0 + 1.0;
            // The whole back texture on every block of the back.
            quad(
                &mut out,
                place,
                [
                    Vec3::new(x0, y0, HALF_DEPTH),
                    Vec3::new(x1, y0, HALF_DEPTH),
                    Vec3::new(x1, y1, HALF_DEPTH),
                    Vec3::new(x0, y1, HALF_DEPTH),
                ],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                Vec3::Z,
                BACK,
            );
            let across = [[0.0, 0.0], [1.0, 0.0], [1.0, strip], [0.0, strip]];
            let down = [[0.0, 0.0], [strip, 0.0], [strip, 1.0], [0.0, 1.0]];
            if j + 1 == painting.height {
                quad(
                    &mut out,
                    place,
                    [
                        Vec3::new(x0, y1, -HALF_DEPTH),
                        Vec3::new(x1, y1, -HALF_DEPTH),
                        Vec3::new(x1, y1, HALF_DEPTH),
                        Vec3::new(x0, y1, HALF_DEPTH),
                    ],
                    across,
                    Vec3::Y,
                    BACK,
                );
            }
            if j == 0 {
                quad(
                    &mut out,
                    place,
                    [
                        Vec3::new(x0, y0, -HALF_DEPTH),
                        Vec3::new(x1, y0, -HALF_DEPTH),
                        Vec3::new(x1, y0, HALF_DEPTH),
                        Vec3::new(x0, y0, HALF_DEPTH),
                    ],
                    across,
                    Vec3::NEG_Y,
                    BACK,
                );
            }
            if i + 1 == painting.width {
                quad(
                    &mut out,
                    place,
                    [
                        Vec3::new(x1, y0, -HALF_DEPTH),
                        Vec3::new(x1, y0, HALF_DEPTH),
                        Vec3::new(x1, y1, HALF_DEPTH),
                        Vec3::new(x1, y1, -HALF_DEPTH),
                    ],
                    down,
                    Vec3::X,
                    BACK,
                );
            }
            if i == 0 {
                quad(
                    &mut out,
                    place,
                    [
                        Vec3::new(x0, y0, -HALF_DEPTH),
                        Vec3::new(x0, y0, HALF_DEPTH),
                        Vec3::new(x0, y1, HALF_DEPTH),
                        Vec3::new(x0, y1, -HALF_DEPTH),
                    ],
                    down,
                    Vec3::NEG_X,
                    BACK,
                );
            }
        }
    }
    Some(out)
}

/// One opaque face through `place`, wound counter-clockwise seen from the
/// side `normal` points to.
fn quad(
    out: &mut Geometry,
    place: Mat4,
    corners: [Vec3; 4],
    uvs: [[f32; 2]; 4],
    normal: Vec3,
    texture: &str,
) {
    face(out, place, corners, uvs, normal, texture, false);
}

/// One face through `place`, wound counter-clockwise seen from the side
/// `normal` points to; `transparent` sends it to the blended pass.
pub(crate) fn face(
    out: &mut Geometry,
    place: Mat4,
    corners: [Vec3; 4],
    mut uvs: [[f32; 2]; 4],
    normal: Vec3,
    texture: &str,
    transparent: bool,
) {
    let mut placed = corners.map(|corner| (place * corner.extend(1.0)).truncate());
    let normal = (place * Vec4::new(normal.x, normal.y, normal.z, 0.0))
        .truncate()
        .normalize_or_zero();
    if (placed[1] - placed[0])
        .cross(placed[2] - placed[0])
        .dot(normal)
        < 0.0
    {
        placed.swap(1, 3);
        uvs.swap(1, 3);
    }
    let (vertices, indices, faces) = out;
    let start = vertices.len() as u32;
    for (position, uv) in placed.iter().zip(uvs) {
        vertices.push(Vertex::new((*position).into(), normal.into(), uv));
    }
    indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    faces.push(EntityFaceTexture {
        texture: texture.to_owned(),
        is_transparent: transparent,
    });
}

/// A flat picture facing +Z between `from` and `to` (blocks), the texture's
/// `uv` rectangle (`[u0, v0, u1, v1]`) on it, placed by `place` (T35).
pub(crate) fn flat(
    out: &mut Geometry,
    place: Mat4,
    from: [f32; 2],
    to: [f32; 2],
    uv: [f32; 4],
    texture: &str,
    transparent: bool,
) {
    let [u0, v0, u1, v1] = uv;
    face(
        out,
        place,
        [
            Vec3::new(from[0], from[1], 0.0),
            Vec3::new(to[0], from[1], 0.0),
            Vec3::new(to[0], to[1], 0.0),
            Vec3::new(from[0], to[1], 0.0),
        ],
        [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
        Vec3::Z,
        texture,
        transparent,
    );
}
