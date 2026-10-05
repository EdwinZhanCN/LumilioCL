use super::*;
use std::io::{Read, Write};

fn jar(dir: &Path, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
    let path = dir.join("client.jar");
    let mut writer = ZipWriter::new(File::create(&path).unwrap());
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    path
}

fn names(pack: &ResourcePack) -> Vec<String> {
    let mut archive = ZipArchive::new(Cursor::new(&pack.bytes)).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|index| archive.by_index(index).unwrap().name().to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn only_what_a_renderer_draws_blocks_with_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = jar(
        dir.path(),
        &[
            ("pack.mcmeta", b"{}"),
            ("net/minecraft/Main.class", b"code"),
            ("data/minecraft/loot_table/a.json", b"{}"),
            ("assets/minecraft/blockstates/stone.json", b"{}"),
            ("assets/minecraft/models/block/stone.json", b"{}"),
            ("assets/minecraft/textures/block/stone.png", b"png"),
            ("assets/minecraft/atlases/blocks.json", b"{}"),
            ("assets/minecraft/items/apple.json", b"{}"),
            ("assets/minecraft/lang/en_us.json", b"{}"),
            ("assets/minecraft/sounds/a.ogg", b"ogg"),
            ("assets/realms/textures/x.png", b"png"),
        ],
    );
    let pack = build_pack(&path, "1.21.11").unwrap();
    assert_eq!(
        names(&pack),
        [
            "assets/minecraft/atlases/blocks.json",
            "assets/minecraft/blockstates/stone.json",
            "assets/minecraft/items/apple.json",
            "assets/minecraft/models/block/stone.json",
            "assets/minecraft/textures/block/stone.png",
            "pack.mcmeta",
        ]
    );
    let mut archive = ZipArchive::new(Cursor::new(&pack.bytes)).unwrap();
    let mut texture = Vec::new();
    archive
        .by_name("assets/minecraft/textures/block/stone.png")
        .unwrap()
        .read_to_end(&mut texture)
        .unwrap();
    assert_eq!(texture, b"png");
    assert_eq!(
        archive.by_name("pack.mcmeta").unwrap().compression(),
        CompressionMethod::Stored
    );
}

#[test]
fn the_id_names_the_version_and_the_jar_and_is_safe_in_a_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = jar(
        dir.path(),
        &[("assets/minecraft/textures/block/a.png", b"png")],
    );
    let id = build_pack(&path, "26.2 /snap?").unwrap().id;
    assert!(id.starts_with("jar-26.2__snap_-"), "{id}");
    assert!(
        id.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')),
        "{id}"
    );
    // A different jar for the same version is a different pack.
    std::fs::write(&path, b"").unwrap();
    let other = jar(
        dir.path(),
        &[
            ("assets/minecraft/textures/block/a.png", b"png"),
            ("assets/minecraft/textures/block/b.png", b"png2"),
        ],
    );
    assert_ne!(build_pack(&other, "26.2 /snap?").unwrap().id, id);
}

#[test]
fn a_jar_without_textures_or_a_broken_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let no_textures = jar(dir.path(), &[("net/minecraft/Main.class", b"code")]);
    assert!(matches!(
        build_pack(&no_textures, "1.0"),
        Err(ModelAssetsError::NotAClient)
    ));
    let broken = dir.path().join("broken.jar");
    std::fs::write(&broken, b"not a zip").unwrap();
    assert!(matches!(
        build_pack(&broken, "1.0"),
        Err(ModelAssetsError::Zip(_))
    ));
    assert!(matches!(
        build_pack(&dir.path().join("missing.jar"), "1.0"),
        Err(ModelAssetsError::Io(_))
    ));
}

#[test]
fn names_that_climb_out_are_not_kept() {
    assert!(!kept("assets/minecraft/textures/../../secret"));
    assert!(!kept("assets/minecraft/other/a"));
    assert!(kept("assets/minecraft/textures/block/a.png"));
}
