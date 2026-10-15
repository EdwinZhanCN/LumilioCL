//! Decorated pot sides: the `sherds` property picks each side's pattern,
//! drawn over the plain side as `DecoratedPotRenderer` does.
//!
//! The compositing follows schematic-mesher's original `element.rs`
//! `composite_pot_side` (AGPL-3.0-only, Schem-at); the pot itself is the
//! game's `decorated_pot_base` and `decorated_pot_sides` layers (LumilioCL
//! local change, plan schematic-studio T7).

use crate::resource_pack::{ResourcePack, TextureData};
use crate::types::InputBlock;

/// The plain side, also every side without a sherd.
pub(crate) const SIDE_TEXTURE: &str = "entity/decorated_pot/decorated_pot_side";

/// The sides of the `decorated_pot_sides` layer in the order Minecraft lists
/// a pot's decorations (`PotDecorations`: back, left, right, front).
pub(crate) const SIDES: [&str; 4] = ["back", "left", "right", "front"];

/// The pattern name of each side (`angler`), `None` for a plain side. The
/// property lists sherd items (`minecraft:angler_pottery_sherd`) or pattern
/// names; `brick` and empty entries are plain.
pub(crate) fn patterns(block: &InputBlock) -> [Option<String>; 4] {
    let mut out: [Option<String>; 4] = Default::default();
    let Some(list) = block.properties.get("sherds") else {
        return out;
    };
    for (slot, entry) in out.iter_mut().zip(list.split(',')) {
        let name = entry.trim();
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        let name = name.strip_suffix("_pottery_sherd").unwrap_or(name);
        if !name.is_empty() && name != "brick" {
            *slot = Some(name.to_owned());
        }
    }
    out
}

/// A pattern drawn over the opaque plain side, so the face has no holes.
pub(crate) fn composite_side(pack: &ResourcePack, pattern: &str) -> Option<TextureData> {
    let side = pack.get_texture(SIDE_TEXTURE)?.first_frame();
    let mut pixels = side.pixels.clone();
    let (w, h) = (side.width, side.height);
    if let Some(pat) = pack.get_texture(&format!("entity/decorated_pot/{pattern}_pottery_pattern"))
    {
        let pat = pat.first_frame();
        for y in 0..pat.height.min(h) {
            for x in 0..pat.width.min(w) {
                let src = ((y * pat.width + x) * 4) as usize;
                let dst = ((y * w + x) * 4) as usize;
                if src + 3 >= pat.pixels.len() || dst + 3 >= pixels.len() {
                    continue;
                }
                let alpha = pat.pixels[src + 3] as f32 / 255.0;
                if alpha < 0.01 {
                    continue;
                }
                for c in 0..3 {
                    let over = pat.pixels[src + c] as f32;
                    let under = pixels[dst + c] as f32;
                    pixels[dst + c] = (over * alpha + under * (1.0 - alpha)).min(255.0) as u8;
                }
                pixels[dst + 3] = 255;
            }
        }
    }
    Some(TextureData {
        width: w,
        height: h,
        pixels,
        is_animated: false,
        frame_count: 1,
        animation: None,
    })
}
