use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Slime model — texture `entity/slime/slime`, 64x32.
/// From SlimeModel.java (MC 1.21.4).
/// Outer transparent cube + inner smaller cube + eyes + mouth.
pub(super) fn slime_model() -> EntityModelDef {
    // Outer translucent cube (8x8x8 per MC 1.21.5 createOuterBodyLayer)
    let outer_cube = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 16.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Inner cube (6x6x6 per MC 1.21.5 createInnerBodyLayer)
    let inner_cube = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, 17.0, -3.0],
            dimensions: [6.0, 6.0, 6.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Right eye
    let right_eye = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.25, 18.0, -3.5],
            dimensions: [2.0, 2.0, 2.0],
            tex_offset: [32, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Left eye
    let left_eye = EntityPart {
        cubes: vec![EntityCube {
            origin: [1.25, 18.0, -3.5],
            dimensions: [2.0, 2.0, 2.0],
            tex_offset: [32, 4],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Mouth
    let mouth = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 21.0, -3.5],
            dimensions: [1.0, 1.0, 1.0],
            tex_offset: [32, 8],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Y-down → Y-up root wrapper
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![outer_cube, inner_cube, right_eye, left_eye, mouth],
    };

    EntityModelDef {
        texture_path: "entity/slime/slime".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: false, // Outer cube is translucent
    }
}
