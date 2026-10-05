use super::EntityFaceTexture;
use crate::mesher::geometry::Vertex;
use crate::types::Direction;
use glam::{Mat4, Vec3, Vec4};

/// Per-face definition: which direction, and the UV rect [u0,v0,u1,v1] in [0,16] pixel space.
struct FaceDef {
    dir: Direction,
    uv: [f32; 4], // [u0, v0, u1, v1] in [0,16] space
}

/// Generate item frame geometry directly (not via EntityModelDef).
///
/// Item frames use block textures (birch_planks + item_frame), not a single
/// entity texture sheet, so they bypass the standard EntityModelDef pipeline.
/// UVs and geometry taken from `template_item_frame.json` in the resource pack.
///
/// Returns (vertices, indices, face_textures) ready for MeshBuilder integration.
pub(super) fn generate_item_frame_geometry(
    facing: &str,
    is_glow: bool,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    let frame_tex: &str = if is_glow {
        "block/glow_item_frame"
    } else {
        "block/item_frame"
    };
    let wood_tex: &str = "block/birch_planks";

    // All UVs from template_item_frame.json, in [0,16] pixel space.
    // Element 1: Backing (from [3,3,15.5] to [13,13,16])
    let backing_faces: &[FaceDef] = &[
        FaceDef { dir: Direction::North, uv: [3.0, 3.0, 13.0, 13.0] },
        FaceDef { dir: Direction::South, uv: [3.0, 3.0, 13.0, 13.0] },
    ];

    // Element 2: Top border (from [2,2,15] to [14,3,16])
    let top_faces: &[FaceDef] = &[
        FaceDef { dir: Direction::Down,  uv: [2.0,  0.0, 14.0,  1.0] },
        FaceDef { dir: Direction::Up,    uv: [2.0, 15.0, 14.0, 16.0] },
        FaceDef { dir: Direction::North, uv: [2.0, 13.0, 14.0, 14.0] },
        FaceDef { dir: Direction::South, uv: [2.0, 13.0, 14.0, 14.0] },
        FaceDef { dir: Direction::West,  uv: [15.0, 13.0, 16.0, 14.0] },
        FaceDef { dir: Direction::East,  uv: [0.0, 13.0, 1.0, 14.0] },
    ];

    // Element 3: Bottom border (from [2,13,15] to [14,14,16])
    let bottom_faces: &[FaceDef] = &[
        FaceDef { dir: Direction::Down,  uv: [2.0,  0.0, 14.0,  1.0] },
        FaceDef { dir: Direction::Up,    uv: [2.0, 15.0, 14.0, 16.0] },
        FaceDef { dir: Direction::North, uv: [2.0,  2.0, 14.0,  3.0] },
        FaceDef { dir: Direction::South, uv: [2.0,  2.0, 14.0,  3.0] },
        FaceDef { dir: Direction::West,  uv: [15.0,  2.0, 16.0,  3.0] },
        FaceDef { dir: Direction::East,  uv: [0.0,  2.0,  1.0,  3.0] },
    ];

    // Element 4: Left border (from [2,3,15] to [3,13,16])
    let left_faces: &[FaceDef] = &[
        FaceDef { dir: Direction::North, uv: [13.0, 3.0, 14.0, 13.0] },
        FaceDef { dir: Direction::South, uv: [2.0, 3.0, 3.0, 13.0] },
        FaceDef { dir: Direction::West,  uv: [15.0, 3.0, 16.0, 13.0] },
        FaceDef { dir: Direction::East,  uv: [0.0, 3.0, 1.0, 13.0] },
    ];

    // Element 5: Right border (from [13,3,15] to [14,13,16])
    let right_faces: &[FaceDef] = &[
        FaceDef { dir: Direction::North, uv: [2.0, 3.0, 3.0, 13.0] },
        FaceDef { dir: Direction::South, uv: [13.0, 3.0, 14.0, 13.0] },
        FaceDef { dir: Direction::West,  uv: [15.0, 3.0, 16.0, 13.0] },
        FaceDef { dir: Direction::East,  uv: [0.0, 3.0, 1.0, 13.0] },
    ];

    let elements: &[(&[f32; 3], &[f32; 3], &str, bool, &[FaceDef])] = &[
        (&[3.0, 3.0, 15.5], &[13.0, 13.0, 16.0], frame_tex, true, backing_faces),
        (&[2.0, 2.0, 15.0], &[14.0, 3.0, 16.0], wood_tex, false, top_faces),
        (&[2.0, 13.0, 15.0], &[14.0, 14.0, 16.0], wood_tex, false, bottom_faces),
        (&[2.0, 3.0, 15.0], &[3.0, 13.0, 16.0], wood_tex, false, left_faces),
        (&[13.0, 3.0, 15.0], &[14.0, 13.0, 16.0], wood_tex, false, right_faces),
    ];

    for &(from, to, texture, is_transparent, faces) in elements {
        add_element(
            *from, *to, texture, is_transparent, faces,
            &mut vertices, &mut indices, &mut face_textures,
        );
    }

    // Apply facing rotation: 6-direction around block center (0.5, 0.5, 0.5)
    let center = Vec3::new(0.5, 0.5, 0.5);
    let rot_mat = match facing {
        "south" => Mat4::IDENTITY, // default orientation
        "north" => Mat4::from_rotation_y(std::f32::consts::PI),
        "east" => Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        "west" => Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
        "up" => Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
        "down" => Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        _ => Mat4::IDENTITY,
    };
    let facing_mat = Mat4::from_translation(center) * rot_mat * Mat4::from_translation(-center);

    // Transform all vertices
    for v in &mut vertices {
        let p = facing_mat * Vec4::new(v.position[0], v.position[1], v.position[2], 1.0);
        v.position = [p.x, p.y, p.z];
        let n = facing_mat * Vec4::new(v.normal[0], v.normal[1], v.normal[2], 0.0);
        let nv = Vec3::new(n.x, n.y, n.z).normalize_or_zero();
        v.normal = [nv.x, nv.y, nv.z];
    }

    (vertices, indices, face_textures)
}

/// Add a box element with per-face UVs, using block-model coordinates [0,16].
fn add_element(
    from: [f32; 3], to: [f32; 3],
    texture: &str,
    is_transparent: bool,
    faces: &[FaceDef],
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    // Normalize from [0,16] to [0,1]
    let x0 = from[0] / 16.0;
    let y0 = from[1] / 16.0;
    let z0 = from[2] / 16.0;
    let x1 = to[0] / 16.0;
    let y1 = to[1] / 16.0;
    let z1 = to[2] / 16.0;

    // 8 corners
    let corners: [[f32; 3]; 8] = [
        [x0, y0, z0], // 0: ---
        [x1, y0, z0], // 1: +--
        [x1, y1, z0], // 2: ++-
        [x0, y1, z0], // 3: -+-
        [x0, y0, z1], // 4: --+
        [x1, y0, z1], // 5: +-+
        [x1, y1, z1], // 6: +++
        [x0, y1, z1], // 7: -++
    ];

    // Corner indices and normals for each face direction
    fn corner_info(dir: Direction) -> ([usize; 4], [f32; 3]) {
        match dir {
            Direction::Down  => ([4, 5, 1, 0], [0.0, -1.0, 0.0]),
            Direction::Up    => ([3, 2, 6, 7], [0.0, 1.0, 0.0]),
            Direction::North => ([1, 0, 3, 2], [0.0, 0.0, -1.0]),
            Direction::South => ([4, 5, 6, 7], [0.0, 0.0, 1.0]),
            Direction::West  => ([0, 4, 7, 3], [-1.0, 0.0, 0.0]),
            Direction::East  => ([5, 1, 2, 6], [1.0, 0.0, 0.0]),
        }
    }

    for face in faces {
        let (ci, normal) = corner_info(face.dir);

        // UV rect in [0,16] pixel space → normalize to [0,1] for a 16x16 texture
        let u0 = face.uv[0] / 16.0;
        let v0 = face.uv[1] / 16.0;
        let u1 = face.uv[2] / 16.0;
        let v1 = face.uv[3] / 16.0;

        // Vertex UV assignment: match Minecraft's block model convention
        // corners[0]=TL, [1]=TR, [2]=BR, [3]=BL when viewed from outside
        let uvs: [[f32; 2]; 4] = [
            [u1, v0], // corner 0: right-top
            [u0, v0], // corner 1: left-top
            [u0, v1], // corner 2: left-bottom
            [u1, v1], // corner 3: right-bottom
        ];

        let v_start = vertices.len() as u32;
        for (i, &corner_idx) in ci.iter().enumerate() {
            vertices.push(Vertex::new(corners[corner_idx], normal, uvs[i]));
        }

        // Winding: sides get (0,1,2)(0,2,3), top/bottom get (0,2,1)(0,3,2)
        let is_side = matches!(face.dir,
            Direction::North | Direction::South | Direction::West | Direction::East);
        if is_side {
            indices.extend_from_slice(&[
                v_start, v_start + 1, v_start + 2,
                v_start, v_start + 2, v_start + 3,
            ]);
        } else {
            indices.extend_from_slice(&[
                v_start, v_start + 2, v_start + 1,
                v_start, v_start + 3, v_start + 2,
            ]);
        }

        face_textures.push(EntityFaceTexture {
            texture: texture.to_string(),
            is_transparent,
        });
    }
}
