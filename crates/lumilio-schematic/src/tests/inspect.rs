//! The inspector's reading of a block entity (T32): names, items in both
//! saved shapes, sign and command text, and the rest as sorted SNBT.

use nucleation::block_entity::BlockEntity;
use nucleation::block_position::BlockPosition;
use nucleation::nbt::{NbtMap, NbtValue};

use super::{room, state, through_file};
use crate::inspect::{Grid, Stack, block_entity, grid};
use crate::{Region, UniversalSchematic};

fn compound(pairs: &[(&str, NbtValue)]) -> NbtValue {
    let mut map = NbtMap::new();
    for (key, value) in pairs {
        map.insert((*key).to_owned(), value.clone());
    }
    NbtValue::Compound(map)
}

fn string(text: &str) -> NbtValue {
    NbtValue::String(text.to_owned())
}

/// One block with `entity` at the origin of a one-block schematic.
fn holding(block: &str, entity: BlockEntity) -> UniversalSchematic {
    let mut schematic = UniversalSchematic::new("One".to_owned());
    let mut region = Region::new("Main".to_owned(), (0, 0, 0), (1, 1, 1));
    region.set_block(0, 0, 0, &state(block));
    region.set_block_entity(BlockPosition { x: 0, y: 0, z: 0 }, entity);
    schematic.add_region(region);
    schematic
}

#[test]
fn a_chest_gives_its_name_and_items_in_slot_order() {
    let items = NbtValue::List(vec![
        // Saved by 1.20.5 and later.
        compound(&[
            ("Slot", NbtValue::Byte(4)),
            ("id", string("minecraft:diamond")),
            ("count", NbtValue::Int(3)),
        ]),
        // Saved before.
        compound(&[
            ("Slot", NbtValue::Byte(0)),
            ("id", string("minecraft:iron_ingot")),
            ("Count", NbtValue::Byte(64)),
        ]),
        compound(&[
            ("Slot", NbtValue::Byte(9)),
            ("id", string("minecraft:stick")),
        ]),
    ]);
    let chest = BlockEntity::new("minecraft:chest".to_owned(), (0, 0, 0))
        .with_nbt_data("Items".to_owned(), items)
        .with_nbt_data("CustomName".to_owned(), string(r#"{"text":"Loot"}"#))
        .with_nbt_data("x".to_owned(), NbtValue::Int(0));
    let contents = block_entity(&holding("minecraft:chest[facing=north]", chest), [0, 0, 0])
        .expect("the chest's block entity");
    assert_eq!(contents.id, "minecraft:chest");
    assert_eq!(contents.name.as_deref(), Some("Loot"));
    let stack = |slot, id: &str, count| Stack {
        slot: Some(slot),
        id: id.to_owned(),
        count,
    };
    assert_eq!(
        contents.items,
        Some(vec![
            stack(0, "minecraft:iron_ingot", 64),
            stack(4, "minecraft:diamond", 3),
            stack(9, "minecraft:stick", 1),
        ])
    );
    assert!(contents.text.is_empty());
    let lines: Vec<&str> = contents.data.lines().collect();
    assert_eq!(lines.len(), 2, "{}", contents.data);
    assert_eq!(lines[0], r#"CustomName: "{\"text\":\"Loot\"}""#);
    assert!(
        lines[1].starts_with(r#"Items: [{Slot: 4b, count: 3, id: "minecraft:diamond"}"#),
        "keys sorted, types marked: {}",
        lines[1]
    );
    assert!(
        block_entity(
            &holding("minecraft:stone", BlockEntity::new("x".into(), (0, 0, 0))),
            [1, 0, 0]
        )
        .is_none()
    );
}

#[test]
fn a_sign_gives_its_lines_front_then_back_in_either_text_shape() {
    let front = compound(&[(
        "messages",
        NbtValue::List(vec![
            string(r#""Hello""#),
            string(r#"{"text":"World","extra":[{"text":"!"}]}"#),
            string(r#""""#),
            string(r#""""#),
        ]),
    )]);
    // 1.21.5 and later keep components as NBT, or as a plain string.
    let back = compound(&[(
        "messages",
        NbtValue::List(vec![
            compound(&[("text", string("Back"))]),
            string("plain"),
            string(""),
            string(""),
        ]),
    )]);
    let sign = BlockEntity::new("minecraft:sign".to_owned(), (0, 0, 0))
        .with_nbt_data("front_text".to_owned(), front)
        .with_nbt_data("back_text".to_owned(), back);
    let contents =
        block_entity(&holding("minecraft:oak_sign[rotation=0]", sign), [0, 0, 0]).unwrap();
    assert_eq!(contents.text, ["Hello", "World!", "Back", "plain"]);
    assert_eq!(contents.items, None, "a sign holds no items");

    let old = BlockEntity::new("minecraft:sign".to_owned(), (0, 0, 0))
        .with_nbt_data("Text1".to_owned(), string(r#"{"text":"Old"}"#))
        .with_nbt_data("Text2".to_owned(), string(r#""""#));
    let contents =
        block_entity(&holding("minecraft:oak_sign[rotation=0]", old), [0, 0, 0]).unwrap();
    assert_eq!(contents.text, ["Old"]);

    let command = BlockEntity::new("minecraft:command_block".to_owned(), (0, 0, 0))
        .with_nbt_data("Command".to_owned(), string("say hi"));
    let contents = block_entity(&holding("minecraft:command_block", command), [0, 0, 0]).unwrap();
    assert_eq!(contents.text, ["say hi"]);
}

#[test]
fn a_record_or_a_book_is_one_item_and_an_empty_chest_holds_none() {
    let jukebox = BlockEntity::new("minecraft:jukebox".to_owned(), (0, 0, 0)).with_nbt_data(
        "RecordItem".to_owned(),
        compound(&[
            ("id", string("minecraft:music_disc_cat")),
            ("count", NbtValue::Int(1)),
        ]),
    );
    let contents = block_entity(
        &holding("minecraft:jukebox[has_record=true]", jukebox),
        [0, 0, 0],
    )
    .unwrap();
    assert_eq!(
        contents.items,
        Some(vec![Stack {
            slot: None,
            id: "minecraft:music_disc_cat".to_owned(),
            count: 1
        }])
    );
    let empty = BlockEntity::new("minecraft:barrel".to_owned(), (0, 0, 0))
        .with_nbt_data("Items".to_owned(), NbtValue::List(Vec::new()));
    let contents = block_entity(&holding("minecraft:barrel", empty), [0, 0, 0]).unwrap();
    assert_eq!(contents.items, Some(Vec::new()), "a container, empty");
    assert_eq!(
        contents.grid,
        Some(Grid {
            columns: 9,
            slots: 27
        }),
        "still its 27 slots"
    );
    assert_eq!(contents.data, "Items: []");
}

/// The slots are laid out as the game's screen shows them; a slot past what
/// the table knows adds rows instead of being lost.
#[test]
fn slots_are_laid_out_as_the_game_shows_them() {
    let in_slot = |slot: i32| Stack {
        slot: Some(slot),
        id: "minecraft:stick".to_owned(),
        count: 1,
    };
    let laid = |id: &str, items: &[Stack]| {
        let Grid { columns, slots } = grid(id, items);
        (columns, slots)
    };
    assert_eq!(laid("minecraft:chest", &[in_slot(0)]), (9, 27));
    assert_eq!(laid("minecraft:shulker_box", &[]), (9, 27));
    assert_eq!(laid("minecraft:hopper", &[in_slot(4)]), (5, 5));
    assert_eq!(laid("minecraft:dropper", &[]), (3, 9));
    assert_eq!(laid("minecraft:furnace", &[in_slot(2)]), (3, 3));
    assert_eq!(
        laid("minecraft:chest", &[in_slot(30)]),
        (9, 36),
        "a modded chest"
    );
    assert_eq!(
        laid("othermod:crate", &[in_slot(12)]),
        (9, 18),
        "rows of nine"
    );
    let record = Stack {
        slot: None,
        id: "minecraft:music_disc_cat".to_owned(),
        count: 1,
    };
    assert_eq!(
        laid("minecraft:jukebox", &[record]),
        (1, 1),
        "one item, one slot"
    );
}

#[test]
fn a_saved_file_keeps_its_block_entities_where_the_picture_picks_them() {
    let contents = block_entity(&through_file(&room()), [0, 1, 0]).expect("the named chest");
    assert_eq!(contents.name.as_deref(), Some("Loot"));
    assert!(block_entity(&through_file(&room()), [2, 1, 0]).is_none());
}
