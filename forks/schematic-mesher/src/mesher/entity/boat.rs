use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Boat model from BoatModel.java (MC 1.21.5).
///
/// - `wood`: texture variant (e.g. "oak", "spruce", "mangrove")
/// - `is_chest`: if true, adds the chest parts and uses the 128×128 chest_boat
///   texture instead of the 128×64 boat texture.
pub(crate) fn boat_model(wood: &str, is_chest: bool) -> EntityModelDef {
    const PADDLE_Z_ROT: f32 = 0.19634955;
    // Resting paddle pose values from AbstractBoatModel.animatePaddle at rowingTime=0.
    // xRot = midpoint of the clampedLerp range = (-PI/3 + -PI/12) / 2 ≈ -0.6545.
    // yRot = clampedLerp(-PI/4, PI/4, (sin(1)+1)/2) ≈ 0.661; right is PI - 0.661.
    const PADDLE_X_ROT: f32 = -0.6544985;
    const PADDLE_Y_LEFT: f32 = 0.6611432;
    const PADDLE_Y_RIGHT: f32 = std::f32::consts::PI - PADDLE_Y_LEFT;

    let bottom = EntityPart {
        cubes: vec![EntityCube {
            origin: [-14.0, -9.0, -3.0],
            dimensions: [28.0, 16.0, 3.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 3.0, 1.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let back = EntityPart {
        cubes: vec![EntityCube {
            origin: [-13.0, -7.0, -1.0],
            dimensions: [18.0, 6.0, 2.0],
            tex_offset: [0, 19],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-15.0, 4.0, 4.0],
            rotation: [0.0, 4.712389, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let front = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -7.0, -1.0],
            dimensions: [16.0, 6.0, 2.0],
            tex_offset: [0, 27],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [15.0, 4.0, 0.0],
            rotation: [0.0, std::f32::consts::FRAC_PI_2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_plank = EntityPart {
        cubes: vec![EntityCube {
            origin: [-14.0, -7.0, -1.0],
            dimensions: [28.0, 6.0, 2.0],
            tex_offset: [0, 35],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, -9.0],
            rotation: [0.0, std::f32::consts::PI, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_plank = EntityPart {
        cubes: vec![EntityCube {
            origin: [-14.0, -7.0, -1.0],
            dimensions: [28.0, 6.0, 2.0],
            tex_offset: [0, 43],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, 9.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Paddles have two cubes each — shaft + blade.
    let left_paddle = EntityPart {
        cubes: vec![
            EntityCube {
                origin: [-1.0, 0.0, -5.0],
                dimensions: [2.0, 2.0, 18.0],
                tex_offset: [62, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [-1.001, -3.0, 8.0],
                dimensions: [1.0, 6.0, 7.0],
                tex_offset: [62, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
        ],
        pose: EntityPartPose {
            position: [3.0, -5.0, 9.0],
            rotation: [PADDLE_X_ROT, PADDLE_Y_LEFT, PADDLE_Z_ROT],
            ..Default::default()
        },
        children: vec![],
    };

    let right_paddle = EntityPart {
        cubes: vec![
            EntityCube {
                origin: [-1.0, 0.0, -5.0],
                dimensions: [2.0, 2.0, 18.0],
                tex_offset: [62, 20],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [0.001, -3.0, 8.0],
                dimensions: [1.0, 6.0, 7.0],
                tex_offset: [62, 20],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
        ],
        pose: EntityPartPose {
            position: [3.0, -5.0, -9.0],
            rotation: [PADDLE_X_ROT, PADDLE_Y_RIGHT, PADDLE_Z_ROT],
            ..Default::default()
        },
        children: vec![],
    };

    let mut parts = vec![bottom, back, front, right_plank, left_plank, left_paddle, right_paddle];

    if is_chest {
        parts.push(EntityPart {
            cubes: vec![EntityCube {
                origin: [0.0, 0.0, 0.0],
                dimensions: [12.0, 8.0, 12.0],
                tex_offset: [0, 76],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [-2.0, -5.0, -6.0],
                rotation: [0.0, -std::f32::consts::FRAC_PI_2, 0.0],
                ..Default::default()
            },
            children: vec![],
        });
        parts.push(EntityPart {
            cubes: vec![EntityCube {
                origin: [0.0, 0.0, 0.0],
                dimensions: [12.0, 4.0, 12.0],
                tex_offset: [0, 59],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [-2.0, -9.0, -6.0],
                rotation: [0.0, -std::f32::consts::FRAC_PI_2, 0.0],
                ..Default::default()
            },
            children: vec![],
        });
        parts.push(EntityPart {
            cubes: vec![EntityCube {
                origin: [0.0, 0.0, 0.0],
                dimensions: [2.0, 4.0, 1.0],
                tex_offset: [0, 59],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [-1.0, -6.0, -1.0],
                rotation: [0.0, -std::f32::consts::FRAC_PI_2, 0.0],
                ..Default::default()
            },
            children: vec![],
        });
    }

    let (texture_path, tex_w, tex_h) = if is_chest {
        (format!("entity/chest_boat/{}", wood), 128u32, 128u32)
    } else {
        (format!("entity/boat/{}", wood), 128u32, 64u32)
    };

    EntityModelDef {
        texture_path,
        texture_size: [tex_w, tex_h],
        parts,
        is_opaque: true,
    }
}
