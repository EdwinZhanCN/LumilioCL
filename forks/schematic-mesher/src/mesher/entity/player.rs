use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};
use super::armor_stand::parse_pose;
use super::mob::mob_root;
use crate::types::InputBlock;

/// Determine whether the player should use slim (Alex) arms.
fn is_slim(block: &InputBlock) -> bool {
    // Explicit slim property
    if block.properties.get("slim").map(|v| v == "true").unwrap_or(false) {
        return true;
    }
    // UUID parity: odd first hex digit → Alex (slim)
    if let Some(uuid) = block.properties.get("uuid") {
        let first_char = uuid.chars().find(|c| c.is_ascii_hexdigit());
        if let Some(ch) = first_char {
            let digit = ch.to_digit(16).unwrap_or(0);
            if digit % 2 == 1 {
                return true;
            }
        }
    }
    false
}

/// Full player model — 64x64 texture.
///
/// 6 base parts + 6 overlay layers (each overlay is a child of its base part
/// so it inherits pose transforms). Supports wide (Steve) and slim (Alex) arms.
pub(crate) fn player_model(block: &InputBlock, texture_path: &str) -> EntityModelDef {
    let slim = is_slim(block);
    let arm_width: f32 = if slim { 3.0 } else { 4.0 };

    // ── Head + Hat overlay ──
    let mut head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![
            // Hat overlay
            EntityPart {
                cubes: vec![EntityCube {
                    origin: [-4.0, -8.0, -4.0],
                    dimensions: [8.0, 8.0, 8.0],
                    tex_offset: [32, 0],
                    inflate: 0.25,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: Default::default(),
                children: vec![],
            },
        ],
    };

    // ── Body + Jacket overlay ──
    let mut body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![
            // Jacket overlay
            EntityPart {
                cubes: vec![EntityCube {
                    origin: [-4.0, 0.0, -2.0],
                    dimensions: [8.0, 12.0, 4.0],
                    tex_offset: [16, 32],
                    inflate: 0.25,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: Default::default(),
                children: vec![],
            },
        ],
    };

    // ── Right Arm + Right Sleeve overlay ──
    // Wide: origin [-3, -2, -2], dims [4, 12, 4]
    // Slim: origin [-2, -2, -2], dims [3, 12, 4]
    let right_arm_origin = if slim { [-2.0, -2.0, -2.0] } else { [-3.0, -2.0, -2.0] };
    let mut right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: right_arm_origin,
            dimensions: [arm_width, 12.0, 4.0],
            tex_offset: [40, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![
            // Right sleeve overlay
            EntityPart {
                cubes: vec![EntityCube {
                    origin: right_arm_origin,
                    dimensions: [arm_width, 12.0, 4.0],
                    tex_offset: [40, 32],
                    inflate: 0.25,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: Default::default(),
                children: vec![],
            },
        ],
    };

    // ── Left Arm + Left Sleeve overlay ──
    // Player left arm has its own UVs at (32, 48) — NOT mirrored from right arm
    // Wide: origin [-1, -2, -2], dims [4, 12, 4]
    // Slim: origin [-1, -2, -2], dims [3, 12, 4]
    let mut left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -2.0],
            dimensions: [arm_width, 12.0, 4.0],
            tex_offset: [32, 48],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![
            // Left sleeve overlay
            EntityPart {
                cubes: vec![EntityCube {
                    origin: [-1.0, -2.0, -2.0],
                    dimensions: [arm_width, 12.0, 4.0],
                    tex_offset: [48, 48],
                    inflate: 0.25,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: Default::default(),
                children: vec![],
            },
        ],
    };

    // ── Right Leg + Right Pant overlay ──
    let mut right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.9, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![
            // Right pant overlay
            EntityPart {
                cubes: vec![EntityCube {
                    origin: [-2.0, 0.0, -2.0],
                    dimensions: [4.0, 12.0, 4.0],
                    tex_offset: [0, 32],
                    inflate: 0.25,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: Default::default(),
                children: vec![],
            },
        ],
    };

    // ── Left Leg + Left Pant overlay ──
    // Player left leg has its own UVs at (16, 48) — NOT mirrored from right leg
    let mut left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [16, 48],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.9, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![
            // Left pant overlay
            EntityPart {
                cubes: vec![EntityCube {
                    origin: [-2.0, 0.0, -2.0],
                    dimensions: [4.0, 12.0, 4.0],
                    tex_offset: [0, 48],
                    inflate: 0.25,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: Default::default(),
                children: vec![],
            },
        ],
    };

    // ── Apply pose properties ──
    if let Some(pose) = block.properties.get("HeadPose") {
        head.pose.rotation = parse_pose(pose);
    }
    if let Some(pose) = block.properties.get("BodyPose") {
        body.pose.rotation = parse_pose(pose);
    }
    if let Some(pose) = block.properties.get("RightArmPose") {
        right_arm.pose.rotation = parse_pose(pose);
    }
    if let Some(pose) = block.properties.get("LeftArmPose") {
        left_arm.pose.rotation = parse_pose(pose);
    }
    if let Some(pose) = block.properties.get("RightLegPose") {
        right_leg.pose.rotation = parse_pose(pose);
    }
    if let Some(pose) = block.properties.get("LeftLegPose") {
        left_leg.pose.rotation = parse_pose(pose);
    }

    let root = mob_root(vec![head, body, right_arm, left_arm, right_leg, left_leg]);

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // Overlay layers have transparent pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesher::entity::{EntityPart, MobType};
    use crate::types::InputBlock;

    /// Count cubes recursively in an entity model.
    fn count_cubes(parts: &[EntityPart]) -> usize {
        let mut count = 0;
        for part in parts {
            count += part.cubes.len();
            count += count_cubes(&part.children);
        }
        count
    }

    #[test]
    fn test_player_model_wide_geometry_count() {
        let block = InputBlock::new("entity:player")
            .with_property("facing", "south");
        let model = player_model(&block, "entity/player/wide/steve");
        // 12 cubes (6 base + 6 overlay) × 6 faces = 72 faces
        assert_eq!(count_cubes(&model.parts), 12);
    }

    #[test]
    fn test_player_model_slim_geometry_count() {
        let block = InputBlock::new("entity:player")
            .with_property("facing", "south")
            .with_property("slim", "true");
        let model = player_model(&block, "entity/player/slim/alex");
        // Same cube count: 12 (6 base + 6 overlay)
        assert_eq!(count_cubes(&model.parts), 12);
    }

    #[test]
    fn test_player_model_slim_arm_width() {
        let block = InputBlock::new("entity:player")
            .with_property("slim", "true");
        let model = player_model(&block, "test");
        // Find arm cubes — they should have width 3
        let root = &model.parts[0];
        // root children: head, body, right_arm, left_arm, right_leg, left_leg
        let right_arm = &root.children[2];
        assert_eq!(right_arm.cubes[0].dimensions[0], 3.0);
        let left_arm = &root.children[3];
        assert_eq!(left_arm.cubes[0].dimensions[0], 3.0);
    }

    #[test]
    fn test_player_model_wide_arm_width() {
        let block = InputBlock::new("entity:player");
        let model = player_model(&block, "test");
        let root = &model.parts[0];
        let right_arm = &root.children[2];
        assert_eq!(right_arm.cubes[0].dimensions[0], 4.0);
        let left_arm = &root.children[3];
        assert_eq!(left_arm.cubes[0].dimensions[0], 4.0);
    }

    #[test]
    fn test_player_left_arm_not_mirrored() {
        let block = InputBlock::new("entity:player");
        let model = player_model(&block, "test");
        let root = &model.parts[0];
        let left_arm = &root.children[3];
        // Left arm should use (32, 48) UVs, NOT mirrored
        assert_eq!(left_arm.cubes[0].tex_offset, [32, 48]);
        assert!(!left_arm.cubes[0].mirror);
    }

    #[test]
    fn test_player_left_leg_not_mirrored() {
        let block = InputBlock::new("entity:player");
        let model = player_model(&block, "test");
        let root = &model.parts[0];
        let left_leg = &root.children[5];
        // Left leg should use (16, 48) UVs, NOT mirrored
        assert_eq!(left_leg.cubes[0].tex_offset, [16, 48]);
        assert!(!left_leg.cubes[0].mirror);
    }

    #[test]
    fn test_player_pose_applied() {
        let block = InputBlock::new("entity:player")
            .with_property("HeadPose", "45,0,0");
        let model = player_model(&block, "test");
        let root = &model.parts[0];
        let head = &root.children[0];
        // Head rotation should be 45 degrees in radians
        let expected = 45.0_f32.to_radians();
        assert!((head.pose.rotation[0] - expected).abs() < 0.001);
    }

    #[test]
    fn test_detect_player_mob() {
        let block = InputBlock::new("entity:player");
        assert!(matches!(
            crate::mesher::entity::detect_mob(&block),
            Some(MobType::Player)
        ));
    }
}
