use super::super::materials_width;
use super::super::tree::Tree;
use lumilio_schematic::fingerprint::Fingerprint;

fn tree(files: &[&str]) -> Tree {
    let mut tree = Tree::default();
    tree.set_files(
        "schematics",
        files.iter().map(|file| (*file).to_owned()).collect(),
    );
    tree
}

#[test]
fn files_sit_in_a_section_per_folder_and_other_files_are_left_out() {
    let tree = tree(&[
        "schematics/b.litematic",
        "schematics/farms/iron.litematic",
        "schematics/farms/deep/gold.litematic",
        "schematics/notes.txt",
        "saves/x.litematic",
    ]);
    assert_eq!(
        tree.sections("schematics", ""),
        [
            ("".to_owned(), vec!["schematics/b.litematic".to_owned()]),
            (
                "farms".to_owned(),
                vec!["schematics/farms/iron.litematic".to_owned()]
            ),
            (
                "farms/deep".to_owned(),
                vec!["schematics/farms/deep/gold.litematic".to_owned()]
            ),
        ]
    );
}

#[test]
fn a_search_keeps_only_the_matching_files_and_their_sections() {
    let tree = tree(&[
        "schematics/farms/iron.litematic",
        "schematics/farms/deep/gold.litematic",
        "schematics/house.litematic",
    ]);
    assert_eq!(
        tree.sections("schematics", "GOLD"),
        [(
            "farms/deep".to_owned(),
            vec!["schematics/farms/deep/gold.litematic".to_owned()]
        )]
    );
    assert_eq!(
        tree.sections("schematics", "farms").len(),
        2,
        "a folder's name matches its files"
    );
    assert!(tree.sections("schematics", "castle").is_empty());
}

#[test]
fn duplicate_groups_are_numbered_by_their_first_path() {
    let mut tree = tree(&[
        "schematics/a.litematic",
        "schematics/b.litematic",
        "schematics/c.litematic",
        "schematics/d.litematic",
    ]);
    tree.set_fingerprint("schematics/d.litematic", Fingerprint([1; 32]));
    tree.set_fingerprint("schematics/b.litematic", Fingerprint([1; 32]));
    tree.set_fingerprint("schematics/a.litematic", Fingerprint([9; 32]));
    assert_eq!(
        tree.duplicates(),
        [vec![
            "schematics/b.litematic".to_owned(),
            "schematics/d.litematic".to_owned()
        ]]
    );
    // Groups are numbered by their first file's path.
    tree.set_fingerprint("schematics/c.litematic", Fingerprint([9; 32]));
    assert_eq!(
        tree.duplicates()[0],
        ["schematics/a.litematic", "schematics/c.litematic"]
    );
    // A file that left the folder leaves its group.
    tree.set_files("schematics", vec!["schematics/a.litematic".into()]);
    assert!(tree.duplicates().is_empty());
}

#[test]
fn at_the_minimum_window_the_viewport_keeps_room_and_the_grid_four_tiles() {
    // The studio window at its 720 px minimum: the window's inset and the
    // 12 px column gap come off the width (D27).
    let materials = f32::from(materials_width(720.));
    let viewport = 720. - 24. - materials - 12.;
    assert!(viewport >= 380., "viewport {viewport}");
    // Four 60 px tiles and their 4 px gaps fit the narrowest column.
    assert!(materials >= 4. * 60. + 3. * 4., "materials {materials}");
    assert_eq!(f32::from(materials_width(1600.)), 400.);
}
