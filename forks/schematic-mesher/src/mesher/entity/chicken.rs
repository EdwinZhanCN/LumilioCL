use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Chicken model — texture `entity/chicken/temperate_chicken`, 64x32.
/// From ChickenModel.java (MC 1.21.4).
pub(super) fn chicken_model() -> EntityModelDef {
    let beak = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -4.0, -4.0],
            dimensions: [4.0, 2.0, 2.0],
            tex_offset: [14, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let wattle = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -3.0],
            dimensions: [2.0, 2.0, 2.0],
            tex_offset: [14, 4],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -6.0, -2.0],
            dimensions: [4.0, 6.0, 3.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, -4.0],
            ..Default::default()
        },
        children: vec![beak, wattle],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -4.0, -3.0],
            dimensions: [6.0, 8.0, 6.0],
            tex_offset: [0, 9],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 16.0, 0.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -3.0],
            dimensions: [3.0, 5.0, 3.0],
            tex_offset: [26, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 19.0, 1.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -3.0],
            dimensions: [3.0, 5.0, 3.0],
            tex_offset: [26, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.0, 19.0, 1.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_wing = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, -3.0],
            dimensions: [1.0, 4.0, 6.0],
            tex_offset: [24, 13],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-4.0, 13.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_wing = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -3.0],
            dimensions: [1.0, 4.0, 6.0],
            tex_offset: [24, 13],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [4.0, 13.0, 0.0],
            ..Default::default()
        },
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
        children: vec![head, body, right_leg, left_leg, right_wing, left_wing],
    };

    EntityModelDef {
        texture_path: "entity/chicken/temperate_chicken".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        // Legs/feet use the toe texture region which has transparent pixels.
        is_opaque: false,
    }
}
