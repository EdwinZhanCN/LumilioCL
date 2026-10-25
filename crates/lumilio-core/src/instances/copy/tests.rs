use super::*;

fn source() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    for (path, body) in [
        ("mods/a.jar", "mod"),
        ("config/x.toml", "cfg"),
        ("options.txt", "fov"),
        ("saves/W/level.dat", "world"),
        ("logs/latest.log", "log"),
        ("crash-reports/c.txt", "crash"),
    ] {
        let full = game.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, body).unwrap();
    }
    (dir, game)
}

#[test]
fn copies_independent_files_and_leaves_out_volatile_output_and_links() {
    let (dir, game) = source();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path(), game.join("mods/link")).unwrap();
    let target = dir.path().join("copy");
    let copied = copy_game(&game, &target, true, &CancellationToken::new()).unwrap();
    assert_eq!(copied, 4);
    assert_eq!(
        fs::read(target.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(!target.join("logs").exists() && !target.join("crash-reports").exists());
    assert!(!target.join("mods/link").exists());
    // Not hard-linked: editing the copy cannot change the source.
    fs::write(target.join("options.txt"), "changed").unwrap();
    assert_eq!(fs::read(game.join("options.txt")).unwrap(), b"fov");
}

#[test]
fn worlds_are_optional_and_a_missing_source_is_an_empty_copy() {
    let (dir, game) = source();
    let target = dir.path().join("copy");
    copy_game(&game, &target, false, &CancellationToken::new()).unwrap();
    assert!(!target.join("saves").exists() && target.join("mods/a.jar").is_file());
    let empty = dir.path().join("empty");
    assert_eq!(
        copy_game(
            &dir.path().join("nothing"),
            &empty,
            true,
            &CancellationToken::new()
        )
        .unwrap(),
        0
    );
}

#[test]
fn cancelling_stops_the_copy_and_a_linked_game_folder_is_refused() {
    let (dir, game) = source();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        copy_game(&game, &dir.path().join("c1"), true, &cancel),
        Err(CopyError::Cancelled)
    ));
    #[cfg(unix)]
    {
        let link = dir.path().join("linked-game");
        std::os::unix::fs::symlink(&game, &link).unwrap();
        assert!(matches!(
            copy_game(
                &link,
                &dir.path().join("c2"),
                true,
                &CancellationToken::new()
            ),
            Err(CopyError::Io(_))
        ));
    }
}
