use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Sheep base model — texture `entity/sheep/sheep`, 64x32.
/// From SheepModel.java (MC 1.21.4). Extends QuadrupedModel with leg_height=12.
///
/// Same as QuadrupedModel.createBodyMesh(12, CubeDeformation.NONE):
///   head: [-4,-4,-8] 8x8x8 at pos(0, 6, -6)
///   body: [-5,-10,-7] 10x16x8 tex(28,8) at pos(0, 5, 2) RotX(pi/2)
///   legs: [-2,0,-2] 4x12x4 tex(0,16) at y=12
pub(super) fn sheep_model() -> EntityModelDef {
    // Sheep overrides QuadrupedModel's head and body with smaller dims.
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -4.0, -6.0],
            dimensions: [6.0, 6.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 6.0, -8.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -10.0, -7.0],
            dimensions: [8.0, 16.0, 6.0],
            tex_offset: [28, 8],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 5.0, 2.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 12.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 12.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 12.0, -5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 12.0, -5.0],
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
        children: vec![head, body, right_hind_leg, left_hind_leg, right_front_leg, left_front_leg],
    };

    EntityModelDef {
        texture_path: "entity/sheep/sheep".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}

/// Sheep wool overlay model — texture `entity/sheep/sheep_wool`, 64x32.
/// From SheepFurModel.java (MC 1.21.4).
///
/// Same part layout as base sheep but with inflate values:
///   head: inflate 0.6
///   body: inflate 1.75
///   legs: inflate 0.5
///
/// Wool is tinted by dye color via vertex colors.
pub(crate) fn sheep_wool_model() -> EntityModelDef {
    // Wool overlay per SheepFurModel: head 6x6x6 (not 6x6x8!), legs only 6 tall.
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -4.0, -4.0],
            dimensions: [6.0, 6.0, 6.0],
            tex_offset: [0, 0],
            inflate: 0.6,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 6.0, -8.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -10.0, -7.0],
            dimensions: [8.0, 16.0, 6.0],
            tex_offset: [28, 8],
            inflate: 1.75,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 5.0, 2.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 12.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 12.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 12.0, -5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 12.0, -5.0],
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
        children: vec![head, body, right_hind_leg, left_hind_leg, right_front_leg, left_front_leg],
    };

    EntityModelDef {
        texture_path: "entity/sheep/sheep_wool".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: false, // Wool texture has transparent regions
    }
}
