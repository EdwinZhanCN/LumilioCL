use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose, MobType};
use super::armor_stand;
use super::bat;
use super::boat;
use super::cat;
use super::chicken;
use super::cow;
use super::enderman;
use super::horse;
use super::iron_golem;
use super::minecart;
use super::sheep;
use super::blaze;
use super::ghast;
use super::magma_cube;
use super::slime;
use super::spider;
use super::villager;
use super::wolf;
use crate::types::InputBlock;

pub(crate) fn build_mob_model(mob_type: MobType, block: &InputBlock) -> EntityModelDef {
    let mut model = match mob_type {
        MobType::Zombie => zombie_model(),
        MobType::Skeleton => skeleton_model(),
        MobType::Creeper => creeper_model(),
        MobType::Pig => pig_model(),
        MobType::Chicken => chicken::chicken_model(),
        MobType::Cow => cow::cow_model(),
        MobType::Sheep => sheep::sheep_model(),
        MobType::Villager => villager::villager_model(),
        MobType::ArmorStand => armor_stand::armor_stand_model(block),
        MobType::Minecart => minecart::minecart_model(),
        MobType::Wolf => wolf::wolf_model(),
        MobType::Cat => cat::cat_model(),
        MobType::Spider => spider::spider_model(),
        MobType::Horse => horse::horse_model(),
        MobType::Enderman => enderman::enderman_model(),
        MobType::Slime => slime::slime_model(),
        MobType::MagmaCube => magma_cube::magma_cube_model(),
        MobType::Blaze => blaze::blaze_model(),
        MobType::Ghast => ghast::ghast_model(),
        MobType::WitherSkeleton => {
            // The skeleton's build with the wither skeleton's skin — how
            // vanilla draws it too, modulo the 1.2x renderer scale.
            let mut model = skeleton_model();
            model.texture_path = "entity/skeleton/wither_skeleton".to_string();
            model
        }
        MobType::IronGolem => iron_golem::iron_golem_model(),
        MobType::Bat => bat::bat_model(),
        MobType::Boat => boat::boat_model(
            block.properties.get("wood").map(|s| s.as_str()).unwrap_or("oak"),
            false,
        ),
        MobType::ChestBoat => boat::boat_model(
            block.properties.get("wood").map(|s| s.as_str()).unwrap_or("oak"),
            true,
        ),
        MobType::ItemFrame | MobType::GlowItemFrame | MobType::DroppedItem | MobType::Player => {
            unreachable!("Item frames, dropped items, and players handled in generate_mob_geometry/add_mob")
        }
    };

    // Apply baby scaling if is_baby property is set
    if block.properties.get("is_baby").map(|v| v == "true").unwrap_or(false)
        && supports_baby(mob_type)
    {
        apply_baby_scaling(&mut model, mob_type);
    }

    model
}

/// Mob types that support baby variants in Minecraft.
fn supports_baby(mob_type: MobType) -> bool {
    matches!(
        mob_type,
        MobType::Cow
            | MobType::Pig
            | MobType::Sheep
            | MobType::Chicken
            | MobType::Wolf
            | MobType::Cat
            | MobType::Horse
            | MobType::Villager
            | MobType::Zombie
    )
}

/// Head-part indices within each mob's top-level children. MC's
/// `BabyModelTransform` uses part names (`Set.of("head")` etc.), but our
/// EntityPart tree is indexed by position — so we encode the index here.
/// Almost every mob puts `head` at index 0; horses are `(body, head_parts, …)`.
fn baby_head_indices(mob_type: MobType) -> &'static [usize] {
    match mob_type {
        MobType::Horse => &[1],
        _ => &[0],
    }
}

/// Extra multiplier applied to the head on top of the uniform 0.5× baby scale,
/// so the head ends up proportionally bigger than the body. Values chosen to
/// match MC's `BabyModelTransform(scaleHead, babyHeadScale, babyBodyScale)`
/// ratio of `(scaleHead ? 1.5/babyHeadScale : 1.0) / (1/babyBodyScale)`.
fn baby_head_extra_scale(mob_type: MobType) -> f32 {
    match mob_type {
        // Humanoids: 1.5/2 for head ÷ 1/2 for body = 1.5
        MobType::Zombie | MobType::Skeleton => 1.5,
        // Villager: MC uses uniform scaling (no big head)
        MobType::Villager => 1.0,
        // Horse: 1.5/2.7272 ÷ 1/2 ≈ 1.1
        MobType::Horse => 1.1,
        // Everything else: head f=1.0 ÷ body f=0.5 = 2.0 (cow/pig/sheep default)
        _ => 2.0,
    }
}

/// Scale a mob model to baby proportions: uniform 0.5× on the root, plus an
/// extra multiplier on the head part(s) so the head remains proportionally
/// larger than the body — the characteristic "big head baby" look.
fn apply_baby_scaling(model: &mut EntityModelDef, mob_type: MobType) {
    let root = match model.parts.first_mut() {
        Some(r) => r,
        None => return,
    };
    // Uniform 0.5× root scale; Y translate halves so feet stay on the ground.
    root.pose.scale = [0.5, 0.5, 0.5];
    root.pose.position[1] = 12.0;

    let head_mul = baby_head_extra_scale(mob_type);
    if head_mul == 1.0 {
        return;
    }
    for (i, child) in root.children.iter_mut().enumerate() {
        if baby_head_indices(mob_type).contains(&i) {
            child.pose.scale[0] *= head_mul;
            child.pose.scale[1] *= head_mul;
            child.pose.scale[2] *= head_mul;
        }
    }
}

/// Wrap mob body parts in a root part that converts Java Y-down to Y-up.
/// RotX(PI) flips Y and Z. Position [8, 24, 8] centers X/Z and translates up
/// so feet land at ground level (24/16 = 1.5 blocks up).
pub(super) fn mob_root(children: Vec<EntityPart>) -> EntityPart {
    EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children,
    }
}

/// Zombie model — texture `entity/zombie/zombie`, 64x64.
fn zombie_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let hat = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [32, 0],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -2.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [40, 16],
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

    let left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [40, 16],
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

    let right_leg = EntityPart {
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
        children: vec![],
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
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

    let root = mob_root(vec![head, hat, body, right_arm, left_arm, right_leg, left_leg]);

    EntityModelDef {
        texture_path: "entity/zombie/zombie".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // Hat overlay has transparent pixels
    }
}

/// Skeleton model — texture `entity/skeleton/skeleton`, 64x32.
/// Same structure as zombie but 2-wide arms/legs.
fn skeleton_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [40, 16],
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

    let left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [40, 16],
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

    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [0, 16],
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
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [0, 16],
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

    let root = mob_root(vec![head, body, right_arm, left_arm, right_leg, left_leg]);

    EntityModelDef {
        texture_path: "entity/skeleton/skeleton".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: false, // Ribcage has transparent pixels
    }
}

/// Creeper model — texture `entity/creeper/creeper`, 64x32.
/// Quadruped with 4 identical short legs.
fn creeper_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 6.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 6.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 18.0, 4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, 18.0, 4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 18.0, -4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, 18.0, -4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let root = mob_root(vec![head, body, right_hind_leg, left_hind_leg, right_front_leg, left_front_leg]);

    EntityModelDef {
        texture_path: "entity/creeper/creeper".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}

/// Pig model — texture `entity/pig/temperate_pig`, 64x64.
/// Snout is a child of head. Body has RotX(PI/2).
fn pig_model() -> EntityModelDef {
    let snout = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -9.0],
            dimensions: [4.0, 3.0, 1.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -4.0, -8.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 12.0, -6.0],
            ..Default::default()
        },
        children: vec![snout],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-5.0, -10.0, -7.0],
            dimensions: [10.0, 16.0, 8.0],
            tex_offset: [28, 8],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 11.0, 2.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 18.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 18.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 18.0, -5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 18.0, -5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let root = mob_root(vec![head, body, right_hind_leg, left_hind_leg, right_front_leg, left_front_leg]);

    EntityModelDef {
        texture_path: "entity/pig/temperate_pig".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}
