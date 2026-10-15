//! Box models and their faces: the shape every entity layer is drawn from.
//!
//! The box unwrap and face order follow schematic-mesher's original entity
//! module (`src/mesher/entity/mod.rs`, AGPL-3.0-only, Schem-at); the parts
//! now come from the game's own layer definitions (`layers.rs`) instead of
//! hand-copied files (LumilioCL local change, plan schematic-studio T7).

use glam::{Mat4, Vec3, Vec4};

use crate::mesher::geometry::Vertex;
use crate::types::Direction;

/// A box of a part, in 1/16 block, in the part's space.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityCube {
    /// The corner with the smallest coordinates, before `inflate`.
    pub origin: [f32; 3],
    /// Size along X, Y and Z.
    pub dimensions: [f32; 3],
    /// Texture offset in pixels of the layer's sheet.
    pub tex_offset: [f32; 2],
    /// Scale of the box unwrap on the sheet.
    pub uv_scale: [f32; 2],
    /// Growth on each side along X, Y and Z.
    pub inflate: [f32; 3],
    pub mirror: bool,
    /// Faces not drawn.
    pub skip_faces: Vec<Direction>,
}

/// Translation (1/16 block), rotation (radians, applied Z, Y, X) and scale.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityPartPose {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for EntityPartPose {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

impl EntityPartPose {
    /// The part's matrix in its parent, in blocks.
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_translation(Vec3::from(self.position) / 16.0)
            * Mat4::from_rotation_z(self.rotation[2])
            * Mat4::from_rotation_y(self.rotation[1])
            * Mat4::from_rotation_x(self.rotation[0])
            * Mat4::from_scale(Vec3::from(self.scale))
    }
}

/// A part of a model, with its boxes and children.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityPart {
    pub name: String,
    pub cubes: Vec<EntityCube>,
    pub pose: EntityPartPose,
    pub children: Vec<EntityPart>,
    /// A texture for this part and its children instead of the model's.
    pub texture: Option<String>,
}

/// One drawable model: parts on one texture sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityModelDef {
    /// Texture key, such as `entity/chest/normal` or a composed `_banner/…`.
    pub texture_path: String,
    /// The sheet size the texture offsets are in.
    pub texture_size: [u32; 2],
    pub parts: Vec<EntityPart>,
    /// False when see-through pixels show, so faces go to the blended pass.
    pub is_opaque: bool,
}

/// The texture of one generated face.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityFaceTexture {
    pub texture: String,
    pub is_transparent: bool,
}

/// Vertices, indices (6 per face) and one texture per face.
pub type Geometry = (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>);

/// Adds `model`'s faces under `transform` (blocks, block-local) to `out`.
pub fn emit(model: &EntityModelDef, transform: Mat4, out: &mut Geometry) {
    for part in &model.parts {
        emit_part(part, transform, &model.texture_path, model, out);
    }
}

fn emit_part(
    part: &EntityPart,
    parent: Mat4,
    texture: &str,
    model: &EntityModelDef,
    out: &mut Geometry,
) {
    let transform = parent * part.pose.matrix();
    let texture = part.texture.as_deref().unwrap_or(texture);
    for cube in &part.cubes {
        emit_cube(cube, transform, texture, model, out);
    }
    for child in &part.children {
        emit_part(child, transform, texture, model, out);
    }
}

/// The six faces as corner indices into a box's corners, with their direction.
const FACES: [(Direction, [usize; 4]); 6] = [
    (Direction::Down, [4, 5, 1, 0]),
    (Direction::Up, [3, 2, 6, 7]),
    (Direction::North, [1, 0, 3, 2]),
    (Direction::South, [4, 5, 6, 7]),
    (Direction::West, [0, 4, 7, 3]),
    (Direction::East, [5, 1, 2, 6]),
];

fn emit_cube(
    cube: &EntityCube,
    transform: Mat4,
    texture: &str,
    model: &EntityModelDef,
    out: &mut Geometry,
) {
    let low: [f32; 3] = std::array::from_fn(|i| (cube.origin[i] - cube.inflate[i]) / 16.0);
    let high: [f32; 3] =
        std::array::from_fn(|i| (cube.origin[i] + cube.dimensions[i] + cube.inflate[i]) / 16.0);
    let [x0, y0, z0] = low;
    let [x1, y1, z1] = high;
    let corners = [
        Vec3::new(x0, y0, z0),
        Vec3::new(x1, y0, z0),
        Vec3::new(x1, y1, z0),
        Vec3::new(x0, y1, z0),
        Vec3::new(x0, y0, z1),
        Vec3::new(x1, y0, z1),
        Vec3::new(x1, y1, z1),
        Vec3::new(x0, y1, z1),
    ];
    let placed: Vec<[f32; 3]> = corners
        .iter()
        .map(|c| {
            let p = transform * Vec4::new(c.x, c.y, c.z, 1.0);
            [p.x, p.y, p.z]
        })
        .collect();
    let extent = [x1 - x0, y1 - y0, z1 - z0];
    let (vertices, indices, faces) = out;
    for (direction, corner) in FACES {
        if cube.skip_faces.contains(&direction) {
            continue;
        }
        // A flat box (a page, a pot side) has edge-on faces of no area.
        let [a, b] = match direction {
            Direction::Down | Direction::Up => [extent[0], extent[2]],
            Direction::North | Direction::South => [extent[0], extent[1]],
            Direction::West | Direction::East => [extent[1], extent[2]],
        };
        if a.abs() < 1e-6 || b.abs() < 1e-6 {
            continue;
        }
        let uvs = cube_face_uvs(cube, direction, model.texture_size);
        let d = direction.normal();
        let n = (transform * Vec4::new(d[0], d[1], d[2], 0.0)).truncate();
        let n = n.normalize_or_zero();
        let start = vertices.len() as u32;
        for (i, &c) in corner.iter().enumerate() {
            vertices.push(Vertex::new(placed[c], [n.x, n.y, n.z], uvs[i]));
        }
        // Counter-clockwise from outside; the side faces' corner order runs
        // the other way round from the top and bottom faces'.
        if matches!(direction, Direction::Down | Direction::Up) {
            indices.extend_from_slice(&[start, start + 2, start + 1, start, start + 3, start + 2]);
        } else {
            indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
        }
        faces.push(EntityFaceTexture {
            texture: texture.to_owned(),
            is_transparent: !model.is_opaque,
        });
    }
}

/// One face's UVs in Minecraft's box unwrap. For a box of size (W, H, D) at
/// texture offset (u0, v0):
///
/// ```text
///                u0     u0+D   u0+D+W  u0+2D+W  u0+2D+2W
/// v0             | DOWN  |  UP   |       |        |
/// v0+D    WEST   | NORTH | EAST  | SOUTH |
/// v0+D+H         |       |       |       |
/// ```
pub fn cube_face_uvs(cube: &EntityCube, face: Direction, texture_size: [u32; 2]) -> [[f32; 2]; 4] {
    let [u0, v0] = cube.tex_offset;
    let [su, sv] = cube.uv_scale;
    let w = cube.dimensions[0] * su;
    let h = cube.dimensions[1] * sv;
    let du = cube.dimensions[2] * su;
    let dv = cube.dimensions[2] * sv;
    let tw = texture_size[0] as f32;
    let th = texture_size[1] as f32;
    let (left, top, right, bottom) = match face {
        Direction::Down => (u0 + du, v0, u0 + du + w, v0 + dv),
        Direction::Up => (u0 + du + w, v0, u0 + du + w + w, v0 + dv),
        Direction::North => (u0 + du, v0 + dv, u0 + du + w, v0 + dv + h),
        Direction::South => (u0 + du + w + du, v0 + dv, u0 + du + w + du + w, v0 + dv + h),
        Direction::West => (u0, v0 + dv, u0 + du, v0 + dv + h),
        Direction::East => (u0 + du + w, v0 + dv, u0 + du + w + du, v0 + dv + h),
    };
    let (l, t, r, b) = (left / tw, top / th, right / tw, bottom / th);
    // The game's polygons start at the right-top corner; the top face's corner
    // order differs from the game's, so it keeps U and flips V instead.
    match (face, cube.mirror) {
        (Direction::Up, false) => [[l, b], [r, b], [r, t], [l, t]],
        (Direction::Up, true) => [[r, b], [l, b], [l, t], [r, t]],
        (_, false) => [[r, t], [l, t], [l, b], [r, b]],
        (_, true) => [[l, t], [r, t], [r, b], [l, b]],
    }
}
