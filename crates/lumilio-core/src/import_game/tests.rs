use super::*;

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

#[test]
fn a_prism_instance_is_read_with_its_name_game_and_loader() {
    let dir = tempfile::tempdir().unwrap();
    let instance = dir.path().join("inst");
    write(
        &instance.join("instance.cfg"),
        "InstanceType=OneSix\nname=我的生存\n",
    );
    write(
        &instance.join("mmc-pack.json"),
        r#"{"components":[
            {"uid":"org.lwjgl3","version":"3.3.1"},
            {"uid":"net.minecraft","version":"1.20.1"},
            {"uid":"net.fabricmc.fabric-loader","version":"0.15.7"}],"formatVersion":1}"#,
    );
    write(&instance.join(".minecraft/mods/a.jar"), "mod");
    let found = detect(&instance).unwrap();
    assert_eq!(found.len(), 1);
    let game = &found[0];
    assert_eq!(
        (game.name.as_str(), game.game_version.as_str(), game.loader),
        ("我的生存", "1.20.1", Loader::Fabric)
    );
    assert_eq!(game.loader_version.as_deref(), Some("0.15.7"));
    assert!(game.game_dir.ends_with(".minecraft"));

    // No game folder, or no Minecraft component, is unreadable.
    let bare = dir.path().join("bare");
    write(&bare.join("instance.cfg"), "name=x");
    write(&bare.join("mmc-pack.json"), r#"{"components":[]}"#);
    assert!(matches!(detect(&bare), Err(ImportError::Unreadable(_))));
}

#[test]
fn version_ids_of_the_common_loaders_are_taken_apart() {
    let read = |id: &str, from: Option<&str>| read_version(id, from);
    assert_eq!(
        read("fabric-loader-0.15.7-1.20.1", Some("1.20.1")),
        Some(("1.20.1".into(), Loader::Fabric, Some("0.15.7".into())))
    );
    assert_eq!(
        read("quilt-loader-0.26.0-1.21.1", Some("1.21.1")),
        Some(("1.21.1".into(), Loader::Quilt, Some("0.26.0".into())))
    );
    assert_eq!(
        read("1.20.1-forge-47.2.0", Some("1.20.1")),
        Some(("1.20.1".into(), Loader::Forge, Some("47.2.0".into())))
    );
    assert_eq!(
        read("neoforge-21.1.50", Some("1.21.1")),
        Some(("1.21.1".into(), Loader::NeoForge, Some("21.1.50".into())))
    );
    assert_eq!(
        read("1.21.1", None),
        Some(("1.21.1".into(), Loader::Vanilla, None))
    );
    assert_eq!(
        read("23w31a", None),
        Some(("23w31a".into(), Loader::Vanilla, None))
    );
    assert_eq!(
        read("optifine-thing", Some("1.12.2")),
        None,
        "unnameable mods are not guessed"
    );
}

#[test]
fn a_minecraft_folder_lists_each_readable_version_and_finds_isolated_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(".minecraft");
    write(
        &root.join("versions/1.21.1/1.21.1.json"),
        r#"{"id":"1.21.1"}"#,
    );
    write(
        &root.join("versions/fab/fab.json"),
        r#"{"id":"fabric-loader-0.16.0-1.21.1","inheritsFrom":"1.21.1"}"#,
    );
    write(&root.join("versions/fab/mods/a.jar"), "mod");
    write(&root.join("versions/junk/junk.json"), "not json");
    write(&root.join("saves/W/level.dat"), "world");
    let found = detect(&root).unwrap();
    let names: Vec<_> = found.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["1.21.1", "fab"]);
    assert_eq!(
        found[0].game_dir, root,
        "shared folder when nothing is isolated"
    );
    assert_eq!(found[1].game_dir, root.join("versions/fab"));
    assert_eq!(found[1].loader, Loader::Fabric);
    // The folder that contains .minecraft works too.
    assert_eq!(detect(dir.path()).unwrap().len(), 2);
    assert!(matches!(
        detect(&dir.path().join("nothing")),
        Err(ImportError::Unrecognised)
    ));
}

#[test]
fn copying_takes_the_players_files_and_leaves_the_other_launchers_behind() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(".minecraft");
    for path in [
        "mods/a.jar",
        "saves/W/level.dat",
        "options.txt",
        "libraries/x.jar",
        "assets/y",
        "versions/1.21.1/1.21.1.json",
        "logs/latest.log",
        ".hidden",
        "launcher_profiles.json",
    ] {
        write(&root.join(path), "x");
    }
    let game = FoundGame {
        name: "g".into(),
        game_version: "1.21.1".into(),
        loader: Loader::Vanilla,
        loader_version: None,
        game_dir: root.clone(),
        origin: GameOrigin::MultiMcPrism,
    };
    let into = dir.path().join("into");
    let copied = copy_game_files(&game, &into, &crate::activity::CancellationToken::new()).unwrap();
    assert_eq!(copied, 3);
    for kept in ["mods/a.jar", "saves/W/level.dat", "options.txt"] {
        assert!(into.join(kept).is_file(), "{kept}");
    }
    for left in [
        "libraries",
        "assets",
        "versions",
        "logs",
        ".hidden",
        "launcher_profiles.json",
    ] {
        assert!(!into.join(left).exists(), "{left}");
    }
    assert!(root.join("mods/a.jar").is_file(), "the source is untouched");
}
