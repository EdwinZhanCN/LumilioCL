//! Equipment overlays rendered on top of a mob's base model.
//!
//! MC 1.21.5 uses the equipment asset system: saddles, horse armor, and similar
//! items are rendered as a second pass over the mob's own model, with all cubes
//! inflated slightly and a different texture sampled.
//!
//! We reproduce that here with `inflated_overlay`, which clones a model and
//! bumps every cube's `inflate` field while swapping the texture path.

use super::{EntityModelDef, EntityPart, MobType};
use crate::types::InputBlock;

/// An equipment overlay: a clone of the mob's model rendered with a different
/// texture and a uniform inflate amount (to sit on top of the base skin).
pub(crate) struct EquipmentOverlay {
    pub model: EntityModelDef,
}

/// Return any equipment overlays that apply to this mob's current state.
/// Reads properties like `saddle`, `horse_armor`, or the `rider` prop (a ridden
/// pig or horse always shows a saddle even if `saddle` wasn't set explicitly).
pub(crate) fn overlays_for(
    mob_type: MobType,
    block: &InputBlock,
    base_model: &EntityModelDef,
) -> Vec<EquipmentOverlay> {
    let mut out = Vec::new();

    let saddled = block.properties.get("saddle").map(|v| v == "true").unwrap_or(false)
        || block.properties.contains_key("rider");

    match mob_type {
        MobType::Pig if saddled => {
            // MC's PIG_SADDLE layer uses the pig model with CubeDeformation(0.5).
            out.push(EquipmentOverlay {
                model: inflated_overlay(
                    base_model.clone(),
                    0.5,
                    "entity/equipment/pig_saddle/saddle".to_string(),
                ),
            });
        }
        MobType::Horse => {
            if saddled {
                out.push(EquipmentOverlay {
                    model: inflated_overlay(
                        base_model.clone(),
                        0.1,
                        "entity/equipment/horse_saddle/saddle".to_string(),
                    ),
                });
            }
            if let Some(material) = block.properties.get("horse_armor") {
                let path = format!("entity/equipment/horse_body/{}", material);
                out.push(EquipmentOverlay {
                    model: inflated_overlay(base_model.clone(), 0.1, path),
                });
            }
        }
        _ => {}
    }

    out
}

/// Clone a model, add `amount` to every cube's inflate, and swap the texture.
/// Used to build equipment overlays from a mob's base model.
pub(crate) fn inflated_overlay(
    mut base: EntityModelDef,
    amount: f32,
    texture_path: String,
) -> EntityModelDef {
    inflate_parts(&mut base.parts, amount);
    base.texture_path = texture_path;
    base.is_opaque = false;
    base
}

fn inflate_parts(parts: &mut Vec<EntityPart>, amount: f32) {
    for p in parts {
        for c in &mut p.cubes {
            c.inflate += amount;
        }
        inflate_parts(&mut p.children, amount);
    }
}
