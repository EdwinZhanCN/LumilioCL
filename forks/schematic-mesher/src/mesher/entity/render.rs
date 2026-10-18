//! A bound block or entity as faces: the row's layers, placed and posed,
//! then its overlays and armor (LumilioCL local change, plan schematic-studio
//! T7). Composition code hands in the textures it made ([`Look`]).

use std::collections::{BTreeMap, HashMap};
use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Mat4, Vec3};
use lumilio_block_assets::EntityLayer;

use super::armor;
use super::bindings::{Bound, Facing, LayerRef, OneOrMany, Step};
use super::layers::{part_of, LayerView};
use super::model::{emit, EntityModelDef, Geometry};
use crate::resource_pack::ResourcePack;

/// Textures and variables composition code decided for one block.
#[derive(Debug, Default, Clone)]
pub struct Look {
    /// The main layers' texture key instead of the row's.
    pub texture: Option<String>,
    /// Texture keys by bone name instead of the row's.
    pub bone_textures: BTreeMap<String, String>,
    /// Variables instead of the block's.
    pub vars: HashMap<String, String>,
    /// Draws the main layers in the blended pass, or not.
    pub transparent: Option<bool>,
}

impl Bound {
    /// A variable, looking at `look` first.
    fn var_in<'a>(&'a self, look: &'a Look, name: &str) -> Option<&'a str> {
        look.vars
            .get(name)
            .map(String::as_str)
            .or_else(|| self.var(name))
    }

    fn expand_in(&self, look: &Look, pattern: &str) -> Option<String> {
        super::bindings::expand(pattern, |name| self.var_in(look, name).map(str::to_owned))
    }
}

/// The layer a reference names, for this block.
fn layer<'p>(
    pack: &'p ResourcePack,
    bound: &Bound,
    look: &Look,
    reference: &LayerRef,
) -> Option<&'p EntityLayer> {
    pack.entity_layers
        .get(&bound.expand_in(look, reference.layer())?)
}

/// The first texture of `candidates` the pack has; else the first, which then
/// draws as the missing texture.
fn pick_texture(
    pack: &ResourcePack,
    bound: &Bound,
    look: &Look,
    candidates: &OneOrMany,
) -> Option<String> {
    let expanded: Vec<String> = candidates
        .items()
        .iter()
        .filter_map(|pattern| bound.expand_in(look, pattern))
        .collect();
    expanded
        .iter()
        .find(|key| pack.get_texture(key).is_some())
        .or(expanded.first())
        .cloned()
}

/// The row's own texture for `bound`, the first candidate the pack has.
pub fn row_texture(pack: &ResourcePack, bound: &Bound) -> Option<String> {
    pick_texture(
        pack,
        bound,
        &Look::default(),
        bound.binding.texture.as_ref()?,
    )
}

/// Whether the pack has every layer the row's main draw needs.
pub fn has_geometry(pack: &ResourcePack, bound: &Bound) -> bool {
    let look = Look::default();
    bound
        .binding
        .layers
        .iter()
        .any(|reference| layer(pack, bound, &look, reference).is_some())
}

/// The renderer's placement of the model in its block: the facing turn, then
/// the row's steps. Block-local, in blocks.
pub fn placement(bound: &Bound, look: &Look) -> Mat4 {
    facing_matrix(bound, look) * steps_matrix(bound.binding.steps())
}

/// The row's facing turn alone, about the block (T35: a carried block turns
/// with its minecart).
pub fn facing(bound: &Bound) -> Mat4 {
    facing_matrix(bound, &Look::default())
}

pub fn steps_matrix(steps: &[Step]) -> Mat4 {
    steps.iter().fold(Mat4::IDENTITY, |m, step| {
        m * match *step {
            Step::Translate(t) => Mat4::from_translation(Vec3::from(t) / 16.0),
            Step::Rotate([x, y, z]) => {
                Mat4::from_rotation_z(z.to_radians())
                    * Mat4::from_rotation_y(y.to_radians())
                    * Mat4::from_rotation_x(x.to_radians())
            }
            Step::Scale(s) => Mat4::from_scale(Vec3::from(s)),
        }
    })
}

/// Turn about the block's vertical centre line.
fn about_centre(angle: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
        * Mat4::from_rotation_y(angle)
        * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5))
}

fn facing_matrix(bound: &Bound, look: &Look) -> Mat4 {
    let facing = bound.var_in(look, "facing").unwrap_or("north");
    let rotation16 = || {
        let step: f32 = bound
            .var_in(look, "rotation")
            .and_then(|r| r.parse().ok())
            .unwrap_or(0.0);
        step * PI / 8.0
    };
    match bound.binding.facing {
        Facing::None => Mat4::IDENTITY,
        Facing::Horizontal => about_centre(super::facing_rotation_rad(facing)),
        Facing::Rotation16 => about_centre(rotation16()),
        Facing::Six => {
            let centre = Vec3::splat(0.5);
            let turn = match facing {
                "down" => Mat4::from_rotation_x(PI),
                "north" => Mat4::from_rotation_x(FRAC_PI_2),
                "south" => Mat4::from_rotation_x(-FRAC_PI_2),
                "east" => Mat4::from_rotation_z(-FRAC_PI_2),
                "west" => Mat4::from_rotation_z(FRAC_PI_2),
                _ => Mat4::IDENTITY,
            };
            Mat4::from_translation(centre) * turn * Mat4::from_translation(-centre)
        }
        // `SkullBlockRenderer` faces (sin yRot, 0, -cos yRot); the model's face
        // is +Z, so the turn is π - yRot.
        Facing::Skull => about_centre(PI - rotation16()),
        Facing::WallSkull => {
            let (yrot, dx, dz) = match facing {
                "south" => (PI, 0.0, -0.25),
                "east" => (FRAC_PI_2, -0.25, 0.0),
                "west" => (-FRAC_PI_2, 0.25, 0.0),
                _ => (0.0, 0.0, 0.25),
            };
            Mat4::from_translation(Vec3::new(dx, 0.25, dz)) * about_centre(PI - yrot)
        }
        // AbstractBoatRenderer: T(0, 0.375, 0) · Ry(180° - yRot) · S(-1, -1, 1) · Ry(90°).
        Facing::Boat => {
            let yrot: f32 = match facing {
                "west" => 90.0,
                "north" => 180.0,
                "east" => 270.0,
                _ => 0.0,
            };
            Mat4::from_translation(Vec3::new(0.5, 0.375, 0.5))
                * Mat4::from_rotation_y((180.0 - yrot).to_radians())
                * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
                * Mat4::from_rotation_y(FRAC_PI_2)
        }
    }
}

/// Colours `vertices` by a dye: `wool` as a sheep's wool from the `color`
/// variable, `dye:<name>` the named variable's diffuse colour; white
/// without it.
fn tint(
    vertices: &mut [crate::mesher::geometry::Vertex],
    bound: &Bound,
    look: &Look,
    spec: Option<&str>,
) {
    let color = match spec {
        Some("wool") => super::wool_rgb(bound.var_in(look, "color").unwrap_or("white")),
        Some(spec) => match spec.strip_prefix("dye:") {
            Some(name) => super::dye_rgb(bound.var_in(look, name).unwrap_or("white")),
            None => return,
        },
        None => return,
    };
    for vertex in vertices {
        for (channel, factor) in vertex.color.iter_mut().zip(color) {
            *channel *= factor;
        }
    }
}

/// Every face of `bound`: its main layers, overlays and worn armor, in block
/// space (0..1). `None` when the pack has none of the main layers.
pub fn draw(pack: &ResourcePack, bound: &Bound, look: &Look) -> Option<Geometry> {
    let binding = bound.binding;
    let place = placement(bound, look);
    let mut out: Geometry = (Vec::new(), Vec::new(), Vec::new());

    // A baby uses its own layers where the version has them, else the
    // adult's scaled down.
    let baby = binding.baby.as_ref().filter(|_| bound.flag("is_baby"));
    let own_baby = baby.filter(|baby| {
        !baby.layers.is_empty()
            && baby
                .layers
                .iter()
                .all(|reference| layer(pack, bound, look, reference).is_some())
    });
    let (layers, texture, inner) = match (own_baby, baby) {
        (Some(baby), _) => (
            &baby.layers,
            baby.texture.as_ref().or(binding.texture.as_ref()),
            Mat4::IDENTITY,
        ),
        (None, Some(baby)) => (
            &binding.layers,
            binding.texture.as_ref(),
            steps_matrix(&baby.steps),
        ),
        (None, None) => (&binding.layers, binding.texture.as_ref(), Mat4::IDENTITY),
    };
    let bone_scale = baby
        .filter(|_| own_baby.is_none())
        .map(|baby| &baby.bone_scale);
    // A renderer scale the layer does not carry yet, about the feet (model y 24).
    let unscaled = layers
        .iter()
        .find_map(|reference| layer(pack, bound, look, reference))
        .is_some_and(|found| found.root.scale == [1.0; 3]);
    let inner = match binding.renderer_scale {
        Some(scale) if unscaled => {
            steps_matrix(&[
                Step::Translate([0.0, 24.0, 0.0]),
                Step::Scale([scale; 3]),
                Step::Translate([0.0, -24.0, 0.0]),
            ]) * inner
        }
        _ => inner,
    };
    let angles: BTreeMap<String, [f32; 3]> = binding
        .pose_properties
        .iter()
        .filter_map(|(property, bone)| {
            Some((
                bone.clone(),
                armor::parse_angles(bound.var_in(look, property)?),
            ))
        })
        .collect();

    let mut row_pose = binding.pose.clone();
    for conditional in &binding.pose_when {
        if bound.holds(&conditional.when) {
            row_pose.extend(conditional.pose.clone());
        }
    }

    let mut any = false;
    for reference in layers {
        let Some(found) = layer(pack, bound, look, reference) else {
            continue;
        };
        any = true;
        let texture_path = look
            .texture
            .clone()
            .or_else(|| pick_texture(pack, bound, look, reference.texture().or(texture)?))
            .unwrap_or_default();
        let mut bone_textures: BTreeMap<String, String> = reference
            .bone_textures()
            .into_iter()
            .flatten()
            .filter_map(|(bone, candidates)| {
                Some((bone.clone(), pick_texture(pack, bound, look, candidates)?))
            })
            .collect();
        bone_textures.extend(look.bone_textures.clone());
        let pose = reference.pose_over(&row_pose);
        let view = LayerView {
            pose: Some(&pose),
            angles: angles.clone(),
            bones: reference.bones(),
            inflate: 0.0,
            bone_scale,
            bone_textures,
        };
        let model = EntityModelDef {
            texture_path,
            texture_size: found.texture_size,
            parts: vec![part_of(found, &view)],
            is_opaque: look.transparent.map_or(binding.opaque, |t| !t),
        };
        let first = out.0.len();
        emit(&model, place * inner, &mut out);
        tint(&mut out.0[first..], bound, look, binding.tint.as_deref());
    }
    if !any {
        return None;
    }

    for overlay in &binding.overlays {
        if !bound.holds(&overlay.when) {
            continue;
        }
        let Some((reference, found)) = overlay
            .layers
            .iter()
            .find_map(|reference| Some((reference, layer(pack, bound, look, reference)?)))
        else {
            continue;
        };
        let Some(texture_path) = pick_texture(pack, bound, look, &overlay.texture) else {
            continue;
        };
        let pose = reference.pose_over(&row_pose);
        let view = LayerView {
            pose: Some(&pose),
            angles: angles.clone(),
            bones: reference.bones(),
            inflate: overlay.inflate,
            bone_scale,
            bone_textures: BTreeMap::new(),
        };
        let model = EntityModelDef {
            texture_path,
            texture_size: found.texture_size,
            parts: vec![part_of(found, &view)],
            is_opaque: overlay.opaque,
        };
        let first = out.0.len();
        emit(&model, place * inner, &mut out);
        tint(&mut out.0[first..], bound, look, overlay.tint.as_deref());
    }

    for slot in armor::SLOTS {
        let Some(texture_path) = bound
            .var_in(look, slot)
            .and_then(|item| armor::texture(item, slot))
        else {
            continue;
        };
        let Some((reference, found)) = binding.armor.get(slot).and_then(|candidates| {
            candidates
                .iter()
                .find_map(|reference| Some((reference, layer(pack, bound, look, reference)?)))
        }) else {
            continue;
        };
        let pose = reference.pose_over(&row_pose);
        let view = LayerView {
            pose: Some(&pose),
            angles: angles.clone(),
            bones: reference.bones(),
            inflate: 0.0,
            bone_scale,
            bone_textures: BTreeMap::new(),
        };
        let model = EntityModelDef {
            texture_path,
            texture_size: found.texture_size,
            parts: vec![part_of(found, &view)],
            is_opaque: false,
        };
        emit(&model, place * inner, &mut out);
    }
    Some(out)
}
