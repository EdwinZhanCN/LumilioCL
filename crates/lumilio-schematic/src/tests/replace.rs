use nucleation::block_position::BlockPosition;

use super::{cells, room, state};
use crate::replace::{PropertyMatch, Replacement, Scope, StateMatcher, apply, preview_count};
use crate::stats::stats;
use crate::{Region, UniversalSchematic};

const NORTH: &str =
    "minecraft:oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]";
const SOUTH: &str =
    "minecraft:oak_stairs[facing=south,half=bottom,shape=straight,waterlogged=false]";

fn run(schematic: &mut UniversalSchematic, plan: &Replacement) -> u64 {
    let preview = preview_count(schematic, plan);
    let changed = apply(schematic, plan);
    assert_eq!(
        changed, preview,
        "apply must change what the preview showed"
    );
    changed
}

fn count(schematic: &UniversalSchematic, text: &str) -> usize {
    let mut wanted = state(text);
    wanted.properties.sort();
    let wanted = wanted.to_string();
    cells(schematic)
        .into_iter()
        .filter(|(_, block)| *block == wanted)
        .count()
}

#[test]
fn matches_only_the_named_property() {
    let mut schematic = room();
    let plan = Replacement {
        source: StateMatcher::any_state("minecraft:oak_stairs").with_property("facing", "north"),
        target: state(SOUTH),
        scope: Scope::All,
    };
    assert_eq!(run(&mut schematic, &plan), 2);
    assert_eq!(count(&schematic, NORTH), 0);
    assert_eq!(count(&schematic, SOUTH), 2);
    // The east-facing stair has another facing and stays.
    assert_eq!(stats(&schematic).blocks[1].states.len(), 2);
    assert_eq!(stats(&schematic).block_count, 21);
}

#[test]
fn wildcard_matches_every_state() {
    let mut schematic = room();
    let mut source = StateMatcher::any_state("minecraft:oak_stairs");
    source
        .properties
        .push(("facing".to_owned(), PropertyMatch::Any));
    let plan = Replacement {
        source,
        target: state("minecraft:spruce_planks"),
        scope: Scope::All,
    };
    assert_eq!(run(&mut schematic, &plan), 3);
    assert_eq!(count(&schematic, "minecraft:spruce_planks"), 3);
    assert!(
        !stats(&schematic)
            .blocks
            .iter()
            .any(|b| b.id == "minecraft:oak_stairs")
    );
}

#[test]
fn same_id_keeps_the_block_entity_and_another_id_drops_it() {
    let at = BlockPosition { x: 0, y: 1, z: 0 };
    let mut schematic = room();
    let turn = Replacement {
        source: StateMatcher::any_state("minecraft:chest").with_property("facing", "north"),
        target: state("minecraft:chest[facing=west,type=single]"),
        scope: Scope::All,
    };
    assert_eq!(run(&mut schematic, &turn), 1);
    let kept = schematic.get_block_entity(at).expect("block entity kept");
    assert_eq!(kept.id, "minecraft:chest");
    assert!(kept.nbt.get("CustomName").is_some());

    let swap = Replacement {
        source: StateMatcher::any_state("minecraft:chest"),
        target: state("minecraft:stone"),
        scope: Scope::All,
    };
    assert_eq!(run(&mut schematic, &swap), 2);
    assert!(schematic.get_block_entity(at).is_none());
}

#[test]
fn region_scope_leaves_other_regions_alone() {
    let stone = state("minecraft:stone");
    let mut schematic = UniversalSchematic::new("scoped".to_owned());
    for (name, x) in [("a", 0), ("b", 5)] {
        let mut region = Region::new(name.to_owned(), (x, 0, 0), (1, 1, 1));
        region.set_block(x, 0, 0, &stone);
        schematic.add_region(region);
    }
    let plan = Replacement {
        source: StateMatcher::any_state("minecraft:stone"),
        target: state("minecraft:dirt"),
        scope: Scope::Region("b".to_owned()),
    };
    assert_eq!(run(&mut schematic, &plan), 1);
    assert_eq!(schematic.get_block(0, 0, 0), Some(&stone));
    assert_eq!(schematic.get_block(5, 0, 0), Some(&state("minecraft:dirt")));
}

#[test]
fn nothing_matches_when_the_target_equals_the_source() {
    let mut schematic = room();
    let plan = Replacement {
        source: StateMatcher::any_state("minecraft:stone"),
        target: state("minecraft:stone"),
        scope: Scope::All,
    };
    assert_eq!(run(&mut schematic, &plan), 0);
}

/// T25: a box selected in the picture limits a replacement to its cells,
/// both corners included, across regions; the rest of the floor stays.
#[test]
fn a_box_of_cells_limits_the_replacement() {
    let mut schematic = room();
    let plan = Replacement {
        source: StateMatcher::any_state("minecraft:stone"),
        target: state("minecraft:dirt"),
        scope: Scope::Cells {
            min: [1, 0, 1],
            max: [2, 0, 3],
        },
    };
    assert_eq!(run(&mut schematic, &plan), 6);
    assert_eq!(count(&schematic, "minecraft:dirt"), 6);
    assert_eq!(count(&schematic, "minecraft:stone"), 10);
    assert_eq!(schematic.get_block(1, 0, 1), Some(&state("minecraft:dirt")));
    assert_eq!(schematic.get_block(2, 0, 3), Some(&state("minecraft:dirt")));
    assert_eq!(
        schematic.get_block(0, 0, 1),
        Some(&state("minecraft:stone"))
    );
    assert_eq!(
        schematic.get_block(1, 0, 0),
        Some(&state("minecraft:stone"))
    );
}

fn stairs_catalog() -> crate::replace::BlockCatalog {
    let mut catalog = crate::replace::BlockCatalog::default();
    for facing in ["north", "south", "east", "west"] {
        for half in ["top", "bottom"] {
            catalog.add_variant(
                "minecraft:spruce_stairs",
                &format!("facing={facing},half={half},shape=straight"),
            );
        }
    }
    catalog.add_block("minecraft:dirt");
    catalog.add_block("minecraft:stone");
    catalog
}

/// T24: a row's target keeps each property the target block has with the
/// same name, from the block it replaces; a property the target does not
/// have is dropped, and one the row fixes wins.
#[test]
fn a_target_keeps_the_properties_it_shares_with_the_source() {
    use crate::replace::{Mapping, MappingRow, apply_mapping, mapping_count};
    let mut schematic = room();
    let catalog = stairs_catalog();
    let row = |fixed: Vec<(String, String)>| MappingRow {
        source: StateMatcher::any_state("minecraft:oak_stairs"),
        target: "minecraft:spruce_stairs".into(),
        fixed,
    };
    let mapping = Mapping {
        rows: vec![row(Vec::new())],
        scope: Scope::All,
    };
    let (total, per_row) = mapping_count(&schematic, &mapping, Some(&catalog));
    assert_eq!((total, per_row), (3, vec![3]));
    assert_eq!(apply_mapping(&mut schematic, &mapping, Some(&catalog)), 3);
    // The two north stairs and the east one keep their facing and half.
    assert_eq!(
        count(
            &schematic,
            "minecraft:spruce_stairs[facing=north,half=bottom,shape=straight]"
        ),
        2
    );
    assert_eq!(
        count(
            &schematic,
            "minecraft:spruce_stairs[facing=east,half=bottom,shape=straight]"
        ),
        1
    );

    let mut schematic = room();
    let fixed = Mapping {
        rows: vec![row(vec![("half".into(), "top".into())])],
        scope: Scope::All,
    };
    apply_mapping(&mut schematic, &fixed, Some(&catalog));
    assert_eq!(
        count(
            &schematic,
            "minecraft:spruce_stairs[facing=north,half=top,shape=straight]"
        ),
        2,
        "the row's value wins"
    );
}

/// T24: rows apply at once to what each cell held, so two rows can swap
/// blocks; the count is what the write changes, row by row.
#[test]
fn rows_apply_at_once_and_count_what_they_change() {
    use crate::replace::{Mapping, MappingRow, apply_mapping, mapping_count};
    let mut schematic = room();
    let catalog = stairs_catalog();
    schematic.set_block(3, 1, 0, &state("minecraft:dirt"));
    let mapping = Mapping {
        rows: vec![
            MappingRow {
                source: StateMatcher::any_state("minecraft:stone"),
                target: "minecraft:dirt".into(),
                fixed: Vec::new(),
            },
            MappingRow {
                source: StateMatcher::any_state("minecraft:dirt"),
                target: "minecraft:stone".into(),
                fixed: Vec::new(),
            },
        ],
        scope: Scope::All,
    };
    let (total, per_row) = mapping_count(&schematic, &mapping, Some(&catalog));
    assert_eq!(per_row, vec![16, 1]);
    assert_eq!(
        apply_mapping(&mut schematic, &mapping, Some(&catalog)),
        total
    );
    assert_eq!(
        count(&schematic, "minecraft:dirt"),
        16,
        "the floor became dirt"
    );
    assert_eq!(
        count(&schematic, "minecraft:stone"),
        1,
        "the dirt became stone, not dirt again"
    );
}
