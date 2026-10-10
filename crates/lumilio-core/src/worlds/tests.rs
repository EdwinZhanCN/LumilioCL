use super::*;
use crate::nbt::{Tag, to_bytes};
use std::collections::BTreeMap;

fn level(name: &str, played: i64, version: &str, hardcore: i8) -> Vec<u8> {
    let root = Tag::Compound(BTreeMap::from([(
        "Data".into(),
        Tag::Compound(BTreeMap::from([
            ("LevelName".into(), Tag::String(name.into())),
            ("LastPlayed".into(), Tag::Long(played)),
            ("hardcore".into(), Tag::Byte(hardcore)),
            (
                "Version".into(),
                Tag::Compound(BTreeMap::from([(
                    "Name".into(),
                    Tag::String(version.into()),
                )])),
            ),
        ])),
    )]));
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    io::Write::write_all(&mut encoder, &to_bytes(&root)).unwrap();
    encoder.finish().unwrap()
}

fn add_world(game: &Path, folder: &str, data: &[u8]) {
    let dir = game.join("saves").join(folder);
    fs::create_dir_all(dir.join("region")).unwrap();
    fs::write(dir.join("level.dat"), data).unwrap();
    fs::write(dir.join("region/r.0.0.mca"), vec![0_u8; 100]).unwrap();
}

fn game() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(&game).unwrap();
    (dir, game)
}

#[test]
fn lists_worlds_newest_first_with_their_details() {
    let (_dir, game) = game();
    add_world(&game, "old", &level("Old World", 1000, "1.20.4", 0));
    add_world(&game, "new", &level("New World", 2000, "1.21.1", 1));
    fs::write(game.join("saves/new/icon.png"), "png").unwrap();
    fs::create_dir_all(game.join("saves/not-a-world")).unwrap();
    fs::write(game.join("saves/loose-file"), "x").unwrap();
    let worlds = scan(&game).unwrap();
    let names: Vec<_> = worlds.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(names, ["New World", "Old World"]);
    assert_eq!(worlds[0].game_version.as_deref(), Some("1.21.1"));
    assert!(worlds[0].hardcore && !worlds[0].damaged);
    let icon = worlds[0].icon.as_deref().expect("new has a cover");
    assert_eq!(icon, game.join("saves/new/icon.png"));
    assert!(!worlds[1].hardcore && worlds[1].icon.is_none());
    assert_eq!(worlds[0].last_played_ms, Some(2000));
}

#[test]
fn a_world_reports_when_its_lock_was_last_touched() {
    let (_dir, game) = game();
    add_world(&game, "idle", &level("Idle", 1, "1", 0));
    add_world(&game, "open", &level("Open", 2, "1", 0));
    fs::write(game.join("saves/open/session.lock"), "x").unwrap();
    let worlds = scan(&game).unwrap();
    let find = |folder: &str| worlds.iter().find(|w| w.folder == folder).unwrap();
    assert_eq!(find("idle").lock_touched_ms, None);
    let touched = find("open")
        .lock_touched_ms
        .expect("a lock file has a time");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    assert!((now - touched).abs() < 60_000);
}

#[test]
fn a_damaged_level_file_still_lists_the_world_by_folder_name() {
    let (_dir, game) = game();
    add_world(&game, "broken-world", b"garbage");
    let worlds = scan(&game).unwrap();
    assert_eq!(worlds.len(), 1);
    assert!(worlds[0].damaged);
    assert_eq!(worlds[0].name, "broken-world");
    assert_eq!(worlds[0].last_played_ms, None);
}

#[test]
fn a_missing_saves_folder_is_empty() {
    let (_dir, game) = game();
    assert!(scan(&game).unwrap().is_empty());
}

#[test]
fn size_sums_nested_files() {
    let (_dir, game) = game();
    let data = level("W", 1, "1", 0);
    add_world(&game, "w", &data);
    assert_eq!(size(&game, "w").unwrap(), data.len() as u64 + 100);
}

fn zip_of(path: &Path, entries: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, data) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        io::Write::write_all(&mut writer, data).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn a_world_survives_the_trip_through_a_zip_without_its_lock() {
    let (dir, game) = game();
    add_world(&game, "w", &level("W", 1, "1", 0));
    fs::write(game.join("saves/w/session.lock"), "lock").unwrap();
    let archive = dir.path().join("w.zip");
    let size = export_zip(&game, "w", &archive).unwrap();
    assert!(size > 0 && !dir.path().join("w.zip.part").exists());

    let other = tempfile::tempdir().unwrap();
    let elsewhere = other.path().join("game");
    assert_eq!(import_zip(&elsewhere, &archive).unwrap(), "w");
    assert!(elsewhere.join("saves/w/level.dat").is_file());
    assert!(elsewhere.join("saves/w/region/r.0.0.mca").is_file());
    assert!(!elsewhere.join("saves/w/session.lock").exists());
    // Importing again never overwrites: the second copy gets a number.
    assert_eq!(import_zip(&elsewhere, &archive).unwrap(), "w 2");
    assert_eq!(scan(&elsewhere).unwrap().len(), 2);
}

#[test]
fn a_world_at_the_top_of_the_archive_is_named_after_the_file() {
    let (dir, game) = game();
    let archive = dir.path().join("Skyblock.zip");
    zip_of(
        &archive,
        &[
            ("level.dat", b"x"),
            ("region/r.0.0.mca", b"y"),
            ("__MACOSX/junk", b"z"),
        ],
    );
    assert_eq!(import_zip(&game, &archive).unwrap(), "Skyblock");
    assert!(game.join("saves/Skyblock/region/r.0.0.mca").is_file());
    assert!(!game.join("saves/Skyblock/__MACOSX").exists());
}

#[test]
fn an_archive_without_a_world_or_with_escaping_paths_changes_nothing() {
    let (dir, game) = game();
    let plain = dir.path().join("docs.zip");
    zip_of(&plain, &[("readme.txt", b"hi")]);
    assert!(matches!(
        import_zip(&game, &plain),
        Err(WorldError::NotAWorld)
    ));

    let hostile = dir.path().join("hostile.zip");
    zip_of(
        &hostile,
        &[
            ("w/level.dat", b"x"),
            ("w/../../escape.txt", b"bad"),
            ("/abs.txt", b"bad"),
        ],
    );
    assert_eq!(import_zip(&game, &hostile).unwrap(), "w");
    assert!(!dir.path().join("escape.txt").exists());
    assert!(!game.join("escape.txt").exists());
    assert!(!game.join("saves/escape.txt").exists());
    assert!(!game.join("abs.txt").exists());

    let garbage = dir.path().join("garbage.zip");
    fs::write(&garbage, b"not a zip").unwrap();
    assert!(matches!(
        import_zip(&game, &garbage),
        Err(WorldError::BadArchive(_))
    ));
    // No half-made folders are left behind.
    let leftovers: Vec<_> = fs::read_dir(game.join("saves"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, ["w"]);
}

#[test]
fn duplicate_copies_everything_and_never_overwrites() {
    let (_dir, game) = game();
    add_world(&game, "w", &level("W", 1, "1", 0));
    assert_eq!(copy_name(&game, "w"), "w copy");
    duplicate(&game, "w", "w copy").unwrap();
    assert!(game.join("saves/w copy/region/r.0.0.mca").is_file());
    assert_eq!(copy_name(&game, "w"), "w copy 2");
    assert!(matches!(
        duplicate(&game, "w", "w copy"),
        Err(WorldError::AlreadyExists(_))
    ));
    assert_eq!(size(&game, "w").unwrap(), size(&game, "w copy").unwrap());
}

#[test]
fn linked_saves_or_worlds_are_refused_and_never_followed() {
    let (dir, game) = game();
    let outside = dir.path().join("elsewhere");
    add_world(&outside, "victim", &level("V", 1, "1", 0));
    // A linked world inside a real saves folder.
    fs::create_dir_all(game.join("saves")).unwrap();
    std::os::unix::fs::symlink(outside.join("saves/victim"), game.join("saves/linked")).unwrap();
    assert!(scan(&game).unwrap().is_empty());
    for result in [
        delete(&game, "linked"),
        duplicate(&game, "linked", "copy").map(|_| ()),
    ] {
        assert!(matches!(result, Err(WorldError::Linked(_))));
    }
    assert!(outside.join("saves/victim/level.dat").is_file());
    // A linked saves folder.
    let (dir2, game2) = self::game();
    let target = dir2.path().join("real-saves");
    fs::create_dir_all(target.join("w")).unwrap();
    fs::write(target.join("w/level.dat"), "x").unwrap();
    std::os::unix::fs::symlink(&target, game2.join("saves")).unwrap();
    assert!(matches!(scan(&game2), Err(WorldError::Linked(_))));
    assert!(matches!(delete(&game2, "w"), Err(WorldError::Linked(_))));
    assert!(target.join("w/level.dat").is_file());
}

#[test]
fn a_copy_is_published_whole_and_an_interrupted_one_is_invisible_and_swept() {
    let (_dir, game) = game();
    add_world(&game, "w", &level("W", 1, "1", 0));
    // A crash mid-copy leaves a hidden half-made folder with a level file.
    let stale = game.join("saves/.w copy.copying");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("level.dat"), "half").unwrap();
    assert_eq!(scan(&game).unwrap().len(), 1, "not listed as a world");
    // A fresh copy of the same name replaces the leftover, not merges with it.
    duplicate(&game, "w", "w copy").unwrap();
    assert!(!stale.exists());
    assert_eq!(scan(&game).unwrap().len(), 2);
    // The copy is built under the hidden name: when that name cannot be
    // used, nothing appears under the real one.
    fs::write(game.join("saves/.w copy 2.copying"), "in the way").unwrap();
    assert!(duplicate(&game, "w", "w copy 2").is_err());
    assert!(!game.join("saves/w copy 2").exists());
    fs::remove_file(game.join("saves/.w copy 2.copying")).unwrap();
    // Leftovers of other names are swept on request.
    let other = game.join("saves/.zzz.copying");
    fs::create_dir_all(&other).unwrap();
    assert_eq!(sweep_temporary(&game), 1);
    assert!(!other.exists() && game.join("saves/w copy").is_dir());
}

#[test]
fn delete_and_names_are_guarded() {
    let (_dir, game) = game();
    add_world(&game, "w", &level("W", 1, "1", 0));
    fs::write(game.join("precious.txt"), "keep").unwrap();
    for bad in ["../..", "a/b", "..", ""] {
        assert!(
            matches!(delete(&game, bad), Err(WorldError::UnsafeName(_))),
            "{bad}"
        );
    }
    assert!(matches!(
        duplicate(&game, "w", "../evil"),
        Err(WorldError::UnsafeName(_))
    ));
    assert!(matches!(
        delete(&game, "ghost"),
        Err(WorldError::NotFound(_))
    ));
    delete(&game, "w").unwrap();
    assert!(!game.join("saves/w").exists());
    assert!(game.join("precious.txt").exists());
}
