use super::*;

#[test]
fn importing_copies_once_and_never_replaces_other_content() {
    let game = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let jar = outside.path().join("sodium.jar");
    fs::write(&jar, b"mod").unwrap();
    assert_eq!(
        import(game.path(), ProjectKind::Mod, &jar).unwrap(),
        "sodium.jar"
    );
    assert_eq!(
        fs::read(game.path().join("mods/sodium.jar")).unwrap(),
        b"mod"
    );
    // The same content again is fine; other content under that name is not.
    assert!(import(game.path(), ProjectKind::Mod, &jar).is_ok());
    fs::write(&jar, b"other").unwrap();
    assert!(matches!(
        import(game.path(), ProjectKind::Mod, &jar),
        Err(ContentError::Conflict(_))
    ));
    assert_eq!(
        fs::read(game.path().join("mods/sodium.jar")).unwrap(),
        b"mod"
    );
    // A disabled copy blocks the name too.
    let other = outside.path().join("lithium.jar");
    fs::write(&other, b"l").unwrap();
    fs::write(game.path().join("mods/lithium.jar.disabled"), b"l").unwrap();
    assert!(matches!(
        import(game.path(), ProjectKind::Mod, &other),
        Err(ContentError::Conflict(_))
    ));
    // Wrong type for the kind.
    let pack = outside.path().join("pack.zip");
    fs::write(&pack, b"zip").unwrap();
    assert!(matches!(
        import(game.path(), ProjectKind::Mod, &pack),
        Err(ContentError::WrongType(_))
    ));
    assert_eq!(
        import(game.path(), ProjectKind::ResourcePack, &pack).unwrap(),
        "pack.zip"
    );
    let leftovers: Vec<_> = fs::read_dir(game.path().join("mods"))
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty());
}
use std::io::Write;
use zip::write::SimpleFileOptions;

fn write_zip(path: &Path, entries: &[(&str, &str)]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, body) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

fn game() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(game.join("mods")).unwrap();
    (dir, game)
}

#[test]
fn scans_mods_with_state_sizes_and_stable_order() {
    let (_dir, game) = game();
    fs::write(game.join("mods/b.jar"), "12345").unwrap();
    fs::write(game.join("mods/A.jar.disabled"), "1").unwrap();
    fs::write(game.join("mods/readme.txt"), "x").unwrap();
    fs::write(game.join("mods/.DS_Store"), "x").unwrap();
    fs::create_dir(game.join("mods/some-folder")).unwrap();
    let items = scan(&game, ProjectKind::Mod).unwrap();
    let names: Vec<_> = items
        .iter()
        .map(|i| (i.display_name.as_str(), i.enabled))
        .collect();
    assert_eq!(names, [("A.jar", false), ("b.jar", true)]);
    assert_eq!(items[1].size, 5);
    assert_eq!(items[0].file_name, "A.jar.disabled");
}

#[test]
fn resource_packs_may_be_folders_and_a_missing_folder_is_empty() {
    let (_dir, game) = game();
    assert!(scan(&game, ProjectKind::ResourcePack).unwrap().is_empty());
    fs::create_dir_all(game.join("resourcepacks/folder-pack")).unwrap();
    fs::write(game.join("resourcepacks/pack.zip"), "z").unwrap();
    fs::write(game.join("resourcepacks/pack.rar"), "z").unwrap();
    let items = scan(&game, ProjectKind::ResourcePack).unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .any(|i| i.is_directory && i.display_name == "folder-pack")
    );
    assert!(matches!(
        scan(&game, ProjectKind::Modpack),
        Err(ContentError::NotAFileKind)
    ));
}

#[test]
fn disable_and_enable_rename_and_are_idempotent() {
    let (_dir, game) = game();
    fs::write(game.join("mods/a.jar"), "x").unwrap();
    assert_eq!(
        set_enabled(&game, ProjectKind::Mod, "a.jar", false).unwrap(),
        "a.jar.disabled"
    );
    assert!(game.join("mods/a.jar.disabled").is_file());
    assert_eq!(
        set_enabled(&game, ProjectKind::Mod, "a.jar.disabled", false).unwrap(),
        "a.jar.disabled"
    );
    assert_eq!(
        set_enabled(&game, ProjectKind::Mod, "a.jar.disabled", true).unwrap(),
        "a.jar"
    );
    assert!(game.join("mods/a.jar").is_file());
}

#[test]
fn enabling_refuses_to_overwrite_a_twin() {
    let (_dir, game) = game();
    fs::write(game.join("mods/a.jar"), "new").unwrap();
    fs::write(game.join("mods/a.jar.disabled"), "old").unwrap();
    assert!(matches!(
        set_enabled(&game, ProjectKind::Mod, "a.jar.disabled", true),
        Err(ContentError::Conflict(_))
    ));
    assert_eq!(fs::read_to_string(game.join("mods/a.jar")).unwrap(), "new");
}

#[test]
fn removal_deletes_files_and_folders_and_rejects_bad_names() {
    let (_dir, game) = game();
    fs::write(game.join("mods/a.jar"), "x").unwrap();
    fs::write(game.join("outside.jar"), "keep").unwrap();
    remove(&game, ProjectKind::Mod, "a.jar").unwrap();
    assert!(!game.join("mods/a.jar").exists());
    assert!(matches!(
        remove(&game, ProjectKind::Mod, "a.jar"),
        Err(ContentError::NotFound(_))
    ));
    for name in ["../outside.jar", "a/b.jar", ".."] {
        assert!(matches!(
            remove(&game, ProjectKind::Mod, name),
            Err(ContentError::UnsafeFileName(_))
        ));
    }
    assert!(game.join("outside.jar").is_file());
    fs::create_dir_all(game.join("shaderpacks/dir/inner")).unwrap();
    remove(&game, ProjectKind::Shader, "dir").unwrap();
    assert!(!game.join("shaderpacks/dir").exists());
}

#[test]
fn a_linked_kind_folder_is_refused_and_a_linked_file_is_only_itself() {
    let (dir, game) = game();
    let outside = dir.path().join("elsewhere");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("victim.jar"), "keep").unwrap();
    std::os::unix::fs::symlink(&outside, game.join("resourcepacks")).unwrap();
    for result in [
        scan(&game, ProjectKind::ResourcePack).map(|_| ()),
        remove(&game, ProjectKind::ResourcePack, "victim.jar"),
        set_enabled(&game, ProjectKind::ResourcePack, "victim.jar", false).map(|_| ()),
    ] {
        assert!(matches!(result, Err(ContentError::EscapesInstance(_))));
    }
    assert_eq!(fs::read(outside.join("victim.jar")).unwrap(), b"keep");
    // A link inside a legitimate folder is just a name: acting on it never
    // reaches through to the target.
    std::os::unix::fs::symlink(outside.join("victim.jar"), game.join("mods/link.jar")).unwrap();
    remove(&game, ProjectKind::Mod, "link.jar").unwrap();
    assert_eq!(fs::read(outside.join("victim.jar")).unwrap(), b"keep");
}

#[test]
fn sha1_matches_a_known_digest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    fs::write(&path, "abc").unwrap();
    assert_eq!(
        sha1_hex(&path).unwrap(),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn reads_metadata_for_each_loader_family() {
    let dir = tempfile::tempdir().unwrap();
    let fabric = dir.path().join("f.jar");
    write_zip(
        &fabric,
        &[(
            "fabric.mod.json",
            r#"{"id":"sodium","name":"Sodium","version":"0.6.0"}"#,
        )],
    );
    assert_eq!(
        read_mod_metadata(&fabric).unwrap(),
        ModMetadata {
            id: "sodium".into(),
            name: Some("Sodium".into()),
            version: Some("0.6.0".into()),
            loader: "fabric"
        }
    );

    let quilt = dir.path().join("q.jar");
    write_zip(
        &quilt,
        &[(
            "quilt.mod.json",
            r#"{"quilt_loader":{"id":"qm","version":"1.0","metadata":{"name":"Q"}}}"#,
        )],
    );
    let meta = read_mod_metadata(&quilt).unwrap();
    assert_eq!(
        (meta.id.as_str(), meta.loader, meta.name.as_deref()),
        ("qm", "quilt", Some("Q"))
    );

    let forge = dir.path().join("g.jar");
    write_zip(
        &forge,
        &[(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\n[[mods]]\nmodId=\"jei\"\ndisplayName=\"JEI\"\nversion=\"${file.jarVersion}\"\n",
        )],
    );
    let meta = read_mod_metadata(&forge).unwrap();
    assert_eq!((meta.id.as_str(), meta.loader), ("jei", "forge"));
    assert_eq!(meta.version, None, "a build placeholder is not a version");

    let neo = dir.path().join("n.jar");
    write_zip(
        &neo,
        &[(
            "META-INF/neoforge.mods.toml",
            "[[mods]]\nmodId=\"nm\"\nversion=\"2.0\"\n",
        )],
    );
    assert_eq!(read_mod_metadata(&neo).unwrap().loader, "neoforge");
}

#[test]
fn unreadable_or_anonymous_archives_have_no_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.jar");
    fs::write(&bad, "not a zip").unwrap();
    assert!(read_mod_metadata(&bad).is_none());
    let empty = dir.path().join("e.jar");
    write_zip(&empty, &[("readme.txt", "hi")]);
    assert!(read_mod_metadata(&empty).is_none());
    assert!(read_mod_metadata(&dir.path().join("missing.jar")).is_none());
    let no_id = dir.path().join("n.jar");
    write_zip(&no_id, &[("fabric.mod.json", r#"{"name":"x"}"#)]);
    assert!(read_mod_metadata(&no_id).is_none());
}
