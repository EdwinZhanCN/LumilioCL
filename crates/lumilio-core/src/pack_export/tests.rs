use super::*;
use crate::discover::{ReleaseChannel, VersionFile};
use crate::modpack::{extract_overrides, parse_index, read_index};

fn spec(include: &[&str]) -> ExportSpec {
    ExportSpec {
        format: PackFormat::Modrinth,
        name: " My Pack ".into(),
        version: "1.0.0".into(),
        summary: Some("  hello ".into()),
        include: include.iter().map(|s| (*s).to_owned()).collect(),
    }
}

fn game() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    for (path, data) in [
        ("mods/sodium.jar", "jar"),
        ("mods/local.jar", "mine"),
        ("mods/off.jar.disabled", "off"),
        ("config/sodium.json", "{}"),
        ("config/deep/a.toml", "a"),
        ("saves/w/level.dat", "world"),
        ("options.txt", "fov:70"),
    ] {
        let path = game.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
    fs::write(game.join("saves/w/session.lock"), "x").unwrap();
    (dir, game)
}

#[test]
fn only_the_chosen_files_are_collected_and_nothing_escapes() {
    let (_dir, game) = game();
    let files = collect_files(
        &game,
        &["mods".into(), "config/".into(), "options.txt".into()],
    )
    .unwrap();
    assert_eq!(
        files,
        [
            "config/deep/a.toml",
            "config/sodium.json",
            "mods/local.jar",
            "mods/off.jar.disabled",
            "mods/sodium.jar",
            "options.txt",
        ]
    );
    let with_world = collect_files(&game, &["saves".into()]).unwrap();
    assert_eq!(with_world, ["saves/w/level.dat"], "the lock file stays out");
    for bad in ["../outside", "/etc", "mods/../../x"] {
        assert!(matches!(
            collect_files(&game, &[bad.into()]),
            Err(ExportError::UnsafePath(_))
        ));
    }
    assert!(matches!(
        collect_files(&game, &["nothing-here".into()]),
        Err(ExportError::NothingChosen)
    ));
}

#[test]
fn only_switched_on_archives_in_the_content_folders_can_be_linked() {
    let files: Vec<String> = [
        "mods/a.jar",
        "mods/b.jar.disabled",
        "mods/sub/c.jar",
        "resourcepacks/p.zip",
        "shaderpacks/s.zip",
        "config/x.zip",
        "options.txt",
    ]
    .map(String::from)
    .into();
    let linkable: Vec<_> = linkable(&files).into_iter().map(String::as_str).collect();
    assert_eq!(
        linkable,
        ["mods/a.jar", "resourcepacks/p.zip", "shaderpacks/s.zip"]
    );
}

fn version(sha1: &str, url: &str) -> Version {
    Version {
        id: "v".into(),
        project_id: "p".into(),
        name: "n".into(),
        number: "1".into(),
        channel: ReleaseChannel::Release,
        game_versions: Vec::new(),
        loaders: Vec::new(),
        published: String::new(),
        files: vec![VersionFile {
            url: url.into(),
            filename: "f.jar".into(),
            primary: true,
            size: 3,
            sha1: Some(sha1.into()),
        }],
        dependencies: Vec::new(),
        downloads: 0,
        changelog: String::new(),
    }
}

#[test]
fn identified_files_are_linked_only_from_trusted_addresses() {
    let hashed = vec![
        ("mods/a.jar".to_owned(), "aa".to_owned(), "AA".to_owned(), 3),
        ("mods/b.jar".to_owned(), "bb".to_owned(), "BB".to_owned(), 3),
        ("mods/c.jar".to_owned(), "cc".to_owned(), "CC".to_owned(), 3),
    ];
    let identified = BTreeMap::from([
        (
            "aa".to_owned(),
            version("aa", "https://cdn.modrinth.com/data/a.jar"),
        ),
        ("bb".to_owned(), version("bb", "https://evil.example/b.jar")),
        // Modrinth names a file with another hash: not this one.
        (
            "cc".to_owned(),
            version("zz", "https://cdn.modrinth.com/data/c.jar"),
        ),
    ]);
    let linked = link_identified(&hashed, &identified);
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].path, "mods/a.jar");
    assert_eq!(linked[0].url, "https://cdn.modrinth.com/data/a.jar");
}

#[test]
fn the_index_reads_back_through_the_importer() {
    let linked = vec![LinkedFile {
        path: "mods/a.jar".into(),
        sha1: "aa".into(),
        sha512: "AA".into(),
        size: 3,
        url: "https://cdn.modrinth.com/data/a.jar".into(),
    }];
    let json = index_json(
        &spec(&[]),
        "1.21.1",
        Loader::Fabric,
        Some("0.16.0"),
        &linked,
    )
    .unwrap();
    let index = parse_index(&json).unwrap();
    assert_eq!(index.name, "My Pack");
    assert_eq!(index.version, "1.0.0");
    assert_eq!(index.summary.as_deref(), Some("hello"));
    assert_eq!(index.minecraft, "1.21.1");
    assert_eq!(
        (index.loader, index.loader_version.as_deref()),
        (Loader::Fabric, Some("0.16.0"))
    );
    assert_eq!(index.files.len(), 1);
    assert_eq!(
        index.files[0].downloads,
        ["https://cdn.modrinth.com/data/a.jar"]
    );

    let vanilla = index_json(&spec(&[]), "1.21.1", Loader::Vanilla, None, &[]).unwrap();
    assert_eq!(parse_index(&vanilla).unwrap().loader, Loader::Vanilla);
    assert!(matches!(
        index_json(&spec(&[]), "1.21.1", Loader::Forge, None, &[]),
        Err(ExportError::NoLoaderVersion)
    ));
    let mut unnamed = spec(&[]);
    unnamed.name = "  ".into();
    assert!(matches!(
        index_json(&unnamed, "1", Loader::Vanilla, None, &[]),
        Err(ExportError::MissingDetails)
    ));
}

#[test]
fn a_written_pack_restores_its_carried_files_through_the_importer() {
    let (dir, game) = game();
    let carried = vec!["config/sodium.json".to_owned(), "mods/local.jar".to_owned()];
    let index = index_json(&spec(&[]), "1.21.1", Loader::Vanilla, None, &[]).unwrap();
    let pack = dir.path().join("out.mrpack");
    let size = write_pack(&pack, &game, &index, &carried, &|| false).unwrap();
    assert!(size > 0 && !dir.path().join("out.mrpack.part").exists());

    assert_eq!(read_index(&pack).unwrap().name, "My Pack");
    let restored = dir.path().join("restored");
    fs::create_dir_all(&restored).unwrap();
    assert_eq!(extract_overrides(&pack, &restored).unwrap(), 2);
    assert_eq!(fs::read(restored.join("mods/local.jar")).unwrap(), b"mine");
    assert_eq!(
        fs::read(restored.join("config/sodium.json")).unwrap(),
        b"{}"
    );

    // A failure leaves neither a pack nor a half pack.
    let missing = vec!["mods/ghost.jar".to_owned()];
    let broken = dir.path().join("broken.mrpack");
    assert!(write_pack(&broken, &game, &index, &missing, &|| false).is_err());
    assert!(!broken.exists() && !dir.path().join("broken.mrpack.part").exists());
}

#[test]
fn a_cancelled_export_leaves_neither_a_pack_nor_a_half_pack() {
    let (dir, game) = game();
    let index = index_json(&spec(&[]), "1.21.1", Loader::Vanilla, None, &[]).unwrap();
    let pack = dir.path().join("never.mrpack");
    let carried = vec!["config/sodium.json".to_owned(), "mods/local.jar".to_owned()];
    assert!(matches!(
        write_pack(&pack, &game, &index, &carried, &|| true),
        Err(ExportError::Cancelled)
    ));
    assert!(!pack.exists() && !dir.path().join("never.mrpack.part").exists());
}

#[test]
fn a_prism_zip_reads_back_through_the_importer_of_other_launchers() {
    let (dir, game) = game();
    let pack = prism_pack_json("1.21.1", Loader::Fabric, Some("0.16.0")).unwrap();
    let files = vec!["config/sodium.json".to_owned(), "mods/local.jar".to_owned()];
    let zip = dir.path().join("p.zip");
    let size = write_prism(&zip, &game, "My\nPack", &pack, &files, &|| false).unwrap();
    assert!(size > 0 && !dir.path().join("p.zip.part").exists());

    let unpacked = dir.path().join("unpacked");
    let mut archive = zip::ZipArchive::new(fs::File::open(&zip).unwrap()).unwrap();
    archive.extract(&unpacked).unwrap();
    let found = crate::import_game::detect(&unpacked).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "My Pack");
    assert_eq!(
        (found[0].game_version.as_str(), found[0].loader),
        ("1.21.1", Loader::Fabric)
    );
    assert_eq!(found[0].loader_version.as_deref(), Some("0.16.0"));
    assert_eq!(
        fs::read(unpacked.join(".minecraft/mods/local.jar")).unwrap(),
        b"mine"
    );

    assert!(matches!(
        prism_pack_json("1.21.1", Loader::Forge, None),
        Err(ExportError::NoLoaderVersion)
    ));
    assert!(matches!(
        write_prism(
            &dir.path().join("c.zip"),
            &game,
            "x",
            &pack,
            &files,
            &|| true
        ),
        Err(ExportError::Cancelled)
    ));
    assert!(!dir.path().join("c.zip").exists() && !dir.path().join("c.zip.part").exists());
}

#[test]
fn hashes_match_known_digests() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a");
    fs::write(&file, b"abc").unwrap();
    assert_eq!(
        sha512_hex(&file).unwrap(),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );
}
