//! A [`ResourcePack`] from a baked pack (LumilioCL local change; see
//! `forks/README.md` and plan schematic-studio, D1).
//!
//! `lumilio-block-assets` has already settled every game version's spelling:
//! models have no parents left, texture variables are texture ids, cull faces
//! are `down`/`up`, multipart values are strings and moved entity textures
//! carry their newest names. This module only changes the shape into the
//! mesher's types; it knows no game version.

use std::collections::HashMap;

use lumilio_block_assets::{
    BakedBlockstate, BakedCase, BakedElement, BakedFace, BakedModel, BakedPack, BakedVariant,
    Condition, Direction as BakedDirection, Rotation,
};

use super::blockstate::{ApplyValue, MultipartCase, MultipartCondition};
use super::texture::{AnimFrame, AnimationMeta};
use super::{BlockModel, BlockstateDefinition, ModelElement, ModelFace, ModelVariant};
use super::{ResourcePack, TextureData};
use crate::types::{Axis, Direction, ElementRotation};

impl ResourcePack {
    /// The mesher's view of a baked pack.
    pub fn from_baked(baked: &BakedPack) -> Self {
        let assets = &baked.assets;
        let mut pack = ResourcePack::new();
        for (id, blockstate) in &assets.blockstates {
            let (namespace, path) = split(id);
            pack.add_blockstate(namespace, path, blockstate_of(blockstate));
        }
        for (id, model) in &assets.models {
            let (namespace, path) = split(id);
            pack.add_model(namespace, path, model_of(model));
        }
        for (id, texture) in &assets.textures {
            let Some(image) = assets.images.get(&texture.image) else {
                continue;
            };
            let mut data = TextureData::new(image.width, image.height, image.rgba.clone());
            if let Some(animation) = &texture.animation {
                data.apply_mcmeta(AnimationMeta {
                    frametime: animation.frametime,
                    interpolate: animation.interpolate,
                    frames: animation.frames.as_ref().map(|frames| {
                        frames
                            .iter()
                            .map(|&(index, time)| AnimFrame { index, time })
                            .collect()
                    }),
                    frame_width: animation.width,
                    frame_height: animation.height,
                });
            }
            let (namespace, path) = split(id);
            pack.add_texture(namespace, path, data);
        }
        pack.paintings = assets
            .paintings
            .iter()
            .map(|(id, variant)| (id.clone(), variant.clone()))
            .collect();
        pack.colormap_samples = assets
            .tint_samples
            .iter()
            .map(|(name, rgb)| (name.clone(), *rgb))
            .collect();
        pack
    }
}

fn split(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

fn blockstate_of(blockstate: &BakedBlockstate) -> BlockstateDefinition {
    match blockstate {
        BakedBlockstate::Variants(variants) => BlockstateDefinition::Variants(
            variants
                .iter()
                .map(|(key, list)| (key.clone(), list.iter().map(variant_of).collect()))
                .collect(),
        ),
        BakedBlockstate::Multipart(cases) => {
            BlockstateDefinition::Multipart(cases.iter().map(case_of).collect())
        }
    }
}

fn variant_of(variant: &BakedVariant) -> ModelVariant {
    ModelVariant {
        model: variant.model.clone(),
        x: variant.x,
        y: variant.y,
        uvlock: variant.uvlock,
        weight: variant.weight,
    }
}

fn case_of(case: &BakedCase) -> MultipartCase {
    MultipartCase {
        when: case.when.as_ref().map(condition_of),
        apply: ApplyValue::Multiple(case.apply.iter().map(variant_of).collect()),
    }
}

fn condition_of(condition: &Condition) -> MultipartCondition {
    match condition {
        Condition::Any(list) => MultipartCondition::Or {
            OR: list.iter().map(condition_of).collect(),
        },
        Condition::All(list) => MultipartCondition::And {
            AND: list.iter().map(condition_of).collect(),
        },
        Condition::Properties(values) => MultipartCondition::Simple(
            values
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
        ),
    }
}

fn model_of(model: &BakedModel) -> BlockModel {
    BlockModel {
        parent: None,
        ambient_occlusion: Some(model.ambient_occlusion),
        textures: model
            .textures
            .iter()
            .map(|(name, id)| (name.clone(), id.clone()))
            .collect(),
        elements: model.elements.iter().map(element_of).collect(),
        display: (!model.display.is_empty()).then(|| {
            model
                .display
                .iter()
                .map(|(context, transform)| {
                    let value = serde_json::json!({
                        "rotation": transform.rotation,
                        "translation": transform.translation,
                        "scale": transform.scale,
                    });
                    (context.clone(), value)
                })
                .collect::<serde_json::Map<_, _>>()
                .into()
        }),
    }
}

fn element_of(element: &BakedElement) -> ModelElement {
    ModelElement {
        from: element.from,
        to: element.to,
        rotation: element.rotation.as_ref().map(rotation_of),
        shade: element.shade,
        shade_direction_override: element.shade_direction_override.map(direction_of),
        faces: element
            .faces
            .iter()
            .map(|(direction, face)| (direction_of(*direction), face_of(face)))
            .collect::<HashMap<_, _>>(),
    }
}

/// A face whose variable led nowhere reads as an unresolved reference, which
/// the mesher draws with its missing-texture tile, as the game does.
fn face_of(face: &BakedFace) -> ModelFace {
    ModelFace {
        uv: face.uv,
        texture: face
            .texture
            .clone()
            .unwrap_or_else(|| "#missing".to_owned()),
        cullface: face.cullface.map(direction_of),
        rotation: face.rotation,
        tintindex: face.tintindex.unwrap_or(-1),
    }
}

/// Angles about one axis are the single-axis form every game version writes;
/// anything else is the 26.x Euler form. Both mean the same rotation.
fn rotation_of(rotation: &Rotation) -> ElementRotation {
    let [x, y, z] = rotation.angles;
    let single = match (x != 0.0, y != 0.0, z != 0.0) {
        (true, false, false) => Some((Axis::X, x)),
        (false, true, false) => Some((Axis::Y, y)),
        (false, false, true) | (false, false, false) => Some((Axis::Z, z)),
        _ => None,
    };
    match single {
        Some((axis, angle)) => {
            ElementRotation::single(rotation.origin, axis, angle, rotation.rescale)
        }
        None => ElementRotation {
            origin: rotation.origin,
            axis: None,
            angle: 0.0,
            x,
            y,
            z,
            rescale: rotation.rescale,
        },
    }
}

fn direction_of(direction: BakedDirection) -> Direction {
    match direction {
        BakedDirection::Down => Direction::Down,
        BakedDirection::Up => Direction::Up,
        BakedDirection::North => Direction::North,
        BakedDirection::South => Direction::South,
        BakedDirection::West => Direction::West,
        BakedDirection::East => Direction::East,
    }
}
