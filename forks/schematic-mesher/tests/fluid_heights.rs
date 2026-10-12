//! Regression test: a fluid surface slopes toward open air like Minecraft's.
//!
//! Backstory: corner heights averaged only neighboring fluid, so the edge of a
//! pool that opens onto air stayed full height instead of dipping as in
//! Minecraft (`LiquidBlockRenderer.calculateAverageHeight`), and waterlogged
//! neighbors did not count as water. This is a LumilioCL local change (see
//! `forks/README.md`).

use schematic_mesher::mesher::liquid::{corner_heights, FluidState};
use schematic_mesher::{BlockPosition, InputBlock};

type Map<'a> = rustc_hash::FxHashMap<BlockPosition, &'a InputBlock>;

fn at(x: i32, y: i32, z: i32) -> BlockPosition {
    BlockPosition::new(x, y, z)
}

fn heights(map: &Map) -> [f32; 4] {
    let water = map[&at(0, 0, 0)];
    corner_heights(at(0, 0, 0), &FluidState::from_block(water).unwrap(), map)
}

#[test]
fn a_source_open_to_air_slopes_at_its_corners() {
    let water = InputBlock::new("minecraft:water").with_property("level", "0");
    let mut map = Map::default();
    map.insert(at(0, 0, 0), &water);
    // Own height 8/9 weighs 10; the two empty sides weigh 1 each at height 0.
    let expected = (8.0 / 9.0 * 10.0) / 12.0;
    for h in heights(&map) {
        assert!((h - expected).abs() < 1e-4, "{h}");
    }
}

#[test]
fn walls_keep_a_source_level_and_waterlogged_blocks_count_as_water() {
    let water = InputBlock::new("minecraft:water").with_property("level", "0");
    let stone = InputBlock::new("minecraft:stone");
    let mut map = Map::default();
    map.insert(at(0, 0, 0), &water);
    for (x, z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        map.insert(at(x, 0, z), &stone);
    }
    for h in heights(&map) {
        assert!((h - 8.0 / 9.0).abs() < 1e-4, "{h}");
    }

    let shallow = InputBlock::new("minecraft:water").with_property("level", "7");
    let stairs = InputBlock::new("minecraft:oak_stairs").with_property("waterlogged", "true");
    let mut map = Map::default();
    map.insert(at(0, 0, 0), &shallow);
    map.insert(at(1, 0, 0), &stairs);
    // The east corners lean toward the waterlogged stairs' full source height.
    let [_, ne, se, _] = heights(&map);
    assert!(ne > 0.5 && se > 0.5, "{ne} {se}");
}
