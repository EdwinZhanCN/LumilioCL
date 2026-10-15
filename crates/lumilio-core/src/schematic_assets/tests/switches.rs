//! Which overlays each studio folder draws with (plan schematic-studio, T21).

use std::path::{Path, PathBuf};

use super::super::{forget_switch, switch, switched_on};

#[test]
fn every_source_is_off_until_switched_on_and_the_switch_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("profile/schematic-sources.json");
    let create = PathBuf::from("/game/mods/create.jar");
    let faithful = PathBuf::from("/game/resourcepacks/Faithful.zip");
    let file = "schematics/farms/a.litematic";

    assert!(switched_on(&list, file).is_empty(), "off by default");
    switch(&list, "schematics", &create, true).unwrap();
    switch(&list, "schematics", &faithful, true).unwrap();
    switch(&list, "schematics", &create, true).unwrap();
    // Read from the file alone, as a newly opened window does.
    assert_eq!(switched_on(&list, file), [create.clone(), faithful.clone()]);

    switch(&list, "schematics", &create, false).unwrap();
    assert_eq!(switched_on(&list, file), std::slice::from_ref(&faithful));
    forget_switch(&list, &faithful).unwrap();
    assert!(switched_on(&list, file).is_empty());
}

#[test]
fn the_switches_belong_to_the_folder_that_holds_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("schematic-sources.json");
    let create = Path::new("/game/mods/create.jar");
    switch(&list, "schematics", create, true).unwrap();
    assert_eq!(switched_on(&list, "schematics/a.litematic"), [create]);
    assert!(switched_on(&list, "schematics-old/a.litematic").is_empty());
    assert!(switched_on(&list, "config/schematics/a.litematic").is_empty());

    // A nested folder has switches of its own.
    switch(&list, "schematics/farms", create, false).unwrap();
    let iron = Path::new("/game/resourcepacks/iron.zip");
    switch(&list, "schematics/farms", iron, true).unwrap();
    assert_eq!(switched_on(&list, "schematics/farms/a.litematic"), [iron]);
    assert_eq!(switched_on(&list, "schematics/a.litematic"), [create]);
}

#[test]
fn a_damaged_list_reads_as_all_off() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("schematic-sources.json");
    std::fs::write(&list, b"{not json").unwrap();
    assert!(switched_on(&list, "schematics/a.litematic").is_empty());
    switch(&list, "schematics", Path::new("/a.zip"), true).unwrap();
    assert_eq!(
        switched_on(&list, "schematics/a.litematic"),
        [Path::new("/a.zip")]
    );
}
