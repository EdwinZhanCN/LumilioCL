use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Spider model — texture `entity/spider/spider`, 64x32.
/// From SpiderModel.java (MC 1.21.5).
pub(super) fn spider_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -4.0, -8.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [32, 4],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, -3.0],
            ..Default::default()
        },
        children: vec![],
    };

    // body0 (neck)
    let neck = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -3.0, -3.0],
            dimensions: [6.0, 6.0, 6.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // body1 (abdomen)
    let abdomen = EntityPart {
        cubes: vec![EntityCube {
            origin: [-5.0, -4.0, -6.0],
            dimensions: [10.0, 8.0, 12.0],
            tex_offset: [0, 12],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, 9.0],
            ..Default::default()
        },
        children: vec![],
    };

    // MC uses two cube builders: right legs get box(-15, -1, -1, 16, 2, 2)
    // extending toward -X from the pivot; left legs are mirrored with
    // box(-1, -1, -1, 16, 2, 2) extending toward +X.
    let f = 0.7853982_f32;   // PI/4
    let f2 = 0.3926991_f32;  // PI/8
    let f3 = 0.58119464_f32; // inner-leg Z rotation

    let legs = [
        // (side_is_right, position, rot_y, rot_z)
        (true,  [-4.0, 15.0,  2.0],  f,  -f),
        (false, [ 4.0, 15.0,  2.0], -f,   f),
        (true,  [-4.0, 15.0,  1.0],  f2, -f3),
        (false, [ 4.0, 15.0,  1.0], -f2,  f3),
        (true,  [-4.0, 15.0,  0.0], -f2, -f3),
        (false, [ 4.0, 15.0,  0.0],  f2,  f3),
        (true,  [-4.0, 15.0, -1.0], -f,  -f),
        (false, [ 4.0, 15.0, -1.0],  f,   f),
    ];

    let leg_parts: Vec<EntityPart> = legs.iter()
        .map(|&(right, pos, ry, rz)| spider_leg(right, pos, ry, rz))
        .collect();

    // Y-down → Y-up root wrapper
    let mut children = vec![head, neck, abdomen];
    children.extend(leg_parts);
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children,
    };

    EntityModelDef {
        texture_path: "entity/spider/spider".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}

fn spider_leg(is_right: bool, position: [f32; 3], rot_y: f32, rot_z: f32) -> EntityPart {
    let (origin, mirror) = if is_right {
        ([-15.0, -1.0, -1.0], false)
    } else {
        ([-1.0, -1.0, -1.0], true)
    };
    EntityPart {
        cubes: vec![EntityCube {
            origin,
            dimensions: [16.0, 2.0, 2.0],
            tex_offset: [18, 0],
            inflate: 0.0,
            mirror,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position,
            rotation: [0.0, rot_y, rot_z],
            ..Default::default()
        },
        children: vec![],
    }
}
