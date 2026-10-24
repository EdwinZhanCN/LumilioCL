use super::*;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    game: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    let game = root.join("instances/a/game");
    for (path, body) in [
        ("saves/One/level.dat", "one"),
        ("saves/One/region/r.0.0.mca", "region-one"),
        ("saves/Two/level.dat", "two"),
        ("config/mod.toml", "cfg"),
        ("options.txt", "fov:70"),
    ] {
        let full = game.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, body).unwrap();
    }
    Fixture {
        root,
        game,
        _dir: dir,
    }
}

#[test]
fn a_full_snapshot_lists_with_its_details() {
    let f = fixture();
    let id = create(
        &f.root,
        "a",
        &f.game,
        SnapshotScope::Full,
        " before update ",
        100,
    )
    .unwrap();
    assert_eq!(id, "snapshot-100");
    let second = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 100).unwrap();
    assert_eq!(second, "snapshot-100-2");
    create(
        &f.root,
        "a",
        &f.game,
        SnapshotScope::World("One".into()),
        "one",
        200,
    )
    .unwrap();
    let infos = list(&f.root, "a").unwrap();
    assert_eq!(infos.len(), 3);
    assert_eq!(infos[0].scope, SnapshotScope::World("One".into()));
    let full = infos.iter().find(|i| i.id == "snapshot-100").unwrap();
    assert_eq!(full.label, "before update");
    assert!(full.size > 0);
}

#[test]
fn nothing_to_back_up_and_bad_worlds_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(&game).unwrap();
    assert!(matches!(
        create(dir.path(), "a", &game, SnapshotScope::Full, "", 1),
        Err(SnapshotError::Empty)
    ));
    let f = fixture();
    assert!(matches!(
        create(
            &f.root,
            "a",
            &f.game,
            SnapshotScope::World("../x".into()),
            "",
            1
        ),
        Err(SnapshotError::UnsafeName(_))
    ));
    assert!(matches!(
        create(
            &f.root,
            "a",
            &f.game,
            SnapshotScope::World("Ghost".into()),
            "",
            1
        ),
        Err(SnapshotError::NotFound(_))
    ));
    assert!(
        list(&f.root, "a").unwrap().is_empty(),
        "failures leave no snapshot behind"
    );
    assert!(!directory(&f.root, "a").join("snapshot-1.zip.tmp").exists());
}

#[test]
fn restoring_a_world_replaces_only_that_world() {
    let f = fixture();
    let id = create(
        &f.root,
        "a",
        &f.game,
        SnapshotScope::World("One".into()),
        "",
        1,
    )
    .unwrap();
    fs::write(f.game.join("saves/One/level.dat"), "changed").unwrap();
    fs::write(f.game.join("saves/One/extra.dat"), "added later").unwrap();
    fs::write(f.game.join("saves/Two/level.dat"), "two-changed").unwrap();
    let units = restore(&f.root, "a", &id, &f.game).unwrap();
    assert_eq!(units, [PathBuf::from("saves/One")]);
    assert_eq!(
        fs::read_to_string(f.game.join("saves/One/level.dat")).unwrap(),
        "one"
    );
    assert!(
        !f.game.join("saves/One/extra.dat").exists(),
        "the world is replaced, not merged"
    );
    assert_eq!(
        fs::read_to_string(f.game.join("saves/Two/level.dat")).unwrap(),
        "two-changed"
    );
    assert!(
        !Layout::new(&f.root).operations().exists()
            || fs::read_dir(Layout::new(&f.root).operations())
                .unwrap()
                .next()
                .is_none()
    );
}

#[test]
fn restoring_a_full_snapshot_brings_back_deleted_worlds_and_settings() {
    let f = fixture();
    let id = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
    fs::remove_dir_all(f.game.join("saves/Two")).unwrap();
    fs::remove_dir_all(f.game.join("config")).unwrap();
    fs::write(f.game.join("options.txt"), "fov:110").unwrap();
    fs::create_dir_all(f.game.join("saves/Three")).unwrap();
    fs::write(f.game.join("saves/Three/level.dat"), "three").unwrap();
    restore(&f.root, "a", &id, &f.game).unwrap();
    assert_eq!(
        fs::read_to_string(f.game.join("saves/Two/level.dat")).unwrap(),
        "two"
    );
    assert_eq!(
        fs::read_to_string(f.game.join("config/mod.toml")).unwrap(),
        "cfg"
    );
    assert_eq!(
        fs::read_to_string(f.game.join("options.txt")).unwrap(),
        "fov:70"
    );
    assert!(
        f.game.join("saves/Three/level.dat").exists(),
        "worlds not in the snapshot survive"
    );
}

#[test]
fn a_hostile_archive_cannot_write_outside_the_restorable_units() {
    let f = fixture();
    let folder = directory(&f.root, "a");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join("evil.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    let options = SimpleFileOptions::default();
    writer.start_file(MANIFEST, options).unwrap();
    writer
        .write_all(br#"{"label":"","created":1,"scope":{"kind":"full"}}"#)
        .unwrap();
    for name in ["../escaped.txt", "mods/evil.jar", "saves/Good/level.dat"] {
        writer.start_file(name, options).unwrap();
        writer.write_all(b"x").unwrap();
    }
    writer.finish().unwrap();
    let units = restore(&f.root, "a", "evil", &f.game).unwrap();
    assert_eq!(units, [PathBuf::from("saves/Good")]);
    assert!(!f.game.parent().unwrap().join("escaped.txt").exists());
    assert!(!f.game.join("mods/evil.jar").exists());
}

#[test]
fn foreign_archives_are_not_snapshots_and_ids_are_guarded() {
    let f = fixture();
    let folder = directory(&f.root, "a");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("junk.zip"), "not a zip").unwrap();
    assert!(list(&f.root, "a").unwrap().is_empty());
    assert!(matches!(
        restore(&f.root, "a", "junk", &f.game),
        Err(SnapshotError::Zip(_))
    ));
    assert!(matches!(
        restore(&f.root, "a", "../x", &f.game),
        Err(SnapshotError::UnsafeName(_))
    ));
    assert!(matches!(
        delete(&f.root, "a", "ghost"),
        Err(SnapshotError::NotFound(_))
    ));
}

/// All files of the fixture game, to compare before/after.
fn tree(game: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.push((
                    path.strip_prefix(base).unwrap().display().to_string(),
                    fs::read_to_string(&path).unwrap(),
                ));
            }
        }
    }
    walk(game, game, &mut out);
    out
}

fn operations(f: &Fixture) -> Vec<PathBuf> {
    fs::read_dir(Layout::new(&f.root).operations())
        .map(|entries| entries.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default()
}

#[test]
fn a_commit_that_fails_at_any_unit_puts_every_original_back() {
    use std::os::unix::fs::PermissionsExt;
    // Writable saves but read-only game folder: saves/One and saves/Two are
    // replaced, then `config` (an entry of the game folder) cannot be moved.
    // Read-only saves: the very first unit fails.
    for locked in ["game", "saves"] {
        let f = fixture();
        let id = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
        fs::write(f.game.join("saves/One/level.dat"), "changed-one").unwrap();
        fs::write(f.game.join("saves/Two/level.dat"), "changed-two").unwrap();
        fs::write(f.game.join("options.txt"), "fov:110").unwrap();
        let before = tree(&f.game);
        let lock = if locked == "game" {
            f.game.clone()
        } else {
            f.game.join("saves")
        };
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o555)).unwrap();
        let result = restore(&f.root, "a", &id, &f.game);
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            matches!(result, Err(SnapshotError::Io(_))),
            "{locked}: {result:?}"
        );
        assert_eq!(
            tree(&f.game),
            before,
            "{locked}: the game is exactly as it was"
        );
        assert!(operations(&f).is_empty(), "{locked}: nothing left behind");
        // The snapshot is intact and a retry succeeds.
        restore(&f.root, "a", &id, &f.game).unwrap();
        assert_eq!(
            fs::read_to_string(f.game.join("options.txt")).unwrap(),
            "fov:70"
        );
    }
}

#[test]
fn an_archive_that_cannot_be_staged_leaves_the_game_alone() {
    let f = fixture();
    let folder = directory(&f.root, "a");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join("cut.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    let options = SimpleFileOptions::default();
    writer.start_file(MANIFEST, options).unwrap();
    writer
        .write_all(br#"{"label":"","created":1,"scope":{"kind":"full"}}"#)
        .unwrap();
    writer.start_file("saves/One/level.dat", options).unwrap();
    writer.write_all(b"new-one").unwrap();
    writer.finish().unwrap();
    // Truncate the archive so extraction fails after the manifest is readable.
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..bytes.len() - 40]).unwrap();
    let before = tree(&f.game);
    assert!(restore(&f.root, "a", "cut", &f.game).is_err());
    assert_eq!(tree(&f.game), before);
    assert!(operations(&f).is_empty());
}

#[test]
fn a_restore_the_launcher_died_in_is_undone_at_the_next_start() {
    let f = fixture();
    let layout = Layout::new(&f.root);
    let id = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
    fs::write(f.game.join("saves/One/level.dat"), "changed-one").unwrap();
    fs::write(f.game.join("saves/Two/level.dat"), "changed-two").unwrap();
    let before = tree(&f.game);
    // Rehearse a crash after the first unit was replaced and the second's
    // original moved aside but not yet replaced.
    let operation = restore_dir(&f.root, "a", &id);
    let staged = operation.join(NEW);
    fs::create_dir_all(staged.join("saves/One")).unwrap();
    fs::create_dir_all(staged.join("saves/Two")).unwrap();
    fs::write(staged.join("saves/One/level.dat"), "one").unwrap();
    fs::write(staged.join("saves/Two/level.dat"), "two").unwrap();
    fs::create_dir_all(operation.join(OLD).join("saves")).unwrap();
    fs::rename(
        f.game.join("saves/One"),
        operation.join(OLD).join("saves/One"),
    )
    .unwrap();
    fs::rename(staged.join("saves/One"), f.game.join("saves/One")).unwrap();
    fs::rename(
        f.game.join("saves/Two"),
        operation.join(OLD).join("saves/Two"),
    )
    .unwrap();
    fs::write(
        operation.join(RESTORE_JOURNAL),
        serde_json::to_vec(&RestoreJournal {
            schema: 1,
            instance_id: "a".into(),
            snapshot: id.clone(),
            units: vec![PathBuf::from("saves/One"), PathBuf::from("saves/Two")],
        })
        .unwrap(),
    )
    .unwrap();
    // The recovery looks for the game under the layout's profile folder.
    let profile_game = layout.game("a");
    fs::create_dir_all(profile_game.parent().unwrap()).unwrap();
    fs::rename(&f.game, &profile_game).unwrap();
    let found = recover_interrupted(&layout);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].snapshot, id);
    assert!(found[0].outcome.is_ok(), "{:?}", found[0].outcome);
    assert_eq!(tree(&profile_game), before);
    assert!(operations(&f).is_empty());
    // A second pass finds nothing to do.
    assert!(recover_interrupted(&layout).is_empty());
}

#[test]
fn a_folder_without_a_journal_is_garbage_and_a_recreated_target_is_never_overwritten() {
    let f = fixture();
    let layout = Layout::new(&f.root);
    let stale = layout.operations().join("restore-a-old");
    fs::create_dir_all(stale.join(NEW).join("saves/X")).unwrap();
    assert!(recover_interrupted(&layout).is_empty());
    assert!(!stale.exists(), "no journal: the game was never touched");

    let operation = layout.operations().join("restore-a-s");
    fs::create_dir_all(operation.join(NEW).join("saves/One")).unwrap();
    fs::create_dir_all(operation.join(OLD).join("saves/One")).unwrap();
    fs::write(operation.join(OLD).join("saves/One/level.dat"), "original").unwrap();
    fs::write(
        operation.join(RESTORE_JOURNAL),
        serde_json::to_vec(&RestoreJournal {
            schema: 1,
            instance_id: "a".into(),
            snapshot: "s".into(),
            units: vec![PathBuf::from("saves/One")],
        })
        .unwrap(),
    )
    .unwrap();
    let game = layout.game("a");
    fs::create_dir_all(game.join("saves/One")).unwrap();
    fs::write(game.join("saves/One/level.dat"), "made by someone else").unwrap();
    let found = recover_interrupted(&layout);
    assert!(found[0].outcome.is_err());
    assert_eq!(
        fs::read_to_string(game.join("saves/One/level.dat")).unwrap(),
        "made by someone else"
    );
    assert!(
        operation.join(OLD).join("saves/One/level.dat").is_file(),
        "evidence kept"
    );
}

#[test]
fn delete_removes_only_the_named_snapshot() {
    let f = fixture();
    let a = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
    let b = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 2).unwrap();
    delete(&f.root, "a", &a).unwrap();
    let ids: Vec<_> = list(&f.root, "a")
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, [b]);
}
