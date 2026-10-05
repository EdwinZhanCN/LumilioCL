use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose, SignWood};

/// Get the texture path for a sign wood type.
pub(crate) fn sign_texture_path(wood: SignWood) -> &'static str {
    match wood {
        SignWood::Oak => "entity/signs/oak",
        SignWood::Spruce => "entity/signs/spruce",
        SignWood::Birch => "entity/signs/birch",
        SignWood::Jungle => "entity/signs/jungle",
        SignWood::Acacia => "entity/signs/acacia",
        SignWood::DarkOak => "entity/signs/dark_oak",
        SignWood::Crimson => "entity/signs/crimson",
        SignWood::Warped => "entity/signs/warped",
        SignWood::Mangrove => "entity/signs/mangrove",
        SignWood::Cherry => "entity/signs/cherry",
        SignWood::Bamboo => "entity/signs/bamboo",
    }
}

/// Sign model.
pub(super) fn sign_model(wood: SignWood, is_wall: bool) -> EntityModelDef {
    let texture_path = sign_texture_path(wood);

    // Sign uses Java Y-down coords. Renderer applies RotX(PI) + 2/3 scale.
    // Standing: position [8, 8, 8] (centered, stick bottom at y=0)
    // Wall: position [8, 3, 15] (centered X, lower Y, pushed against +Z wall face)
    let (pos_x, pos_y, pos_z) = if is_wall {
        (8.0, 3.0, 15.0)
    } else {
        (8.0, 8.0, 8.0)
    };
    let scale = 2.0 / 3.0;

    // Board: origin [-12,-14,-1], dims [24,12,2], texOffs(0,0)
    let board = EntityPart {
        cubes: vec![EntityCube {
            origin: [-12.0, -14.0, -1.0],
            dimensions: [24.0, 12.0, 2.0],
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

    let mut parts = vec![board];

    // Standing signs have a stick
    if !is_wall {
        // Stick: origin [-1,-2,-1], dims [2,14,2], texOffs(0,14)
        let stick = EntityPart {
            cubes: vec![EntityCube {
                origin: [-1.0, -2.0, -1.0],
                dimensions: [2.0, 14.0, 2.0],
                tex_offset: [0, 14],
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
        parts.push(stick);
    }

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 32],
        parts,
        is_opaque: true,
    }
}

/// Sign model with a custom (dynamic) texture path. The composited texture image
/// itself is 4x upscaled (256x128) to fit readable text, but we keep `texture_size`
/// at the base [64, 32] so UV normalization matches MC's tex_offset layout —
/// normalized UVs then sample the full 256x128 image, including the text region.
pub(crate) fn sign_model_upscaled(custom_texture: &str, is_wall: bool) -> EntityModelDef {
    let (pos_x, pos_y, pos_z) = if is_wall {
        (8.0, 3.0, 15.0)
    } else {
        (8.0, 8.0, 8.0)
    };
    let scale = 2.0 / 3.0;

    let board = EntityPart {
        cubes: vec![EntityCube {
            origin: [-12.0, -14.0, -1.0],
            dimensions: [24.0, 12.0, 2.0],
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

    let mut parts = vec![board];

    if !is_wall {
        let stick = EntityPart {
            cubes: vec![EntityCube {
                origin: [-1.0, -2.0, -1.0],
                dimensions: [2.0, 14.0, 2.0],
                tex_offset: [0, 14],
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
        parts.push(stick);
    }

    EntityModelDef {
        texture_path: custom_texture.to_string(),
        texture_size: [64, 32],
        parts,
        is_opaque: true,
    }
}
