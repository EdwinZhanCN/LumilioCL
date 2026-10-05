use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Wolf model — texture `entity/wolf/wolf`, 64x32.
/// From WolfModel.java (MC 1.21.4).
pub(super) fn wolf_model() -> EntityModelDef {
    let right_ear = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -5.0, 0.0],
            dimensions: [2.0, 2.0, 1.0],
            tex_offset: [16, 14],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let left_ear = EntityPart {
        cubes: vec![EntityCube {
            origin: [2.0, -5.0, 0.0],
            dimensions: [2.0, 2.0, 1.0],
            tex_offset: [16, 14],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let snout = EntityPart {
        cubes: vec![EntityCube {
            origin: [-0.5, -0.001, -5.0],
            dimensions: [3.0, 3.0, 4.0],
            tex_offset: [0, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -3.0, -2.0],
            dimensions: [6.0, 6.0, 4.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.0, 13.5, -7.0],
            ..Default::default()
        },
        children: vec![right_ear, left_ear, snout],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -2.0, -3.0],
            dimensions: [6.0, 9.0, 6.0],
            tex_offset: [18, 14],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 14.0, 2.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Tail cube extends from pivot along +X (box origin x=0). MC's wolf tail
    // is tilted down ~36° (0.62831855 rad) around X.
    let tail = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, -1.0],
            dimensions: [2.0, 8.0, 2.0],
            tex_offset: [9, 18],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.0, 12.0, 8.0],
            rotation: [0.62831855, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Upper body (chest/shoulders) — missing in the old port.
    let upper_body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -3.0, -3.0],
            dimensions: [8.0, 6.0, 7.0],
            tex_offset: [21, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.0, 14.0, -3.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, -1.0],
            dimensions: [2.0, 8.0, 2.0],
            tex_offset: [0, 18],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.5, 16.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, -1.0],
            dimensions: [2.0, 8.0, 2.0],
            tex_offset: [0, 18],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.5, 16.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, -1.0],
            dimensions: [2.0, 8.0, 2.0],
            tex_offset: [0, 18],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.5, 16.0, -4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, -1.0],
            dimensions: [2.0, 8.0, 2.0],
            tex_offset: [0, 18],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.5, 16.0, -4.0],
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
        children: vec![
            head, body, upper_body, tail,
            right_hind_leg, left_hind_leg, right_front_leg, left_front_leg,
        ],
    };

    EntityModelDef {
        texture_path: "entity/wolf/wolf".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}
