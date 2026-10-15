//! Block entities and entities: chests, beds, signs, banners, heads, mobs,
//! vehicles, item frames.
//!
//! Their shapes are not block models; the game builds them in code. The
//! launcher bakes that code's output, every model layer of a game version,
//! into `entities.dat.z`, and the caller hands one version's layers to the
//! [`ResourcePack`](crate::ResourcePack). Which block or entity draws which
//! layer with which texture, posed and placed how, is the binding table
//! `data/entity_bindings.json` ([`bindings`]); [`render`] draws a bound
//! block from it. Composition (banner patterns, sign text, pot sherds, skins,
//! villager clothes, frame contents) stays code, switched on by the table.
//!
//! LumilioCL local change (plan schematic-studio T7): this replaces
//! schematic-mesher's `detect_block_entity`, `detect_mob` and its hand-copied
//! geometry files (AGPL-3.0-only, Schem-at); see `forks/README.md`.

mod armor;
pub(crate) mod banner;
pub mod bindings;
pub(crate) mod decorated_pot;
pub(crate) mod inventory;
pub(crate) mod item_frame;
pub mod item_render;
mod layers;
pub mod model;
pub mod painting;
pub(crate) mod particle;
pub mod render;
pub(crate) mod sign_text;
pub(crate) mod skin;
pub(crate) mod villager_texture;

pub use model::{EntityCube, EntityFaceTexture, EntityModelDef, EntityPart, EntityPartPose};

/// Y rotation (radians, for `Mat4::from_rotation_y`) of a `facing`. Entity
/// models face south (+Z) unrotated. glam turns counter-clockwise seen from
/// above, so +Z reaches +X (east) at +90° — the opposite sign of Minecraft's
/// yaw (east = 270°); Minecraft's renderers use `Axis.YP.rotationDegrees(-yaw)`.
pub(crate) fn facing_rotation_rad(facing: &str) -> f32 {
    match facing {
        "south" => 0.0,
        "east" => std::f32::consts::FRAC_PI_2,
        "west" => -std::f32::consts::FRAC_PI_2,
        _ => std::f32::consts::PI,
    }
}

/// `DyeColor.getTextureDiffuseColor()` by dye name (LumilioCL local change,
/// plan schematic-studio T35; read from the 26.3 client's `DyeColor`, the
/// same since 1.12): what tints banner patterns, a tropical fish, a
/// shulker's or bed's dyed texture. White for an unknown name.
pub(crate) fn dye_diffuse(color: &str) -> [u8; 3] {
    let rgb: u32 = match color {
        "orange" => 0xF9801D,
        "magenta" => 0xC74EBD,
        "light_blue" => 0x3AB3DA,
        "yellow" => 0xFED83D,
        "lime" => 0x80C71F,
        "pink" => 0xF38BAA,
        "gray" => 0x474F52,
        "light_gray" => 0x9D9D97,
        "cyan" => 0x169C9C,
        "purple" => 0x8932B8,
        "blue" => 0x3C44AA,
        "brown" => 0x835432,
        "green" => 0x5E7C16,
        "red" => 0xB02E26,
        "black" => 0x1D1D21,
        _ => 0xF9FFFE,
    };
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

/// A dye's diffuse colour as a vertex tint.
pub(crate) fn dye_rgb(color: &str) -> [f32; 4] {
    let [r, g, b] = dye_diffuse(color);
    [
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    ]
}

/// A sheep's wool tint (`ColorLerper.getModifiedColor` for `SHEEP`): white
/// is 0xE6E6E6, any other dye its diffuse colour at three quarters.
pub(crate) fn wool_rgb(color: &str) -> [f32; 4] {
    if dye_diffuse(color) == dye_diffuse("white") {
        let white = f32::from(0xE6_u8) / 255.0;
        return [white, white, white, 1.0];
    }
    let [r, g, b] = dye_diffuse(color).map(|channel| (f32::from(channel) * 0.75).floor() / 255.0);
    [r, g, b, 1.0]
}
