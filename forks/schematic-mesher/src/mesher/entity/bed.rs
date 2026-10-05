use crate::types::Direction;

use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Bed model (64x64 texture).
/// The mattress is modeled upright (16x16x6) and rotated +90° X to lay flat.
/// Minecraft uses xRot=PI/2: North face UV becomes the visible sleeping surface.
/// Legs are simple unrotated 3x3x3 cubes at world positions.
pub(super) fn bed_model(color: &str, is_head: bool) -> EntityModelDef {
    let texture_path = format!("entity/bed/{}", color);

    let tex_main: [u32; 2] = if is_head { [0, 0] } else { [0, 22] };

    // Main mattress: origin [0,0,0], dims [16,16,6], rotated +90° X.
    // RotX(PI/2): cube [0,0,0]-[16,16,6] → [0,-6,0]-[16,0,16].
    // Position [0, 9, 0] raises it to y=[3,9] in 1/16 units.
    // Skip the face at the head/foot boundary to prevent z-fighting:
    // RotX(PI/2) maps +Y→+Z, -Y→-Z, so head skips Up (→+Z shared face),
    // foot skips Down (→-Z shared face).
    let shared_face = if is_head { Direction::Up } else { Direction::Down };
    let main = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, 0.0],
            dimensions: [16.0, 16.0, 6.0],
            tex_offset: tex_main,
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![shared_face],
        }],
        pose: EntityPartPose {
            position: [0.0, 9.0, 0.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Legs: 3x3x3 unrotated cubes at fixed positions.
    let (leg_tex_left, leg_tex_right, leg_z) = if is_head {
        // Head legs at z=0 side (headboard)
        ([50u32, 6u32], [50u32, 18u32], 0.0)
    } else {
        // Foot legs at z=13 side (footboard)
        ([50, 0], [50, 12], 13.0)
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, leg_z],
            dimensions: [3.0, 3.0, 3.0],
            tex_offset: leg_tex_left,
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [13.0, 0.0, leg_z],
            dimensions: [3.0, 3.0, 3.0],
            tex_offset: leg_tex_right,
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    EntityModelDef {
        texture_path,
        texture_size: [64, 64],
        parts: vec![main, left_leg, right_leg],
        is_opaque: true,
    }
}
