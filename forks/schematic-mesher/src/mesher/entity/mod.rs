//! Block entity model geometry generation.
//!
//! Hardcodes entity model geometry for block entities like chests, beds, bells,
//! signs, skulls, and shulker boxes. These blocks have minimal/empty JSON block
//! models — their actual visual geometry is defined in Java code.
//!
//! Follows the liquid module's integration pattern: detect entity type, generate
//! vertices/indices/face textures, then integrate in MeshBuilder::add_block().

pub mod armor_stand;
mod bat;
pub(crate) mod banner;
mod bed;
mod bell;
mod book;
mod cat;
mod chicken;
mod chest;
mod cow;
pub(crate) mod decorated_pot;
mod enderman;
pub(crate) mod hanging_sign;
mod horse;
mod iron_golem;
mod item_frame;
pub(crate) mod inventory;
pub mod item_render;
mod minecart;
pub(crate) mod mob;
pub(crate) mod particle;
pub(crate) mod player;
pub(crate) mod sheep;
mod shulker;
pub(crate) mod sign;
pub(crate) mod sign_text;
pub(crate) mod skull;
pub(crate) mod villager_texture;
pub(crate) mod boat;
pub(crate) mod equipment;
mod blaze;
mod ghast;
mod magma_cube;
mod slime;
mod spider;
mod villager;
mod wolf;

use crate::mesher::geometry::Vertex;
use crate::types::{Direction, InputBlock};
use glam::{Mat4, Vec3, Vec4};

// ── Data Structures ─────────────────────────────────────────────────────────

/// A cube within an entity model part.
#[derive(Debug, Clone)]
pub struct EntityCube {
    /// Origin in 1/16th block units.
    pub origin: [f32; 3],
    /// Dimensions (W, H, D) in 1/16th block units.
    pub dimensions: [f32; 3],
    /// UV offset (u0, v0) in pixels on the texture sheet.
    pub tex_offset: [u32; 2],
    /// Expansion from cube center.
    pub inflate: f32,
    /// Mirror UVs horizontally.
    pub mirror: bool,
    /// Faces to skip (used to prevent z-fighting at block boundaries).
    pub skip_faces: Vec<Direction>,
}

/// Pose/transform for an entity model part.
#[derive(Debug, Clone)]
pub struct EntityPartPose {
    /// Translation in 1/16th block units.
    pub position: [f32; 3],
    /// Rotation (x, y, z) in radians.
    pub rotation: [f32; 3],
    /// Scale factors.
    pub scale: [f32; 3],
}

impl Default for EntityPartPose {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// A part in the entity model hierarchy.
#[derive(Debug, Clone)]
pub struct EntityPart {
    pub cubes: Vec<EntityCube>,
    pub pose: EntityPartPose,
    pub children: Vec<EntityPart>,
}

/// Complete entity model definition.
#[derive(Debug, Clone)]
pub struct EntityModelDef {
    /// Texture path (e.g., "entity/chest/normal").
    pub texture_path: String,
    /// Texture sheet dimensions in pixels.
    pub texture_size: [u32; 2],
    /// Top-level parts.
    pub parts: Vec<EntityPart>,
    /// Whether this entity's geometry is opaque.
    pub is_opaque: bool,
}

/// Chest variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestVariant {
    Normal,
    Trapped,
    Ender,
    Christmas,
}

/// Double chest side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleChestSide {
    Left,
    Right,
}

/// Sign wood types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignWood {
    Oak, Spruce, Birch, Jungle, Acacia, DarkOak,
    Crimson, Warped, Mangrove, Cherry, Bamboo,
}

/// Skull/head types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkullType {
    Skeleton, WitherSkeleton, Zombie, Creeper, Piglin, Dragon, Player,
}

/// Block entity type detected from block ID.
#[derive(Debug, Clone)]
pub enum BlockEntityType {
    Chest(ChestVariant),
    DoubleChest { variant: ChestVariant, side: DoubleChestSide },
    Bed { color: String, is_head: bool },
    Bell,
    Sign { wood: SignWood, is_wall: bool },
    Skull(SkullType),
    ShulkerBox { color: Option<String> },
    Banner { color: String, is_wall: bool },
    HangingSign { wood: SignWood, is_wall: bool },
    DecoratedPot,
    Lectern,
    EnchantingTable,
}

/// Mob entity types (rendered as static models).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobType {
    Zombie,
    Skeleton,
    Creeper,
    Pig,
    Chicken,
    Cow,
    Sheep,
    Villager,
    ArmorStand,
    Minecart,
    ItemFrame,
    GlowItemFrame,
    DroppedItem,
    Wolf,
    Cat,
    Spider,
    Horse,
    Enderman,
    Slime,
    MagmaCube,
    Blaze,
    Ghast,
    WitherSkeleton,
    IronGolem,
    Bat,
    Player,
    Boat,
    ChestBoat,
}

/// Face texture info for a generated entity face.
pub struct EntityFaceTexture {
    pub texture: String,
    pub is_transparent: bool,
}

// ── Detection ───────────────────────────────────────────────────────────────

/// Parse a wood type string into a SignWood enum.
fn parse_sign_wood(s: &str) -> SignWood {
    match s {
        "spruce" => SignWood::Spruce,
        "birch" => SignWood::Birch,
        "jungle" => SignWood::Jungle,
        "acacia" => SignWood::Acacia,
        "dark_oak" => SignWood::DarkOak,
        "crimson" => SignWood::Crimson,
        "warped" => SignWood::Warped,
        "mangrove" => SignWood::Mangrove,
        "cherry" => SignWood::Cherry,
        "bamboo" => SignWood::Bamboo,
        _ => SignWood::Oak,
    }
}

/// Detect if a block is a block entity that needs hardcoded geometry.
pub fn detect_block_entity(block: &InputBlock) -> Option<BlockEntityType> {
    let block_id = block.block_id();

    match block_id {
        // Chests
        "chest" => {
            if let Some(side) = detect_double_chest(block) {
                Some(BlockEntityType::DoubleChest { variant: ChestVariant::Normal, side })
            } else {
                Some(BlockEntityType::Chest(ChestVariant::Normal))
            }
        }
        "trapped_chest" => {
            if let Some(side) = detect_double_chest(block) {
                Some(BlockEntityType::DoubleChest { variant: ChestVariant::Trapped, side })
            } else {
                Some(BlockEntityType::Chest(ChestVariant::Trapped))
            }
        }
        "ender_chest" => Some(BlockEntityType::Chest(ChestVariant::Ender)),

        // Beds
        id if id.ends_with("_bed") => {
            let color = id.strip_suffix("_bed").unwrap_or("red").to_string();
            let is_head = block.properties.get("part")
                .map(|p| p == "head")
                .unwrap_or(false);
            Some(BlockEntityType::Bed { color, is_head })
        }

        // Bell
        "bell" => Some(BlockEntityType::Bell),

        // Hanging signs (must check before regular signs)
        id if id.ends_with("_hanging_sign") => {
            let is_wall = id.contains("_wall_hanging_sign");
            let wood_str = if is_wall {
                id.strip_suffix("_wall_hanging_sign").unwrap_or("oak")
            } else {
                id.strip_suffix("_hanging_sign").unwrap_or("oak")
            };
            let wood = parse_sign_wood(wood_str);
            Some(BlockEntityType::HangingSign { wood, is_wall })
        }

        // Regular signs
        id if id.ends_with("_sign") => {
            let is_wall = id.starts_with("wall_") || id.contains("_wall_");
            let wood_str = id.strip_suffix("_sign")
                .and_then(|s| s.strip_prefix("wall_").or(Some(s)))
                .unwrap_or("oak");
            let wood = parse_sign_wood(wood_str);
            Some(BlockEntityType::Sign { wood, is_wall })
        }

        // Decorated pots
        "decorated_pot" => Some(BlockEntityType::DecoratedPot),

        // Skulls / Heads
        "skeleton_skull" | "skeleton_wall_skull" =>
            Some(BlockEntityType::Skull(SkullType::Skeleton)),
        "wither_skeleton_skull" | "wither_skeleton_wall_skull" =>
            Some(BlockEntityType::Skull(SkullType::WitherSkeleton)),
        "zombie_head" | "zombie_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Zombie)),
        "creeper_head" | "creeper_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Creeper)),
        "piglin_head" | "piglin_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Piglin)),
        "dragon_head" | "dragon_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Dragon)),
        "player_head" | "player_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Player)),

        // Banners
        id if id.ends_with("_banner") || id.ends_with("_wall_banner") => {
            let is_wall = id.contains("wall_banner");
            let color = if is_wall {
                id.strip_suffix("_wall_banner").unwrap_or("white").to_string()
            } else {
                id.strip_suffix("_banner").unwrap_or("white").to_string()
            };
            Some(BlockEntityType::Banner { color, is_wall })
        }

        // Lectern (only when it has a book)
        "lectern" => {
            let has_book = block.properties.get("has_book")
                .map(|v| v == "true")
                .unwrap_or(false);
            if has_book {
                Some(BlockEntityType::Lectern)
            } else {
                None
            }
        }

        // Enchanting table (always shows a book)
        "enchanting_table" => Some(BlockEntityType::EnchantingTable),

        // Shulker Boxes
        "shulker_box" => Some(BlockEntityType::ShulkerBox { color: None }),
        id if id.ends_with("_shulker_box") => {
            let color = id.strip_suffix("_shulker_box").unwrap_or("purple").to_string();
            Some(BlockEntityType::ShulkerBox { color: Some(color) })
        }

        _ => None,
    }
}

/// Detect if a block is a mob entity (custom `entity:` namespace convention).
/// World-space offset where a rider sits on this mob/vehicle, approximated from
/// MC's `Entity.getPassengerRidingPosition`. Returned in block units.
pub fn rider_offset(mob_type: MobType) -> [f32; 3] {
    // Y values approximate MC's seat-height per host mob, calibrated so the
    // rider's feet land on the mob's back/saddle/seat after our `pos.y - 0.5`
    // block-to-world shift. Smaller than MC's raw getPassengerRidingPosition
    // numbers because our root already lifts the rider into block coords.
    match mob_type {
        MobType::Pig => [0.0, 0.25, 0.0],
        MobType::Horse => [0.0, 0.65, -0.05],
        MobType::Cow | MobType::Sheep => [0.0, 0.35, 0.0],
        MobType::Minecart => [0.0, 0.0, 0.0],
        MobType::Boat | MobType::ChestBoat => [0.0, 0.3, 0.0],
        MobType::Wolf => [0.0, 0.15, 0.0],
        _ => [0.0, 0.3, 0.0],
    }
}

pub fn detect_mob(block: &InputBlock) -> Option<MobType> {
    let block_id = block.block_id();
    match block_id {
        "zombie" => Some(MobType::Zombie),
        "skeleton" => Some(MobType::Skeleton),
        "creeper" => Some(MobType::Creeper),
        "pig" => Some(MobType::Pig),
        "chicken" => Some(MobType::Chicken),
        "cow" => Some(MobType::Cow),
        "sheep" => Some(MobType::Sheep),
        "villager" => Some(MobType::Villager),
        "armor_stand" => Some(MobType::ArmorStand),
        "minecart" => Some(MobType::Minecart),
        "item_frame" => Some(MobType::ItemFrame),
        "glow_item_frame" => Some(MobType::GlowItemFrame),
        "item" => Some(MobType::DroppedItem),
        "wolf" => Some(MobType::Wolf),
        "cat" => Some(MobType::Cat),
        "spider" => Some(MobType::Spider),
        "horse" => Some(MobType::Horse),
        "enderman" => Some(MobType::Enderman),
        "slime" => Some(MobType::Slime),
        "magma_cube" => Some(MobType::MagmaCube),
        "blaze" => Some(MobType::Blaze),
        "ghast" | "happy_ghast" => Some(MobType::Ghast),
        "wither_skeleton" => Some(MobType::WitherSkeleton),
        "iron_golem" => Some(MobType::IronGolem),
        "bat" => Some(MobType::Bat),
        "player" => Some(MobType::Player),
        "boat" => Some(MobType::Boat),
        "chest_boat" => Some(MobType::ChestBoat),
        _ => None,
    }
}

fn detect_double_chest(block: &InputBlock) -> Option<DoubleChestSide> {
    match block.properties.get("type").map(|s| s.as_str()) {
        Some("left") => Some(DoubleChestSide::Left),
        Some("right") => Some(DoubleChestSide::Right),
        _ => None,
    }
}

// ── Facing Helpers ──────────────────────────────────────────────────────────

/// Get facing direction from block properties. Defaults to north.
pub(crate) fn get_facing(block: &InputBlock) -> &str {
    block.properties.get("facing").map(|s| s.as_str()).unwrap_or("north")
}

/// Y rotation angle for a facing direction (radians).
/// In Minecraft, entity models face south by default (toward +Z).
/// North = 180deg, South = 0deg, East = -90deg (270), West = 90deg.
pub(crate) fn facing_rotation_rad(facing: &str) -> f32 {
    match facing {
        "north" => std::f32::consts::PI,
        "south" => 0.0,
        "east" => -std::f32::consts::FRAC_PI_2,
        "west" => std::f32::consts::FRAC_PI_2,
        _ => std::f32::consts::PI,
    }
}

/// Standing sign/skull rotation from `rotation` property (0-15, each = 22.5 degrees).
pub(crate) fn standing_rotation_rad(block: &InputBlock) -> f32 {
    let rot: u8 = block.properties.get("rotation")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // Rotation 0 = south (facing +Z), each step is 22.5 degrees CW from above
    rot as f32 * std::f32::consts::PI / 8.0
}

// ── UV Computation ──────────────────────────────────────────────────────────

/// Compute UVs for one face of a cube using Minecraft's box-unwrap layout.
///
/// For a cube of size (W, H, D) at UV origin (u0, v0):
/// ```text
///                u0     u0+D   u0+D+W  u0+2D+W  u0+2D+2W
/// v0             | DOWN  |  UP   |       |        |
/// v0+D           |       |       |       |        |
/// v0+D    WEST   | NORTH | EAST  | SOUTH |
/// v0+D+H         |       |       |       |
/// ```
fn cube_face_uvs(
    tex_offset: [u32; 2],
    dimensions: [f32; 3],
    face: Direction,
    texture_size: [u32; 2],
    mirror: bool,
) -> [[f32; 2]; 4] {
    let u0 = tex_offset[0] as f32;
    let v0 = tex_offset[1] as f32;
    let w = dimensions[0]; // X dimension
    let h = dimensions[1]; // Y dimension
    let d = dimensions[2]; // Z dimension
    let tw = texture_size[0] as f32;
    let th = texture_size[1] as f32;

    // UV region in pixel space (left, top, right, bottom)
    let (left, top, right, bottom) = match face {
        Direction::Down => (u0 + d, v0, u0 + d + w, v0 + d),
        Direction::Up => (u0 + d + w, v0, u0 + d + w + w, v0 + d),
        Direction::North => (u0 + d, v0 + d, u0 + d + w, v0 + d + h),
        Direction::South => (u0 + d + w + d, v0 + d, u0 + d + w + d + w, v0 + d + h),
        Direction::West => (u0, v0 + d, u0 + d, v0 + d + h),
        Direction::East => (u0 + d + w, v0 + d, u0 + d + w + d, v0 + d + h),
    };

    // Normalize to [0,1] UV space
    let (nl, nt, nr, nb) = (left / tw, top / th, right / tw, bottom / th);

    // Minecraft's Polygon constructor assigns vertex[0]=(u1,v0) i.e. RIGHT-top first.
    // Our corner ordering matches Minecraft's for Down/N/S/E/W, so the default
    // (non-mirrored) state swaps U (puts u_right first).
    // The Up face has a different vertex order from Minecraft ([3,2,6,7] vs [2,3,7,6]),
    // so U should NOT be swapped, but V should be flipped (Minecraft inverts V for Up).
    match face {
        Direction::Up => {
            if mirror {
                [[nr, nb], [nl, nb], [nl, nt], [nr, nt]]
            } else {
                [[nl, nb], [nr, nb], [nr, nt], [nl, nt]]
            }
        }
        _ => {
            if mirror {
                [[nl, nt], [nr, nt], [nr, nb], [nl, nb]]
            } else {
                [[nr, nt], [nl, nt], [nl, nb], [nr, nb]]
            }
        }
    }
}

// ── Geometry Generation ─────────────────────────────────────────────────────

/// Generate all entity geometry for a block entity.
///
/// Returns (vertices, indices, face_textures) ready for MeshBuilder integration.
pub fn generate_entity_geometry(
    block: &InputBlock,
    entity_type: &BlockEntityType,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let model = build_model_def(entity_type);

    let facing = get_facing(block);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    // Build facing rotation matrix
    let facing_mat = if matches!(entity_type, BlockEntityType::Lectern | BlockEntityType::EnchantingTable) {
        // Match MC's EnchantTableRenderer / LecternRenderer poseStack exactly.
        match entity_type {
            BlockEntityType::EnchantingTable => {
                // T(0.5, 0.75, 0.5) · T(0, 0.1, 0) · Rz(80°)
                // (Y rotation is a time-based spin animation; static = 0.)
                Mat4::from_translation(Vec3::new(0.5, 0.85, 0.5))
                    * Mat4::from_rotation_z(80.0_f32.to_radians())
            }
            _ => {
                // Lectern: T(0.5, 1.0625, 0.5) · Ry(-clockwise(facing).yaw) · Rz(67.5°) · T(0, -0.125, 0)
                let yaw_deg = match facing {
                    "north" => 270.0_f32, // clockWise=east
                    "south" => 90.0_f32,  // clockWise=west
                    "east"  => 0.0_f32,   // clockWise=south
                    "west"  => 180.0_f32, // clockWise=north
                    _       => 0.0_f32,
                };
                Mat4::from_translation(Vec3::new(0.5, 1.0625, 0.5))
                    * Mat4::from_rotation_y(-yaw_deg.to_radians())
                    * Mat4::from_rotation_z(67.5_f32.to_radians())
                    * Mat4::from_translation(Vec3::new(0.0, -0.125, 0.0))
            }
        }
    } else if matches!(entity_type, BlockEntityType::ShulkerBox { .. }) {
        // Shulker boxes use 6-direction facing (up/down/north/south/east/west)
        // Rotate around full block center (0.5, 0.5, 0.5)
        let center = Vec3::new(0.5, 0.5, 0.5);
        let rot_mat = match facing {
            "up" => Mat4::IDENTITY,
            "down" => Mat4::from_rotation_x(std::f32::consts::PI),
            "north" => Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
            "south" => Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            "east" => Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            "west" => Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2),
            _ => Mat4::IDENTITY,
        };
        Mat4::from_translation(center) * rot_mat * Mat4::from_translation(-center)
    } else {
        // Standard Y-rotation around (0.5, 0, 0.5)
        let facing_angle = match entity_type {
            BlockEntityType::Banner { is_wall: false, .. } => standing_rotation_rad(block),
            BlockEntityType::Sign { is_wall: false, .. } => standing_rotation_rad(block),
            BlockEntityType::HangingSign { is_wall: false, .. } => standing_rotation_rad(block),
            BlockEntityType::Skull(_) => {
                // Match MC SkullBlockRenderer: world face direction is
                // (sin(yRot_mc), 0, -cos(yRot_mc)). Our model's face is on +Z local,
                // so we need α = π - yRot_mc to produce the same world direction.
                let yrot_mc = if block.block_id().contains("wall") {
                    // Wall skulls rotate by the opposite of `facing`.
                    match facing {
                        "south" => std::f32::consts::PI,
                        "north" => 0.0,
                        "east" => std::f32::consts::FRAC_PI_2,
                        "west" => -std::f32::consts::FRAC_PI_2,
                        _ => 0.0,
                    }
                } else {
                    standing_rotation_rad(block)
                };
                std::f32::consts::PI - yrot_mc
            }
            _ => facing_rotation_rad(facing),
        };
        let base = Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
            * Mat4::from_rotation_y(facing_angle)
            * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5));

        // Wall skulls: MC renders at (0.5 - stepX*0.25, 0.25, 0.5 - stepZ*0.25).
        // The wall is on the -facing side; skull face points in +facing direction.
        if matches!(entity_type, BlockEntityType::Skull(_)) && block.block_id().contains("wall") {
            let (dx, dz) = match facing {
                "south" => (0.0, -0.25),
                "north" => (0.0, 0.25),
                "east" => (-0.25, 0.0),
                "west" => (0.25, 0.0),
                _ => (0.0, 0.0),
            };
            Mat4::from_translation(Vec3::new(dx, 0.25, dz)) * base
        } else {
            base
        }
    };

    traverse_parts(
        &model.parts,
        Mat4::IDENTITY,
        &facing_mat,
        &model,
        &mut vertices,
        &mut indices,
        &mut face_textures,
    );

    (vertices, indices, face_textures)
}

/// Generate all geometry for a mob entity.
///
/// Returns (vertices, indices, face_textures) ready for MeshBuilder integration.
pub fn generate_mob_geometry(
    block: &InputBlock,
    mob_type: MobType,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    // Item frames use block textures, not an entity texture sheet
    if matches!(mob_type, MobType::ItemFrame | MobType::GlowItemFrame) {
        let is_glow = matches!(mob_type, MobType::GlowItemFrame);
        let facing = get_facing(block);
        return item_frame::generate_item_frame_geometry(facing, is_glow);
    }

    // Dropped items and players are rendered entirely in add_mob() with resource pack access
    if matches!(mob_type, MobType::DroppedItem | MobType::Player) {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let model = mob::build_mob_model(mob_type, block);

    let facing = get_facing(block);
    let facing_angle = facing_rotation_rad(facing);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    // Boats have a unique render transform per AbstractBoatRenderer:
    //   T(0, 0.375, 0) · Ry(180° - yRot) · S(-1,-1,1) · Ry(90°)
    // No RotX(π)+T(8,24,8) root wrapper is needed — the model already renders
    // upright under this chain.
    let facing_mat = if matches!(mob_type, MobType::Boat | MobType::ChestBoat) {
        let yrot_deg = match facing {
            "south" => 0.0_f32,
            "west" => 90.0_f32,
            "north" => 180.0_f32,
            "east" => 270.0_f32,
            _ => 0.0_f32,
        };
        Mat4::from_translation(Vec3::new(0.5, 0.375, 0.5))
            * Mat4::from_rotation_y((180.0_f32 - yrot_deg).to_radians())
            * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
            * Mat4::from_rotation_y(90.0_f32.to_radians())
    } else {
        // Standard mobs use their root wrapper for Y-down→Y-up; this matrix
        // just rotates the model around the block center for facing.
        Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
            * Mat4::from_rotation_y(facing_angle)
            * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5))
    };

    traverse_parts(
        &model.parts,
        Mat4::IDENTITY,
        &facing_mat,
        &model,
        &mut vertices,
        &mut indices,
        &mut face_textures,
    );

    (vertices, indices, face_textures)
}

/// Recursively traverse part hierarchy, accumulating transforms.
pub(crate) fn traverse_parts(
    parts: &[EntityPart],
    parent_transform: Mat4,
    facing_mat: &Mat4,
    model: &EntityModelDef,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    for part in parts {
        // Build part's local transform: translate -> rotateZYX -> scale
        let pose = &part.pose;
        let local = Mat4::from_translation(Vec3::new(
            pose.position[0] / 16.0,
            pose.position[1] / 16.0,
            pose.position[2] / 16.0,
        ))
        * Mat4::from_rotation_z(pose.rotation[2])
        * Mat4::from_rotation_y(pose.rotation[1])
        * Mat4::from_rotation_x(pose.rotation[0])
        * Mat4::from_scale(Vec3::new(pose.scale[0], pose.scale[1], pose.scale[2]));

        let combined = parent_transform * local;

        // Generate geometry for each cube in this part
        for cube in &part.cubes {
            generate_cube_faces(
                cube,
                &combined,
                facing_mat,
                model,
                vertices,
                indices,
                face_textures,
            );
        }

        // Recurse into children
        traverse_parts(
            &part.children,
            combined,
            facing_mat,
            model,
            vertices,
            indices,
            face_textures,
        );
    }
}

/// Generate 6 face quads for an entity cube.
fn generate_cube_faces(
    cube: &EntityCube,
    transform: &Mat4,
    facing_mat: &Mat4,
    model: &EntityModelDef,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    let [ox, oy, oz] = cube.origin;
    let [w, h, d] = cube.dimensions;
    let inf = cube.inflate;

    // Cube corners in 1/16th block units, with inflate
    let x0 = (ox - inf) / 16.0;
    let y0 = (oy - inf) / 16.0;
    let z0 = (oz - inf) / 16.0;
    let x1 = (ox + w + inf) / 16.0;
    let y1 = (oy + h + inf) / 16.0;
    let z1 = (oz + d + inf) / 16.0;

    // The 8 corners of the cube in local space
    let corners = [
        Vec3::new(x0, y0, z0), // 0: ---
        Vec3::new(x1, y0, z0), // 1: +--
        Vec3::new(x1, y1, z0), // 2: ++-
        Vec3::new(x0, y1, z0), // 3: -+-
        Vec3::new(x0, y0, z1), // 4: --+
        Vec3::new(x1, y0, z1), // 5: +-+
        Vec3::new(x1, y1, z1), // 6: +++
        Vec3::new(x0, y1, z1), // 7: -++
    ];

    // Apply part transform then facing transform.
    let full_transform = *facing_mat * *transform;
    let transformed: Vec<[f32; 3]> = corners.iter().map(|c| {
        let p = full_transform * Vec4::new(c.x, c.y, c.z, 1.0);
        [p.x, p.y, p.z]
    }).collect();

    // 6 faces: each defined by 4 corner indices and a direction
    let face_defs: [(Direction, [usize; 4]); 6] = [
        // Down: bottom face (y0), CCW from outside → looking from -Y
        (Direction::Down, [4, 5, 1, 0]),
        // Up: top face (y1)
        (Direction::Up, [3, 2, 6, 7]),
        // North: front face (z0)
        (Direction::North, [1, 0, 3, 2]),
        // South: back face (z1)
        (Direction::South, [4, 5, 6, 7]),
        // West: left face (x0)
        (Direction::West, [0, 4, 7, 3]),
        // East: right face (x1)
        (Direction::East, [5, 1, 2, 6]),
    ];

    for &(direction, corner_indices) in &face_defs {
        if cube.skip_faces.contains(&direction) {
            continue;
        }

        let uvs = cube_face_uvs(
            cube.tex_offset,
            cube.dimensions,
            direction,
            model.texture_size,
            cube.mirror,
        );

        // Compute normal from known direction, transformed by the full rotation
        let dn = direction.normal();
        let n4 = full_transform * Vec4::new(dn[0], dn[1], dn[2], 0.0);
        let normal = Vec3::new(n4.x, n4.y, n4.z).normalize_or_zero();
        let n = [normal.x, normal.y, normal.z];

        let v_start = vertices.len() as u32;

        for (i, &ci) in corner_indices.iter().enumerate() {
            vertices.push(Vertex::new(transformed[ci], n, uvs[i]));
        }

        // Two triangles: CCW winding for glTF
        // Down/Up faces are correct with (0,2,1)(0,3,2) winding;
        // side faces need reversed winding (0,1,2)(0,2,3) to face outward
        let is_side = matches!(direction,
            Direction::North | Direction::South | Direction::West | Direction::East);
        if is_side {
            indices.extend_from_slice(&[
                v_start, v_start + 1, v_start + 2,
                v_start, v_start + 2, v_start + 3,
            ]);
        } else {
            indices.extend_from_slice(&[
                v_start, v_start + 2, v_start + 1,
                v_start, v_start + 3, v_start + 2,
            ]);
        }

        face_textures.push(EntityFaceTexture {
            texture: model.texture_path.clone(),
            is_transparent: !model.is_opaque,
        });
    }
}

// ── Model Definitions ───────────────────────────────────────────────────────

fn build_model_def(entity_type: &BlockEntityType) -> EntityModelDef {
    match entity_type {
        BlockEntityType::Chest(variant) => chest::chest_model(*variant),
        BlockEntityType::DoubleChest { variant, side } => chest::double_chest_model(*variant, *side),
        BlockEntityType::Bed { color, is_head } => bed::bed_model(color, *is_head),
        BlockEntityType::Bell => bell::bell_model(),
        BlockEntityType::Sign { wood, is_wall } => sign::sign_model(*wood, *is_wall),
        BlockEntityType::Skull(skull_type) => skull::skull_model(*skull_type),
        BlockEntityType::ShulkerBox { color } => shulker::shulker_model(color.as_deref()),
        BlockEntityType::Banner { is_wall, .. } => {
            // Texture path will be overridden by dynamic texture in add_entity
            banner::banner_model(!is_wall, "_banner/default")
        }
        BlockEntityType::HangingSign { wood, is_wall } => {
            hanging_sign::hanging_sign_model(*wood, *is_wall)
        }
        BlockEntityType::DecoratedPot => {
            // Decorated pot uses custom per-face geometry, this is a fallback
            unreachable!("Decorated pots handled directly in add_entity")
        }
        BlockEntityType::Lectern | BlockEntityType::EnchantingTable => book::book_model(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chest() {
        let block = InputBlock::new("minecraft:chest");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Chest(ChestVariant::Normal))));
    }

    #[test]
    fn test_detect_trapped_chest() {
        let block = InputBlock::new("minecraft:trapped_chest");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Chest(ChestVariant::Trapped))));
    }

    #[test]
    fn test_detect_ender_chest() {
        let block = InputBlock::new("minecraft:ender_chest");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Chest(ChestVariant::Ender))));
    }

    #[test]
    fn test_detect_double_chest() {
        let block = InputBlock::new("minecraft:chest")
            .with_property("type", "left")
            .with_property("facing", "north");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::DoubleChest {
            variant: ChestVariant::Normal,
            side: DoubleChestSide::Left,
        })));
    }

    #[test]
    fn test_detect_bed() {
        let block = InputBlock::new("minecraft:red_bed")
            .with_property("part", "head");
        let entity = detect_block_entity(&block);
        match entity {
            Some(BlockEntityType::Bed { color, is_head }) => {
                assert_eq!(color, "red");
                assert!(is_head);
            }
            _ => panic!("Expected Bed entity"),
        }
    }

    #[test]
    fn test_detect_bell() {
        let block = InputBlock::new("minecraft:bell");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Bell)));
    }

    #[test]
    fn test_detect_sign() {
        let block = InputBlock::new("minecraft:oak_sign");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Sign { wood: SignWood::Oak, is_wall: false })));

        let block = InputBlock::new("minecraft:wall_birch_sign");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Sign { wood: SignWood::Birch, is_wall: true })));
    }

    #[test]
    fn test_detect_skull() {
        let block = InputBlock::new("minecraft:skeleton_skull");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Skull(SkullType::Skeleton))));

        let block = InputBlock::new("minecraft:creeper_head");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Skull(SkullType::Creeper))));
    }

    #[test]
    fn test_detect_shulker_box() {
        let block = InputBlock::new("minecraft:shulker_box");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::ShulkerBox { color: None })));

        let block = InputBlock::new("minecraft:red_shulker_box");
        match detect_block_entity(&block) {
            Some(BlockEntityType::ShulkerBox { color: Some(c) }) => assert_eq!(c, "red"),
            _ => panic!("Expected ShulkerBox with color"),
        }
    }

    #[test]
    fn test_detect_non_entity() {
        let block = InputBlock::new("minecraft:stone");
        assert!(detect_block_entity(&block).is_none());

        let block = InputBlock::new("minecraft:oak_planks");
        assert!(detect_block_entity(&block).is_none());
    }

    #[test]
    fn test_cube_face_uvs_north() {
        // A 4x4x4 cube at tex offset (0,0) on a 64x64 texture
        let uvs = cube_face_uvs([0, 0], [4.0, 4.0, 4.0], Direction::North, [64, 64], false);
        // North face: (u0+D, v0+D, u0+D+W, v0+D+H) = (4, 4, 8, 8) / 64
        // Non-mirrored: UV[0] = (nr, nt), UV[1] = (nl, nt)
        assert!((uvs[0][0] - 8.0 / 64.0).abs() < 0.001); // nr = right
        assert!((uvs[0][1] - 4.0 / 64.0).abs() < 0.001); // nt = top
        assert!((uvs[1][0] - 4.0 / 64.0).abs() < 0.001); // nl = left
        assert!((uvs[2][1] - 8.0 / 64.0).abs() < 0.001); // nb = bottom
    }

    #[test]
    fn test_cube_face_uvs_mirrored() {
        let normal = cube_face_uvs([0, 0], [4.0, 4.0, 4.0], Direction::North, [64, 64], false);
        let mirrored = cube_face_uvs([0, 0], [4.0, 4.0, 4.0], Direction::North, [64, 64], true);
        // Mirrored: left and right U coords should be swapped vs normal
        assert!((normal[0][0] - mirrored[1][0]).abs() < 0.001);
        assert!((normal[1][0] - mirrored[0][0]).abs() < 0.001);
    }

    #[test]
    fn test_chest_geometry_count() {
        let block = InputBlock::new("minecraft:chest")
            .with_property("facing", "north");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Chest(ChestVariant::Normal),
        );

        // Chest has bottom (1 cube x 6 faces) + lid (1 cube x 6 faces) + lock (1 cube x 6 faces) = 18 faces
        // Each face: 4 vertices, 6 indices
        assert_eq!(faces.len(), 18);
        assert_eq!(verts.len(), 18 * 4);
        assert_eq!(indices.len(), 18 * 6);
    }

    #[test]
    fn test_bell_geometry_count() {
        let block = InputBlock::new("minecraft:bell")
            .with_property("facing", "north");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Bell,
        );

        // Bell: body (1 cube x 6) + lip (1 cube x 6) = 12 faces
        assert_eq!(faces.len(), 12);
        assert_eq!(verts.len(), 12 * 4);
        assert_eq!(indices.len(), 12 * 6);
    }

    #[test]
    fn test_skull_geometry_count_no_hat() {
        // Skeleton skull: no hat overlay → 6 faces (head only)
        let block = InputBlock::new("minecraft:skeleton_skull");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Skull(SkullType::Skeleton),
        );

        assert_eq!(faces.len(), 6);
        assert_eq!(verts.len(), 6 * 4);
        assert_eq!(indices.len(), 6 * 6);
    }

    #[test]
    fn test_skull_geometry_count_with_hat() {
        // Zombie head: has hat overlay → 12 faces (head + hat)
        let block = InputBlock::new("minecraft:zombie_head");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Skull(SkullType::Zombie),
        );

        assert_eq!(faces.len(), 12);
        assert_eq!(verts.len(), 12 * 4);
        assert_eq!(indices.len(), 12 * 6);
    }

    #[test]
    fn test_shulker_geometry_count() {
        let block = InputBlock::new("minecraft:shulker_box")
            .with_property("facing", "up");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::ShulkerBox { color: None },
        );

        // Shulker: base (1 cube x 5 faces, skip Down) + lid (1 cube x 5 faces, skip Up) = 10 faces
        assert_eq!(faces.len(), 10);
        assert_eq!(verts.len(), 10 * 4);
        assert_eq!(indices.len(), 10 * 6);
    }

    #[test]
    fn test_sign_standing_geometry_count() {
        let block = InputBlock::new("minecraft:oak_sign");
        let (_, _, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Sign { wood: SignWood::Oak, is_wall: false },
        );

        // Standing sign: board (1 cube x 6) + stick (1 cube x 6) = 12 faces
        assert_eq!(faces.len(), 12);
    }

    #[test]
    fn test_sign_wall_geometry_count() {
        let block = InputBlock::new("minecraft:wall_oak_sign")
            .with_property("facing", "north");
        let (_, _, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Sign { wood: SignWood::Oak, is_wall: true },
        );

        // Wall sign: board only (1 cube x 6) = 6 faces
        assert_eq!(faces.len(), 6);
    }

    #[test]
    fn test_facing_rotation() {
        // Generate chest geometry facing north vs south — vertices should differ
        let block_n = InputBlock::new("minecraft:chest")
            .with_property("facing", "north");
        let block_s = InputBlock::new("minecraft:chest")
            .with_property("facing", "south");

        let (verts_n, _, _) = generate_entity_geometry(
            &block_n,
            &BlockEntityType::Chest(ChestVariant::Normal),
        );
        let (verts_s, _, _) = generate_entity_geometry(
            &block_s,
            &BlockEntityType::Chest(ChestVariant::Normal),
        );

        // At least some vertex positions should differ between orientations
        let mut any_different = false;
        for (vn, vs) in verts_n.iter().zip(verts_s.iter()) {
            if (vn.position[0] - vs.position[0]).abs() > 0.01
                || (vn.position[2] - vs.position[2]).abs() > 0.01
            {
                any_different = true;
                break;
            }
        }
        assert!(any_different, "North and south chests should have different vertex positions");
    }

    #[test]
    fn test_detect_mob() {
        assert!(matches!(detect_mob(&InputBlock::new("entity:zombie")), Some(MobType::Zombie)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:skeleton")), Some(MobType::Skeleton)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:creeper")), Some(MobType::Creeper)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:pig")), Some(MobType::Pig)));
        assert!(detect_mob(&InputBlock::new("minecraft:stone")).is_none());
    }

    #[test]
    fn test_zombie_geometry_count() {
        let block = InputBlock::new("entity:zombie")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Zombie);

        // Zombie: 7 parts (head, hat, body, 2 arms, 2 legs) × 6 faces = 42 faces
        assert_eq!(faces.len(), 42);
        assert_eq!(verts.len(), 42 * 4);
        assert_eq!(indices.len(), 42 * 6);
    }

    #[test]
    fn test_creeper_geometry_count() {
        let block = InputBlock::new("entity:creeper")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Creeper);

        // Creeper: 6 parts (head, body, 4 legs) × 6 faces = 36 faces
        assert_eq!(faces.len(), 36);
        assert_eq!(verts.len(), 36 * 4);
        assert_eq!(indices.len(), 36 * 6);
    }

    #[test]
    fn test_pig_geometry_count() {
        let block = InputBlock::new("entity:pig")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Pig);

        // Pig: 7 parts (head with snout child, body, 4 legs) × 6 faces = 42 faces
        assert_eq!(faces.len(), 42);
        assert_eq!(verts.len(), 42 * 4);
        assert_eq!(indices.len(), 42 * 6);
    }

    #[test]
    fn test_detect_armor_stand() {
        assert!(matches!(detect_mob(&InputBlock::new("entity:armor_stand")), Some(MobType::ArmorStand)));
    }

    #[test]
    fn test_detect_minecart() {
        assert!(matches!(detect_mob(&InputBlock::new("entity:minecart")), Some(MobType::Minecart)));
    }

    #[test]
    fn test_detect_item_frame() {
        assert!(matches!(detect_mob(&InputBlock::new("entity:item_frame")), Some(MobType::ItemFrame)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:glow_item_frame")), Some(MobType::GlowItemFrame)));
    }

    #[test]
    fn test_armor_stand_geometry_count() {
        let block = InputBlock::new("entity:armor_stand")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::ArmorStand);

        // Armor stand: 10 cubes × 6 faces = 60 faces
        assert_eq!(faces.len(), 60);
        assert_eq!(verts.len(), 60 * 4);
        assert_eq!(indices.len(), 60 * 6);
    }

    #[test]
    fn test_minecart_geometry_count() {
        let block = InputBlock::new("entity:minecart")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Minecart);

        // Minecart: 5 cubes × 6 faces = 30 faces
        assert_eq!(faces.len(), 30);
        assert_eq!(verts.len(), 30 * 4);
        assert_eq!(indices.len(), 30 * 6);
    }

    #[test]
    fn test_item_frame_geometry_count() {
        let block = InputBlock::new("entity:item_frame")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::ItemFrame);

        // Item frame: 2 + 6 + 6 + 4 + 4 = 22 faces
        assert_eq!(faces.len(), 22);
        assert_eq!(verts.len(), 22 * 4);
        assert_eq!(indices.len(), 22 * 6);
    }

    #[test]
    fn test_detect_new_mobs() {
        assert!(matches!(detect_mob(&InputBlock::new("entity:chicken")), Some(MobType::Chicken)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:cow")), Some(MobType::Cow)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:sheep")), Some(MobType::Sheep)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:villager")), Some(MobType::Villager)));
    }

    #[test]
    fn test_chicken_geometry_count() {
        let block = InputBlock::new("entity:chicken")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Chicken);

        // Chicken: 7 parts (head with beak+wattle children, body, 2 legs, 2 wings) = 8 cubes × 6 = 48 faces
        assert_eq!(faces.len(), 48);
        assert_eq!(verts.len(), 48 * 4);
        assert_eq!(indices.len(), 48 * 6);
    }

    #[test]
    fn test_cow_geometry_count() {
        let block = InputBlock::new("entity:cow")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Cow);

        // Cow: 8 parts (head with 2 horn children, body, 4 legs) = 8 cubes × 6 = 48 faces
        assert_eq!(faces.len(), 48);
        assert_eq!(verts.len(), 48 * 4);
        assert_eq!(indices.len(), 48 * 6);
    }

    #[test]
    fn test_sheep_geometry_count() {
        let block = InputBlock::new("entity:sheep")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Sheep);

        // Sheep base: 6 parts (head, body, 4 legs) × 6 faces = 36 faces
        assert_eq!(faces.len(), 36);
        assert_eq!(verts.len(), 36 * 4);
        assert_eq!(indices.len(), 36 * 6);
    }

    #[test]
    fn test_villager_geometry_count() {
        let block = InputBlock::new("entity:villager")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Villager);

        // Villager: head(1) + hat(1) + hat_rim(1) + nose(1) + body(1) + jacket(1) + arms(3) + 2 legs = 11 cubes × 6 = 66 faces
        assert_eq!(faces.len(), 66);
        assert_eq!(verts.len(), 66 * 4);
        assert_eq!(indices.len(), 66 * 6);
    }

    #[test]
    fn test_detect_new_mobs_wave2() {
        assert!(matches!(detect_mob(&InputBlock::new("entity:wolf")), Some(MobType::Wolf)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:cat")), Some(MobType::Cat)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:spider")), Some(MobType::Spider)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:horse")), Some(MobType::Horse)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:enderman")), Some(MobType::Enderman)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:slime")), Some(MobType::Slime)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:iron_golem")), Some(MobType::IronGolem)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:bat")), Some(MobType::Bat)));
    }

    #[test]
    fn test_wolf_geometry_count() {
        let block = InputBlock::new("entity:wolf").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Wolf);
        // Wolf: head(1) + 2 ears + snout + body + tail + 4 legs = 10 cubes × 6 = 60 faces
        assert_eq!(faces.len(), 60);
        assert_eq!(verts.len(), 60 * 4);
        assert_eq!(indices.len(), 60 * 6);
    }

    #[test]
    fn test_cat_geometry_count() {
        let block = InputBlock::new("entity:cat").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Cat);
        // Cat: head + body + 2 tails + 4 legs = 8 cubes × 6 = 48 faces
        assert_eq!(faces.len(), 48);
        assert_eq!(verts.len(), 48 * 4);
        assert_eq!(indices.len(), 48 * 6);
    }

    #[test]
    fn test_spider_geometry_count() {
        let block = InputBlock::new("entity:spider").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Spider);
        // Spider: head + neck + abdomen + 8 legs = 11 cubes × 6 = 66 faces
        assert_eq!(faces.len(), 66);
        assert_eq!(verts.len(), 66 * 4);
        assert_eq!(indices.len(), 66 * 6);
    }

    #[test]
    fn test_horse_geometry_count() {
        let block = InputBlock::new("entity:horse").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Horse);
        // Horse: head_parts(1) + mane + mouth + body + tail + 4 legs = 9 cubes × 6 = 54 faces
        assert_eq!(faces.len(), 54);
        assert_eq!(verts.len(), 54 * 4);
        assert_eq!(indices.len(), 54 * 6);
    }

    #[test]
    fn test_enderman_geometry_count() {
        let block = InputBlock::new("entity:enderman").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Enderman);
        // Enderman: head + hat + body + 2 arms + 2 legs = 7 cubes × 6 = 42 faces
        assert_eq!(faces.len(), 42);
        assert_eq!(verts.len(), 42 * 4);
        assert_eq!(indices.len(), 42 * 6);
    }

    #[test]
    fn test_slime_geometry_count() {
        let block = InputBlock::new("entity:slime").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Slime);
        // Slime: outer + inner + 2 eyes + mouth = 5 cubes × 6 = 30 faces
        assert_eq!(faces.len(), 30);
        assert_eq!(verts.len(), 30 * 4);
        assert_eq!(indices.len(), 30 * 6);
    }

    #[test]
    fn test_iron_golem_geometry_count() {
        let block = InputBlock::new("entity:iron_golem").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::IronGolem);
        // Iron golem: head(1) + nose + body + waist + 2 arms + 2 legs = 8 cubes × 6 = 48 faces
        assert_eq!(faces.len(), 48);
        assert_eq!(verts.len(), 48 * 4);
        assert_eq!(indices.len(), 48 * 6);
    }

    #[test]
    fn test_bat_geometry_count() {
        let block = InputBlock::new("entity:bat").with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Bat);
        // Bat: head + body + 2 wings (each with wing_tip child) = 6 cubes × 6 = 36 faces
        assert_eq!(faces.len(), 36);
        assert_eq!(verts.len(), 36 * 4);
        assert_eq!(indices.len(), 36 * 6);
    }

    #[test]
    fn test_armor_stand_pose() {
        let block = InputBlock::new("entity:armor_stand")
            .with_property("facing", "south")
            .with_property("RightArmPose", "-10,0,-10");
        let (verts, _indices, faces) = generate_mob_geometry(&block, MobType::ArmorStand);
        // Same face count as normal armor stand
        assert_eq!(faces.len(), 60);
        // Vertices should differ from default pose
        let default_block = InputBlock::new("entity:armor_stand")
            .with_property("facing", "south");
        let (default_verts, _, _) = generate_mob_geometry(&default_block, MobType::ArmorStand);
        let any_different = verts.iter().zip(default_verts.iter())
            .any(|(a, b)| (a.position[0] - b.position[0]).abs() > 0.001
                       || (a.position[1] - b.position[1]).abs() > 0.001);
        assert!(any_different, "Posed armor stand should have different vertex positions");
    }

    #[test]
    fn test_detect_banner() {
        let block = InputBlock::new("minecraft:white_banner");
        match detect_block_entity(&block) {
            Some(BlockEntityType::Banner { color, is_wall }) => {
                assert_eq!(color, "white");
                assert!(!is_wall);
            }
            _ => panic!("Expected Banner entity"),
        }

        let block = InputBlock::new("minecraft:red_wall_banner");
        match detect_block_entity(&block) {
            Some(BlockEntityType::Banner { color, is_wall }) => {
                assert_eq!(color, "red");
                assert!(is_wall);
            }
            _ => panic!("Expected Wall Banner entity"),
        }
    }

    #[test]
    fn test_parse_patterns() {
        let patterns = banner::parse_patterns("stripe_bottom:red,cross:blue");
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], ("stripe_bottom".to_string(), "red".to_string()));
        assert_eq!(patterns[1], ("cross".to_string(), "blue".to_string()));

        let empty = banner::parse_patterns("");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_detect_hanging_sign() {
        let block = InputBlock::new("minecraft:oak_hanging_sign");
        match detect_block_entity(&block) {
            Some(BlockEntityType::HangingSign { wood, is_wall }) => {
                assert_eq!(wood, SignWood::Oak);
                assert!(!is_wall);
            }
            other => panic!("Expected HangingSign, got {:?}", other),
        }

        let block = InputBlock::new("minecraft:birch_wall_hanging_sign");
        match detect_block_entity(&block) {
            Some(BlockEntityType::HangingSign { wood, is_wall }) => {
                assert_eq!(wood, SignWood::Birch);
                assert!(is_wall);
            }
            other => panic!("Expected wall HangingSign, got {:?}", other),
        }
    }

    #[test]
    fn test_hanging_sign_ceiling_geometry_count() {
        let block = InputBlock::new("minecraft:oak_hanging_sign")
            .with_property("rotation", "0");
        let entity_type = BlockEntityType::HangingSign { wood: SignWood::Oak, is_wall: false };
        let (verts, indices, faces) = generate_entity_geometry(&block, &entity_type);

        // Ceiling: board(6) + plank(6) + left_chain(6) + right_chain(6) = 24 faces
        assert_eq!(faces.len(), 24);
        assert_eq!(verts.len(), 24 * 4);
        assert_eq!(indices.len(), 24 * 6);
    }

    #[test]
    fn test_hanging_sign_wall_geometry_count() {
        let block = InputBlock::new("minecraft:oak_wall_hanging_sign")
            .with_property("facing", "north");
        let entity_type = BlockEntityType::HangingSign { wood: SignWood::Oak, is_wall: true };
        let (verts, indices, faces) = generate_entity_geometry(&block, &entity_type);

        // Wall: board only = 6 faces
        assert_eq!(faces.len(), 6);
        assert_eq!(verts.len(), 6 * 4);
        assert_eq!(indices.len(), 6 * 6);
    }

    #[test]
    fn test_detect_decorated_pot() {
        let block = InputBlock::new("minecraft:decorated_pot");
        assert!(matches!(detect_block_entity(&block), Some(BlockEntityType::DecoratedPot)));
    }

    #[test]
    fn test_decorated_pot_geometry_count() {
        let block = InputBlock::new("minecraft:decorated_pot")
            .with_property("facing", "north");
        let (verts, indices, faces) = decorated_pot::generate_decorated_pot_geometry(&block);

        // 3 boxes × 6 faces = 18 faces
        assert_eq!(faces.len(), 18);
        assert_eq!(verts.len(), 18 * 4);
        assert_eq!(indices.len(), 18 * 6);
    }

    #[test]
    fn test_baby_mob_scaling() {
        let adult = InputBlock::new("entity:pig")
            .with_property("facing", "south");
        let baby = InputBlock::new("entity:pig")
            .with_property("facing", "south")
            .with_property("is_baby", "true");

        let (adult_verts, _, adult_faces) = generate_mob_geometry(&adult, MobType::Pig);
        let (baby_verts, _, baby_faces) = generate_mob_geometry(&baby, MobType::Pig);

        // Same face count
        assert_eq!(adult_faces.len(), baby_faces.len());

        // Baby should have smaller Y extent (closer to ground)
        let adult_max_y = adult_verts.iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max);
        let baby_max_y = baby_verts.iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max);
        assert!(baby_max_y < adult_max_y, "Baby should be shorter than adult");
    }

    #[test]
    fn test_baby_scaling_not_applied_to_unsupported() {
        // Spider doesn't support baby scaling
        let normal = InputBlock::new("entity:spider")
            .with_property("facing", "south");
        let baby = InputBlock::new("entity:spider")
            .with_property("facing", "south")
            .with_property("is_baby", "true");

        let (normal_verts, _, _) = generate_mob_geometry(&normal, MobType::Spider);
        let (baby_verts, _, _) = generate_mob_geometry(&baby, MobType::Spider);

        // Positions should be identical since spider doesn't support baby
        let all_same = normal_verts.iter().zip(baby_verts.iter())
            .all(|(a, b)| (a.position[0] - b.position[0]).abs() < 0.001
                       && (a.position[1] - b.position[1]).abs() < 0.001
                       && (a.position[2] - b.position[2]).abs() < 0.001);
        assert!(all_same, "Spider should not be affected by baby scaling");
    }

    #[test]
    fn test_detect_lectern_with_book() {
        let block = InputBlock::new("minecraft:lectern")
            .with_property("has_book", "true")
            .with_property("facing", "north");
        assert!(matches!(detect_block_entity(&block), Some(BlockEntityType::Lectern)));
    }

    #[test]
    fn test_detect_lectern_without_book() {
        let block = InputBlock::new("minecraft:lectern")
            .with_property("has_book", "false")
            .with_property("facing", "north");
        assert!(detect_block_entity(&block).is_none());
    }

    #[test]
    fn test_detect_lectern_no_property() {
        let block = InputBlock::new("minecraft:lectern")
            .with_property("facing", "north");
        assert!(detect_block_entity(&block).is_none());
    }

    #[test]
    fn test_detect_enchanting_table() {
        let block = InputBlock::new("minecraft:enchanting_table");
        assert!(matches!(detect_block_entity(&block), Some(BlockEntityType::EnchantingTable)));
    }

    #[test]
    fn test_lectern_book_geometry_count() {
        let block = InputBlock::new("minecraft:lectern")
            .with_property("has_book", "true")
            .with_property("facing", "north");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Lectern,
        );

        // Book: 7 parts × 6 faces = 42 faces
        assert_eq!(faces.len(), 42);
        assert_eq!(verts.len(), 42 * 4);
        assert_eq!(indices.len(), 42 * 6);
    }

    #[test]
    fn test_enchanting_table_book_geometry_count() {
        let block = InputBlock::new("minecraft:enchanting_table")
            .with_property("facing", "north");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::EnchantingTable,
        );

        // Book: 7 parts × 6 faces = 42 faces
        assert_eq!(faces.len(), 42);
        assert_eq!(verts.len(), 42 * 4);
        assert_eq!(indices.len(), 42 * 6);
    }

    #[test]
    fn test_lectern_book_is_transparent() {
        let block = InputBlock::new("minecraft:lectern")
            .with_property("has_book", "true")
            .with_property("facing", "north");
        let (_, _, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Lectern,
        );

        // All faces should be transparent (book has thin pages)
        assert!(faces.iter().all(|f| f.is_transparent));
    }

    #[test]
    fn test_lectern_vs_enchanting_table_different_positions() {
        let lectern_block = InputBlock::new("minecraft:lectern")
            .with_property("has_book", "true")
            .with_property("facing", "north");
        let table_block = InputBlock::new("minecraft:enchanting_table")
            .with_property("facing", "north");

        let (lectern_verts, _, _) = generate_entity_geometry(
            &lectern_block,
            &BlockEntityType::Lectern,
        );
        let (table_verts, _, _) = generate_entity_geometry(
            &table_block,
            &BlockEntityType::EnchantingTable,
        );

        // Lectern book should be higher (y≈14.25/16) than enchanting table (y≈12/16)
        let lectern_avg_y: f32 = lectern_verts.iter().map(|v| v.position[1]).sum::<f32>() / lectern_verts.len() as f32;
        let table_avg_y: f32 = table_verts.iter().map(|v| v.position[1]).sum::<f32>() / table_verts.len() as f32;
        assert!(lectern_avg_y > table_avg_y, "Lectern book should be higher than enchanting table book");
    }
}
