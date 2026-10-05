use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

const DEG_30: f32 = 0.5235988;

/// Horse model — texture `entity/horse/horse_brown`, 64x64.
/// From AbstractEquineModel.java (MC 1.21.5).
pub(super) fn horse_model() -> EntityModelDef {
    let left_ear = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.55, -13.0, 4.0],
            dimensions: [2.0, 3.0, 1.0],
            tex_offset: [19, 16],
            inflate: -0.001,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let right_ear = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.55, -13.0, 4.0],
            dimensions: [2.0, 3.0, 1.0],
            tex_offset: [19, 16],
            inflate: -0.001,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Head: the forehead/upper skull cube, with ears attached.
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -11.0, -2.0],
            dimensions: [6.0, 5.0, 7.0],
            tex_offset: [0, 13],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![left_ear, right_ear],
    };

    let mane = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -11.0, 5.01],
            dimensions: [2.0, 16.0, 2.0],
            tex_offset: [56, 36],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let upper_mouth = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -11.0, -7.0],
            dimensions: [4.0, 5.0, 5.0],
            tex_offset: [0, 25],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // head_parts = neck box with head/mane/mouth as children.
    let head_parts = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.05, -6.0, -2.0],
            dimensions: [4.0, 12.0, 7.0],
            tex_offset: [0, 35],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, -12.0],
            rotation: [DEG_30, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, mane, upper_mouth],
    };

    // Tail is a CHILD of body in MC, positioned relative to the body.
    let tail = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.5, 0.0, 0.0],
            dimensions: [3.0, 14.0, 4.0],
            tex_offset: [42, 36],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -5.0, 2.0],
            rotation: [DEG_30, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Body box is already horizontal (22 long in Z). No rotation needed.
    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-5.0, -8.0, -17.0],
            dimensions: [10.0, 10.0, 22.0],
            tex_offset: [0, 32],
            inflate: 0.05,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 11.0, 5.0],
            ..Default::default()
        },
        children: vec![tail],
    };

    // Legs: right side uses origin (-1, -1.01, ...), left side is mirrored with
    // origin (-3, -1.01, ...). Hind Z origin -1, front Z origin -1.9.
    fn leg(origin: [f32; 3], mirror: bool, position: [f32; 3]) -> EntityPart {
        EntityPart {
            cubes: vec![EntityCube {
                origin,
                dimensions: [4.0, 11.0, 4.0],
                tex_offset: [48, 21],
                inflate: 0.0,
                mirror,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position,
                ..Default::default()
            },
            children: vec![],
        }
    }

    let right_hind_leg = leg([-1.0, -1.01, -1.0], false, [-4.0, 14.0, 7.0]);
    let left_hind_leg  = leg([-3.0, -1.01, -1.0], true,  [ 4.0, 14.0, 7.0]);
    let right_front_leg = leg([-1.0, -1.01, -1.9], false, [-4.0, 14.0, -10.0]);
    let left_front_leg  = leg([-3.0, -1.01, -1.9], true,  [ 4.0, 14.0, -10.0]);

    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![
            body, head_parts,
            right_hind_leg, left_hind_leg, right_front_leg, left_front_leg,
        ],
    };

    EntityModelDef {
        texture_path: "entity/horse/horse_brown".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}
