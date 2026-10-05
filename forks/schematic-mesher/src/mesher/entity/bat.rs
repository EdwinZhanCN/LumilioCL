use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Bat model — texture `entity/bat`, 32x32.
/// From BatModel.java (MC 1.21.5).
pub(super) fn bat_model() -> EntityModelDef {
    // Ears are children of head.
    let right_ear = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.5, -4.0, 0.0],
            dimensions: [3.0, 5.0, 0.0],
            tex_offset: [1, 15],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.5, -2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_ear = EntityPart {
        cubes: vec![EntityCube {
            origin: [-0.1, -3.0, 0.0],
            dimensions: [3.0, 5.0, 0.0],
            tex_offset: [8, 15],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.1, -3.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -3.0, -1.0],
            dimensions: [4.0, 3.0, 2.0],
            tex_offset: [0, 7],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 17.0, 0.0],
            ..Default::default()
        },
        children: vec![right_ear, left_ear],
    };

    // Wings and feet are children of body.
    let right_wing_tip = EntityPart {
        cubes: vec![EntityCube {
            origin: [-6.0, -2.0, 0.0],
            dimensions: [6.0, 8.0, 0.0],
            tex_offset: [16, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_wing = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -2.0, 0.0],
            dimensions: [2.0, 7.0, 0.0],
            tex_offset: [12, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.5, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![right_wing_tip],
    };

    let left_wing_tip = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -2.0, 0.0],
            dimensions: [6.0, 8.0, 0.0],
            tex_offset: [16, 8],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_wing = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -2.0, 0.0],
            dimensions: [2.0, 7.0, 0.0],
            tex_offset: [12, 7],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.5, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![left_wing_tip],
    };

    let feet = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.5, 0.0, 0.0],
            dimensions: [3.0, 2.0, 0.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 5.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.5, 0.0, -1.0],
            dimensions: [3.0, 5.0, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 17.0, 0.0],
            ..Default::default()
        },
        children: vec![right_wing, left_wing, feet],
    };

    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, body],
    };

    EntityModelDef {
        texture_path: "entity/bat".to_string(),
        texture_size: [32, 32],
        parts: vec![root],
        // Wings/ears are zero-depth quads that may include alpha.
        is_opaque: false,
    }
}
