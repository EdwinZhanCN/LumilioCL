use super::{cells, room, through_file};
use crate::fingerprint::fingerprint;
use crate::metadata::{Edit, EditError, apply, read};

fn edit(name: &str) -> Edit {
    Edit {
        name: name.to_owned(),
        author: "Someone Else".to_owned(),
        description: "Renamed".to_owned(),
    }
}

#[test]
fn reads_what_the_metadata_page_shows() {
    let info = read(&through_file(&room()));
    assert_eq!(info.name, "Room");
    assert_eq!(info.author, "Builder");
    assert_eq!(info.description, "A small room");
    assert_eq!(info.size, (4, 2, 4));
    assert_eq!(info.block_count, 21);
    assert_eq!(info.region_count, 1);
    assert_eq!((info.created, info.modified), (Some(1_000), Some(2_000)));
}

#[test]
fn a_metadata_edit_leaves_block_data_unchanged() {
    let original = through_file(&room());
    let mut edited = original.clone();
    apply(&mut edited, &edit("  New name "), 9_000).unwrap();
    let edited = through_file(&edited);

    let info = read(&edited);
    assert_eq!(info.name, "New name");
    assert_eq!(info.author, "Someone Else");
    assert_eq!(info.description, "Renamed");
    assert_eq!(info.modified, Some(9_000));
    assert_eq!(info.created, Some(1_000), "creation time is kept");

    assert_eq!(cells(&edited), cells(&original));
    assert_eq!(fingerprint(&edited), fingerprint(&original));
    assert_eq!(
        edited.get_block_entities_as_list().len(),
        original.get_block_entities_as_list().len()
    );
    assert_eq!(
        edited.get_entities_as_list().len(),
        original.get_entities_as_list().len()
    );
}

#[test]
fn an_empty_name_is_refused_and_nothing_changes() {
    let mut schematic = room();
    for name in ["", "   "] {
        assert_eq!(
            apply(&mut schematic, &edit(name), 9_000),
            Err(EditError::EmptyName)
        );
    }
    let info = read(&schematic);
    assert_eq!(info.name, "Room");
    assert_eq!(info.author, "Builder");
    assert_eq!(info.modified, Some(2_000));
}

/// `bytes`, a written `.litematic`, without its `MinecraftDataVersion`, as
/// an older Litematica writes it.
fn without_data_version(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut root = lumilio_nbt::parse_maybe_gzip(bytes).unwrap();
    let lumilio_nbt::Tag::Compound(map) = &mut root else {
        panic!("a litematic's root is a compound");
    };
    assert!(map.remove("MinecraftDataVersion").is_some());
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gzip.write_all(&lumilio_nbt::to_bytes(&root)).unwrap();
    gzip.finish().unwrap()
}

#[test]
fn a_file_without_a_data_version_reads_as_unknown() {
    let mut schematic = room();
    schematic.metadata.mc_version = Some(3465);
    schematic.metadata.source_data_version = Some(3465);
    let bytes = crate::to_litematic_bytes(&schematic).unwrap();
    assert_eq!(
        read(&crate::load_litematic(&bytes).unwrap()).data_version,
        Some(3465)
    );

    let unknown = crate::load_litematic(&without_data_version(&bytes)).unwrap();
    assert_eq!(crate::convert::known_data_version(&unknown), None);
    // Not the canonical version Nucleation would convert from.
    assert_eq!(read(&unknown).data_version, None);
    assert_eq!(read(&unknown).block_count, 21, "the blocks still read");
}
