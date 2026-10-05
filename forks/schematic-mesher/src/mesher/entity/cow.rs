use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Cow model — texture `entity/cow/temperate_cow`, 64x32.
/// From CowModel.java (MC 1.21.4). Extends QuadrupedModel with leg_height=12.
///
/// QuadrupedModel.createBodyMesh(12, CubeDeformation.NONE):
///   head: [-4,-4,-8] 8x8x8 at pos(0, 18-12=6, -6)
///   body: [-5,-10,-7] 10x16x8 tex(28,8) at pos(0, 17-12=5, 2) RotX(pi/2)
///   legs: [-2,0,-2] 4x12x4 tex(0,16) at y=24-12=12, z=+-7/+-5, x=+-3
///
/// CowModel adds horns to head:
///   right_horn: [-4,-5,-4] 1x3x1 tex(22,0) at pos(0,0,0) — relative to head
///   left_horn: [3,-5,-4] 1x3x1 tex(22,0) at pos(0,0,0) — relative to head
pub(super) fn cow_model() -> EntityModelDef {
    let right_horn = EntityPart {
        cubes: vec![EntityCube {
            origin: [-5.0, -5.0, -5.0],
            dimensions: [1.0, 3.0, 1.0],
            tex_offset: [22, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let left_horn = EntityPart {
        cubes: vec![EntityCube {
            origin: [4.0, -5.0, -5.0],
            dimensions: [1.0, 3.0, 1.0],
            tex_offset: [22, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![
            EntityCube {
                origin: [-4.0, -4.0, -6.0],
                dimensions: [8.0, 8.0, 6.0],
                tex_offset: [0, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            // Muzzle/snout (added in 1.21.5 for the new biome-variant cow textures).
            EntityCube {
                origin: [-3.0, 1.0, -7.0],
                dimensions: [6.0, 3.0, 1.0],
                tex_offset: [1, 33],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
        ],
        pose: EntityPartPose {
            position: [0.0, 4.0, -8.0],
            ..Default::default()
        },
        children: vec![right_horn, left_horn],
    };

    let body = EntityPart {
        cubes: vec![
            EntityCube {
                origin: [-6.0, -10.0, -7.0],
                dimensions: [12.0, 18.0, 10.0],
                tex_offset: [18, 4],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [-2.0, 2.0, -8.0],
                dimensions: [4.0, 6.0, 1.0],
                tex_offset: [52, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
        ],
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
            position: [-4.0, 12.0, 7.0],
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
            position: [4.0, 12.0, 7.0],
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
            position: [-4.0, 12.0, -5.0],
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
            position: [4.0, 12.0, -5.0],
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
        texture_path: "entity/cow/temperate_cow".to_string(),
        // Pack's temperate_cow.png is 64x64 (MC 1.21.5+). CowModel.java still uses
        // 64x32 pixel coords, but UVs normalize against the actual image size.
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}
