use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Minecart model — texture `entity/minecart`, 64×32.
/// 5 flat panels forming an open-top box, from MinecartModel.java (MC 1.21.4).
pub(super) fn minecart_model() -> EntityModelDef {
    let bottom = EntityPart {
        cubes: vec![EntityCube {
            origin: [-10.0, -8.0, -1.0],
            dimensions: [20.0, 16.0, 2.0],
            tex_offset: [0, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, 0.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let front = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -9.0, -1.0],
            dimensions: [16.0, 8.0, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-9.0, 4.0, 0.0],
            rotation: [0.0, 4.712389, 0.0], // 3π/2 = 270°
            ..Default::default()
        },
        children: vec![],
    };

    let back = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -9.0, -1.0],
            dimensions: [16.0, 8.0, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [9.0, 4.0, 0.0],
            rotation: [0.0, std::f32::consts::FRAC_PI_2, 0.0], // π/2 = 90°
            ..Default::default()
        },
        children: vec![],
    };

    let left = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -9.0, -1.0],
            dimensions: [16.0, 8.0, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, -7.0],
            rotation: [0.0, std::f32::consts::PI, 0.0], // π = 180°
            ..Default::default()
        },
        children: vec![],
    };

    let right = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -9.0, -1.0],
            dimensions: [16.0, 8.0, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Y-down → Y-up root wrapper.
    // Position [8, 6, 8]: centers X/Z at block center, Y=6 gives 6/16 = 0.375 lift.
    // Y rotation PI/2 aligns the minecart's intrinsic "front" (-X in MC coords)
    // with the +Z (south) direction used by humanoid mob models — so
    // `facing_rotation_rad` applies consistently for both the cart and its rider.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 6.0, 8.0],
            rotation: [std::f32::consts::PI, std::f32::consts::FRAC_PI_2, 0.0],
            ..Default::default()
        },
        children: vec![bottom, front, back, left, right],
    };

    EntityModelDef {
        texture_path: "entity/minecart".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}
