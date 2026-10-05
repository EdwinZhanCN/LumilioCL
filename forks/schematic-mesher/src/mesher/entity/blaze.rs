use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Blaze model — texture `entity/blaze`, 64x32.
/// From BlazeModel.java (MC 26.2): an 8x8x8 head centred on the model origin
/// and twelve 2x8x2 rods in three rings of four. The rings orbit in game; the
/// static pose here is the animation at age 0, which is also what the model
/// definition itself builds — radius 9 at phase 0 around the shoulders,
/// radius 7 at phase 45 degrees around the waist, radius 5 at phase 27
/// degrees around the legs, each rod's height rippled by the cosine terms
/// vanilla seeds them with.
pub(super) fn blaze_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -4.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // (x, y, z) part offsets for the twelve rods, straight out of
    // createBodyLayer's three loops evaluated at i = 0..12.
    const RODS: [[f32; 3]; 12] = [
        [9.0, -1.0, 0.0],
        [0.0, -1.1224, 9.0],
        [-9.0, -1.4597, 0.0],
        [0.0, -1.9293, -9.0],
        [4.9497, 1.5839, 4.9497],
        [-4.9497, 1.1989, 4.9497],
        [-4.9497, 1.01, -4.9497],
        [4.9497, 1.0635, -4.9497],
        [4.455, 11.9602, 2.27],
        [-2.27, 11.893, 4.455],
        [-4.455, 11.3466, -2.27],
        [2.27, 10.6143, -4.455],
    ];

    let rods = RODS.iter().map(|offset| EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, 0.0],
            dimensions: [2.0, 8.0, 2.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: *offset,
            ..Default::default()
        },
        children: vec![],
    });

    // Y-down -> Y-up root wrapper. Vanilla's model origin puts the head at
    // -4..4, which lands its top at 1.75 blocks over the feet — a blaze is
    // 1.8 tall, and the rods wrap the body below it.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: std::iter::once(head).chain(rods).collect(),
    };

    EntityModelDef {
        texture_path: "entity/blaze".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}
