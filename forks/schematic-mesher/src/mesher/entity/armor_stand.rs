use super::{EntityCube, EntityFaceTexture, EntityModelDef, EntityPart, EntityPartPose};
use crate::mesher::geometry::Vertex;
use crate::types::InputBlock;
use glam::{Mat4, Vec3};

/// Parse a pose property string "x,y,z" (degrees) into [rx, ry, rz] radians.
pub(super) fn parse_pose(s: &str) -> [f32; 3] {
    let parts: Vec<f32> = s.split(',')
        .map(|p| p.trim().parse::<f32>().unwrap_or(0.0).to_radians())
        .collect();
    [
        *parts.first().unwrap_or(&0.0),
        *parts.get(1).unwrap_or(&0.0),
        *parts.get(2).unwrap_or(&0.0),
    ]
}

/// Armor stand model — texture `entity/armorstand/wood`, 64×64.
/// 10 parts from ArmorStandModel.java (MC 1.21.4).
/// Accepts block to read optional pose properties (HeadPose, BodyPose, etc.).
pub(super) fn armor_stand_model(block: &InputBlock) -> EntityModelDef {
    let mut head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -7.0, -1.0],
            dimensions: [2.0, 7.0, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 1.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let mut body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-6.0, 0.0, -1.5],
            dimensions: [12.0, 3.0, 3.0],
            tex_offset: [0, 26],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let mut right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -2.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [24, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let mut left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -2.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [32, 16],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let mut right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 11.0, 2.0],
            tex_offset: [8, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.9, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let mut left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 11.0, 2.0],
            tex_offset: [40, 16],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.9, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_body_stick = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, 3.0, -1.0],
            dimensions: [2.0, 7.0, 2.0],
            tex_offset: [16, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let left_body_stick = EntityPart {
        cubes: vec![EntityCube {
            origin: [1.0, 3.0, -1.0],
            dimensions: [2.0, 7.0, 2.0],
            tex_offset: [48, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let shoulder_stick = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 10.0, -1.0],
            dimensions: [8.0, 2.0, 2.0],
            tex_offset: [0, 48],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let base_plate = EntityPart {
        cubes: vec![EntityCube {
            origin: [-6.0, 11.0, -6.0],
            dimensions: [12.0, 1.0, 12.0],
            tex_offset: [0, 32],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Apply pose overrides from block properties (degrees → radians)
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

    // Y-down → Y-up root wrapper (same as other humanoid mobs)
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![
            head, body, right_arm, left_arm, right_leg, left_leg,
            right_body_stick, left_body_stick, shoulder_stick, base_plate,
        ],
    };

    EntityModelDef {
        texture_path: "entity/armorstand/wood".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}

// ── Armor Rendering ─────────────────────────────────────────────────────────

/// Armor material type — determines texture path.
#[derive(Debug, Clone, Copy)]
enum ArmorMaterial {
    Leather, Chainmail, Iron, Gold, Diamond, Netherite,
}

impl ArmorMaterial {
    fn from_item(item_id: &str) -> Option<Self> {
        let name = item_id.strip_prefix("minecraft:").unwrap_or(item_id);
        if name.starts_with("leather_") { return Some(Self::Leather); }
        if name.starts_with("chainmail_") { return Some(Self::Chainmail); }
        if name.starts_with("iron_") { return Some(Self::Iron); }
        if name.starts_with("golden_") { return Some(Self::Gold); }
        if name.starts_with("diamond_") { return Some(Self::Diamond); }
        if name.starts_with("netherite_") { return Some(Self::Netherite); }
        None
    }

    fn humanoid_texture(&self) -> &'static str {
        match self {
            Self::Leather => "entity/equipment/humanoid/leather",
            Self::Chainmail => "entity/equipment/humanoid/chainmail",
            Self::Iron => "entity/equipment/humanoid/iron",
            Self::Gold => "entity/equipment/humanoid/gold",
            Self::Diamond => "entity/equipment/humanoid/diamond",
            Self::Netherite => "entity/equipment/humanoid/netherite",
        }
    }

    fn leggings_texture(&self) -> &'static str {
        match self {
            Self::Leather => "entity/equipment/humanoid_leggings/leather",
            Self::Chainmail => "entity/equipment/humanoid_leggings/chainmail",
            Self::Iron => "entity/equipment/humanoid_leggings/iron",
            Self::Gold => "entity/equipment/humanoid_leggings/gold",
            Self::Diamond => "entity/equipment/humanoid_leggings/diamond",
            Self::Netherite => "entity/equipment/humanoid_leggings/netherite",
        }
    }
}

/// Armor piece type — determines which cubes to render.
#[derive(Debug, Clone, Copy)]
enum ArmorSlot {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

impl ArmorSlot {
    fn from_item(item_id: &str) -> Option<Self> {
        let name = item_id.strip_prefix("minecraft:").unwrap_or(item_id);
        if name.ends_with("_helmet") || name == "turtle_helmet" { return Some(Self::Helmet); }
        if name.ends_with("_chestplate") { return Some(Self::Chestplate); }
        if name.ends_with("_leggings") { return Some(Self::Leggings); }
        if name.ends_with("_boots") { return Some(Self::Boots); }
        None
    }
}

/// Generate armor overlay geometry for an armor stand.
///
/// Checks for `helmet`, `chestplate`, `leggings`, `boots` properties on the block
/// and generates inflated entity cubes with armor textures.
pub fn generate_armor_geometry(
    block: &InputBlock,
    facing: &str,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    // Check each armor slot property
    for slot_name in &["helmet", "chestplate", "leggings", "boots"] {
        if let Some(item_id) = block.properties.get(*slot_name) {
            let material = match ArmorMaterial::from_item(item_id) {
                Some(m) => m,
                None => continue,
            };
            let slot = match ArmorSlot::from_item(item_id) {
                Some(s) => s,
                None => continue,
            };

            let texture = match slot {
                ArmorSlot::Leggings => material.leggings_texture(),
                _ => material.humanoid_texture(),
            };

            let armor_model = build_armor_model(slot, texture);

            // Build facing rotation (same as armor stand)
            let facing_angle = super::facing_rotation_rad(facing);
            let facing_mat = Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
                * Mat4::from_rotation_y(facing_angle)
                * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5));

            super::traverse_parts(
                &armor_model.parts,
                Mat4::IDENTITY,
                &facing_mat,
                &armor_model,
                &mut vertices,
                &mut indices,
                &mut face_textures,
            );
        }
    }

    (vertices, indices, face_textures)
}

/// Build an armor overlay model for a specific slot.
///
/// Uses the Minecraft humanoid armor model with slightly inflated cubes (inflate=1.0)
/// positioned to match the armor stand's body parts.
/// Armor textures are 64x32 with standard Minecraft player UV layout.
fn build_armor_model(slot: ArmorSlot, texture: &str) -> EntityModelDef {
    let inflate = 1.0;

    let parts = match slot {
        ArmorSlot::Helmet => {
            // Helmet: head cube 8x8x8 at tex (0,0)
            vec![EntityPart {
                cubes: vec![EntityCube {
                    origin: [-4.0, -8.0, -4.0],
                    dimensions: [8.0, 8.0, 8.0],
                    tex_offset: [0, 0],
                    inflate,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: EntityPartPose {
                    position: [0.0, 1.0, 0.0],
                    ..Default::default()
                },
                children: vec![],
            }]
        }
        ArmorSlot::Chestplate => {
            // Body: 8x12x4 at tex (16,16) + arms
            vec![
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-4.0, 0.0, -2.0],
                        dimensions: [8.0, 12.0, 4.0],
                        tex_offset: [16, 16],
                        inflate,
                        mirror: false,
                        skip_faces: vec![],
                    }],
                    pose: Default::default(),
                    children: vec![],
                },
                // Right arm: 4x12x4 at tex (40,16)
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-3.0, -2.0, -2.0],
                        dimensions: [4.0, 12.0, 4.0],
                        tex_offset: [40, 16],
                        inflate,
                        mirror: false,
                        skip_faces: vec![],
                    }],
                    pose: EntityPartPose {
                        position: [-5.0, 2.0, 0.0],
                        ..Default::default()
                    },
                    children: vec![],
                },
                // Left arm: 4x12x4 at tex (40,16) mirrored
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-1.0, -2.0, -2.0],
                        dimensions: [4.0, 12.0, 4.0],
                        tex_offset: [40, 16],
                        inflate,
                        mirror: true,
                        skip_faces: vec![],
                    }],
                    pose: EntityPartPose {
                        position: [5.0, 2.0, 0.0],
                        ..Default::default()
                    },
                    children: vec![],
                },
            ]
        }
        ArmorSlot::Leggings => {
            // Body (leggings top): 8x12x4 at tex (16,16)
            // Right leg: 4x12x4 at tex (0,16)
            // Left leg: 4x12x4 at tex (0,16) mirrored
            vec![
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-4.0, 0.0, -2.0],
                        dimensions: [8.0, 12.0, 4.0],
                        tex_offset: [16, 16],
                        inflate: inflate * 0.5, // slightly less inflate for inner layer
                        mirror: false,
                        skip_faces: vec![],
                    }],
                    pose: Default::default(),
                    children: vec![],
                },
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-2.0, 0.0, -2.0],
                        dimensions: [4.0, 12.0, 4.0],
                        tex_offset: [0, 16],
                        inflate: inflate * 0.5,
                        mirror: false,
                        skip_faces: vec![],
                    }],
                    pose: EntityPartPose {
                        position: [-1.9, 12.0, 0.0],
                        ..Default::default()
                    },
                    children: vec![],
                },
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-2.0, 0.0, -2.0],
                        dimensions: [4.0, 12.0, 4.0],
                        tex_offset: [0, 16],
                        inflate: inflate * 0.5,
                        mirror: true,
                        skip_faces: vec![],
                    }],
                    pose: EntityPartPose {
                        position: [1.9, 12.0, 0.0],
                        ..Default::default()
                    },
                    children: vec![],
                },
            ]
        }
        ArmorSlot::Boots => {
            // Right boot: 4x12x4 at tex (0,16)
            // Left boot: 4x12x4 at tex (0,16) mirrored
            vec![
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-2.0, 0.0, -2.0],
                        dimensions: [4.0, 12.0, 4.0],
                        tex_offset: [0, 16],
                        inflate,
                        mirror: false,
                        skip_faces: vec![],
                    }],
                    pose: EntityPartPose {
                        position: [-1.9, 12.0, 0.0],
                        ..Default::default()
                    },
                    children: vec![],
                },
                EntityPart {
                    cubes: vec![EntityCube {
                        origin: [-2.0, 0.0, -2.0],
                        dimensions: [4.0, 12.0, 4.0],
                        tex_offset: [0, 16],
                        inflate,
                        mirror: true,
                        skip_faces: vec![],
                    }],
                    pose: EntityPartPose {
                        position: [1.9, 12.0, 0.0],
                        ..Default::default()
                    },
                    children: vec![],
                },
            ]
        }
    };

    // Wrap in Y-down → Y-up root (same as armor stand itself)
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: parts,
    };

    EntityModelDef {
        texture_path: texture.to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: false, // Armor textures have transparent regions
    }
}
