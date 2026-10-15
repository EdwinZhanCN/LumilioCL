//! Biome colormaps sampled for plains (plan schematic-studio, T5).
//!
//! The lookup follows Minecraft's `ColorMapColorUtil.get` as called by
//! `Biome.getGrassColorFromTexture` / `getFoliageColorFromTexture`: downfall is
//! scaled by temperature and both index a 256×256 map from its corner. The
//! biome's climate values are `float`s widened to `double`, as in the game, so
//! the pixel is the one the game picks.

use crate::pack::Image;

/// Plains: `Biomes.PLAINS` is created with temperature 0.8 and downfall 0.4.
const PLAINS_TEMPERATURE: f32 = 0.8;
const PLAINS_DOWNFALL: f32 = 0.4;

/// The colormap's color at plains, or `None` when it is not a 256×256 map.
pub(super) fn plains_sample(map: &Image) -> Option<[u8; 3]> {
    if (map.width, map.height) != (256, 256) {
        return None;
    }
    let temperature = f64::from(PLAINS_TEMPERATURE).clamp(0.0, 1.0);
    let downfall = f64::from(PLAINS_DOWNFALL).clamp(0.0, 1.0) * temperature;
    let x = ((1.0 - temperature) * 255.0) as usize;
    let y = ((1.0 - downfall) * 255.0) as usize;
    let at = (y * 256 + x) * 4;
    map.rgba.get(at..at + 3)?.try_into().ok()
}
