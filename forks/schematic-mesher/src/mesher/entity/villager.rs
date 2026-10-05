use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Villager model — texture `entity/villager/type/plains`, 64x64.
/// From VillagerModel.java (MC 1.21.4).
pub(super) fn villager_model() -> EntityModelDef {
    // Nose: child of head
    let nose = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -1.0, -6.0],
            dimensions: [2.0, 4.0, 2.0],
            tex_offset: [24, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Hat rim: grandchild of head (child of hat)
    let hat_rim = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -8.0, -6.0],
            dimensions: [16.0, 16.0, 1.0],
            tex_offset: [30, 47],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            rotation: [-std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Hat: child of head (inflated overlay)
    let hat = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -10.0, -4.0],
            dimensions: [8.0, 10.0, 8.0],
            tex_offset: [32, 0],
            inflate: 0.51,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![hat_rim],
    };

    // Head
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -10.0, -4.0],
            dimensions: [8.0, 10.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![hat, nose],
    };

    // Jacket: child of body (inflated overlay)
    let jacket = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -3.0],
            dimensions: [8.0, 20.0, 6.0],
            tex_offset: [0, 38],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Body
    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -3.0],
            dimensions: [8.0, 12.0, 6.0],
            tex_offset: [16, 20],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![jacket],
    };

    // Arms (crossed): 3 cubes in one part
    // Right arm cube: [-8,-2,-2] 4x8x4 at tex(44,22)
    // Left arm cube: [4,-2,-2] 4x8x4 at tex(44,22) mirrored
    // Cross piece: [-4,2,-2] 8x4x4 at tex(40,38)
    let arms = EntityPart {
        cubes: vec![
            EntityCube {
                origin: [-8.0, -2.0, -2.0],
                dimensions: [4.0, 8.0, 4.0],
                tex_offset: [44, 22],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [4.0, -2.0, -2.0],
                dimensions: [4.0, 8.0, 4.0],
                tex_offset: [44, 22],
                inflate: 0.0,
                mirror: true,
                skip_faces: vec![],
            },
            EntityCube {
                origin: [-4.0, 2.0, -2.0],
                dimensions: [8.0, 4.0, 4.0],
                tex_offset: [40, 38],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            },
        ],
        pose: EntityPartPose {
            position: [0.0, 3.0, -1.0],
            rotation: [-0.75, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Legs
    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 22],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 22],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, 12.0, 0.0],
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
        children: vec![head, body, arms, right_leg, left_leg],
    };

    EntityModelDef {
        // Base villager skin (contains head). `entity/villager/type/<biome>` is a
        // biome overlay with no head data; it's meant to be layered onto this base.
        texture_path: "entity/villager/villager".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // Hat overlay has transparent pixels
    }
}
