use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Cat model — texture `entity/cat/tabby`, 64x32.
/// From FelineModel.java (MC 1.21.5). CatModel applies a 0.8x mesh scale
/// on top of the base FelineModel.
pub(super) fn cat_model() -> EntityModelDef {
    // Head has 4 cubes in one part: main skull, nose, two ears.
    let head = EntityPart {
        cubes: vec![
            EntityCube {
                origin: [-2.5, -2.0, -3.0],
                dimensions: [5.0, 4.0, 5.0],
                tex_offset: [0, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [-1.5, -0.001, -4.0],
                dimensions: [3.0, 2.0, 2.0],
                tex_offset: [0, 24],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [-2.0, -3.0, 0.0],
                dimensions: [1.0, 1.0, 2.0],
                tex_offset: [0, 10],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [1.0, -3.0, 0.0],
                dimensions: [1.0, 1.0, 2.0],
                tex_offset: [6, 10],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
        ],
        pose: EntityPartPose {
            position: [0.0, 15.0, -9.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 3.0, -8.0],
            dimensions: [4.0, 16.0, 6.0],
            tex_offset: [20, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 12.0, -10.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let tail1 = EntityPart {
        cubes: vec![EntityCube {
            origin: [-0.5, 0.0, 0.0],
            dimensions: [1.0, 8.0, 1.0],
            tex_offset: [0, 15],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, 8.0],
            rotation: [0.9, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // tail2: MC's model has no rotation — 1.7278 was animation state leaking in.
    let tail2 = EntityPart {
        cubes: vec![EntityCube {
            origin: [-0.5, 0.0, 0.0],
            dimensions: [1.0, 8.0, 1.0],
            tex_offset: [4, 15],
            inflate: -0.02,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 20.0, 14.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Hind legs: shorter (6 tall), box(-1, 0, 1, 2, 6, 2), tex(8, 13).
    fn hind_leg(x: f32) -> EntityPart {
        EntityPart {
            cubes: vec![EntityCube {
                origin: [-1.0, 0.0, 1.0],
                dimensions: [2.0, 6.0, 2.0],
                tex_offset: [8, 13],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [x, 18.0, 5.0],
                ..Default::default()
            },
            children: vec![],
        }
    }

    // Front legs: taller (10 tall), box(-1, 0, 0, 2, 10, 2), tex(40, 0).
    fn front_leg(x: f32) -> EntityPart {
        EntityPart {
            cubes: vec![EntityCube {
                origin: [-1.0, 0.0, 0.0],
                dimensions: [2.0, 10.0, 2.0],
                tex_offset: [40, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [x, 14.1, -5.0],
                ..Default::default()
            },
            children: vec![],
        }
    }

    let right_hind_leg = hind_leg(-1.1);
    let left_hind_leg  = hind_leg(1.1);
    let right_front_leg = front_leg(-1.2);
    let left_front_leg  = front_leg(1.2);

    // CatModel applies MeshTransformer.scaling(0.8). Feet land at model y=24,
    // post-scale at y=19.2, so the root translate matches.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 19.2, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            scale: [0.8, 0.8, 0.8],
        },
        children: vec![
            head, body, tail1, tail2,
            right_hind_leg, left_hind_leg, right_front_leg, left_front_leg,
        ],
    };

    EntityModelDef {
        texture_path: "entity/cat/tabby".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}
