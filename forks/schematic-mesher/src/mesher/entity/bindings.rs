//! The entity binding table, `data/entity_bindings.json` (LumilioCL local
//! change; see `forks/README.md`, plan schematic-studio T7 and D5).
//!
//! Which block or entity draws which model layer, with which texture, posed
//! and placed how, comes from Minecraft's renderer classes, not from any
//! resource the game ships, so it is written by hand. It is written once, as
//! data: a row names an id pattern, the layers (geometry baked from the game
//! into `entities.dat.z`), a texture pattern and a static pose. The code that
//! reads the rows knows no entity by name; composition (banner patterns, sign
//! text, pot sherds, skins, frame contents) is code a row switches on.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use lumilio_block_assets::EntityLook;
use serde::Deserialize;

use crate::types::InputBlock;

/// The table format this code reads.
pub const BINDING_FORMAT: u32 = 1;

const TABLE: &str = include_str!("../../../data/entity_bindings.json");

/// The parsed table. It is part of the crate, so a broken file is a bug the
/// `entity_bindings` tests report, not a runtime condition.
pub fn table() -> &'static BindingTable {
    static TABLE_CELL: OnceLock<BindingTable> = OnceLock::new();
    TABLE_CELL.get_or_init(|| parse_table(TABLE).expect("data/entity_bindings.json is valid"))
}

/// A binding table from its JSON text (the lint reads altered copies).
pub fn parse_table(text: &str) -> Result<BindingTable, String> {
    let table: BindingTable = serde_json::from_str(text).map_err(|error| error.to_string())?;
    if table.format != BINDING_FORMAT {
        return Err(format!("binding table format {}", table.format));
    }
    Ok(table)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingTable {
    pub format: u32,
    /// Shared renderer placements, by name, used by rows' `transform`.
    #[serde(default)]
    pub transforms: HashMap<String, Vec<Step>>,
    /// Rows in priority order: the first whose pattern and conditions match wins.
    pub bindings: Vec<Binding>,
    /// Entities the game draws nothing for, beyond those whose renderer is
    /// `NoopRenderer` (LumilioCL local change, plan schematic-studio T35).
    #[serde(default)]
    pub nothing: Vec<String>,
    /// Rows by renderer class (`ArrowRenderer`), for entities without a row
    /// of their own whose look names that renderer: one placement per kind of
    /// renderer, not per entity (T35, D32). Their patterns are not read; the
    /// first row whose conditions hold wins. `{look_texture}` is the look's
    /// texture, else the first texture its renderer keeps in a field.
    #[serde(default)]
    pub renderers: BTreeMap<String, Vec<Binding>>,
}

/// One step of a renderer placement, in 1/16 block and degrees. Steps apply
/// outermost first, like a pose stack.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Translate([f32; 3]),
    /// Rotation about X, Y and Z, applied Z, then Y, then X like a model part.
    Rotate([f32; 3]),
    Scale([f32; 3]),
}

/// How the block's or entity's own state turns the model; renderer code.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Facing {
    /// No turn.
    #[default]
    None,
    /// The `facing` property, about the block's vertical centre line.
    Horizontal,
    /// The 0–15 `rotation` property of standing signs, banners and skulls.
    Rotation16,
    /// The six-way `facing` of shulker boxes, about the block centre.
    Six,
    /// Standing skulls: `rotation`, turned as `SkullBlockRenderer` does.
    Skull,
    /// Wall skulls: `facing`, against the wall behind them.
    WallSkull,
    /// Boats: `AbstractBoatRenderer`'s placement for the `facing` yaw.
    Boat,
}

impl Facing {
    /// Whether an entity's renderer turns it by its yaw. Such an entity can
    /// be drawn once facing south and placed at any yaw by its instance
    /// transform, about the model's centre (LumilioCL local change, plan
    /// schematic-studio T8).
    pub fn turns_with_yaw(self) -> bool {
        matches!(self, Self::Horizontal | Self::Boat)
    }
}

/// Composition code a row switches on.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Compose {
    /// The base color and the `patterns` property, drawn onto the flag.
    Banner,
    /// `text1`–`text4` drawn onto a standing sign's board.
    StandingSignText,
    /// `text1`–`text4` drawn onto both faces of a wall sign's board.
    WallSignText,
    /// `text1`–`text4` drawn onto a hanging sign's board.
    HangingSignText,
    /// The `sherds` property chooses each side's pattern.
    DecoratedPot,
    /// A `skin_base64` or `skin` property, else Steve or Alex by `uuid`.
    PlayerSkin,
    /// The base skin with the biome and profession overlays.
    Villager,
    /// The frame's own block model and the `item` it holds.
    ItemFrame,
    /// The `item` property's sprite or block model.
    DroppedItem,
    /// The `variant` painting: its picture on the front, the back texture
    /// behind and round the edges, as large as the variant (T35).
    Painting,
    /// The row's sprite showing the experience orb icon its `value` earns
    /// (T35).
    ExperienceOrb,
    /// The `item` drawn in its `item_display` context (T35).
    ItemDisplay,
    /// The `text` lines on their `background` (T35).
    TextDisplay,
}

/// A flat picture facing +Z, `from` and `to` its corners in blocks, the
/// texture's `uv` rectangle on it (all of it without one): the quads
/// sprite renderers draw (LumilioCL local change, plan schematic-studio
/// T35). A row with `billboard` keeps it turned to the camera.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sprite {
    pub texture: String,
    pub from: [f32; 2],
    pub to: [f32; 2],
    #[serde(default)]
    pub uv: Option<[f32; 4]>,
}

/// An entity drawn at its exact position, turned as projectile renderers
/// turn it (LumilioCL local change, plan schematic-studio T35): about Y by
/// its yaw times `yaw`, then about X by its pitch times `pitch` (each 1, -1
/// or 0, the sign the renderer gives), from `origin` blocks off its
/// position. The row's transform places the model inside that turn, about
/// the cell's centre.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Aim {
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default)]
    pub origin: [f32; 3],
}

/// A block a renderer draws with the game's block renderer: a minecart's
/// contents, a falling block, primed TNT (LumilioCL local change, plan
/// schematic-studio T35). The `state` pattern names it (`{block}`); it is
/// drawn as the schematic's own blocks are, in a unit cube placed by
/// `steps`, then `lift` pixels up (a pattern, a minecart's `DisplayOffset`),
/// then `then`, inside the row's facing turn.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CarriedBlock {
    pub state: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub lift: Option<String>,
    #[serde(default)]
    pub then: Vec<Step>,
}

/// A model layer, optionally only some of its bones.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum LayerRef {
    Id(String),
    Part(LayerPart),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LayerPart {
    pub layer: String,
    /// Only these bones (and their children) are drawn.
    #[serde(default)]
    pub bones: Option<Vec<String>>,
    /// Texture candidates for this layer instead of the row's.
    #[serde(default)]
    pub texture: Option<OneOrMany>,
    /// Texture candidates by bone name, for that bone and its children.
    #[serde(default)]
    pub bone_textures: BTreeMap<String, OneOrMany>,
    /// Static poses for this layer only, over the row's.
    #[serde(default)]
    pub pose: BTreeMap<String, BonePose>,
}

impl LayerRef {
    pub fn layer(&self) -> &str {
        match self {
            Self::Id(id) => id,
            Self::Part(part) => &part.layer,
        }
    }

    pub fn bones(&self) -> Option<&[String]> {
        match self {
            Self::Id(_) => None,
            Self::Part(part) => part.bones.as_deref(),
        }
    }

    pub fn texture(&self) -> Option<&OneOrMany> {
        match self {
            Self::Id(_) => None,
            Self::Part(part) => part.texture.as_ref(),
        }
    }

    pub fn bone_textures(&self) -> Option<&BTreeMap<String, OneOrMany>> {
        match self {
            Self::Id(_) => None,
            Self::Part(part) => Some(&part.bone_textures),
        }
    }

    /// The row's poses with this layer's own over them.
    pub fn pose_over(&self, row: &BTreeMap<String, BonePose>) -> BTreeMap<String, BonePose> {
        let mut pose = row.clone();
        if let Self::Part(part) = self {
            pose.extend(part.pose.clone());
        }
        pose
    }
}

/// A string or a list of strings: alternatives tried in order.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn items(&self) -> &[String] {
        match self {
            Self::One(one) => std::slice::from_ref(one),
            Self::Many(many) => many,
        }
    }
}

/// A static pose for one bone, replacing the layer's default.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BonePose {
    /// Degrees about X, Y and Z.
    #[serde(default)]
    pub rotation: Option<[f32; 3]>,
    #[serde(default)]
    pub pivot: Option<[f32; 3]>,
    /// Multiplies the bone's scale.
    #[serde(default)]
    pub scale: Option<[f32; 3]>,
    #[serde(default = "yes")]
    pub visible: bool,
    /// Faces of the bone's own boxes not drawn, where they would fight a
    /// neighbour's at the same depth.
    #[serde(default)]
    pub hide_faces: Vec<crate::types::Direction>,
}

fn yes() -> bool {
    true
}

/// A second pass over the model with other layers and texture (saddles,
/// wool, armor of a horse).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    #[serde(default)]
    pub when: BTreeMap<String, OneOrMany>,
    /// Alternatives in order: the first layer the game version has is drawn.
    pub layers: Vec<LayerRef>,
    pub texture: OneOrMany,
    /// Added to every box's growth, for versions whose overlay layer is the
    /// base model drawn larger.
    #[serde(default)]
    pub inflate: f32,
    /// `wool`: a sheep's wool colour from the `color` variable;
    /// `dye:<name>`: that variable's dye colour (T35).
    #[serde(default)]
    pub tint: Option<String>,
    #[serde(default)]
    pub opaque: bool,
}

/// What a baby draws: its own layers where the game version has them, else
/// the adult's scaled the way `BabyModelTransform` does.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Baby {
    #[serde(default)]
    pub layers: Vec<LayerRef>,
    #[serde(default)]
    pub texture: Option<OneOrMany>,
    /// Steps before the adult model, inside the row's transform.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Extra scale for these bones of the adult model.
    #[serde(default)]
    pub bone_scale: BTreeMap<String, f32>,
}

/// One row.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// Id patterns: `block:<id>` or `entity:<id>`, without namespace; a
    /// `{name}` part captures a variable. Not read for a renderer's rows.
    #[serde(rename = "match", default)]
    pub patterns: Vec<String>,
    /// Required values of properties (or variables), any of a list.
    #[serde(default)]
    pub when: BTreeMap<String, OneOrMany>,
    /// Fixed variables.
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    /// Values for variables and properties the block does not set.
    #[serde(default)]
    pub defaults: BTreeMap<String, String>,
    pub layers: Vec<LayerRef>,
    /// Texture candidates; the first the pack has is used.
    #[serde(default)]
    pub texture: Option<OneOrMany>,
    /// A named placement from `transforms`, or steps written out.
    #[serde(default)]
    pub transform: Option<TransformRef>,
    #[serde(default)]
    pub facing: Facing,
    /// The renderer's own scale about the feet (`CatRenderer` 0.8), for game
    /// versions whose layers do not carry it yet: from 1.21.2 the layer's
    /// root bone is already scaled and this is skipped.
    #[serde(default)]
    pub renderer_scale: Option<f32>,
    #[serde(default)]
    pub pose: BTreeMap<String, BonePose>,
    /// Poses that apply while their conditions hold, over `pose` (parts a
    /// renderer shows or hides by state, such as an older horse's saddle).
    #[serde(default)]
    pub pose_when: Vec<ConditionalPose>,
    /// Pose properties (`HeadPose` → `head`) in degrees, as armor stands store them.
    #[serde(default)]
    pub pose_properties: BTreeMap<String, String>,
    /// Whether the texture has no see-through pixels the model shows.
    #[serde(default = "yes")]
    pub opaque: bool,
    #[serde(default)]
    pub compose: Option<Compose>,
    #[serde(default)]
    pub overlays: Vec<Overlay>,
    /// Armor slot (`helmet`, `chestplate`, `leggings`, `boots`) → layer
    /// alternatives, the first the game version has.
    #[serde(default)]
    pub armor: BTreeMap<String, Vec<LayerRef>>,
    #[serde(default)]
    pub baby: Option<Baby>,
    /// Where a rider sits, in blocks above the entity's block.
    #[serde(default)]
    pub rider_offset: Option<[f32; 3]>,
    /// Sits with bent legs when riding (`HumanoidModel.setupAnim`).
    #[serde(default)]
    pub humanoid: bool,
    /// From 26.2 the block's own model draws it; the entity is drawn only
    /// when the resolved model has no elements.
    #[serde(default)]
    pub model_replaces: bool,
    /// Drawn at the exact position and turned by yaw and pitch (T35).
    #[serde(default)]
    pub aim: Option<Aim>,
    /// The main layers' vertex colour, as an overlay's `tint` (T35: a
    /// tropical fish's body takes its base colour).
    #[serde(default)]
    pub tint: Option<String>,
    /// A block drawn with the model, or instead of one (T35).
    #[serde(default)]
    pub carries: Option<CarriedBlock>,
    /// A flat picture instead of a model (T35).
    #[serde(default)]
    pub sprite: Option<Sprite>,
    /// Kept turned to the camera the way `Display.BillboardConstraints`
    /// says (`center`, `vertical`, `horizontal`): sprites always, display
    /// entities by their `billboard` (T35). Read where the entity is placed.
    #[serde(default)]
    pub billboard: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConditionalPose {
    pub when: BTreeMap<String, OneOrMany>,
    pub pose: BTreeMap<String, BonePose>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TransformRef {
    Named(String),
    Steps(Vec<Step>),
}

impl Binding {
    /// The placement steps, outermost first.
    pub fn steps(&self) -> &[Step] {
        match &self.transform {
            None => &[],
            Some(TransformRef::Steps(steps)) => steps,
            Some(TransformRef::Named(name)) => table()
                .transforms
                .get(name)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        }
    }
}

/// A row matched to one block or entity, with its variables.
#[derive(Clone, Debug)]
pub struct Bound {
    pub binding: &'static Binding,
    /// Captured from the id and fixed by the row.
    pub vars: HashMap<String, String>,
    /// The block's own properties.
    pub properties: HashMap<String, String>,
    pub is_entity: bool,
}

impl Bound {
    /// A variable: captured or fixed by the row, else a property, else the
    /// row's default.
    pub fn var(&self, name: &str) -> Option<&str> {
        self.vars
            .get(name)
            .or_else(|| self.properties.get(name))
            .or_else(|| self.binding.defaults.get(name))
            .map(String::as_str)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.var(name) == Some("true")
    }

    /// `pattern` with every `{name}` replaced; `None` when a variable is unset.
    pub fn expand(&self, pattern: &str) -> Option<String> {
        expand(pattern, |name| self.var(name).map(str::to_owned))
    }

    pub fn holds(&self, when: &BTreeMap<String, OneOrMany>) -> bool {
        when.iter().all(|(name, allowed)| {
            let value = self.var(name).unwrap_or("");
            allowed.items().iter().any(|item| item == value)
        })
    }
}

/// The row for `block`, if any.
pub fn find(block: &InputBlock) -> Option<Bound> {
    find_in(table(), block)
}

/// The row for `block`: the table's, else, for an entity the table has no
/// row for, one made from the look the game version's own renderer gave it
/// (`looks`, by `minecraft:` id; LumilioCL local change, plan
/// schematic-studio T34, D32). The table wins: its rows add what a look
/// cannot say (variants, overlays, riders, composition).
pub fn find_for(looks: &HashMap<String, EntityLook>, block: &InputBlock) -> Option<Bound> {
    find(block).or_else(|| {
        let key = entity_key(block)?;
        let look = looks.get(&key)?;
        by_renderer(table(), look, block).or_else(|| {
            Some(Bound {
                binding: generated(&key, look)?,
                vars: HashMap::new(),
                properties: block.properties.clone(),
                is_entity: true,
            })
        })
    })
}

/// The row of the most specific renderer class of `look` the table has
/// rows for, whose conditions hold (T35).
fn by_renderer(
    table: &'static BindingTable,
    look: &EntityLook,
    block: &InputBlock,
) -> Option<Bound> {
    let rows = look
        .renderer
        .iter()
        .find_map(|class| table.renderers.get(class))?;
    let texture = look
        .texture
        .iter()
        .chain(&look.texture_fields)
        .find_map(|location| texture_id(location));
    let vars: HashMap<String, String> = texture
        .map(|texture| ("look_texture".to_owned(), texture))
        .into_iter()
        .collect();
    rows.iter().find_map(|binding| {
        let mut vars = vars.clone();
        for (name, value) in &binding.vars {
            vars.entry(name.clone()).or_insert_with(|| value.clone());
        }
        let bound = Bound {
            binding,
            vars,
            properties: block.properties.clone(),
            is_entity: true,
        };
        bound.holds(&binding.when).then_some(bound)
    })
}

/// Whether `bound` draws anything with `pack`: composition that needs no
/// layer (a frame, an item, a painting), a carried block, or a layer of its
/// main model the version has (T35).
pub fn draws(pack: &crate::resource_pack::ResourcePack, bound: &Bound) -> bool {
    matches!(
        bound.binding.compose,
        Some(
            Compose::ItemFrame
                | Compose::DroppedItem
                | Compose::Painting
                | Compose::ItemDisplay
                | Compose::TextDisplay
        )
    ) || bound.binding.carries.is_some()
        || bound.binding.sprite.is_some()
        || super::render::has_geometry(pack, bound)
}

/// Whether `block` has a row, in the table whatever its properties, or made
/// from its look, or is an entity the game draws nothing for.
pub fn known_for(looks: &HashMap<String, EntityLook>, block: &InputBlock) -> bool {
    find_ignoring_conditions(block)
        || find_for(looks, block).is_some()
        || draws_nothing(looks, block)
}

/// The `minecraft:` id of an `entity:` block, or `None` for a block.
fn entity_key(block: &InputBlock) -> Option<String> {
    let (is_entity, id) = split_name(&block.name);
    is_entity.then(|| {
        if id.contains(':') {
            id.to_owned()
        } else {
            format!("minecraft:{id}")
        }
    })
}

/// Whether `block` is an entity the game draws nothing for: its renderer is
/// `NoopRenderer` (markers, interaction boxes, effect clouds), or the table
/// lists it (lightning, a flash) (T35). Not drawn, and not reported either.
pub fn draws_nothing(looks: &HashMap<String, EntityLook>, block: &InputBlock) -> bool {
    let Some(key) = entity_key(block) else {
        return false;
    };
    let bare = key.strip_prefix("minecraft:").unwrap_or(&key);
    table().nothing.iter().any(|listed| listed == bare)
        || looks
            .get(&key)
            .is_some_and(|look| look.renderer.iter().any(|class| class == "NoopRenderer"))
}

/// What each entity type of the version `looks` come from cannot be drawn
/// with `pack`, and why: no row and no look to make one from, or a row whose
/// layers the version lacks (T35). Sorted; empty when every type is drawn or
/// draws nothing.
pub fn coverage(
    pack: &crate::resource_pack::ResourcePack,
    looks: &HashMap<String, EntityLook>,
) -> Vec<(String, &'static str)> {
    let mut missing: Vec<(String, &'static str)> = looks
        .keys()
        .filter_map(|key| {
            let bare = key.strip_prefix("minecraft:").unwrap_or(key);
            let block = InputBlock::new(format!("entity:{bare}"));
            if draws_nothing(looks, &block) {
                return None;
            }
            let Some(bound) = find_for(looks, &block) else {
                return Some((key.clone(), "no row, and no look a row can be made from"));
            };
            (!draws(pack, &bound))
                .then(|| (key.clone(), "its row's layers are not in this version"))
        })
        .collect();
    missing.sort();
    missing
}

/// Renderers that draw the item their entity carries (`Item`), as a dropped
/// item is drawn: thrown snowballs, eggs, pearls and potions, fireworks, the
/// ominous item spawner (T35).
const ITEM_RENDERERS: [&str; 3] = [
    "ThrownItemRenderer",
    "FireworkEntityRenderer",
    "OminousItemSpawnerRenderer",
];

/// A row made from what a living entity's renderer used: its first (main)
/// layer with its default texture, placed as `LivingEntityRenderer` places
/// every living entity (the `mob` transform) and turning with its yaw; its
/// baby's own layer and texture where the renderer has them. `None` for any
/// other renderer, whose placement is its own. Made once per distinct look
/// and kept for the process, as the table's rows are.
fn generated(id: &str, look: &EntityLook) -> Option<&'static Binding> {
    let bare = id.strip_prefix("minecraft:").unwrap_or(id);
    if look
        .renderer
        .iter()
        .any(|class| ITEM_RENDERERS.contains(&class.as_str()))
    {
        // Its own item when it saves none: a snowball is a snowball.
        let binding = Binding {
            defaults: [("item".to_owned(), id.to_owned())].into(),
            ..item_row(bare)
        };
        return Some(made(binding));
    }
    if !look.is_living() {
        return None;
    }
    let main = look.layers.first()?.clone();
    let texture = texture_id(look.texture.as_deref()?)?;
    let baby = baby_layer(&main)
        .filter(|layer| look.layers.contains(layer))
        .zip(look.baby_texture.as_deref().and_then(texture_id))
        .map(|(layer, texture)| Baby {
            layers: vec![LayerRef::Id(layer)],
            texture: Some(OneOrMany::One(texture)),
            steps: Vec::new(),
            bone_scale: BTreeMap::new(),
        });
    let binding = Binding {
        patterns: vec![format!(
            "entity:{}",
            id.strip_prefix("minecraft:").unwrap_or(id)
        )],
        when: BTreeMap::new(),
        vars: BTreeMap::new(),
        defaults: BTreeMap::new(),
        layers: vec![LayerRef::Id(main)],
        texture: Some(OneOrMany::One(texture)),
        transform: Some(TransformRef::Named("mob".to_owned())),
        facing: Facing::Horizontal,
        renderer_scale: None,
        pose: BTreeMap::new(),
        pose_when: Vec::new(),
        pose_properties: BTreeMap::new(),
        // Cut out: a mob's texture may have see-through pixels.
        opaque: false,
        compose: None,
        overlays: Vec::new(),
        armor: BTreeMap::new(),
        baby,
        rider_offset: None,
        humanoid: look
            .renderer
            .iter()
            .any(|class| class == "HumanoidMobRenderer"),
        model_replaces: false,
        aim: None,
        tint: None,
        carries: None,
        sprite: None,
        billboard: None,
    };
    Some(made(binding))
}

/// A row drawing the entity's `item` as a dropped item is drawn.
fn item_row(bare: &str) -> Binding {
    Binding {
        patterns: vec![format!("entity:{bare}")],
        when: BTreeMap::new(),
        vars: BTreeMap::new(),
        defaults: BTreeMap::new(),
        layers: Vec::new(),
        texture: None,
        transform: None,
        facing: Facing::Horizontal,
        renderer_scale: None,
        pose: BTreeMap::new(),
        pose_when: Vec::new(),
        pose_properties: BTreeMap::new(),
        opaque: false,
        compose: Some(Compose::DroppedItem),
        overlays: Vec::new(),
        armor: BTreeMap::new(),
        baby: None,
        rider_offset: None,
        humanoid: false,
        model_replaces: false,
        aim: None,
        tint: None,
        carries: None,
        sprite: None,
        billboard: None,
    }
}

/// `binding`, kept for the process: made once per distinct row.
fn made(binding: Binding) -> &'static Binding {
    static MADE: OnceLock<Mutex<HashMap<String, &'static Binding>>> = OnceLock::new();
    let mut made = MADE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let key = format!("{binding:?}");
    *made
        .entry(key)
        .or_insert_with(|| Box::leak(Box::new(binding)))
}

/// `minecraft:textures/entity/bee/bee.png` as the table writes it,
/// `entity/bee/bee`; another namespace keeps its prefix.
fn texture_id(location: &str) -> Option<String> {
    let (namespace, path) = location.split_once(':').unwrap_or(("minecraft", location));
    let path = path.strip_prefix("textures/")?.strip_suffix(".png")?;
    Some(if namespace == "minecraft" {
        path.to_owned()
    } else {
        format!("{namespace}:{path}")
    })
}

/// The game's name for a model's baby layer: `minecraft:bee#main` →
/// `minecraft:bee_baby#main`.
fn baby_layer(main: &str) -> Option<String> {
    let (model, part) = main.split_once('#')?;
    Some(format!("{model}_baby#{part}"))
}

/// A block state as written, `minecraft:chest[facing=north]`, as a block
/// (T35); the namespace defaults to `minecraft`.
pub fn parse_state(state: &str) -> InputBlock {
    let state = state.trim();
    let (name, properties) = match state.split_once('[') {
        Some((name, rest)) => (name, rest.trim_end_matches(']')),
        None => (state, ""),
    };
    let name = if name.contains(':') {
        name.to_owned()
    } else {
        format!("minecraft:{name}")
    };
    let mut block = InputBlock::new(name);
    for pair in properties.split(',').filter(|pair| !pair.is_empty()) {
        if let Some((key, value)) = pair.split_once('=') {
            block
                .properties
                .insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    block
}

/// The row of `table` for `block`, if any.
pub fn find_in(table: &'static BindingTable, block: &InputBlock) -> Option<Bound> {
    let (is_entity, id) = split_name(&block.name);
    for binding in &table.bindings {
        for pattern in &binding.patterns {
            let (kind, pattern) = pattern.split_once(':').unwrap_or(("block", pattern));
            if (kind == "entity") != is_entity {
                continue;
            }
            let Some(captures) = capture(pattern, id) else {
                continue;
            };
            let mut vars: HashMap<String, String> = captures.into_iter().collect();
            for (name, value) in &binding.vars {
                vars.entry(name.clone()).or_insert_with(|| value.clone());
            }
            let bound = Bound {
                binding,
                vars,
                properties: block.properties.clone(),
                is_entity,
            };
            if bound.holds(&binding.when) {
                return Some(bound);
            }
        }
    }
    None
}

/// Whether some row's id pattern matches `block`, whatever its properties.
pub fn find_ignoring_conditions(block: &InputBlock) -> bool {
    let (is_entity, id) = split_name(&block.name);
    table().bindings.iter().any(|binding| {
        binding.patterns.iter().any(|pattern| {
            let (kind, pattern) = pattern.split_once(':').unwrap_or(("block", pattern));
            (kind == "entity") == is_entity && capture(pattern, id).is_some()
        })
    })
}

fn split_name(name: &str) -> (bool, &str) {
    match name.split_once(':') {
        Some(("entity", id)) => (true, id),
        Some((_, id)) => (false, id),
        None => (false, name),
    }
}

/// Matches `id` against a pattern of literal text and `{name}` parts; each
/// part takes at least one character, the shortest that lets the rest match.
pub fn capture(pattern: &str, id: &str) -> Option<Vec<(String, String)>> {
    fn go(pattern: &str, id: &str, out: &mut Vec<(String, String)>) -> bool {
        let Some(open) = pattern.find('{') else {
            return pattern == id;
        };
        let (literal, rest) = pattern.split_at(open);
        let Some(id) = id.strip_prefix(literal) else {
            return false;
        };
        let Some(close) = rest.find('}') else {
            return false;
        };
        let name = &rest[1..close];
        let after = &rest[close + 1..];
        for end in id.char_indices().map(|(at, c)| at + c.len_utf8()) {
            out.push((name.to_owned(), id[..end].to_owned()));
            if go(after, &id[end..], out) {
                return true;
            }
            out.pop();
        }
        false
    }
    let mut out = Vec::new();
    go(pattern, id, &mut out).then_some(out)
}

/// `pattern` with every `{name}` replaced by `value(name)`.
pub fn expand(pattern: &str, value: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::with_capacity(pattern.len());
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('}')? + open;
        out.push_str(&value(&rest[open + 1..close])?);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    Some(out)
}
