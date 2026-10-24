//! Replacing one block state with another (schematic-studio T13).
//!
//! The plan is previewed with [`preview_count`] and carried out with [`apply`],
//! which edits the schematic in memory; the host writes the result.

pub use lumilio_block_assets::BlockCatalog;

use crate::{BlockState, Region, UniversalSchematic};

/// How one property of the source block must look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyMatch {
    /// Any value, or the property missing.
    Any,
    /// Exactly this value.
    Exact(String),
}

/// Which block states to replace: one block id, with each listed property
/// matched exactly or by wildcard. Properties not listed match any value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMatcher {
    pub id: String,
    pub properties: Vec<(String, PropertyMatch)>,
}

impl StateMatcher {
    /// Every state of a block id.
    pub fn any_state(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            properties: Vec::new(),
        }
    }

    /// Requires `name` to be `value`.
    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties
            .push((name.into(), PropertyMatch::Exact(value.into())));
        self
    }

    pub fn matches(&self, state: &BlockState) -> bool {
        state.name == self.id.as_str()
            && self.properties.iter().all(|(name, rule)| match rule {
                PropertyMatch::Any => true,
                PropertyMatch::Exact(value) => state
                    .get_property(name)
                    .is_some_and(|actual| actual.as_str() == value),
            })
    }
}

/// Where the replacement applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    All,
    /// One region, by name.
    Region(String),
    /// The cells from `min` to `max`, both included, in schematic
    /// coordinates, in every region: a box selected in the picture (T25).
    Cells {
        min: [i32; 3],
        max: [i32; 3],
    },
}

impl Scope {
    fn holds(&self, (x, y, z): (i32, i32, i32)) -> bool {
        match self {
            Self::All | Self::Region(_) => true,
            Self::Cells { min, max } => [x, y, z]
                .iter()
                .zip(min.iter().zip(max))
                .all(|(at, (low, high))| (*low..=*high).contains(at)),
        }
    }
}

/// A block-state replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub source: StateMatcher,
    pub target: BlockState,
    pub scope: Scope,
}

/// How many stored cells [`apply`] would change. Where regions overlap, each
/// region's cell is changed and counted.
pub fn preview_count(schematic: &UniversalSchematic, plan: &Replacement) -> u64 {
    regions_in_scope(schematic, &plan.scope)
        .map(|region| matching_cells(region, plan).len() as u64)
        .sum()
}

/// Replaces every matching cell in scope and returns how many changed.
///
/// A block entity stays when the target has the same block id (a chest turned
/// to face another way keeps its items); otherwise it is removed with its block.
pub fn apply(schematic: &mut UniversalSchematic, plan: &Replacement) -> u64 {
    let names: Vec<String> = regions_in_scope(schematic, &plan.scope)
        .map(|region| region.name.clone())
        .collect();
    let keeps_block_entity = plan.target.name == plan.source.id.as_str();
    let mut changed = 0;
    for name in names {
        let Some(region) = schematic.get_region_mut(&name) else {
            continue;
        };
        for (x, y, z) in matching_cells(region, plan) {
            region.set_block(x, y, z, &plan.target);
            if !keeps_block_entity {
                region.remove_block_entity((x, y, z));
            }
            changed += 1;
        }
    }
    changed
}

fn regions_in_scope<'a>(
    schematic: &'a UniversalSchematic,
    scope: &'a Scope,
) -> impl Iterator<Item = &'a Region> + 'a {
    crate::regions_in_precedence(schematic)
        .into_iter()
        .filter(move |region| match scope {
            Scope::All | Scope::Cells { .. } => true,
            Scope::Region(name) => &region.name == name,
        })
}

fn matching_cells(region: &Region, plan: &Replacement) -> Vec<(i32, i32, i32)> {
    let matches: Vec<bool> = region
        .get_palette()
        .iter()
        .map(|state| plan.source.matches(state) && *state != plan.target)
        .collect();
    region
        .blocks
        .iter()
        .enumerate()
        .filter(|&(_, &palette_index)| matches.get(palette_index).copied().unwrap_or(false))
        .map(|(index, _)| region.index_to_coords(index))
        .filter(|&cell| plan.scope.holds(cell))
        .collect()
}

/// One row of a mapping table (T24): blocks `source` matches become
/// `target`, with the properties `fixed` sets and, carried over from each
/// replaced block, every property the target block has with the same name
/// and a value it allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingRow {
    pub source: StateMatcher,
    /// The target block id (`minecraft:spruce_stairs`).
    pub target: String,
    pub fixed: Vec<(String, String)>,
}

/// A replacement of several kinds at once (T24): each cell in scope takes
/// the first row whose source matches what the cell held before any row
/// was applied, so rows never feed each other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub rows: Vec<MappingRow>,
    pub scope: Scope,
}

impl MappingRow {
    /// What `state` becomes: the target id, then the properties carried
    /// over that `catalog` (the target version's) says the target has and
    /// allows, then `fixed` over them. Without a catalog nothing is
    /// carried: a property the target lacks would make a state the game
    /// does not know.
    pub fn target_for(&self, state: &BlockState, catalog: Option<&BlockCatalog>) -> BlockState {
        let mut properties: Vec<(String, String)> = match catalog {
            Some(catalog) => state
                .properties
                .iter()
                .filter(|(name, value)| {
                    catalog
                        .property(&self.target, name)
                        .is_some_and(|values| values.allows(value))
                })
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            None => Vec::new(),
        };
        for (name, value) in &self.fixed {
            match properties.iter_mut().find(|(have, _)| have == name) {
                Some(slot) => slot.1.clone_from(value),
                None => properties.push((name.clone(), value.clone())),
            }
        }
        properties.sort();
        let mut target = BlockState::new(self.target.clone());
        for (name, value) in properties {
            target = target.with_property(name, value);
        }
        target
    }
}

/// How many stored cells [`apply_mapping`] would change, and how many each
/// row changes, in row order.
pub fn mapping_count(
    schematic: &UniversalSchematic,
    mapping: &Mapping,
    catalog: Option<&BlockCatalog>,
) -> (u64, Vec<u64>) {
    let mut per_row = vec![0; mapping.rows.len()];
    for region in regions_in_scope(schematic, &mapping.scope) {
        for (_, row, _) in mapped_cells(region, mapping, catalog) {
            per_row[row] += 1;
        }
    }
    (per_row.iter().sum(), per_row)
}

/// Applies every row at once and returns how many cells changed. A block
/// entity stays when its block keeps its id, as [`apply`] does.
pub fn apply_mapping(
    schematic: &mut UniversalSchematic,
    mapping: &Mapping,
    catalog: Option<&BlockCatalog>,
) -> u64 {
    let names: Vec<String> = regions_in_scope(schematic, &mapping.scope)
        .map(|region| region.name.clone())
        .collect();
    let mut changed = 0;
    for name in names {
        let Some(region) = schematic.get_region_mut(&name) else {
            continue;
        };
        let cells = mapped_cells(region, mapping, catalog);
        for ((x, y, z), _, (target, keeps)) in cells {
            region.set_block(x, y, z, &target);
            if !keeps {
                region.remove_block_entity((x, y, z));
            }
            changed += 1;
        }
    }
    changed
}

/// The cells of `region` in scope that some row changes: where, which row,
/// and the new state with whether the block entity stays.
#[allow(clippy::type_complexity)]
fn mapped_cells(
    region: &Region,
    mapping: &Mapping,
    catalog: Option<&BlockCatalog>,
) -> Vec<((i32, i32, i32), usize, (BlockState, bool))> {
    // Each palette entry once: the row that takes it and what it becomes.
    let targets: Vec<Option<(usize, BlockState, bool)>> = region
        .get_palette()
        .iter()
        .map(|state| {
            let (row, rule) = mapping
                .rows
                .iter()
                .enumerate()
                .find(|(_, rule)| rule.source.matches(state))?;
            let target = rule.target_for(state, catalog);
            (target != *state).then(|| {
                let keeps = target.name == state.name;
                (row, target, keeps)
            })
        })
        .collect();
    region
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, &palette_index)| {
            let (row, target, keeps) = targets.get(palette_index)?.as_ref()?;
            let cell = region.index_to_coords(index);
            mapping
                .scope
                .holds(cell)
                .then(|| (cell, *row, (target.clone(), *keeps)))
        })
        .collect()
}
