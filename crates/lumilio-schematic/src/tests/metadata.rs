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
