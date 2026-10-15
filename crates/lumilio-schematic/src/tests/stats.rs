use super::{room, state, through_file};
use crate::stats::{StateCount, TypeCount, stats};
use crate::{Region, UniversalSchematic};

fn props(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn counts_blocks_by_id_and_groups_states() {
    for schematic in [room(), through_file(&room())] {
        let stats = stats(&schematic);
        assert_eq!(stats.block_count, 21);
        assert_eq!(stats.size, (4, 2, 4));
        assert_eq!(stats.volume, 32);

        let ids: Vec<(&str, u64)> = stats
            .blocks
            .iter()
            .map(|b| (b.id.as_str(), b.count))
            .collect();
        assert_eq!(
            ids,
            [
                ("minecraft:stone", 16),
                ("minecraft:oak_stairs", 3),
                ("minecraft:chest", 2)
            ]
        );

        let stairs = &stats.blocks[1];
        let shape = [
            ("half", "bottom"),
            ("shape", "straight"),
            ("waterlogged", "false"),
        ];
        let with = |facing| {
            let mut pairs = vec![("facing", facing)];
            pairs.extend(shape);
            props(&pairs)
        };
        assert_eq!(
            stairs.states,
            [
                StateCount {
                    properties: with("north"),
                    count: 2
                },
                StateCount {
                    properties: with("east"),
                    count: 1
                },
            ]
        );
        assert!(stats.blocks[0].states[0].properties.is_empty());

        assert_eq!(
            stats.entities,
            [
                TypeCount {
                    id: "minecraft:armor_stand".to_owned(),
                    count: 1
                },
                TypeCount {
                    id: "minecraft:pig".to_owned(),
                    count: 1
                },
            ]
        );
        assert_eq!(
            stats.block_entities,
            [TypeCount {
                id: "minecraft:chest".to_owned(),
                count: 1
            }]
        );

        assert_eq!(stats.regions.len(), 1);
        let region = &stats.regions[0];
        assert_eq!(region.name, "Main");
        assert_eq!((region.block_count, region.volume), (21, 32));
        assert_eq!(region.blocks, stats.blocks);
    }
}

/// Matches the renderer's `overlapping_regions_draw_one_block_per_cell`
/// guard in forks/nucleation: one cell, one block, whatever region holds it.
#[test]
fn overlapping_regions_count_each_cell_once() {
    let stone = state("minecraft:stone");
    let mut schematic = UniversalSchematic::new("overlap".to_owned());
    schematic.set_block(0, 0, 0, &stone);
    for name in ["b", "c"] {
        let mut region = Region::new(name.to_owned(), (0, 0, 0), (2, 1, 1));
        region.set_block(0, 0, 0, &stone);
        schematic.add_region(region);
    }
    schematic
        .get_region_mut("c")
        .unwrap()
        .set_block(1, 0, 0, &state("minecraft:dirt"));

    let stats = stats(&schematic);
    assert_eq!(stats.block_count, 2);
    let ids: Vec<(&str, u64)> = stats
        .blocks
        .iter()
        .map(|b| (b.id.as_str(), b.count))
        .collect();
    assert_eq!(ids, [("minecraft:dirt", 1), ("minecraft:stone", 1)]);

    // The cell belongs to the first region in precedence order.
    let per_region: Vec<(&str, u64)> = stats
        .regions
        .iter()
        .map(|r| (r.name.as_str(), r.block_count))
        .collect();
    assert_eq!(per_region, [("Main", 1), ("b", 0), ("c", 1)]);
    assert_eq!(
        stats.regions.iter().map(|r| r.block_count).sum::<u64>(),
        stats.block_count
    );
}
