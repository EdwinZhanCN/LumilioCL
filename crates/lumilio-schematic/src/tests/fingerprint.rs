use super::{room, state, through_file};
use crate::fingerprint::fingerprint;
use crate::{Region, UniversalSchematic, load_litematic, to_litematic_bytes};

#[test]
fn same_content_with_other_metadata_is_a_duplicate() {
    let original = room();
    let mut copy = room();
    copy.metadata.name = Some("Room (copy)".to_owned());
    copy.metadata.author = Some("Another".to_owned());
    copy.metadata.description = Some("Saved again".to_owned());
    copy.metadata.created = Some(5_000);
    copy.metadata.modified = Some(6_000);

    let original_bytes = to_litematic_bytes(&original).unwrap();
    let copy_bytes = to_litematic_bytes(&copy).unwrap();
    assert_ne!(original_bytes, copy_bytes, "the files differ");
    assert_eq!(
        fingerprint(&load_litematic(&original_bytes).unwrap()),
        fingerprint(&load_litematic(&copy_bytes).unwrap())
    );
}

#[test]
fn one_different_block_is_not_a_duplicate() {
    let original = room();
    let mut changed = room();
    changed.set_block(3, 1, 3, &state("minecraft:torch"));
    assert_ne!(fingerprint(&original), fingerprint(&changed));

    let mut renamed_chest = room();
    let region = renamed_chest.get_region_mut("Main").unwrap();
    let mut chest = region.block_entities.get(&(0, 1, 0)).unwrap().clone();
    chest.nbt_mut().insert(
        "CustomName".to_owned(),
        nucleation::nbt::NbtValue::String("Other".to_owned()),
    );
    region.block_entities.insert((0, 1, 0), chest);
    assert_ne!(fingerprint(&original), fingerprint(&renamed_chest));
}

#[test]
fn fingerprint_is_stable() {
    let schematic = room();
    let first = fingerprint(&schematic);
    assert_eq!(first, fingerprint(&schematic));
    assert_eq!(first, fingerprint(&through_file(&schematic)));
    assert_eq!(first, fingerprint(&through_file(&through_file(&schematic))));
    assert_eq!(first.to_string().len(), 64);
}

/// Palette order is an accident of the order blocks were placed in.
#[test]
fn palette_order_does_not_matter() {
    let build = |order: &[(i32, &str)]| {
        let mut schematic = UniversalSchematic::new("p".to_owned());
        let mut region = Region::new("Main".to_owned(), (0, 0, 0), (3, 1, 1));
        for (x, block) in order {
            region.set_block(*x, 0, 0, &state(block));
        }
        schematic.add_region(region);
        schematic
    };
    let a = build(&[
        (0, "minecraft:stone"),
        (1, "minecraft:dirt"),
        (2, "minecraft:glass"),
    ]);
    let b = build(&[
        (2, "minecraft:glass"),
        (1, "minecraft:dirt"),
        (0, "minecraft:stone"),
    ]);
    assert_eq!(fingerprint(&a), fingerprint(&b));
}
