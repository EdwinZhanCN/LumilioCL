use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose, SignWood};

/// Get the texture path for a hanging sign wood type.
pub(crate) fn hanging_sign_texture_path(wood: SignWood) -> &'static str {
    match wood {
        SignWood::Oak => "entity/signs/hanging/oak",
        SignWood::Spruce => "entity/signs/hanging/spruce",
        SignWood::Birch => "entity/signs/hanging/birch",
        SignWood::Jungle => "entity/signs/hanging/jungle",
        SignWood::Acacia => "entity/signs/hanging/acacia",
        SignWood::DarkOak => "entity/signs/hanging/dark_oak",
        SignWood::Crimson => "entity/signs/hanging/crimson",
        SignWood::Warped => "entity/signs/hanging/warped",
        SignWood::Mangrove => "entity/signs/hanging/mangrove",
        SignWood::Cherry => "entity/signs/hanging/cherry",
        SignWood::Bamboo => "entity/signs/hanging/bamboo",
    }
}

/// Hanging sign model.
/// From HangingSignRenderer.java / CeilingHangingSignModel + WallHangingSignModel.
///
/// Texture layout: 64x32
///   - Board: texOffs(0, 12), origin [-7, 0, -1], dims [14, 10, 2]
///   - V-chains (ceiling): texOffs(0, 6), two chain strips at x=±5
///   - Plank (ceiling): texOffs(0, 0), origin [-8, -6, -2], dims [16, 2, 4]
///
/// All parts use RotX(PI) + scale 2/3 like regular signs.
/// Ceiling: position [8, 8, 8], Wall: position [8, 3, 15]
pub(super) fn hanging_sign_model(wood: SignWood, is_wall: bool) -> EntityModelDef {
    let texture_path = hanging_sign_texture_path(wood);

    // MC's translateBase is translate(0.5, 0.9375, 0.5) + translate(0, -0.3125, 0)
    // = final (0.5, 0.625, 0.5) block-local. In MC units: (8, 10, 8).
    let (pos_x, pos_y, pos_z) = if is_wall {
        (8.0, 10.0, 15.0)
    } else {
        (8.0, 10.0, 8.0)
    };
    // MC's HangingSignRenderer MODEL_RENDER_SCALE = 1.0 (regular signs use 2/3).
    let scale = 1.0;

    // Board: origin [-7, 0, -1], dims [14, 10, 2], texOffs(0, 12)
    let board = EntityPart {
        cubes: vec![EntityCube {
            origin: [-7.0, 0.0, -1.0],
            dimensions: [14.0, 10.0, 2.0],
            tex_offset: [0, 12],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [pos_x, pos_y, pos_z],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            scale: [scale, scale, scale],
        },
        children: vec![],
    };

    let mut parts = vec![board];

    if !is_wall {
        // Ceiling mount: V-shaped chains + plank bar

        // Plank (V-bar): origin [-8, -6, -2], dims [16, 2, 4], texOffs(0, 0)
        let plank = EntityPart {
            cubes: vec![EntityCube {
                origin: [-8.0, -6.0, -2.0],
                dimensions: [16.0, 2.0, 4.0],
                tex_offset: [0, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [pos_x, pos_y, pos_z],
                rotation: [std::f32::consts::PI, 0.0, 0.0],
                scale: [scale, scale, scale],
            },
            children: vec![],
        };
        parts.push(plank);

        // Chain segments: two vertical thin cubes at x=±5
        // Left chain: texOffs(0, 6), origin [-6, -6, 0], dims [2, 6, 0] (flat quad)
        // Right chain: mirrors on the other side
        // Approximate chains as thin cubes for simplicity
        let left_chain = EntityPart {
            cubes: vec![EntityCube {
                origin: [-6.0, -6.0, -1.0],
                dimensions: [2.0, 6.0, 2.0],
                tex_offset: [0, 6],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [pos_x, pos_y, pos_z],
                rotation: [std::f32::consts::PI, 0.0, 0.0],
                scale: [scale, scale, scale],
            },
            children: vec![],
        };
        let right_chain = EntityPart {
            cubes: vec![EntityCube {
                origin: [4.0, -6.0, -1.0],
                dimensions: [2.0, 6.0, 2.0],
                tex_offset: [0, 6],
                inflate: 0.0,
                mirror: true,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [pos_x, pos_y, pos_z],
                rotation: [std::f32::consts::PI, 0.0, 0.0],
                scale: [scale, scale, scale],
            },
            children: vec![],
        };
        parts.push(left_chain);
        parts.push(right_chain);
    }

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 32],
        parts,
        is_opaque: true,
    }
}

/// Hanging sign model with custom (dynamic) texture path and 4x upscaled texture_size.
/// Used when text is composited onto the hanging sign texture.
pub(crate) fn hanging_sign_model_upscaled(custom_texture: &str, is_wall: bool) -> EntityModelDef {
    // MC's translateBase is translate(0.5, 0.9375, 0.5) + translate(0, -0.3125, 0)
    // = final (0.5, 0.625, 0.5) block-local. In MC units: (8, 10, 8).
    let (pos_x, pos_y, pos_z) = if is_wall {
        (8.0, 10.0, 15.0)
    } else {
        (8.0, 10.0, 8.0)
    };
    // MC's HangingSignRenderer MODEL_RENDER_SCALE = 1.0 (regular signs use 2/3).
    let scale = 1.0;

    let board = EntityPart {
        cubes: vec![EntityCube {
            origin: [-7.0, 0.0, -1.0],
            dimensions: [14.0, 10.0, 2.0],
            tex_offset: [0, 12],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [pos_x, pos_y, pos_z],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            scale: [scale, scale, scale],
        },
        children: vec![],
    };

    let mut parts = vec![board];

    if !is_wall {
        let plank = EntityPart {
            cubes: vec![EntityCube {
                origin: [-8.0, -6.0, -2.0],
                dimensions: [16.0, 2.0, 4.0],
                tex_offset: [0, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [pos_x, pos_y, pos_z],
                rotation: [std::f32::consts::PI, 0.0, 0.0],
                scale: [scale, scale, scale],
            },
            children: vec![],
        };
        parts.push(plank);

        let left_chain = EntityPart {
            cubes: vec![EntityCube {
                origin: [-6.0, -6.0, -1.0],
                dimensions: [2.0, 6.0, 2.0],
                tex_offset: [0, 6],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [pos_x, pos_y, pos_z],
                rotation: [std::f32::consts::PI, 0.0, 0.0],
                scale: [scale, scale, scale],
            },
            children: vec![],
        };
        let right_chain = EntityPart {
            cubes: vec![EntityCube {
                origin: [4.0, -6.0, -1.0],
                dimensions: [2.0, 6.0, 2.0],
                tex_offset: [0, 6],
                inflate: 0.0,
                mirror: true,
                skip_faces: vec![],
            }],
            pose: EntityPartPose {
                position: [pos_x, pos_y, pos_z],
                rotation: [std::f32::consts::PI, 0.0, 0.0],
                scale: [scale, scale, scale],
            },
            children: vec![],
        };
        parts.push(left_chain);
        parts.push(right_chain);
    }

    EntityModelDef {
        texture_path: custom_texture.to_string(),
        texture_size: [64, 32],
        parts,
        is_opaque: true,
    }
}
