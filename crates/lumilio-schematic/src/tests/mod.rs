mod convert;
mod fingerprint;
mod inspect;
mod metadata;
mod replace;
mod stats;

use crate::{BlockState, Region, UniversalSchematic};
use nucleation::Entity;
use nucleation::block_entity::BlockEntity;
use nucleation::nbt::NbtValue;

pub(super) fn state(text: &str) -> BlockState {
    BlockState::from_block_string(text).unwrap()
}

/// A 4×2×4 room: a stone floor, three oak stairs in two states, two chests
/// (one with a named block entity), a pig and an armor stand.
pub(super) fn room() -> UniversalSchematic {
    let mut schematic = UniversalSchematic::new("Room".to_owned());
    schematic.metadata.author = Some("Builder".to_owned());
    schematic.metadata.description = Some("A small room".to_owned());
    schematic.metadata.created = Some(1_000);
    schematic.metadata.modified = Some(2_000);

    let mut region = Region::new("Main".to_owned(), (0, 0, 0), (4, 2, 4));
    for x in 0..4 {
        for z in 0..4 {
            region.set_block(x, 0, z, &state("minecraft:stone"));
        }
    }
    let north = "minecraft:oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]";
    let east = "minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]";
    region.set_block(1, 1, 3, &state(north));
    region.set_block(2, 1, 3, &state(north));
    region.set_block(3, 1, 2, &state(east));
    region.set_block(0, 1, 0, &state("minecraft:chest[facing=north,type=single]"));
    region.set_block(1, 1, 0, &state("minecraft:chest[facing=south,type=single]"));
    region.set_block_entity(
        nucleation::block_position::BlockPosition { x: 0, y: 1, z: 0 },
        BlockEntity::new("minecraft:chest".to_owned(), (0, 1, 0))
            .with_nbt_data("CustomName".to_owned(), NbtValue::String("Loot".to_owned())),
    );
    region.add_entity(Entity::new("minecraft:pig".to_owned(), (1.5, 1.0, 1.5)));
    region.add_entity(Entity::new(
        "minecraft:armor_stand".to_owned(),
        (2.5, 1.0, 1.5),
    ));
    schematic.add_region(region);
    schematic
}

/// Saves and reloads, as the studio sees a file.
pub(super) fn through_file(schematic: &UniversalSchematic) -> UniversalSchematic {
    crate::load_litematic(&crate::to_litematic_bytes(schematic).unwrap()).unwrap()
}

/// Every non-air cell as `(position, canonical state)`, sorted.
pub(super) fn cells(schematic: &UniversalSchematic) -> Vec<((i32, i32, i32), String)> {
    let mut cells: Vec<_> = schematic
        .iter_blocks()
        .filter(|(_, block)| !crate::is_air(&block.name))
        .map(|(pos, block)| {
            let mut block = block.clone();
            block.properties.sort();
            ((pos.x, pos.y, pos.z), block.to_string())
        })
        .collect();
    cells.sort();
    cells
}
