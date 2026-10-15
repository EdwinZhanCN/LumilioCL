//! A baked model layer (`lumilio-block-assets`, from the game's
//! `LayerDefinitions`) as mesher parts, with a row's static pose applied
//! (LumilioCL local change, plan schematic-studio T7).

use std::collections::BTreeMap;

use lumilio_block_assets::{Bone, Direction as BakedDirection, EntityLayer};

use super::bindings::BonePose;
use super::model::{EntityCube, EntityPart, EntityPartPose};
use crate::types::Direction;

/// How a layer is drawn.
#[derive(Default)]
pub struct LayerView<'a> {
    /// Static poses by bone name.
    pub pose: Option<&'a BTreeMap<String, BonePose>>,
    /// Pose angles (degrees) by bone name, as armor stands store them.
    pub angles: BTreeMap<String, [f32; 3]>,
    /// Only these bones and their children are drawn.
    pub bones: Option<&'a [String]>,
    /// Added to every box's growth.
    pub inflate: f32,
    /// Extra scale by bone name.
    pub bone_scale: Option<&'a BTreeMap<String, f32>>,
    /// Texture keys by bone name.
    pub bone_textures: BTreeMap<String, String>,
}

/// The layer's root bone as one part.
pub fn part_of(layer: &EntityLayer, view: &LayerView) -> EntityPart {
    let selected = view.bones.is_none();
    convert("root", &layer.root, view, selected)
}

fn convert(name: &str, bone: &Bone, view: &LayerView, selected: bool) -> EntityPart {
    let selected = selected
        || view
            .bones
            .is_some_and(|bones| bones.iter().any(|b| b == name));
    let pose = view.pose.and_then(|pose| pose.get(name));
    let mut part_pose = EntityPartPose {
        position: bone.pivot,
        rotation: bone.rotation,
        scale: bone.scale,
    };
    let mut hidden: &[Direction] = &[];
    if let Some(pose) = pose {
        if let Some(pivot) = pose.pivot {
            part_pose.position = pivot;
        }
        if let Some(rotation) = pose.rotation {
            part_pose.rotation = rotation.map(f32::to_radians);
        }
        if let Some(scale) = pose.scale {
            part_pose.scale = std::array::from_fn(|i| part_pose.scale[i] * scale[i]);
        }
        hidden = &pose.hide_faces;
    }
    if let Some(angles) = view.angles.get(name) {
        part_pose.rotation = angles.map(f32::to_radians);
    }
    if let Some(&extra) = view.bone_scale.and_then(|scale| scale.get(name)) {
        part_pose.scale = part_pose.scale.map(|s| s * extra);
    }
    let visible = pose.is_none_or(|pose| pose.visible);
    let cubes = if visible && selected {
        bone.cubes
            .iter()
            .map(|cube| EntityCube {
                origin: cube.origin,
                dimensions: cube.size,
                tex_offset: cube.uv,
                uv_scale: cube.uv_scale,
                inflate: cube.inflate.map(|grow| grow + view.inflate),
                mirror: cube.mirror,
                skip_faces: Direction::ALL
                    .into_iter()
                    .filter(|face| {
                        hidden.contains(face)
                            || cube
                                .faces
                                .as_ref()
                                .is_some_and(|drawn| !drawn.contains(&baked(*face)))
                    })
                    .collect(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let children = if visible {
        bone.children
            .iter()
            .map(|(child, bone)| convert(child, bone, view, selected))
            .collect()
    } else {
        Vec::new()
    };
    EntityPart {
        name: name.to_owned(),
        cubes,
        pose: part_pose,
        children,
        texture: view.bone_textures.get(name).cloned(),
    }
}

fn baked(face: Direction) -> BakedDirection {
    match face {
        Direction::Down => BakedDirection::Down,
        Direction::Up => BakedDirection::Up,
        Direction::North => BakedDirection::North,
        Direction::South => BakedDirection::South,
        Direction::West => BakedDirection::West,
        Direction::East => BakedDirection::East,
    }
}
