//! Block and biome colours for the save map, from the tables committed under
//! `data/block-colors/` (made by `cargo xtask block-colors`, plan W13).
//!
//! A world uses the newest table whose game is not newer than the world;
//! a world older than every table uses the oldest. A block missing from the
//! table (a mod's block, or one newer than the table) is drawn in
//! [`UNKNOWN`] and reported by name, so the map can say how many it met.
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The colour of a block no table knows: a muted violet that no vanilla
/// surface has, so it reads as "not drawn from data" rather than as stone.
pub(crate) const UNKNOWN: [u8; 3] = [150, 128, 160];

pub(crate) const SEE_THROUGH: u8 = 1;
const TINT_GRASS: u8 = 2;
const TINT_FOLIAGE: u8 = 4;
const TINT_WATER: u8 = 8;

/// Plains: the colours a biome the table does not know is tinted with.
const DEFAULT_BIOME: [u32; 3] = [0x91bd59, 0x77ab2f, 0x3f76e4];

const SOURCES: [&str; 2] = [
    include_str!("../../data/block-colors/1.21.4.json"),
    include_str!("../../data/block-colors/26.3.json"),
];

/// A block's table entry: its colour before any biome tint, and its flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Block {
    pub rgb: [u8; 3],
    pub flags: u8,
}

pub(crate) struct Table {
    pub version: String,
    pub data_version: i32,
    blocks: HashMap<String, Block>,
    biomes: HashMap<String, [u32; 3]>,
}

impl Table {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let blocks = value
            .get("blocks")?
            .as_object()?
            .iter()
            .filter_map(|(name, entry)| {
                let entry: Vec<u8> = entry
                    .as_array()?
                    .iter()
                    .map(|part| part.as_u64().and_then(|n| u8::try_from(n).ok()))
                    .collect::<Option<_>>()?;
                let [r, g, b, flags] = entry[..] else {
                    return None;
                };
                Some((
                    name.clone(),
                    Block {
                        rgb: [r, g, b],
                        flags,
                    },
                ))
            })
            .collect();
        let biomes = value
            .get("biomes")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, entry)| {
                let entry: Vec<u32> = entry
                    .as_array()?
                    .iter()
                    .map(|part| part.as_u64().and_then(|n| u32::try_from(n).ok()))
                    .collect::<Option<_>>()?;
                let [grass, foliage, water] = entry[..] else {
                    return None;
                };
                Some((name.clone(), [grass, foliage, water]))
            })
            .collect();
        Some(Self {
            version: value.get("version")?.as_str()?.to_owned(),
            data_version: i32::try_from(value.get("data_version")?.as_i64()?).ok()?,
            blocks,
            biomes,
        })
    }

    /// The entry for a block, under its current name if it was renamed since.
    pub(crate) fn block(&self, name: &str) -> Option<Block> {
        self.blocks
            .get(name)
            .or_else(|| renamed_block(name).and_then(|name| self.blocks.get(name)))
            .copied()
    }

    /// The colour of a block as it looks in `biome`.
    pub(crate) fn color(&self, block: Block, biome: Option<&str>) -> [u8; 3] {
        let slot = if block.flags & TINT_GRASS != 0 {
            0
        } else if block.flags & TINT_FOLIAGE != 0 {
            1
        } else if block.flags & TINT_WATER != 0 {
            2
        } else {
            return block.rgb;
        };
        let tint = self.biome(biome)[slot];
        let tint = [(tint >> 16) as u8, (tint >> 8) as u8, tint as u8];
        [0, 1, 2].map(|at| (u16::from(block.rgb[at]) * u16::from(tint[at]) / 255) as u8)
    }

    fn biome(&self, name: Option<&str>) -> [u32; 3] {
        let Some(name) = name else {
            return DEFAULT_BIOME;
        };
        self.biomes
            .get(name)
            .or_else(|| renamed_biome(name).and_then(|name| self.biomes.get(name)))
            .copied()
            .unwrap_or(DEFAULT_BIOME)
    }
}

/// Blocks renamed since the oldest table's game, by their old names.
fn renamed_block(name: &str) -> Option<&'static str> {
    Some(match name {
        // 1.20.3
        "minecraft:grass" => "minecraft:short_grass",
        // 1.17
        "minecraft:grass_path" => "minecraft:dirt_path",
        _ => return None,
    })
}

/// Biomes as 1.18 renamed or merged them, for worlds saved before it. The
/// "hills" and "modified" variants that 1.18 dropped take their base biome's
/// colours.
fn renamed_biome(name: &str) -> Option<&'static str> {
    let name = name.strip_prefix("minecraft:")?;
    Some(match name {
        "mountains" | "mountain_edge" => "minecraft:windswept_hills",
        "wooded_mountains" => "minecraft:windswept_forest",
        "gravelly_mountains" | "modified_gravelly_mountains" => {
            "minecraft:windswept_gravelly_hills"
        }
        "shattered_savanna" | "shattered_savanna_plateau" => "minecraft:windswept_savanna",
        "snowy_tundra" | "snowy_mountains" => "minecraft:snowy_plains",
        "giant_tree_taiga" | "giant_tree_taiga_hills" => "minecraft:old_growth_pine_taiga",
        "giant_spruce_taiga" | "giant_spruce_taiga_hills" => "minecraft:old_growth_spruce_taiga",
        "tall_birch_forest" | "tall_birch_hills" => "minecraft:old_growth_birch_forest",
        "jungle_edge" | "modified_jungle_edge" => "minecraft:sparse_jungle",
        "wooded_badlands_plateau" | "modified_wooded_badlands_plateau" => {
            "minecraft:wooded_badlands"
        }
        "badlands_plateau" | "modified_badlands_plateau" => "minecraft:badlands",
        "stone_shore" => "minecraft:stony_shore",
        "mushroom_field_shore" => "minecraft:mushroom_fields",
        "desert_hills" | "desert_lakes" => "minecraft:desert",
        "wooded_hills" => "minecraft:forest",
        "taiga_hills" | "taiga_mountains" => "minecraft:taiga",
        "snowy_taiga_hills" | "snowy_taiga_mountains" => "minecraft:snowy_taiga",
        "jungle_hills" | "modified_jungle" => "minecraft:jungle",
        "bamboo_jungle_hills" => "minecraft:bamboo_jungle",
        "birch_forest_hills" => "minecraft:birch_forest",
        "dark_forest_hills" => "minecraft:dark_forest",
        "swamp_hills" => "minecraft:swamp",
        _ => return None,
    })
}

/// Every committed table, oldest game first.
pub(crate) fn tables() -> &'static [Table] {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut tables: Vec<Table> = SOURCES
            .iter()
            .filter_map(|text| Table::parse(text))
            .collect();
        tables.sort_by_key(|table| table.data_version);
        tables
    })
}

/// The table for a world: the newest one not newer than the world, by data
/// version when the world records one and by version name otherwise; the
/// oldest for a world older than all of them; the newest when nothing about
/// the world's version is known.
pub(crate) fn pick<'a>(
    tables: &'a [Table],
    data_version: Option<i32>,
    version: Option<&str>,
) -> &'a Table {
    let not_newer = |table: &&Table| match (data_version, version.and_then(release)) {
        (Some(world), _) => table.data_version <= world,
        (None, Some(world)) => release(&table.version).is_some_and(|table| table <= world),
        (None, None) => true,
    };
    tables
        .iter()
        .rev()
        .find(not_newer)
        .or_else(|| tables.first())
        .expect("at least one block colour table is committed")
}

/// A release name as numbers (`1.21.4` → `[1, 21, 4]`, `26.3` → `[26, 3]`),
/// which order the way the releases came out.
fn release(name: &str) -> Option<Vec<u32>> {
    name.split('.').map(|part| part.parse().ok()).collect()
}
