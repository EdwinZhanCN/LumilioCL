//! Block tinting for grass, foliage, water, redstone, and other colored blocks.
//!
//! In Minecraft, certain blocks have their textures tinted with colors that can
//! depend on biome, block state, or other factors.

use crate::types::InputBlock;

/// Tint colors for different block categories.
#[derive(Debug, Clone)]
pub struct TintColors {
    /// Grass tint (for grass blocks, grass, tall grass, etc.)
    pub grass: [f32; 4],
    /// Foliage tint (for leaves, vines, etc.)
    pub foliage: [f32; 4],
    /// Water tint
    pub water: [f32; 4],
    /// Redstone dust colors by power level (0-15)
    pub redstone: [[f32; 4]; 16],
    /// Stem colors (for melon/pumpkin stems by growth stage)
    pub stem: [[f32; 4]; 8],
    /// Lily pad tint
    pub lily_pad: [f32; 4],
    /// Potion/cauldron water color
    pub cauldron_water: [f32; 4],
}

impl Default for TintColors {
    fn default() -> Self {
        Self {
            // Default grass color (plains biome approximate)
            grass: [0.56, 0.74, 0.35, 1.0],
            // Default foliage color (plains biome approximate)
            foliage: [0.47, 0.66, 0.23, 1.0],
            // Default water color (light blue)
            water: [0.247, 0.463, 0.894, 1.0],
            // Redstone dust colors from power 0 to 15
            redstone: Self::default_redstone_colors(),
            // Stem colors from stage 0 to 7
            stem: Self::default_stem_colors(),
            // Lily pad has a darker green
            lily_pad: [0.13, 0.55, 0.13, 1.0],
            // Cauldron water (same as regular water by default)
            cauldron_water: [0.247, 0.463, 0.894, 1.0],
        }
    }
}

impl TintColors {
    /// Create tint colors for a specific biome.
    pub fn for_biome(biome: &str) -> Self {
        let mut colors = Self::default();

        // Override based on biome
        match biome {
            "swamp" | "minecraft:swamp" | "mangrove_swamp" | "minecraft:mangrove_swamp" => {
                colors.grass = [0.41, 0.55, 0.27, 1.0];
                colors.foliage = [0.41, 0.55, 0.27, 1.0];
                colors.water = [0.38, 0.48, 0.27, 1.0];
            }
            "badlands" | "minecraft:badlands" | "wooded_badlands" | "minecraft:wooded_badlands"
            | "eroded_badlands" | "minecraft:eroded_badlands" => {
                colors.grass = [0.56, 0.50, 0.30, 1.0];
                colors.foliage = [0.62, 0.56, 0.35, 1.0];
            }
            "jungle" | "minecraft:jungle" | "bamboo_jungle" | "minecraft:bamboo_jungle"
            | "sparse_jungle" | "minecraft:sparse_jungle" => {
                colors.grass = [0.35, 0.75, 0.15, 1.0];
                colors.foliage = [0.30, 0.72, 0.20, 1.0];
            }
            "dark_forest" | "minecraft:dark_forest" => {
                colors.grass = [0.31, 0.55, 0.20, 1.0];
                colors.foliage = [0.31, 0.55, 0.20, 1.0];
            }
            "snowy_plains" | "minecraft:snowy_plains" | "snowy_taiga" | "minecraft:snowy_taiga"
            | "snowy_beach" | "minecraft:snowy_beach" | "snowy_slopes" | "minecraft:snowy_slopes" => {
                colors.grass = [0.50, 0.70, 0.50, 1.0];
                colors.foliage = [0.39, 0.61, 0.39, 1.0];
            }
            "desert" | "minecraft:desert" => {
                colors.grass = [0.75, 0.72, 0.45, 1.0];
                colors.foliage = [0.68, 0.68, 0.40, 1.0];
            }
            "ocean" | "minecraft:ocean" | "deep_ocean" | "minecraft:deep_ocean"
            | "cold_ocean" | "minecraft:cold_ocean" | "deep_cold_ocean" | "minecraft:deep_cold_ocean" => {
                colors.water = [0.24, 0.36, 0.75, 1.0];
            }
            "warm_ocean" | "minecraft:warm_ocean" | "lukewarm_ocean" | "minecraft:lukewarm_ocean"
            | "deep_lukewarm_ocean" | "minecraft:deep_lukewarm_ocean" => {
                colors.water = [0.26, 0.53, 0.80, 1.0];
            }
            "frozen_ocean" | "minecraft:frozen_ocean" | "deep_frozen_ocean" | "minecraft:deep_frozen_ocean" => {
                colors.water = [0.24, 0.30, 0.60, 1.0];
            }
            _ => {
                // Keep defaults for plains and other biomes
            }
        }

        colors
    }

    /// Generate default redstone colors for power levels 0-15.
    fn default_redstone_colors() -> [[f32; 4]; 16] {
        let mut colors = [[0.0; 4]; 16];
        for power in 0..16 {
            // Minecraft redstone color formula (approximation)
            // At power 0: dim red, at power 15: bright red
            let brightness = (power as f32) / 15.0;
            let r = 0.3 + brightness * 0.7;
            let g = brightness * 0.1;
            let b = brightness * 0.1;
            colors[power] = [r, g, b, 1.0];
        }
        colors
    }

    /// Generate default stem colors for growth stages 0-7.
    fn default_stem_colors() -> [[f32; 4]; 8] {
        let mut colors = [[0.0; 4]; 8];
        for stage in 0..8 {
            // Young stems are green, mature stems are yellow/orange
            let t = stage as f32 / 7.0;
            let r = 0.2 + t * 0.6;
            let g = 0.7 - t * 0.2;
            let b = 0.1;
            colors[stage] = [r, g, b, 1.0];
        }
        colors
    }
}

/// Provides tint colors for blocks based on their type and properties.
#[derive(Debug, Clone)]
pub struct TintProvider {
    /// Base tint colors.
    colors: TintColors,
}

impl TintProvider {
    /// Create a new tint provider with default colors.
    pub fn new() -> Self {
        Self {
            colors: TintColors::default(),
        }
    }

    /// Create a tint provider with specific colors.
    pub fn with_colors(colors: TintColors) -> Self {
        Self { colors }
    }

    /// Create a tint provider for a specific biome.
    pub fn for_biome(biome: &str) -> Self {
        Self {
            colors: TintColors::for_biome(biome),
        }
    }

    /// Get the tint color for a block face.
    /// Returns white [1,1,1,1] if no tinting should be applied.
    pub fn get_tint(&self, block: &InputBlock, tint_index: i32) -> [f32; 4] {
        if tint_index < 0 {
            return [1.0, 1.0, 1.0, 1.0];
        }

        let block_id = block.block_id();

        // Determine tint category based on block name
        match self.categorize_block(block_id) {
            TintCategory::Grass => self.colors.grass,
            TintCategory::Foliage => self.colors.foliage,
            TintCategory::Water => self.colors.water,
            TintCategory::Redstone => self.get_redstone_tint(block),
            TintCategory::Stem => self.get_stem_tint(block),
            TintCategory::LilyPad => self.colors.lily_pad,
            TintCategory::None => [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Get the tint colors reference for direct access.
    pub fn colors(&self) -> &TintColors {
        &self.colors
    }

    /// Categorize a block by its tinting behavior.
    fn categorize_block(&self, block_id: &str) -> TintCategory {
        // Grass-colored blocks
        if matches!(block_id,
            "grass_block" | "grass" | "tall_grass" | "fern" | "large_fern" |
            "potted_fern" | "short_grass"
        ) {
            return TintCategory::Grass;
        }

        // Foliage-colored blocks (leaves, vines)
        if block_id.ends_with("_leaves") && !block_id.starts_with("azalea") {
            return TintCategory::Foliage;
        }
        if matches!(block_id, "vine" | "oak_leaves" | "jungle_leaves" | "acacia_leaves" |
                    "dark_oak_leaves" | "mangrove_leaves") {
            return TintCategory::Foliage;
        }

        // Water-colored blocks
        if matches!(block_id, "water" | "bubble_column") {
            return TintCategory::Water;
        }

        // Cauldron with water
        if block_id == "water_cauldron" {
            return TintCategory::Water;
        }

        // Redstone
        if block_id == "redstone_wire" || block_id == "redstone_dust" {
            return TintCategory::Redstone;
        }

        // Stems
        if matches!(block_id, "melon_stem" | "pumpkin_stem" | "attached_melon_stem" | "attached_pumpkin_stem") {
            return TintCategory::Stem;
        }

        // Lily pad
        if block_id == "lily_pad" {
            return TintCategory::LilyPad;
        }

        // Sugar cane has a slight green tint in some conditions
        if block_id == "sugar_cane" {
            return TintCategory::Grass;
        }

        TintCategory::None
    }

    /// Get redstone tint based on power level.
    fn get_redstone_tint(&self, block: &InputBlock) -> [f32; 4] {
        let power = block.properties
            .get("power")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            .min(15);
        self.colors.redstone[power]
    }

    /// Get stem tint based on growth stage.
    fn get_stem_tint(&self, block: &InputBlock) -> [f32; 4] {
        let block_id = block.block_id();

        // Attached stems are fully grown
        if block_id.starts_with("attached_") {
            return self.colors.stem[7];
        }

        let age = block.properties
            .get("age")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            .min(7);
        self.colors.stem[age]
    }
}

impl Default for TintProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Categories of block tinting.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TintCategory {
    /// Grass/greenery tinting (biome-dependent)
    Grass,
    /// Foliage tinting for leaves (biome-dependent)
    Foliage,
    /// Water tinting (biome-dependent)
    Water,
    /// Redstone dust tinting (power-level dependent)
    Redstone,
    /// Stem tinting (growth-stage dependent)
    Stem,
    /// Lily pad specific tinting
    LilyPad,
    /// No tinting
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tint_provider() {
        let provider = TintProvider::new();

        // No tint for regular blocks
        let stone = InputBlock::new("minecraft:stone");
        assert_eq!(provider.get_tint(&stone, -1), [1.0, 1.0, 1.0, 1.0]);

        // Grass tint for grass blocks
        let grass = InputBlock::new("minecraft:grass_block");
        let tint = provider.get_tint(&grass, 0);
        assert!(tint[0] < 1.0); // Should be greenish, not white
        assert!(tint[1] > tint[0]); // Green > Red
    }

    #[test]
    fn test_redstone_tint() {
        let provider = TintProvider::new();

        let redstone_0 = InputBlock::new("minecraft:redstone_wire")
            .with_property("power", "0");
        let redstone_15 = InputBlock::new("minecraft:redstone_wire")
            .with_property("power", "15");

        let tint_0 = provider.get_tint(&redstone_0, 0);
        let tint_15 = provider.get_tint(&redstone_15, 0);

        // Higher power = brighter red
        assert!(tint_15[0] > tint_0[0]);
    }

    #[test]
    fn test_stem_tint() {
        let provider = TintProvider::new();

        let stem_0 = InputBlock::new("minecraft:melon_stem")
            .with_property("age", "0");
        let stem_7 = InputBlock::new("minecraft:melon_stem")
            .with_property("age", "7");

        let tint_0 = provider.get_tint(&stem_0, 0);
        let tint_7 = provider.get_tint(&stem_7, 0);

        // Older stems are more yellow/orange (higher red)
        assert!(tint_7[0] > tint_0[0]);
    }

    #[test]
    fn test_biome_tints() {
        let plains = TintProvider::new();
        let swamp = TintProvider::for_biome("swamp");
        let jungle = TintProvider::for_biome("jungle");

        // Swamp has different grass color than plains
        assert_ne!(plains.colors().grass, swamp.colors().grass);

        // Jungle has vibrant green
        assert!(jungle.colors().foliage[1] > plains.colors().foliage[1]);
    }
}
