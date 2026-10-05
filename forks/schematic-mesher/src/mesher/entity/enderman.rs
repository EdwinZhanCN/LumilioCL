use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Enderman model — texture `entity/enderman/enderman`, 64x32.
/// From EndermanModel.java (MC 1.21.5). Humanoid with HumanoidModel.createMesh(yOffset=-14),
/// giving the whole body a -14 Y shift relative to a normal humanoid.
pub(super) fn enderman_model() -> EntityModelDef {
    // Hat is a child of head (inflated overlay) — PartPose.ZERO relative to head.
    let hat = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 16],
            inflate: -0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -13.0, 0.0],
            ..Default::default()
        },
        children: vec![hat],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [32, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -14.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Long arms (30 tall, pivot at y=-12).
    let right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-5.0, -12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [5.0, -12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Long legs (30 tall, pivot at y=-5 — feet land at y=25).
    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, -5.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, -5.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Root: enderman feet are at model y=25 (not the usual 24), so root y=25.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 25.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, body, right_arm, left_arm, right_leg, left_leg],
    };

    EntityModelDef {
        texture_path: "entity/enderman/enderman".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: false, // Has transparent hat layer
    }
}
