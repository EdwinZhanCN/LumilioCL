//! Schematic entities as instances of shared models.
//!
//! LumilioCL local change (plan schematic-studio T8; see `forks/README.md`).
//! Each distinct look of an entity — its id and every property the binding
//! table reads, without its place and yaw — is meshed once, facing south at
//! the origin, by the same mesher and pack as the blocks. Every entity is then
//! an instance of that model: a transform to its exact position, turned by its
//! yaw when its renderer turns it ([`Facing::turns_with_yaw`]). The GPU
//! renderer draws all instances of one model in one call and culls them.
//!
//! [`Facing::turns_with_yaw`]: schematic_mesher::mesher::entity::bindings::Facing::turns_with_yaw

use std::collections::{BTreeSet, HashMap};

use schematic_mesher::mesher::entity::bindings;
use schematic_mesher::{
    BlockPosition as MesherBlockPosition, BoundingBox, InputBlock, MeshOutput, Mesher,
};

use super::{MeshConfig, ResourcePackSource, Result};
use crate::entity::{Entity, NbtValue};
use crate::UniversalSchematic;

/// A column-major 4×4 matrix (`m[column][row]`), the layout the shader reads.
pub type Transform = [[f32; 4]; 4];

/// One entity look, meshed in model space (block space of the origin cell).
pub struct EntityModel {
    /// The look's id, `entity:` and the mesher's id (`entity:armor_stand`).
    pub name: String,
    pub mesh: MeshOutput,
    /// Bounding sphere of the mesh, in model space.
    pub center: [f32; 3],
    pub radius: f32,
}

/// One entity: which model, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityInstance {
    /// Index into [`EntityInstances::models`].
    pub model: usize,
    pub transform: Transform,
    /// Turned to the camera each frame instead (T35); `transform` is then
    /// where it stands for culling.
    pub billboard: Option<Billboard>,
}

/// Which way an instance keeps turned to the camera, as
/// `Display.BillboardConstraints` says (LumilioCL local change, plan
/// schematic-studio T35): wholly (`Center`, as sprites always are), about
/// the vertical only, or tilting only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillboardKind {
    Center,
    Vertical,
    Horizontal,
}

/// An instance turned to the camera: at `at`, by the camera's yaw and
/// pitch where `kind` takes them and else by its own (`yaw`, `pitch`,
/// already signed as its renderer turns), then `inner`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Billboard {
    pub kind: BillboardKind,
    pub at: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub inner: Transform,
}

impl Billboard {
    /// Its transform for a camera whose back (toward the viewer) is `back`.
    /// The game turns a camera-facing quad by `rotY(180° - yaw) ·
    /// rotX(-pitch)` of the camera; the back is `(sin yaw cos pitch, sin
    /// pitch, -cos yaw cos pitch)` in those angles.
    pub fn turned(&self, back: [f32; 3]) -> Transform {
        let pitch = back[1].clamp(-1.0, 1.0).asin().to_degrees();
        let yaw = back[0].atan2(-back[2]).to_degrees();
        let (y, x) = match self.kind {
            BillboardKind::Center => (180.0 - yaw, -pitch),
            BillboardKind::Vertical => (180.0 - yaw, self.pitch),
            BillboardKind::Horizontal => (self.yaw, -pitch),
        };
        multiply(&aimed_at(self.at, y, x), &self.inner)
    }
}

/// `a · b`, column-major.
pub fn multiply(a: &Transform, b: &Transform) -> Transform {
    let mut out = [[0.0; 4]; 4];
    for (column, out_column) in out.iter_mut().enumerate() {
        for (row, cell) in out_column.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

const IDENTITY: Transform = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// A schematic's entities, ready for instanced drawing.
#[derive(Default)]
pub struct EntityInstances {
    pub models: Vec<EntityModel>,
    pub instances: Vec<EntityInstance>,
    /// Ids of entities nothing draws: no binding row, or no model layer for
    /// the row in this game version. Sorted and unique, `minecraft:` ids.
    pub undrawable: Vec<String>,
}

impl EntityInstances {
    /// World-space box around every instance's bounding sphere; `None`
    /// without instances.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for instance in &self.instances {
            let model = &self.models[instance.model];
            let center = transform_point(&instance.transform, model.center);
            for axis in 0..3 {
                min[axis] = min[axis].min(center[axis] - model.radius);
                max[axis] = max[axis].max(center[axis] + model.radius);
            }
        }
        (!self.instances.is_empty()).then_some((min, max))
    }
}

impl UniversalSchematic {
    /// The blocks as one mesh, and the entities as instances of shared models
    /// (see the module documentation). Both use one mesher, so one pack.
    pub fn to_mesh_with_entity_instances(
        &self,
        pack: &ResourcePackSource,
        config: &MeshConfig,
    ) -> Result<(MeshOutput, EntityInstances)> {
        let mesher = Mesher::with_config(pack.pack.clone(), config.to_mesher_config());
        let blocks = MeshOutput::from(self.mesh_with(&mesher, false)?);
        let entities = instances(&mesher, &self.get_entities_as_list())?;
        Ok((blocks, entities))
    }
}

/// `point` through `m`.
pub fn transform_point(m: &Transform, point: [f32; 3]) -> [f32; 3] {
    [0, 1, 2]
        .map(|row| m[0][row] * point[0] + m[1][row] * point[1] + m[2][row] * point[2] + m[3][row])
}

/// The mesher puts block `n` at `n - 0.5 ..= n + 0.5`: the model meshed in
/// cell 0 stands with its feet on its centre line at this point.
pub const MODEL_FEET: [f32; 3] = [0.0, -0.5, 0.0];

/// Where Minecraft position `position` is in the mesher's space.
fn mesher_space(position: (f64, f64, f64)) -> [f32; 3] {
    let (x, y, z) = position;
    [x as f32 - 0.5, y as f32 - 0.5, z as f32 - 0.5]
}

/// Facings by Minecraft's six-way id (`Direction.get3DDataValue`).
const FACING_3D: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
/// Horizontal facings by Minecraft's four-way id (`get2DDataValue`).
const FACING_2D: [&str; 4] = ["south", "west", "north", "east"];

/// The facing of an entity hung on a block, from what the game's base class
/// for such entities saves (LumilioCL local change, plan schematic-studio
/// T33, D32): the block it hangs in (`block_pos`, or `TileX`, `TileY` and
/// `TileZ`) and a facing, six-way `Facing` for item frames and four-way
/// `facing` for paintings. `None` for any other entity, and for one hung
/// without a facing (a leash knot). No id list: a new kind of hung entity
/// saves the same.
pub(crate) fn hung_facing(nbt: &HashMap<String, NbtValue>) -> Option<&'static str> {
    if !nbt.contains_key("block_pos") && !nbt.contains_key("TileX") {
        return None;
    }
    let id = |value: Option<&NbtValue>| match value {
        Some(NbtValue::Byte(id)) => usize::try_from(*id).ok(),
        Some(NbtValue::Int(id)) => usize::try_from(*id).ok(),
        _ => None,
    };
    if let Some(id) = id(nbt.get("Facing")) {
        return FACING_3D.get(id).copied();
    }
    id(nbt.get("facing")).and_then(|id| FACING_2D.get(id).copied())
}

/// One block along `facing`.
fn normal(facing: &str) -> [f64; 3] {
    match facing {
        "down" => [0.0, -1.0, 0.0],
        "up" => [0.0, 1.0, 0.0],
        "north" => [0.0, 0.0, -1.0],
        "south" => [0.0, 0.0, 1.0],
        "west" => [-1.0, 0.0, 0.0],
        "east" => [1.0, 0.0, 0.0],
        _ => [0.0; 3],
    }
}

/// The renderer looks the binding table falls back on (T34), by id.
pub(crate) type Looks = HashMap<String, lumilio_block_assets::EntityLook>;

/// The look and placement of one entity; `looks` are the game version's
/// renderer looks, for entities the binding table has no row for.
pub(crate) fn placed(entity: &Entity, looks: &Looks) -> (InputBlock, Transform, Option<Billboard>) {
    let mut look = super::entity_to_input_block(entity);
    let (x, y, z) = entity.position;
    let bound = bindings::find_for(looks, &look);
    if let Some(aim) = bound.as_ref().and_then(|bound| bound.binding.aim) {
        // Drawn at its exact position, turned as its renderer turns it
        // (T35): a hung painting by its facing, others by their rotation;
        // a display entity's `transformation` inside that turn, and kept
        // turned to the camera as its row or its `billboard` says.
        let yaw = aim.yaw
            * hung_facing(&entity.nbt)
                .and_then(facing_yaw)
                .unwrap_or_else(|| yaw(entity));
        let pitch = aim.pitch * pitch(entity);
        let at = mesher_space(entity.position);
        let at = [0, 1, 2].map(|axis| at[axis] + aim.origin[axis]);
        look.properties.remove("facing");
        let inner = display_transformation(&entity.nbt).unwrap_or(IDENTITY);
        let turn = multiply(&aimed_at(at, yaw, pitch), &inner);
        let kind = bound
            .as_ref()
            .and_then(|bound| bound.binding.billboard.as_deref())
            .or(match entity.nbt.get("billboard") {
                Some(NbtValue::String(kind)) => Some(kind.as_str()),
                _ => None,
            })
            .and_then(|kind| match kind {
                "center" => Some(BillboardKind::Center),
                "vertical" => Some(BillboardKind::Vertical),
                "horizontal" => Some(BillboardKind::Horizontal),
                _ => None,
            });
        let billboard = kind.map(|kind| Billboard {
            kind,
            at,
            yaw,
            pitch,
            inner,
        });
        return (look, turn, billboard);
    }
    if let Some(facing) = hung_facing(&entity.nbt) {
        // Hung on a block: drawn in the block it hangs in, whose centre the
        // game's renderer reaches from the entity's position by 15/32 of a
        // block along its facing; the model turns itself (`hanging_pose`).
        let n = normal(facing);
        let cell = [x, y, z]
            .iter()
            .zip(n)
            .map(|(at, along)| (at + along * 15.0 / 32.0).floor() as f32)
            .collect::<Vec<_>>();
        return (
            look,
            turned_at([cell[0], cell[1], cell[2]], 0.0, [0.0; 3]),
            None,
        );
    }
    let turns = bound.is_some_and(|bound| bound.binding.facing.turns_with_yaw());
    if !turns {
        // Drawn in its block as before: item frames, dropped items, and ids
        // without a row (which draw nothing).
        let cell = [x.floor() as f32, y.floor() as f32, z.floor() as f32];
        return (look, turned_at(cell, 0.0, [0.0; 3]), None);
    }
    look.properties
        .insert("facing".to_owned(), "south".to_owned());
    (
        look,
        turned_at(mesher_space(entity.position), yaw(entity), MODEL_FEET),
        None,
    )
}

/// A display entity's `transformation` (`Display`'s save, 1.19.4+): a
/// translation, left rotation, scale and right rotation composed `T · L ·
/// S · R`, each rotation a quaternion `[x, y, z, w]` or an `{angle, axis}`;
/// or sixteen numbers, row by row (T35).
fn display_transformation(nbt: &HashMap<String, NbtValue>) -> Option<Transform> {
    let floats = |value: &NbtValue| -> Option<Vec<f32>> {
        match value {
            NbtValue::List(items) => items
                .iter()
                .map(|item| match item {
                    NbtValue::Float(v) => Some(*v),
                    NbtValue::Double(v) => Some(*v as f32),
                    NbtValue::Int(v) => Some(*v as f32),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    };
    match nbt.get("transformation")? {
        NbtValue::Compound(parts) => {
            let vector = |key: &str, default: [f32; 3]| -> [f32; 3] {
                parts
                    .get(key)
                    .and_then(floats)
                    .filter(|v| v.len() == 3)
                    .map_or(default, |v| [v[0], v[1], v[2]])
            };
            let rotation = |key: &str| -> Transform {
                let quaternion = match parts.get(key) {
                    Some(NbtValue::Compound(angle_axis)) => {
                        let angle = match angle_axis.get("angle") {
                            Some(NbtValue::Float(a)) => *a,
                            Some(NbtValue::Double(a)) => *a as f32,
                            _ => 0.0,
                        };
                        let axis = angle_axis
                            .get("axis")
                            .and_then(floats)
                            .filter(|v| v.len() == 3)
                            .unwrap_or_else(|| vec![0.0, 1.0, 0.0]);
                        let length = axis.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-9);
                        let (s, c) = (angle / 2.0).sin_cos();
                        [
                            axis[0] / length * s,
                            axis[1] / length * s,
                            axis[2] / length * s,
                            c,
                        ]
                    }
                    Some(value) => floats(value)
                        .filter(|v| v.len() == 4)
                        .map_or([0.0, 0.0, 0.0, 1.0], |v| [v[0], v[1], v[2], v[3]]),
                    None => [0.0, 0.0, 0.0, 1.0],
                };
                quaternion_matrix(quaternion)
            };
            let [tx, ty, tz] = vector("translation", [0.0; 3]);
            let [sx, sy, sz] = vector("scale", [1.0; 3]);
            let translate = [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [tx, ty, tz, 1.0],
            ];
            let scale = [
                [sx, 0.0, 0.0, 0.0],
                [0.0, sy, 0.0, 0.0],
                [0.0, 0.0, sz, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            Some(multiply(
                &multiply(&multiply(&translate, &rotation("left_rotation")), &scale),
                &rotation("right_rotation"),
            ))
        }
        value => {
            let m = floats(value).filter(|v| v.len() == 16)?;
            Some(std::array::from_fn(|column| {
                std::array::from_fn(|row| m[row * 4 + column])
            }))
        }
    }
}

/// The rotation of a unit quaternion `[x, y, z, w]`.
fn quaternion_matrix([x, y, z, w]: [f32; 4]) -> Transform {
    let length = (x * x + y * y + z * z + w * w).sqrt().max(1e-9);
    let (x, y, z, w) = (x / length, y / length, z / length, w / length);
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
            0.0,
        ],
        [
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
            0.0,
        ],
        [
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The entity's yaw in degrees (`Rotation[0]`), 0 (south) without one.
fn yaw(entity: &Entity) -> f32 {
    rotation(entity, 0)
}

/// The entity's pitch in degrees (`Rotation[1]`), 0 without one.
fn pitch(entity: &Entity) -> f32 {
    rotation(entity, 1)
}

fn rotation(entity: &Entity, at: usize) -> f32 {
    match entity.nbt.get("Rotation") {
        Some(NbtValue::List(rotation)) => match rotation.get(at) {
            Some(NbtValue::Float(angle)) => *angle,
            Some(NbtValue::Double(angle)) => *angle as f32,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// The yaw `HangingEntity.setDirection` gives a horizontal facing (its
/// two-way id times 90); `None` for up and down.
fn facing_yaw(facing: &str) -> Option<f32> {
    FACING_2D
        .iter()
        .position(|&name| name == facing)
        .map(|id| id as f32 * 90.0)
}

/// Moves the model's origin (the centre of cell 0) to `at`, turned about Y
/// by `yaw_deg`, then about X by `pitch_deg`, right-handed as the game's
/// pose stack and the mesher's steps are (T35).
pub fn aimed_at(at: [f32; 3], yaw_deg: f32, pitch_deg: f32) -> Transform {
    let (sa, ca) = yaw_deg.to_radians().sin_cos();
    let (sb, cb) = pitch_deg.to_radians().sin_cos();
    // Ry(a) · Rx(b), column by column.
    [
        [ca, 0.0, -sa, 0.0],
        [sa * sb, cb, ca * sb, 0.0],
        [sa * cb, -sb, ca * cb, 0.0],
        [at[0], at[1], at[2], 1.0],
    ]
}

/// Moves model point `pivot` to `at`, turned by Minecraft yaw `yaw_deg` about
/// the vertical line through it. Renderers turn by `-yaw` (glam's and the
/// mesher's rotation has the opposite sign of Minecraft yaw), so south (0)
/// leaves the model as meshed and east (-90) turns its face to +X.
pub fn turned_at(at: [f32; 3], yaw_deg: f32, pivot: [f32; 3]) -> Transform {
    let (s, c) = (-yaw_deg.to_radians()).sin_cos();
    // Ry(a) · (-pivot), then + at.
    let px = c * -pivot[0] + s * -pivot[2];
    let pz = -s * -pivot[0] + c * -pivot[2];
    [
        [c, 0.0, -s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [s, 0.0, c, 0.0],
        [at[0] + px, at[1] - pivot[1], at[2] + pz, 1.0],
    ]
}

/// One key per look: the id and every property, sorted.
fn look_key(look: &InputBlock) -> String {
    let mut properties: Vec<_> = look.properties.iter().collect();
    properties.sort();
    let mut key = look.name.clone();
    for (name, value) in properties {
        key.push('\u{1f}');
        key.push_str(name);
        key.push('=');
        key.push_str(value);
    }
    key
}

/// Distinct looks, and one instance per entity naming its look's index.
pub(crate) fn group(
    entities: &[Entity],
    renderers: &Looks,
) -> (Vec<InputBlock>, Vec<EntityInstance>) {
    let mut looks = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut instances = Vec::with_capacity(entities.len());
    for entity in entities {
        let (look, transform, billboard) = placed(entity, renderers);
        let model = *index.entry(look_key(&look)).or_insert_with(|| {
            looks.push(look);
            looks.len() - 1
        });
        instances.push(EntityInstance {
            model,
            transform,
            billboard,
        });
    }
    (looks, instances)
}

/// The `minecraft:` id of a look the mesher calls `entity:<id>`.
fn entity_id(name: &str) -> String {
    let id = name.strip_prefix("entity:").unwrap_or(name);
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

/// Meshes each look of `entities` once and places every entity as an
/// instance; looks that draw nothing are reported and left out.
pub(crate) fn instances(mesher: &Mesher, entities: &[Entity]) -> Result<EntityInstances> {
    let (looks, placed) = group(entities, &mesher.resource_pack().entity_looks);
    let undrawable: BTreeSet<String> =
        schematic_mesher::undrawable_blocks(mesher.resource_pack(), &looks)
            .into_iter()
            .collect();
    let mut models = Vec::new();
    let mut model_of = vec![None; looks.len()];
    for (index, look) in looks.iter().enumerate() {
        if undrawable.contains(&look.name) {
            continue;
        }
        let origin = MesherBlockPosition::new(0, 0, 0);
        let output = mesher
            .mesh_blocks(
                std::iter::once((origin, look)),
                BoundingBox::new([0.0; 3], [1.0; 3]),
            )
            .map_err(|e| super::MeshError::Meshing(e.to_string()))?;
        let mut mesh = MeshOutput::from(output);
        let Some((min, max)) = extent(&mesh) else {
            continue;
        };
        mesh.bounds = BoundingBox::new(min, max);
        let center = [0, 1, 2].map(|axis| (min[axis] + max[axis]) * 0.5);
        let radius = [0, 1, 2]
            .map(|axis| (max[axis] - min[axis]) * 0.5)
            .iter()
            .map(|half| half * half)
            .sum::<f32>()
            .sqrt();
        model_of[index] = Some(models.len());
        models.push(EntityModel {
            name: look.name.clone(),
            mesh,
            center,
            radius,
        });
    }
    let instances = placed
        .into_iter()
        .filter_map(|instance| {
            Some(EntityInstance {
                model: model_of[instance.model]?,
                ..instance
            })
        })
        .collect();
    Ok(EntityInstances {
        models,
        instances,
        undrawable: undrawable
            .iter()
            .map(|name| entity_id(name))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    })
}

/// The box around every vertex of `mesh`; `None` when it has none.
fn extent(mesh: &MeshOutput) -> Option<([f32; 3], [f32; 3])> {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut any = false;
    for layer in [&mesh.opaque, &mesh.cutout, &mesh.transparent] {
        for position in &layer.positions {
            any = true;
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
    }
    any.then_some((min, max))
}

/// Properties for the variant textures the binding table reads: `variant`
/// (cow, pig, chicken, cat, horse) and the villager's `biome` and
/// `profession`, from the entity's NBT in any version's spelling.
pub(super) fn add_variants(id: &str, nbt: &HashMap<String, NbtValue>, input: &mut InputBlock) {
    let plain = |value: &str| value.rsplit(':').next().unwrap_or(value).to_owned();
    let mut set = |name: &str, value: String| {
        input.properties.insert(name.to_owned(), value);
    };
    // 1.19+ registries name the variant: `minecraft:warm`, `minecraft:tabby`.
    if let Some(NbtValue::String(variant)) = nbt.get("variant") {
        set("variant", plain(variant));
    }
    match (id, nbt.get("Variant"), nbt.get("CatType")) {
        // `HorseRenderer`'s textures by `Variant & 0xFF`.
        ("horse", Some(NbtValue::Int(variant)), _) => {
            const HORSES: [&str; 7] = [
                "white",
                "creamy",
                "chestnut",
                "brown",
                "black",
                "gray",
                "darkbrown",
            ];
            if let Some(name) = HORSES.get((variant & 0xFF) as usize) {
                set("variant", (*name).to_owned());
            }
        }
        // Before 1.19 a cat's type was a number.
        ("cat", _, Some(NbtValue::Int(kind))) => {
            const CATS: [&str; 11] = [
                "tabby",
                "black",
                "red",
                "siamese",
                "british_shorthair",
                "calico",
                "persian",
                "ragdoll",
                "white",
                "jellie",
                "all_black",
            ];
            if let Some(name) = CATS.get(*kind as usize) {
                set("variant", (*name).to_owned());
            }
        }
        _ => {}
    }
    if let Some(NbtValue::String(motive)) = nbt.get("Motive") {
        // A painting before 1.19.
        set("variant", plain(motive));
    }
    match (id, nbt.get("RabbitType")) {
        // `Rabbit.Variant` by its saved id; 99 is the killer bunny.
        ("rabbit", Some(NbtValue::Int(kind))) => {
            const RABBITS: [&str; 6] =
                ["brown", "white", "black", "white_splotched", "gold", "salt"];
            let name = match usize::try_from(*kind) {
                Ok(99) => Some("caerbannog"),
                Ok(index) => RABBITS.get(index).copied(),
                Err(_) => None,
            };
            set("variant", name.unwrap_or("brown").to_owned());
        }
        _ => {}
    }
    if id == "rabbit" && custom_name(nbt).as_deref() == Some("Toast") {
        set("variant", "toast".to_owned());
    }
    if let ("tropical_fish", Some(NbtValue::Int(packed))) = (id, nbt.get("Variant")) {
        // `TropicalFish.Variant`: the pattern (its size in the low byte, its
        // index in the next) and two dye colours.
        let packed = *packed as u32;
        set(
            "size",
            if packed & 0xFF == 1 { "large" } else { "small" }.to_owned(),
        );
        set("pattern", (((packed >> 8) & 0xFF).min(5) + 1).to_string());
        set("base_color", dye_name((packed >> 16) & 0xFF).to_owned());
        set("pattern_color", dye_name((packed >> 24) & 0xFF).to_owned());
    }
    if let Some(NbtValue::String(color)) = nbt.get("color") {
        // A cushion's dye.
        set("color", plain(color));
    }
    if flag(nbt, "dangerous") == Some(true) {
        set("dangerous", "true".to_owned());
    }
    if flag(nbt, "ShowBottom") == Some(false) {
        set("show_bottom", "false".to_owned());
    }
    // The block an entity carries, by the field its base class saves: a
    // falling block's `BlockState`, primed TNT's and a block display's
    // `block_state`, a minecart's `DisplayState` (before 1.21 only with
    // `CustomDisplayTile`) and `DisplayOffset`.
    let custom_tile = flag(nbt, "CustomDisplayTile") != Some(false);
    for key in ["BlockState", "block_state", "DisplayState"] {
        if key == "DisplayState" && !custom_tile {
            continue;
        }
        if let Some(state) = nbt.get(key).and_then(block_state) {
            set("block", state);
        }
    }
    if let Some(NbtValue::Int(offset)) = nbt.get("DisplayOffset") {
        if custom_tile {
            set("block_offset", offset.to_string());
        }
    }
    // The default skin of a body drawn with the player's model is the
    // game's pick by UUID: a mannequin's profile's, the NIL UUID without one
    // (`ResolvableProfile.Static.EMPTY`). Only these: a UUID would make
    // every other entity a look of its own.
    if id == "mannequin" || id == "player" {
        let ints = |value: Option<&NbtValue>| match value {
            Some(NbtValue::IntArray(ints)) if ints.len() == 4 => Some(ints.clone()),
            _ => None,
        };
        let profile_id = match nbt.get("profile") {
            Some(NbtValue::Compound(profile)) => ints(profile.get("id")),
            _ => None,
        };
        let uuid = profile_id.or_else(|| ints(nbt.get("UUID"))).map_or_else(
            || "00000000-0000-0000-0000-000000000000".to_owned(),
            |ints| {
                let hex: String = ints
                    .iter()
                    .map(|part| format!("{:08x}", *part as u32))
                    .collect();
                format!(
                    "{}-{}-{}-{}-{}",
                    &hex[..8],
                    &hex[8..12],
                    &hex[12..16],
                    &hex[16..20],
                    &hex[20..]
                )
            },
        );
        set("uuid", uuid);
    }
    // An experience orb's worth, and what a display entity shows: an item
    // display's context, a text display's text (a component as JSON), its
    // wrapping width, background and alignment.
    match nbt.get("Value") {
        Some(NbtValue::Short(value)) => set("value", value.to_string()),
        Some(NbtValue::Int(value)) => set("value", value.to_string()),
        _ => {}
    }
    if let Some(NbtValue::String(context)) = nbt.get("item_display") {
        set("item_display", context.clone());
    }
    match nbt.get("text") {
        Some(NbtValue::String(text)) => set("text", text.clone()),
        Some(value @ (NbtValue::Compound(_) | NbtValue::List(_))) => {
            set("text", nbt_json(value).to_string());
        }
        _ => {}
    }
    if let Some(NbtValue::Int(width)) = nbt.get("line_width") {
        set("line_width", width.to_string());
    }
    if let Some(NbtValue::Int(background)) = nbt.get("background") {
        set("background", background.to_string());
    }
    if let Some(NbtValue::String(alignment)) = nbt.get("alignment") {
        set("alignment", alignment.clone());
    }
    if let Some(NbtValue::Compound(data)) = nbt.get("VillagerData") {
        if let Some(NbtValue::String(biome)) = data.get("type") {
            set("biome", plain(biome));
        }
        if let Some(NbtValue::String(profession)) = data.get("profession") {
            set("profession", plain(profession));
        }
    }
}

/// A saved block state (`{Name, Properties}`) as written:
/// `minecraft:chest[facing=north]`, properties in name order.
fn block_state(value: &NbtValue) -> Option<String> {
    let NbtValue::Compound(state) = value else {
        return None;
    };
    let Some(NbtValue::String(name)) = state.get("Name") else {
        return None;
    };
    let mut properties: Vec<(String, String)> = match state.get("Properties") {
        Some(NbtValue::Compound(properties)) => properties
            .iter()
            .filter_map(|(key, value)| match value {
                NbtValue::String(value) => Some((key.clone(), value.clone())),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    if properties.is_empty() {
        return Some(name.clone());
    }
    properties.sort();
    let inner: Vec<String> = properties
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    Some(format!("{name}[{}]", inner.join(",")))
}

/// An entity's given name as plain text: a string, a JSON text component
/// (`"Toast"`, `{"text":"Toast"}`), or a component compound.
fn custom_name(nbt: &HashMap<String, NbtValue>) -> Option<String> {
    match nbt.get("CustomName")? {
        NbtValue::String(name) => {
            let trimmed = name.trim();
            if let Ok(serde_json::Value::String(text)) = serde_json::from_str(trimmed) {
                return Some(text);
            }
            if let Ok(serde_json::Value::Object(component)) = serde_json::from_str(trimmed) {
                return component.get("text")?.as_str().map(str::to_owned);
            }
            Some(trimmed.to_owned())
        }
        NbtValue::Compound(component) => match component.get("text") {
            Some(NbtValue::String(text)) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Saved NBT as the JSON a text component is written in (a text display's
/// `text` from 1.21.5 is a compound).
fn nbt_json(value: &NbtValue) -> serde_json::Value {
    use serde_json::Value;
    match value {
        NbtValue::String(text) => Value::String(text.clone()),
        NbtValue::Int(v) => Value::from(*v),
        NbtValue::Long(v) => Value::from(*v),
        NbtValue::Float(v) => Value::from(*v),
        NbtValue::Double(v) => Value::from(*v),
        NbtValue::Byte(v) => Value::from(*v),
        NbtValue::Short(v) => Value::from(*v),
        NbtValue::Boolean(v) => Value::Bool(*v),
        NbtValue::IntArray(v) => Value::from(v.clone()),
        NbtValue::LongArray(v) => Value::from(v.clone()),
        NbtValue::ByteArray(v) => Value::from(v.clone()),
        NbtValue::List(items) => Value::Array(items.iter().map(nbt_json).collect()),
        NbtValue::Compound(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), nbt_json(v))).collect())
        }
    }
}

/// A saved flag, as a byte or a boolean.
fn flag(nbt: &HashMap<String, NbtValue>, key: &str) -> Option<bool> {
    match nbt.get(key)? {
        NbtValue::Byte(value) => Some(*value != 0),
        NbtValue::Boolean(value) => Some(*value),
        _ => None,
    }
}

/// A dye by its id (`DyeColor.byId`), white past the last.
fn dye_name(id: u32) -> &'static str {
    const DYES: [&str; 16] = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];
    DYES.get(id as usize).copied().unwrap_or("white")
}

#[cfg(test)]
mod tests;
