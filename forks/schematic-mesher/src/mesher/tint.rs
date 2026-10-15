//! Block tinting for grass, foliage, water, redstone, and other colored blocks.
//!
//! In Minecraft, certain blocks have their textures tinted with colors that can
//! depend on biome, block state, or other factors.

use serde::Deserialize;

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
    /// Dry foliage tint (leaf litter, 1.21.5+).
    pub dry_foliage: [f32; 4],
}

/// An opaque `0xRRGGBB` color as linear-blend RGBA factors.
const fn rgb(hex: u32) -> [f32; 4] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        1.0,
    ]
}

impl Default for TintColors {
    fn default() -> Self {
        Self {
            // Plains: the grass, foliage and dry-foliage colormaps sampled at
            // temperature 0.8, downfall 0.4, as Minecraft does for that biome.
            grass: rgb(0x91BD59),
            foliage: rgb(0x77AB2F),
            water: rgb(0x3F76E4),
            // Redstone dust colors from power 0 to 15
            redstone: Self::default_redstone_colors(),
            // Stem colors from stage 0 to 7
            stem: Self::default_stem_colors(),
            // A placed lily pad's fixed color.
            lily_pad: rgb(0x208030),
            // Cauldron water (same as regular water by default)
            cauldron_water: rgb(0x3F76E4),
            dry_foliage: rgb(0xA37546),
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
            "badlands"
            | "minecraft:badlands"
            | "wooded_badlands"
            | "minecraft:wooded_badlands"
            | "eroded_badlands"
            | "minecraft:eroded_badlands" => {
                colors.grass = [0.56, 0.50, 0.30, 1.0];
                colors.foliage = [0.62, 0.56, 0.35, 1.0];
            }
            "jungle"
            | "minecraft:jungle"
            | "bamboo_jungle"
            | "minecraft:bamboo_jungle"
            | "sparse_jungle"
            | "minecraft:sparse_jungle" => {
                colors.grass = [0.35, 0.75, 0.15, 1.0];
                colors.foliage = [0.30, 0.72, 0.20, 1.0];
            }
            "dark_forest" | "minecraft:dark_forest" => {
                colors.grass = [0.31, 0.55, 0.20, 1.0];
                colors.foliage = [0.31, 0.55, 0.20, 1.0];
            }
            "snowy_plains"
            | "minecraft:snowy_plains"
            | "snowy_taiga"
            | "minecraft:snowy_taiga"
            | "snowy_beach"
            | "minecraft:snowy_beach"
            | "snowy_slopes"
            | "minecraft:snowy_slopes" => {
                colors.grass = [0.50, 0.70, 0.50, 1.0];
                colors.foliage = [0.39, 0.61, 0.39, 1.0];
            }
            "desert" | "minecraft:desert" => {
                colors.grass = [0.75, 0.72, 0.45, 1.0];
                colors.foliage = [0.68, 0.68, 0.40, 1.0];
            }
            "ocean"
            | "minecraft:ocean"
            | "deep_ocean"
            | "minecraft:deep_ocean"
            | "cold_ocean"
            | "minecraft:cold_ocean"
            | "deep_cold_ocean"
            | "minecraft:deep_cold_ocean" => {
                colors.water = [0.24, 0.36, 0.75, 1.0];
            }
            "warm_ocean"
            | "minecraft:warm_ocean"
            | "lukewarm_ocean"
            | "minecraft:lukewarm_ocean"
            | "deep_lukewarm_ocean"
            | "minecraft:deep_lukewarm_ocean" => {
                colors.water = [0.26, 0.53, 0.80, 1.0];
            }
            "frozen_ocean"
            | "minecraft:frozen_ocean"
            | "deep_frozen_ocean"
            | "minecraft:deep_frozen_ocean" => {
                colors.water = [0.24, 0.30, 0.60, 1.0];
            }
            _ => {
                // Keep defaults for plains and other biomes
            }
        }

        colors
    }

    /// Redstone dust colors for power levels 0-15 (Minecraft's
    /// `RedStoneWireBlock.getColorForPower`).
    fn default_redstone_colors() -> [[f32; 4]; 16] {
        let mut colors = [[0.0; 4]; 16];
        for (power, color) in colors.iter_mut().enumerate() {
            let f = power as f32 / 15.0;
            let r = f * 0.6 + if f > 0.0 { 0.4 } else { 0.3 };
            let g = (f * f * 0.7 - 0.5).clamp(0.0, 1.0);
            let b = (f * f * 0.6 - 0.7).clamp(0.0, 1.0);
            *color = [r, g, b, 1.0];
        }
        colors
    }

    /// Stem colors for growth stages 0-7 (Minecraft: `age * 32, 255 - age * 8, age * 4`).
    fn default_stem_colors() -> [[f32; 4]; 8] {
        let mut colors = [[0.0; 4]; 8];
        for (age, color) in colors.iter_mut().enumerate() {
            let age = age as u32;
            *color = rgb((age * 32) << 16 | (255 - age * 8) << 8 | age * 4);
        }
        colors
    }
}

/// Provides tint colors for blocks based on their type and properties.
#[derive(Debug, Clone)]
pub struct TintProvider {
    /// Base tint colors.
    colors: TintColors,
    /// Whether `colors` are the plains defaults, which a pack's own colormap
    /// samples replace (see [`TintProvider::sample_colormaps`]).
    plains: bool,
}

impl TintProvider {
    /// Create a new tint provider with default colors.
    pub fn new() -> Self {
        Self {
            colors: TintColors::default(),
            plains: true,
        }
    }

    /// Create a tint provider with specific colors.
    pub fn with_colors(colors: TintColors) -> Self {
        Self {
            colors,
            plains: false,
        }
    }

    /// Create a tint provider for a specific biome.
    pub fn for_biome(biome: &str) -> Self {
        Self {
            colors: TintColors::for_biome(biome),
            plains: false,
        }
    }

    /// Take the plains grass, foliage and dry-foliage colors from a pack's
    /// own colormaps (`ResourcePack::colormap_samples`), so each game version
    /// tints with its own maps. Colors chosen for a biome or given explicitly
    /// are kept. (LumilioCL local change; see `forks/README.md`.)
    pub fn sample_colormaps(&mut self, samples: &std::collections::HashMap<String, [u8; 3]>) {
        if !self.plains {
            return;
        }
        for (name, slot) in [
            ("grass", &mut self.colors.grass),
            ("foliage", &mut self.colors.foliage),
            ("dry_foliage", &mut self.colors.dry_foliage),
        ] {
            if let Some(&[r, g, b]) = samples.get(name) {
                *slot = rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b));
            }
        }
    }

    /// Get the tint color for a block face.
    /// Returns white [1,1,1,1] if no tinting should be applied.
    pub fn get_tint(&self, block: &InputBlock, tint_index: i32) -> [f32; 4] {
        if tint_index < 0 {
            return [1.0, 1.0, 1.0, 1.0];
        }
        match table().kind(&block.name, tint_index) {
            Some(TintKind::Grass) => self.colors.grass,
            Some(TintKind::Foliage) => self.colors.foliage,
            Some(TintKind::DryFoliage) => self.colors.dry_foliage,
            Some(TintKind::Water) => self.colors.water,
            Some(TintKind::Redstone) => self.get_redstone_tint(block),
            Some(TintKind::Stem) => self.get_stem_tint(block),
            Some(TintKind::LilyPad) => self.colors.lily_pad,
            Some(TintKind::Fixed(color)) => color,
            None => [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Get the tint colors reference for direct access.
    pub fn colors(&self) -> &TintColors {
        &self.colors
    }

    /// Get redstone tint based on power level.
    fn get_redstone_tint(&self, block: &InputBlock) -> [f32; 4] {
        let power = block
            .properties
            .get("power")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            .min(15);
        self.colors.redstone[power]
    }

    /// Get stem tint based on growth stage.
    fn get_stem_tint(&self, block: &InputBlock) -> [f32; 4] {
        let age = block
            .properties
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

// The tint table: data, not code (LumilioCL local change; see
// `forks/README.md`). Adding a tinted block is a line in `data/tints.json`.

static TINTS_JSON: &str = include_str!("../../data/tints.json");

/// How a block is tinted, as `data/tints.json` names it.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TintKind {
    Grass,
    Foliage,
    DryFoliage,
    Water,
    /// By the block's `power` (Minecraft's `RedStoneWireBlock.getColorForPower`).
    Redstone,
    /// By the block's `age`.
    Stem,
    LilyPad,
    #[serde(deserialize_with = "fixed_color")]
    Fixed([f32; 4]),
}

fn fixed_color<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<[f32; 4], D::Error> {
    let text = String::deserialize(deserializer)?;
    text.strip_prefix('#')
        .filter(|hex| hex.len() == 6)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .map(rgb)
        .ok_or_else(|| serde::de::Error::custom(format!("`{text}` is not a #RRGGBB color")))
}

#[derive(Debug, Deserialize)]
struct TintRule {
    blocks: Vec<String>,
    #[serde(default)]
    legacy: Vec<String>,
    #[serde(default)]
    tint_index: Option<i32>,
    tint: TintKind,
}

#[derive(Debug, Deserialize)]
struct TintTable {
    rules: Vec<TintRule>,
}

impl TintTable {
    /// The first rule naming `block` (namespaced or not) for this tint index.
    fn kind(&self, block: &str, tint_index: i32) -> Option<TintKind> {
        let name = if block.contains(':') {
            std::borrow::Cow::Borrowed(block)
        } else {
            std::borrow::Cow::Owned(format!("minecraft:{block}"))
        };
        self.rules
            .iter()
            .filter(|rule| rule.tint_index.is_none_or(|index| index == tint_index))
            .find(|rule| {
                rule.blocks
                    .iter()
                    .chain(&rule.legacy)
                    .any(|pattern| pattern_matches(pattern, &name))
            })
            .map(|rule| rule.tint)
    }
}

/// `pattern` is an id where one `*` matches any run of characters.
fn pattern_matches(pattern: &str, id: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => {
            id.len() >= head.len() + tail.len() && id.starts_with(head) && id.ends_with(tail)
        }
        None => pattern == id,
    }
}

fn table() -> &'static TintTable {
    static TABLE: std::sync::OnceLock<TintTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        // Checked by the unit test `the_table_reads`; an unreadable table
        // tints nothing rather than failing every mesh.
        serde_json::from_str(TINTS_JSON).unwrap_or(TintTable { rules: Vec::new() })
    })
}

/// Every block pattern the tint table names (not its `legacy` names), for the
/// table lint that checks them against a baked pack.
pub fn tint_table_blocks() -> Vec<&'static str> {
    table()
        .rules
        .iter()
        .flat_map(|rule| rule.blocks.iter().map(String::as_str))
        .collect()
}

/// Whether a table `pattern` names the block `id`.
pub fn tint_pattern_matches(pattern: &str, id: &str) -> bool {
    pattern_matches(pattern, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_reads() {
        let table: TintTable = serde_json::from_str(TINTS_JSON).unwrap();
        assert!(!table.rules.is_empty());
        assert!(pattern_matches("minecraft:*_stem", "minecraft:melon_stem"));
        assert!(!pattern_matches("minecraft:*_stem", "minecraft:stem"));
    }

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

        let redstone_0 = InputBlock::new("minecraft:redstone_wire").with_property("power", "0");
        let redstone_15 = InputBlock::new("minecraft:redstone_wire").with_property("power", "15");

        let tint_0 = provider.get_tint(&redstone_0, 0);
        let tint_15 = provider.get_tint(&redstone_15, 0);

        // Higher power = brighter red
        assert!(tint_15[0] > tint_0[0]);
    }

    #[test]
    fn test_stem_tint() {
        let provider = TintProvider::new();

        let stem_0 = InputBlock::new("minecraft:melon_stem").with_property("age", "0");
        let stem_7 = InputBlock::new("minecraft:melon_stem").with_property("age", "7");

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
