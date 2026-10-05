use crate::types::Direction;

use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Shulker box model (64x64 texture).
/// Base (bottom half) + lid (top half), rendered closed.
/// Uses Y-down→Y-up root wrapper like mob models.
pub(super) fn shulker_model(color: Option<&str>) -> EntityModelDef {
    let texture_path = match color {
        Some(c) => format!("entity/shulker/shulker_{}", c),
        None => "entity/shulker/shulker".to_string(),
    };

    // Base: origin (-8, -8, -8), dims [16, 8, 16], texOffset [0, 28]
    // Skip Down face: after Y-flip it becomes the top (y=0.5), hidden inside lid volume.
    let base = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -8.0, -8.0],
            dimensions: [16.0, 8.0, 16.0],
            tex_offset: [0, 28],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![Direction::Down],
        }],
        pose: EntityPartPose {
            position: [0.0, 24.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Lid: origin (-8, -16, -8), dims [16, 12, 16], texOffset [0, 0]
    // Skip Up face: after Y-flip it becomes the bottom (y=0.25), hidden inside base volume.
    let lid = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, -16.0, -8.0],
            dimensions: [16.0, 12.0, 16.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![Direction::Up],
        }],
        pose: EntityPartPose {
            position: [0.0, 24.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Root wrapper: Y-down → Y-up conversion (same pattern as mob models)
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![base, lid],
    };

    EntityModelDef {
        texture_path,
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // Shulker textures have transparent regions
    }
}
