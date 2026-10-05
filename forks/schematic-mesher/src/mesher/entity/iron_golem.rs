use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Iron golem model — texture `entity/iron_golem/iron_golem`, 128x128.
/// From IronGolemModel.java (MC 1.21.4).
/// Large humanoid with thick limbs.
pub(super) fn iron_golem_model() -> EntityModelDef {
    // Nose child of head
    let nose = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -5.0, -7.9],
            dimensions: [2.0, 4.0, 2.0],
            tex_offset: [24, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -12.0, -5.5],
            dimensions: [8.0, 10.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -7.0, -2.0],
            ..Default::default()
        },
        children: vec![nose],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-9.0, -2.0, -6.0],
            dimensions: [18.0, 12.0, 11.0],
            tex_offset: [0, 40],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -7.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Waist/lower body — thinner torso section
    let waist = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.5, 0.0, -3.0],
            dimensions: [9.0, 5.0, 6.0],
            tex_offset: [0, 70],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 5.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Arms — 4×30×6 (very long and thick)
    let right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-13.0, -2.5, -3.0],
            dimensions: [4.0, 30.0, 6.0],
            tex_offset: [60, 21],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -7.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [9.0, -2.5, -3.0],
            dimensions: [4.0, 30.0, 6.0],
            tex_offset: [60, 58],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -7.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Legs — 6×16×5
    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.5, 0.0, -3.0],
            dimensions: [6.0, 16.0, 5.0],
            tex_offset: [37, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-4.0, 11.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.5, 0.0, -3.0],
            dimensions: [6.0, 16.0, 5.0],
            tex_offset: [60, 0],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [5.0, 11.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Y-down → Y-up root wrapper. Iron golem feet land at model-local Y=24:
    // leg pose.Y (11) + leg cube max (cube Y-range -3..13 → +13) = 24. Using
    // 43 puts the golem floating 19/16 blocks above the ground.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, body, waist, right_arm, left_arm, right_leg, left_leg],
    };

    EntityModelDef {
        texture_path: "entity/iron_golem/iron_golem".to_string(),
        texture_size: [128, 128],
        parts: vec![root],
        is_opaque: true,
    }
}
