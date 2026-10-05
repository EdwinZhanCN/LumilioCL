use super::EntityFaceTexture;
use crate::mesher::geometry::Vertex;
use crate::types::InputBlock;
use glam::{Mat4, Vec3, Vec4};

/// Base texture for non-sherd faces (top, bottom, neck interior).
const POT_BASE_TEXTURE: &str = "entity/decorated_pot/decorated_pot_base";

/// Default side texture when no sherd is specified.
const POT_SIDE_TEXTURE: &str = "entity/decorated_pot/decorated_pot_side";

/// Map a sherd name to its texture path.
fn sherd_texture(sherd: &str) -> String {
    if sherd.is_empty() || sherd == "brick" {
        POT_SIDE_TEXTURE.to_string()
    } else {
        format!("entity/decorated_pot/{}_pottery_pattern", sherd)
    }
}

/// Parse the `sherds` property into 4 texture paths (north, east, south, west).
fn parse_sherds(block: &InputBlock) -> [String; 4] {
    if let Some(sherds_str) = block.properties.get("sherds") {
        let parts: Vec<&str> = sherds_str.split(',').collect();
        [
            parts.first().map(|s| sherd_texture(s.trim())).unwrap_or_else(|| POT_SIDE_TEXTURE.to_string()),
            parts.get(1).map(|s| sherd_texture(s.trim())).unwrap_or_else(|| POT_SIDE_TEXTURE.to_string()),
            parts.get(2).map(|s| sherd_texture(s.trim())).unwrap_or_else(|| POT_SIDE_TEXTURE.to_string()),
            parts.get(3).map(|s| sherd_texture(s.trim())).unwrap_or_else(|| POT_SIDE_TEXTURE.to_string()),
        ]
    } else {
        [
            POT_SIDE_TEXTURE.to_string(),
            POT_SIDE_TEXTURE.to_string(),
            POT_SIDE_TEXTURE.to_string(),
            POT_SIDE_TEXTURE.to_string(),
        ]
    }
}

/// Generate decorated pot geometry with per-face textures.
///
/// The pot model (from DecoratedPotRenderer.java) consists of:
/// - Neck: small cube at top, all faces use base texture
/// - Bottom: cube at bottom, all faces use base texture
/// - 4 side panels: each side uses its own sherd texture
///
/// Returns (vertices, indices, face_textures) ready for MeshBuilder integration.
pub(crate) fn generate_decorated_pot_geometry(
    block: &InputBlock,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let sherds = parse_sherds(block);
    let facing = block.properties.get("facing").map(|s| s.as_str()).unwrap_or("north");

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    // MC's DecoratedPotRenderer rotates by `180° - direction.toYRot()` around
    // the block center. Translate to match that convention rather than our
    // humanoid-tuned `facing_rotation_rad`.
    let yrot_deg = match facing {
        "south" => 180.0_f32,
        "west"  => 90.0_f32,
        "north" => 0.0_f32,
        "east"  => 270.0_f32,
        _       => 0.0_f32,
    };
    let facing_mat = Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
        * Mat4::from_rotation_y(yrot_deg.to_radians())
        * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5));

    // Body: a uniform 14×16×14 pot matching MC's SIDES layer, centered X/Z with
    // a 1/16 inset from block boundaries on each horizontal side, full block
    // height. Top/bottom faces use the base texture; the four side faces each
    // get an individual sherd texture.
    let body_min = [1.0 / 16.0, 0.0, 1.0 / 16.0];
    let body_max = [15.0 / 16.0, 1.0, 15.0 / 16.0];
    // Pot sides are opaque: the pattern (sherd) texture is composited over the
    // base pot-side texture in element.rs::composite_pot_side before rendering.
    add_box_with_side_textures(
        &mut vertices, &mut indices, &mut face_textures,
        body_min, body_max, &facing_mat,
        &sherds, false,
    );

    // Neck geometry from MC's NECK part. After `RotX(π) + offset(0, 37, 16)`
    // the two cubes land at:
    //   rim (6×1×6):        y = 16..17  — narrow collar flush with pot top
    //   main neck (8×3×8):  y = 17..20  — wider cube above the rim
    // Both sit slightly above the block boundary (y=16) giving the pot its
    // stepped opening silhouette.
    let rim_min = [5.0 / 16.0, 16.0 / 16.0, 5.0 / 16.0];
    let rim_max = [11.0 / 16.0, 17.0 / 16.0, 11.0 / 16.0];
    add_box(
        &mut vertices, &mut indices, &mut face_textures,
        rim_min, rim_max, &facing_mat,
        POT_SIDE_TEXTURE, false,
    );
    let neck_min = [4.0 / 16.0, 17.0 / 16.0, 4.0 / 16.0];
    let neck_max = [12.0 / 16.0, 20.0 / 16.0, 12.0 / 16.0];
    add_box(
        &mut vertices, &mut indices, &mut face_textures,
        neck_min, neck_max, &facing_mat,
        POT_SIDE_TEXTURE, false,
    );

    (vertices, indices, face_textures)
}

/// Add a box with uniform texture on all 6 faces.
fn add_box(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
    min: [f32; 3],
    max: [f32; 3],
    facing_mat: &Mat4,
    texture: &str,
    is_transparent: bool,
) {
    let corners = box_corners(min, max);
    let transformed = transform_corners(&corners, facing_mat);

    // 6 faces
    let face_defs: [([usize; 4], [f32; 3]); 6] = [
        ([4, 5, 1, 0], [0.0, -1.0, 0.0]),  // Down
        ([3, 2, 6, 7], [0.0, 1.0, 0.0]),   // Up
        ([1, 0, 3, 2], [0.0, 0.0, -1.0]),  // North
        ([4, 5, 6, 7], [0.0, 0.0, 1.0]),   // South
        ([0, 4, 7, 3], [-1.0, 0.0, 0.0]),  // West
        ([5, 1, 2, 6], [1.0, 0.0, 0.0]),   // East
    ];

    for &(ci, normal) in &face_defs {
        add_quad(
            vertices, indices, face_textures,
            &transformed, ci, normal, facing_mat,
            texture, is_transparent,
        );
    }
}

/// Add a box where north/east/south/west faces get individual sherd textures,
/// and top/bottom get base texture.
fn add_box_with_side_textures(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
    min: [f32; 3],
    max: [f32; 3],
    facing_mat: &Mat4,
    sherds: &[String; 4], // north, east, south, west
    is_transparent: bool,
) {
    let corners = box_corners(min, max);
    let transformed = transform_corners(&corners, facing_mat);

    // Faces: (corner_indices, normal, texture). Top/bottom use the plain side
    // texture rather than the base texture — the base is a 32x32 atlas with
    // non-trivial UVs that don't map cleanly across a full face, and MC's
    // pot top/bottom faces are near-uniform terracotta anyway.
    let face_defs: [([usize; 4], [f32; 3], &str); 6] = [
        ([4, 5, 1, 0], [0.0, -1.0, 0.0], POT_SIDE_TEXTURE),   // Down
        ([3, 2, 6, 7], [0.0, 1.0, 0.0], POT_SIDE_TEXTURE),    // Up
        ([1, 0, 3, 2], [0.0, 0.0, -1.0], &sherds[0]),         // North
        ([4, 5, 6, 7], [0.0, 0.0, 1.0], &sherds[2]),          // South
        ([0, 4, 7, 3], [-1.0, 0.0, 0.0], &sherds[3]),         // West
        ([5, 1, 2, 6], [1.0, 0.0, 0.0], &sherds[1]),          // East
    ];

    for &(ci, normal, texture) in &face_defs {
        add_quad(
            vertices, indices, face_textures,
            &transformed, ci, normal, facing_mat,
            texture, is_transparent,
        );
    }
}

fn box_corners(min: [f32; 3], max: [f32; 3]) -> [[f32; 3]; 8] {
    [
        [min[0], min[1], min[2]], // 0: ---
        [max[0], min[1], min[2]], // 1: +--
        [max[0], max[1], min[2]], // 2: ++-
        [min[0], max[1], min[2]], // 3: -+-
        [min[0], min[1], max[2]], // 4: --+
        [max[0], min[1], max[2]], // 5: +-+
        [max[0], max[1], max[2]], // 6: +++
        [min[0], max[1], max[2]], // 7: -++
    ]
}

fn transform_corners(corners: &[[f32; 3]; 8], facing_mat: &Mat4) -> [[f32; 3]; 8] {
    let mut result = [[0.0; 3]; 8];
    for (i, c) in corners.iter().enumerate() {
        let p = *facing_mat * Vec4::new(c[0], c[1], c[2], 1.0);
        result[i] = [p.x, p.y, p.z];
    }
    result
}

fn add_quad(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
    transformed: &[[f32; 3]; 8],
    ci: [usize; 4],
    normal: [f32; 3],
    facing_mat: &Mat4,
    texture: &str,
    is_transparent: bool,
) {
    // Transform normal by facing rotation
    let n4 = *facing_mat * Vec4::new(normal[0], normal[1], normal[2], 0.0);
    let n_vec = Vec3::new(n4.x, n4.y, n4.z).normalize_or_zero();
    let n = [n_vec.x, n_vec.y, n_vec.z];

    // UVs covering the full texture, rotated 180° from the straightforward
    // mapping so the pattern shows right-side up (our vertex order assigns the
    // "top-left of face" to the 4th corner, not the 1st).
    //
    // Inset by 1/16 on the left — MC's pot side uses `texOffs(1, 0)` on a
    // 14-wide box, sampling pixels 1..15 of the 16-wide texture, because the
    // pattern PNGs have a 1-pixel black/transparent margin at column 0.
    // Without this inset that margin shows as a black edge at the pot corner.
    const INSET: f32 = 1.0 / 16.0;
    let uvs = [
        [INSET, 1.0],
        [1.0 - INSET, 1.0],
        [1.0 - INSET, 0.0],
        [INSET, 0.0],
    ];

    let v_start = vertices.len() as u32;

    for (i, &corner_idx) in ci.iter().enumerate() {
        vertices.push(Vertex::new(transformed[corner_idx], n, uvs[i]));
    }

    // Side faces need (0,1,2)(0,2,3), top/bottom need (0,2,1)(0,3,2)
    let is_side = normal[1].abs() < 0.5;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sherds_default() {
        let block = InputBlock::new("minecraft:decorated_pot");
        let sherds = parse_sherds(&block);
        assert!(sherds.iter().all(|s| s == POT_SIDE_TEXTURE));
    }

    #[test]
    fn test_parse_sherds_custom() {
        let block = InputBlock::new("minecraft:decorated_pot")
            .with_property("sherds", "angler,arms_up,blade,brewer");
        let sherds = parse_sherds(&block);
        assert_eq!(sherds[0], "entity/decorated_pot/angler_pottery_pattern");
        assert_eq!(sherds[1], "entity/decorated_pot/arms_up_pottery_pattern");
        assert_eq!(sherds[2], "entity/decorated_pot/blade_pottery_pattern");
        assert_eq!(sherds[3], "entity/decorated_pot/brewer_pottery_pattern");
    }

    #[test]
    fn test_parse_sherds_brick_default() {
        let block = InputBlock::new("minecraft:decorated_pot")
            .with_property("sherds", "brick,angler,brick,brick");
        let sherds = parse_sherds(&block);
        assert_eq!(sherds[0], POT_SIDE_TEXTURE);
        assert_eq!(sherds[1], "entity/decorated_pot/angler_pottery_pattern");
        assert_eq!(sherds[2], POT_SIDE_TEXTURE);
        assert_eq!(sherds[3], POT_SIDE_TEXTURE);
    }

    #[test]
    fn test_decorated_pot_geometry_count() {
        let block = InputBlock::new("minecraft:decorated_pot")
            .with_property("facing", "north");
        let (verts, indices, faces) = generate_decorated_pot_geometry(&block);

        // 3 boxes: body + neck + rim = 18 faces
        assert_eq!(faces.len(), 18);
        assert_eq!(verts.len(), 18 * 4);
        assert_eq!(indices.len(), 18 * 6);
    }

    #[test]
    fn test_decorated_pot_sherd_textures() {
        let block = InputBlock::new("minecraft:decorated_pot")
            .with_property("facing", "south")
            .with_property("sherds", "angler,arms_up,blade,brewer");
        let (_, _, faces) = generate_decorated_pot_geometry(&block);

        // Body is the first box (0-5): Down, Up (side), North/South/West/East (sherds)
        assert_eq!(faces[0].texture, POT_SIDE_TEXTURE); // Down
        assert_eq!(faces[1].texture, POT_SIDE_TEXTURE); // Up
    }

    #[test]
    fn test_decorated_pot_facing_changes_vertices() {
        let block_n = InputBlock::new("minecraft:decorated_pot")
            .with_property("facing", "north");
        let block_e = InputBlock::new("minecraft:decorated_pot")
            .with_property("facing", "east");
        let (verts_n, _, _) = generate_decorated_pot_geometry(&block_n);
        let (verts_e, _, _) = generate_decorated_pot_geometry(&block_e);

        let any_different = verts_n.iter().zip(verts_e.iter())
            .any(|(a, b)| (a.position[0] - b.position[0]).abs() > 0.01
                       || (a.position[2] - b.position[2]).abs() > 0.01);
        assert!(any_different, "Different facings should produce different vertex positions");
    }
}
