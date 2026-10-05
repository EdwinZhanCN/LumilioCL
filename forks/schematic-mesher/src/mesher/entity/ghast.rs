use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Ghast model — texture `entity/ghast/ghast`, 64x32.
/// From GhastModel.java (MC 26.2): a 16x16x16 body and nine 2xLx2 tentacles
/// whose lengths come from a fixed-seed RandomSource(1660) — 8, 13, 9, 11,
/// 11, 10, 12, 9, 12 — the same every ghast, which is why they can be baked.
/// Vanilla applies MeshTransformer.scaling(4.5) after UV layout; here the
/// root pose carries the scale so the UVs still map from the unscaled cube
/// dimensions, and the root is lifted so the longest tentacle's tip rests on
/// the ground plane.
pub(super) fn ghast_model() -> EntityModelDef {
    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -8.0, -8.0],
            dimensions: [16.0, 16.0, 16.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 17.6, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // (x, z, length) per tentacle: the offsets from createBodyLayer's grid
    // walk and the seeded lengths, i = 0..9.
    const TENTACLES: [[f32; 3]; 9] = [
        [-3.75, -5.0, 8.0],
        [1.25, -5.0, 13.0],
        [6.25, -5.0, 9.0],
        [-6.25, 0.0, 11.0],
        [-1.25, 0.0, 11.0],
        [3.75, 0.0, 10.0],
        [-3.75, 5.0, 12.0],
        [1.25, 5.0, 9.0],
        [6.25, 5.0, 12.0],
    ];

    let tentacles = TENTACLES.iter().map(|&[x, z, len]| EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, len, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [x, 24.6, z],
            ..Default::default()
        },
        children: vec![],
    });

    // Y-down -> Y-up root, carrying vanilla's 4.5x scale. The longest
    // tentacle bottoms out at model y = 24.6 + 13 = 37.6; 37.6 * 4.5 = 169.2
    // puts its tip exactly on the ground plane.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 169.2, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            scale: [4.5, 4.5, 4.5],
        },
        children: std::iter::once(body).chain(tentacles).collect(),
    };

    EntityModelDef {
        texture_path: "entity/ghast/ghast".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}
