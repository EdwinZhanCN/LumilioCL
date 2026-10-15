//! What a schematic is made of: blocks by id and state, entities and block
//! entities by type, per region and in total (schematic-studio T11).

use crate::{UniversalSchematic, is_air, regions_in_precedence, regions_overlap};
use std::collections::{BTreeMap, HashSet};

/// How many cells hold one block state of a block id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCount {
    /// The state's properties, sorted by name. Empty for a block without any.
    pub properties: Vec<(String, String)>,
    pub count: u64,
}

/// How many cells hold a block id, expandable to its states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockCount {
    pub id: String,
    pub count: u64,
    /// Most frequent first, then by properties.
    pub states: Vec<StateCount>,
}

/// How many entities or block entities have one type id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCount {
    pub id: String,
    pub count: u64,
}

/// The contents of one region. A cell shared with an earlier region in
/// precedence order is counted there, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionStats {
    pub name: String,
    /// The region's lowest corner.
    pub origin: (i32, i32, i32),
    pub size: (i32, i32, i32),
    pub volume: u64,
    /// Non-air blocks.
    pub block_count: u64,
    pub blocks: Vec<BlockCount>,
    pub entities: Vec<TypeCount>,
    pub block_entities: Vec<TypeCount>,
}

/// The contents of a whole schematic. Each cell is counted once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchematicStats {
    pub regions: Vec<RegionStats>,
    /// The box that encloses every region.
    pub size: (i32, i32, i32),
    /// The enclosing box's volume.
    pub volume: u64,
    pub block_count: u64,
    pub blocks: Vec<BlockCount>,
    pub entities: Vec<TypeCount>,
    pub block_entities: Vec<TypeCount>,
}

/// Block counts keyed by id, then by sorted property list.
type Tally = BTreeMap<String, BTreeMap<Vec<(String, String)>, u64>>;

/// Counts a schematic's contents.
pub fn stats(schematic: &UniversalSchematic) -> SchematicStats {
    let regions = regions_in_precedence(schematic);
    let overlap = regions_overlap(&regions);
    let mut claimed_cells = HashSet::new();
    let mut claimed_block_entities = HashSet::new();

    let mut total_blocks = Tally::new();
    let mut total_entities = BTreeMap::new();
    let mut total_block_entities = BTreeMap::new();
    let mut region_stats = Vec::with_capacity(regions.len());
    let mut bounds: Option<nucleation::BoundingBox> = None;

    for region in &regions {
        let palette = region.get_palette();
        let mut blocks = Tally::new();
        for (index, &palette_index) in region.blocks.iter().enumerate() {
            let Some(state) = palette.get(palette_index) else {
                continue;
            };
            if is_air(&state.name) {
                continue;
            }
            if overlap && !claimed_cells.insert(region.index_to_coords(index)) {
                continue;
            }
            *blocks
                .entry(state.name.to_string())
                .or_default()
                .entry(sorted_properties(state))
                .or_default() += 1;
        }

        let mut entities = BTreeMap::new();
        for entity in &region.entities {
            *entities.entry(entity.id.clone()).or_default() += 1;
        }
        let mut block_entities = BTreeMap::new();
        for (position, block_entity) in region.block_entities.iter() {
            if overlap && !claimed_block_entities.insert(position) {
                continue;
            }
            *block_entities.entry(block_entity.id.clone()).or_default() += 1;
        }

        merge_tally(&mut total_blocks, &blocks);
        merge_counts(&mut total_entities, &entities);
        merge_counts(&mut total_block_entities, &block_entities);

        let bbox = region.get_bounding_box();
        let size = bbox.get_dimensions();
        bounds = Some(match bounds {
            Some(all) => all.union(&bbox),
            None => bbox.clone(),
        });
        let blocks = block_counts(blocks);
        region_stats.push(RegionStats {
            name: region.name.clone(),
            origin: bbox.min,
            size,
            volume: volume(size),
            block_count: blocks.iter().map(|b| b.count).sum(),
            blocks,
            entities: type_counts(entities),
            block_entities: type_counts(block_entities),
        });
    }

    let size = bounds.map_or((0, 0, 0), |b| b.get_dimensions());
    let blocks = block_counts(total_blocks);
    SchematicStats {
        regions: region_stats,
        size,
        volume: volume(size),
        block_count: blocks.iter().map(|b| b.count).sum(),
        blocks,
        entities: type_counts(total_entities),
        block_entities: type_counts(total_block_entities),
    }
}

fn sorted_properties(state: &nucleation::BlockState) -> Vec<(String, String)> {
    let mut properties: Vec<(String, String)> = state
        .properties
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    properties.sort();
    properties
}

fn merge_tally(total: &mut Tally, part: &Tally) {
    for (id, states) in part {
        let entry = total.entry(id.clone()).or_default();
        for (properties, count) in states {
            *entry.entry(properties.clone()).or_default() += count;
        }
    }
}

fn merge_counts(total: &mut BTreeMap<String, u64>, part: &BTreeMap<String, u64>) {
    for (id, count) in part {
        *total.entry(id.clone()).or_default() += count;
    }
}

/// Most frequent first; ties by id so the order is stable.
fn block_counts(tally: Tally) -> Vec<BlockCount> {
    let mut blocks: Vec<BlockCount> = tally
        .into_iter()
        .map(|(id, states)| {
            let mut states: Vec<StateCount> = states
                .into_iter()
                .map(|(properties, count)| StateCount { properties, count })
                .collect();
            states.sort_by(|a, b| {
                b.count
                    .cmp(&a.count)
                    .then_with(|| a.properties.cmp(&b.properties))
            });
            BlockCount {
                id,
                count: states.iter().map(|s| s.count).sum(),
                states,
            }
        })
        .collect();
    blocks.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
    blocks
}

fn type_counts(counts: BTreeMap<String, u64>) -> Vec<TypeCount> {
    let mut counts: Vec<TypeCount> = counts
        .into_iter()
        .map(|(id, count)| TypeCount { id, count })
        .collect();
    counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
    counts
}

fn volume(size: (i32, i32, i32)) -> u64 {
    [size.0, size.1, size.2]
        .iter()
        .map(|d| u64::from(d.unsigned_abs()))
        .product()
}
