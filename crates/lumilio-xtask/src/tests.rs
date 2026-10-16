use std::path::{Path, PathBuf};

use super::*;
use crate::release::{APP_ID, BINARY};

fn release(version: &str) -> Release {
    Release {
        version: Version::parse(version).unwrap(),
        commit: "0123abc".to_owned(),
        arch: "x64",
        root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        target: PathBuf::from("target"),
        dist: PathBuf::from("dist"),
    }
}

fn repo_file(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|error| panic!("{}: {error}", full.display()))
}

#[test]
fn versions_follow_semver() {
    let version = Version::parse("1.2.3-beta.1+exp.sha.5114f85").unwrap();
    assert_eq!((version.major, version.minor, version.patch), (1, 2, 3));
    assert_eq!(version.pre.as_deref(), Some("beta.1"));
    assert_eq!(version.build.as_deref(), Some("exp.sha.5114f85"));
    assert_eq!(version.to_string(), "1.2.3-beta.1+exp.sha.5114f85");
    assert!(version.is_prerelease());
    assert!(!Version::parse("0.1.0").unwrap().is_prerelease());
    for bad in [
        "1.2",
        "1.2.3.4",
        "01.2.3",
        "1.2.3-",
        "1.2.3-01",
        "1.2.3-a..b",
        "v1.2.3",
    ] {
        assert!(Version::parse(bad).is_err(), "{bad} was accepted");
    }
    assert!(Version::parse(env!("CARGO_PKG_VERSION")).is_ok());
}

#[test]
fn platform_versions_derive_from_the_one_version() {
    let pre = Version::parse("0.2.0-beta.1").unwrap();
    assert_eq!(pre.numeric(), "0.2.0");
    assert_eq!(pre.windows(), "0.2.0.0");
    // `~` sorts before the release, so the beta upgrades to 0.2.0.
    assert_eq!(pre.debian(), "0.2.0~beta.1");
    assert_eq!(Version::parse("0.2.0").unwrap().debian(), "0.2.0");
}

#[test]
fn a_release_tag_must_name_the_workspace_version() {
    let version = Version::parse("0.1.0").unwrap();
    assert_eq!(
        release_outputs(&version, Some("v0.1.0")).unwrap(),
        "version=0.1.0\nprerelease=false"
    );
    assert_eq!(
        release_outputs(&version, None).unwrap(),
        "version=0.1.0\nprerelease=false"
    );
    assert!(release_outputs(&version, Some("v0.1.1")).is_err());
    assert!(release_outputs(&version, Some("0.1.0")).is_err());
    let pre = Version::parse("0.2.0-rc.1").unwrap();
    assert!(
        release_outputs(&pre, Some("v0.2.0-rc.1"))
            .unwrap()
            .ends_with("prerelease=true")
    );
}

#[test]
fn artifact_names_carry_version_os_and_arch() {
    assert_eq!(
        release("0.1.0").stem("windows"),
        "LumilioCL-0.1.0-windows-x64"
    );
}

#[test]
fn package_notices_include_cubiomes_and_nucleation_mit_text() {
    let dir = tempfile::tempdir().unwrap();
    release("0.1.0").notices(dir.path()).unwrap();
    let copied = std::fs::read_to_string(dir.path().join("ATTRIBUTIONS.md")).unwrap();
    assert_eq!(copied, repo_file("ATTRIBUTIONS.md"));
    assert!(copied.contains("Copyright (c) 2020 Cubitect"));
    assert!(copied.contains("Nucleation"));
    assert!(copied.contains("Permission is hereby granted"));
}

#[test]
fn info_plist_is_complete_and_names_the_icon_both_ways() {
    let plist = macos::info_plist(&release("0.3.0-rc.2")).unwrap();
    assert!(!plist.contains("{{"));
    // macOS 26 reads the Assets.car icon by name; older systems the .icns file.
    for key in ["CFBundleIconName", "CFBundleIconFile"] {
        assert!(
            plist.contains(&format!("<key>{key}</key>\n\t<string>AppIcon</string>")),
            "{key}"
        );
    }
    assert!(plist.contains(&format!("<string>{APP_ID}</string>")));
    assert!(plist.contains(&format!("<string>{BINARY}</string>")));
    assert!(plist.contains("<key>CFBundleVersion</key>\n\t<string>0.3.0</string>"));
    assert!(plist.contains("<string>0.3.0-rc.2</string>"));
    assert!(plist.contains("<string>0123abc</string>"));
}

#[test]
fn fill_refuses_a_placeholder_left_behind() {
    assert_eq!(release::fill("a {{X}} b", &[("X", "1")]).unwrap(), "a 1 b");
    assert!(release::fill("a {{X}} {{Y}}", &[("X", "1")]).is_err());
}

#[test]
fn debian_control_is_well_formed() {
    let control = linux::control(&release("0.2.0-beta.1"), "amd64", "libc6 (>= 2.34)", 4096);
    assert!(control.contains("Version: 0.2.0~beta.1\n"));
    assert!(control.contains("Installed-Size: 4096\n"));
    assert!(control.ends_with('\n'));
    let mut lines = control.lines();
    let description = lines.find(|line| line.starts_with("Description:")).unwrap();
    assert!(description.len() > "Description: ".len());
    // Every line after the synopsis continues the description.
    for line in lines {
        assert!(line.starts_with(' ') && line.trim().len() > 1, "{line:?}");
    }
}

/// The same identity is written in several files that cannot share a constant;
/// a mismatch would leave Linux windows and Windows taskbar entries without
/// their icon.
#[test]
fn every_platform_file_uses_the_same_identity() {
    // macOS reads it from Info.plist, written from APP_ID here; the app sets it
    // for Windows and Wayland itself.
    let main = repo_file("crates/lumilio-app/src/main.rs");
    assert!(main.contains(&format!("\"{APP_ID}\"")));
    assert!(main.contains(&format!("\"{APP_ID}.dev\"")));

    let desktop = repo_file(&format!("assets/icons/linux/{APP_ID}.desktop"));
    assert!(desktop.contains(&format!("\nIcon={APP_ID}\n")));
    assert!(desktop.contains(&format!("\nStartupWMClass={APP_ID}\n")));
    // install.sh rewrites Exec; the .deb installs exactly here.
    assert!(desktop.contains(&format!("\nExec=/usr/lib/lumiliocl/{BINARY}\n")));
    for size in ["16x16", "32x32", "48x48", "256x256", "512x512"] {
        let icon = format!("assets/icons/linux/hicolor/{size}/apps/{APP_ID}.png");
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(&icon)
                .is_file(),
            "{icon}"
        );
    }

    let install = repo_file("crates/lumilio-xtask/packaging/install.sh");
    assert!(install.contains(&format!("app_id={APP_ID}\n")));

    // Windows knows an installed LumilioCL by this; a new one would install a
    // second copy beside the first instead of upgrading it.
    let inno = repo_file("crates/lumilio-xtask/packaging/LumilioCL.iss");
    assert!(inno.contains("\nAppId={{6927ECC0-2947-4CDB-8209-9B6142815660}\n"));

    let app_manifest = repo_file("crates/lumilio-app/Cargo.toml");
    assert!(app_manifest.contains(&format!("name = \"{BINARY}\"")));
}

#[test]
fn checksums_use_the_sha256sum_format() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("LumilioCL-0.1.0-linux-x64.tar.gz");
    std::fs::write(&artifact, b"abc").unwrap();
    let sums = release::write_checksum(&artifact).unwrap();
    assert_eq!(
        sums,
        dir.path().join("LumilioCL-0.1.0-linux-x64.tar.gz.sha256")
    );
    assert_eq!(
        std::fs::read_to_string(sums).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  LumilioCL-0.1.0-linux-x64.tar.gz\n"
    );
}

#[test]
fn the_portable_zip_unpacks_into_one_folder() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload");
    std::fs::create_dir_all(payload.join("sub")).unwrap();
    std::fs::write(payload.join("lumiliocl.exe"), b"exe").unwrap();
    std::fs::write(payload.join("sub/notes.txt"), b"notes").unwrap();
    let archive = dir.path().join("out.zip");
    release("0.1.0").notices(&payload).unwrap();
    windows::zip_folder(&payload, "LumilioCL", &archive).unwrap();
    let zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
    let mut names: Vec<_> = zip.file_names().map(|name| name.unwrap()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "LumilioCL/ATTRIBUTIONS.md",
            "LumilioCL/lumiliocl.exe",
            "LumilioCL/sub/notes.txt"
        ]
    );
}

#[test]
fn inno_paths_strip_windows_verbatim_prefixes() {
    assert_eq!(
        windows::inno_path(Path::new(r"\\?\D:\a\LumilioCL\target\package")),
        r"D:\a\LumilioCL\target\package"
    );
    assert_eq!(
        windows::inno_path(Path::new(r"\\?\UNC\server\share\LumilioCL")),
        r"\\server\share\LumilioCL"
    );
    assert_eq!(
        windows::inno_path(Path::new(r"D:\a\LumilioCL\target\package")),
        r"D:\a\LumilioCL\target\package"
    );
}

mod block_colors {
    use crate::block_colors::*;
    use std::io::{Cursor, Write};

    fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_fn(width, height, |x, y| image::Rgba(pixel(x, y)));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn jar(files: &[(&str, Vec<u8>)]) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in files {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        zip::ZipArchive::new(Cursor::new(writer.finish().unwrap().into_inner())).unwrap()
    }

    fn text(value: &str) -> Vec<u8> {
        value.as_bytes().to_vec()
    }

    fn sample() -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let red = |_, _| [200, 0, 0, 255];
        jar(&[
            // A cube whose model names top and side textures through a parent.
            (
                "assets/minecraft/blockstates/stone.json",
                text(r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#),
            ),
            (
                "assets/minecraft/models/block/stone.json",
                text(
                    r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##,
                ),
            ),
            (
                "assets/minecraft/models/block/cube_all.json",
                text(r##"{"parent":"block/cube","textures":{"particle":"#all","down":"#all"}}"##),
            ),
            (
                "assets/minecraft/models/block/cube.json",
                text(r##"{"textures":{"particle":"#missing"}}"##),
            ),
            (
                "assets/minecraft/textures/block/stone.png",
                png(16, 16, |_, _| [100, 120, 140, 255]),
            ),
            // Top beats side; the child's texture beats the parent's.
            (
                "assets/minecraft/blockstates/grass_block.json",
                text(
                    r#"{"variants":{"snowy=false":[{"model":"block/grass_block"},{"model":"block/other"}]}}"#,
                ),
            ),
            (
                "assets/minecraft/models/block/grass_block.json",
                text(
                    r##"{"parent":"block/cube_bottom_top","textures":{"top":"block/grass_block_top","side":"block/grass_block_side"}}"##,
                ),
            ),
            (
                "assets/minecraft/models/block/cube_bottom_top.json",
                text(r##"{"textures":{"top":"block/wrong","bottom":"block/dirt"}}"##),
            ),
            (
                "assets/minecraft/textures/block/grass_block_top.png",
                png(16, 16, |_, _| [150, 150, 150, 255]),
            ),
            (
                "assets/minecraft/textures/block/grass_block_side.png",
                png(16, 16, red),
            ),
            // Multipart blocks use their first applied model.
            (
                "assets/minecraft/blockstates/oak_fence.json",
                text(
                    r#"{"multipart":[{"apply":{"model":"block/fence_post"}},{"when":{"north":"true"},"apply":{"model":"block/fence_side"}}]}"#,
                ),
            ),
            (
                "assets/minecraft/models/block/fence_post.json",
                text(r#"{"textures":{"texture":"block/planks"}}"#),
            ),
            (
                "assets/minecraft/textures/block/planks.png",
                png(16, 16, |_, _| [160, 130, 80, 255]),
            ),
            // Mostly transparent textures are see-through; animation strips use frame one.
            (
                "assets/minecraft/blockstates/short_grass.json",
                text(r#"{"variants":{"":{"model":"block/short_grass"}}}"#),
            ),
            (
                "assets/minecraft/models/block/short_grass.json",
                text(r#"{"textures":{"cross":"block/short_grass"}}"#),
            ),
            (
                "assets/minecraft/textures/block/short_grass.png",
                png(16, 16, |x, y| {
                    if x == 8 && y < 8 {
                        [0, 200, 0, 255]
                    } else {
                        [0, 0, 0, 0]
                    }
                }),
            ),
            (
                "assets/minecraft/blockstates/magma_block.json",
                text(r#"{"variants":{"":{"model":"block/magma"}}}"#),
            ),
            (
                "assets/minecraft/models/block/magma.json",
                text(r#"{"textures":{"all":"block/magma"}}"#),
            ),
            (
                "assets/minecraft/textures/block/magma.png",
                png(16, 48, |_, y| {
                    if y < 16 {
                        [255, 0, 0, 255]
                    } else {
                        [0, 0, 255, 255]
                    }
                }),
            ),
            (
                "assets/minecraft/blockstates/oak_leaves.json",
                text(r#"{"variants":{"":{"model":"block/leaves"}}}"#),
            ),
            (
                "assets/minecraft/models/block/leaves.json",
                text(r#"{"textures":{"all":"block/leaves"}}"#),
            ),
            (
                "assets/minecraft/textures/block/leaves.png",
                png(16, 16, |_, _| [90, 90, 90, 255]),
            ),
            // Broken or missing pieces just leave the block out.
            (
                "assets/minecraft/blockstates/ghost.json",
                text(r#"{"variants":{"":{"model":"block/nowhere"}}}"#),
            ),
            ("assets/minecraft/blockstates/bad.json", text("{ not json")),
            ("assets/minecraft/blockstates/empty.json", text("{}")),
            // Fluids have no blockstate; their textures are looked up by name.
            (
                "assets/minecraft/textures/block/water_still.png",
                png(16, 32, |_, _| [60, 90, 200, 255]),
            ),
            (
                "assets/minecraft/textures/block/lava_still.png",
                png(16, 16, |_, _| [220, 100, 20, 255]),
            ),
            // A fixed tint is multiplied in rather than flagged.
            (
                "assets/minecraft/blockstates/birch_leaves.json",
                text(r#"{"variants":{"":{"model":"block/leaves"}}}"#),
            ),
            (
                "version.json",
                text(r#"{"id":"1.21.4","world_version":4189}"#),
            ),
            // Colormaps: red grows across (temperature), green down (downfall).
            (
                "assets/minecraft/textures/colormap/grass.png",
                png(256, 256, |x, y| [x as u8, y as u8, 0, 255]),
            ),
            (
                "assets/minecraft/textures/colormap/foliage.png",
                png(256, 256, |x, y| [0, x as u8, y as u8, 255]),
            ),
            (
                "data/minecraft/worldgen/biome/plains.json",
                text(r#"{"temperature":0.8,"downfall":0.4,"effects":{"water_color":4159204}}"#),
            ),
            // 26.x writes colours as strings.
            (
                "data/minecraft/worldgen/biome/badlands.json",
                text(
                    r##"{"temperature":2.0,"downfall":0.0,"effects":{"grass_color":"#90814d","foliage_color":"#9e814d","water_color":"#3f76e4"}}"##,
                ),
            ),
            (
                "data/minecraft/worldgen/biome/swamp.json",
                text(
                    r#"{"temperature":0.8,"downfall":0.9,"effects":{"grass_color_modifier":"swamp","water_color":6388580}}"#,
                ),
            ),
            (
                "data/minecraft/worldgen/biome/dark_forest.json",
                text(
                    r#"{"temperature":0.7,"downfall":0.8,"effects":{"grass_color_modifier":"dark_forest"}}"#,
                ),
            ),
            (
                "data/minecraft/worldgen/biome/broken.json",
                text(r#"{"effects":{}}"#),
            ),
        ])
    }

    #[test]
    fn biomes_take_their_own_colours_or_their_colormap_spot() {
        let biomes = build_biomes(&mut sample());
        // Plains: temperature 0.8 → x (1 - 0.8) × 255, which truncates to 50
        // in floating point as it does in the game; downfall 0.4 × 0.8 → y 173.
        assert_eq!(
            biomes["minecraft:plains"],
            [50 << 16 | 173 << 8, 50 << 8 | 173, 0x3f76e4]
        );
        assert_eq!(biomes["minecraft:badlands"], [0x90814d, 0x9e814d, 0x3f76e4]);
        assert_eq!(biomes["minecraft:swamp"][0], 0x6a7039);
        assert_eq!(biomes["minecraft:swamp"][2], 6388580);
        // Dark forest: x 76, y 112 → 0x4c7000, darkened.
        assert_eq!(
            biomes["minecraft:dark_forest"][0],
            ((0x4c7000 & 0xfefefe) + 0x28340a) >> 1
        );
        assert_eq!(
            biomes["minecraft:dark_forest"][2], 0x3f76e4,
            "default water"
        );
        assert!(!biomes.contains_key("minecraft:broken"));
        assert!(
            build_biomes(&mut jar(&[])).is_empty(),
            "no colormaps, no biomes"
        );
    }

    #[test]
    fn blocks_get_the_mean_colour_of_the_texture_they_show_from_above() {
        let table = build_table(&mut sample());
        assert_eq!(table["minecraft:stone"], [100, 120, 140, 0]);
        assert_eq!(
            table["minecraft:grass_block"],
            [150, 150, 150, TINT_GRASS],
            "top, tinted"
        );
        assert_eq!(table["minecraft:oak_fence"], [160, 130, 80, 0]);
        assert_eq!(
            table["minecraft:magma_block"],
            [255, 0, 0, 0],
            "first frame of the strip"
        );
        assert_eq!(table["minecraft:oak_leaves"], [90, 90, 90, TINT_FOLIAGE]);
        assert_eq!(
            table["minecraft:birch_leaves"],
            [45, 58, 30, 0],
            "90 × the birch colour 0x80a755"
        );
        assert_eq!(table["minecraft:water"], [60, 90, 200, TINT_WATER]);
        assert_eq!(table["minecraft:lava"], [220, 100, 20, 0]);
        let grass = table["minecraft:short_grass"];
        assert_eq!(
            grass[3] & SEE_THROUGH,
            SEE_THROUGH,
            "a few pixels of a plant are not a surface"
        );
        assert_eq!(
            grass[..3],
            [0, 200, 0],
            "the colour is of the visible pixels only"
        );
        for left_out in ["minecraft:ghost", "minecraft:bad", "minecraft:empty"] {
            assert!(!table.contains_key(left_out), "{left_out}");
        }
        assert_eq!(table.len(), 9);
        assert_eq!(data_version(&mut sample()), Some(4189));
        assert_eq!(data_version(&mut jar(&[])), None);
    }

    #[test]
    fn the_table_renders_the_same_way_every_time() {
        let table = build_table(&mut sample());
        let biomes = build_biomes(&mut sample());
        let first = render(&table, &biomes, "1.21.4", 4189);
        assert_eq!(
            first,
            render(&build_table(&mut sample()), &biomes, "1.21.4", 4189)
        );
        assert!(first.contains("    \"minecraft:stone\": [100, 120, 140, 0],\n"));
        let parsed: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(parsed["blocks"]["minecraft:water"][3], TINT_WATER);
        assert_eq!(parsed["version"], "1.21.4");
        assert_eq!(parsed["data_version"], 4189);
        assert_eq!(parsed["biomes"]["minecraft:swamp"][0], 0x6a7039);
        assert!(first.ends_with("  }\n}\n"));
        assert!(!first.contains(",\n  }"), "no trailing comma");
        let blocks = first.split("\"biomes\"").next().unwrap();
        let names: Vec<&str> = blocks
            .lines()
            .filter_map(|line| line.trim().split('"').nth(1))
            .filter(|n| n.starts_with("minecraft:"))
            .collect();
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]), "sorted");
    }

    #[test]
    fn source_notes_replace_their_own_version_and_keep_the_others() {
        let first = source_section("", "1.21.4", &"a".repeat(40), "c0ffee", 900);
        assert!(first.starts_with("# Block colour tables"));
        assert!(
            first.contains("`cargo xtask block-colors 1.21.4`") && first.contains(&"a".repeat(40))
        );
        let both = source_section(&first, "26.3", &"b".repeat(40), "decade", 1000);
        assert!(both.contains("## 1.21.4") && both.contains("## 26.3"));
        let again = source_section(&both, "1.21.4", &"d".repeat(40), "feed", 901);
        assert_eq!(again.matches("## 1.21.4").count(), 1);
        assert!(again.contains(&"d".repeat(40)) && !again.contains(&"a".repeat(40)));
        assert!(
            again.contains("## 26.3") && again.contains(&"b".repeat(40)),
            "the other version is kept"
        );
        assert!(again.contains("Blocks: 901") && again.contains("Blocks: 1000"));
    }
}

mod bake_blocks {
    use std::collections::BTreeMap;

    use crate::bake_blocks::*;
    use crate::entity_dump::EntityBake;
    use lumilio_block_assets::{BUNDLE_FORMAT, BakedTexture, ContentHash};
    use serde_json::{Value, json};

    fn entry(id: &str, kind: &str, time: &str) -> Value {
        json!({
            "id": id,
            "type": kind,
            "url": format!("https://example.invalid/{id}.json"),
            "sha1": format!("sha1-{id}"),
            "releaseTime": time,
        })
    }

    #[test]
    fn releases_run_from_1_13_2_to_the_newest_release_only() {
        // Mojang lists newest first; an old release sorted wrong must not slip in.
        let manifest = json!({ "versions": [
            entry("26.3", "release", "2026-09-01T10:00:00+00:00"),
            entry("26.3-pre1", "snapshot", "2026-08-20T10:00:00+00:00"),
            entry("1.14", "release", "2019-04-23T14:52:44+00:00"),
            entry("19w14a", "snapshot", "2019-04-03T13:40:44+00:00"),
            entry("1.13.2", "release", "2018-10-22T11:41:07+00:00"),
            entry("1.13.1", "release", "2018-08-22T14:03:42+00:00"),
            entry("b1.7.3", "old_beta", "2011-07-07T22:00:00+00:00"),
        ]});
        let releases = releases_since(&manifest, FIRST).unwrap();
        let ids: Vec<&str> = releases.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["1.13.2", "1.14", "26.3"]);
        assert_eq!(releases[0].sha1, "sha1-1.13.2");
        assert_eq!(releases[0].url, "https://example.invalid/1.13.2.json");

        let missing =
            json!({ "versions": [entry("26.3", "release", "2026-09-01T10:00:00+00:00")] });
        assert!(
            releases_since(&missing, FIRST)
                .unwrap_err()
                .contains("1.13.2")
        );
        assert!(releases_since(&json!({}), FIRST).is_err());
    }

    fn textures(entries: &[(&str, u8)]) -> BTreeMap<String, BakedTexture> {
        entries
            .iter()
            .map(|(id, pixels)| {
                (
                    (*id).to_owned(),
                    BakedTexture {
                        image: ContentHash([*pixels; 32]),
                        animation: None,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn a_moved_entity_texture_is_reported_and_others_are_not() {
        let older = textures(&[
            ("minecraft:entity/bell", 1),
            ("minecraft:entity/shulker", 2),
            ("minecraft:entity/gone", 3),
            ("minecraft:block/old_stone", 4),
            ("minecraft:entity/kept", 5),
        ]);
        let newer = textures(&[
            // Moved: same pixels, new name.
            ("minecraft:entity/bell/bell_body", 1),
            // Kept under the same name (a table entry already settled it).
            ("minecraft:entity/shulker", 2),
            // A block texture move is not this check's business.
            ("minecraft:block/stone", 4),
            // New name with new pixels: a redraw, not a move.
            ("minecraft:entity/fresh", 9),
            // Same pixels as a texture that is still there: a copy, not a move.
            ("minecraft:entity/kept_copy", 5),
            ("minecraft:entity/kept", 5),
        ]);
        assert_eq!(
            moved_textures(&older, &newer),
            [(
                "minecraft:entity/bell".to_owned(),
                "minecraft:entity/bell/bell_body".to_owned()
            )]
        );
        assert!(moved_textures(&newer, &newer).is_empty());
    }

    #[test]
    fn every_rename_points_at_a_final_name() {
        // Canonical is the newest name: a target that is itself renamed would
        // leave two versions of one texture under different names.
        let table: BTreeMap<String, String> = serde_json::from_str(&super::repo_file(
            "crates/lumilio-block-assets/data/texture_renames.json",
        ))
        .unwrap();
        for (old, new) in &table {
            assert!(!table.contains_key(new), "{old} -> {new} is renamed again");
            assert_ne!(old, new);
        }
    }

    #[test]
    fn the_index_names_the_bundle_and_every_version() {
        let versions = [
            BakedVersion {
                id: "1.13.2".to_owned(),
                data_version: 1631,
                jar_sha1: "aaaa".to_owned(),
            },
            BakedVersion {
                id: "26.3".to_owned(),
                data_version: 4700,
                jar_sha1: "bbbb".to_owned(),
            },
        ];
        let text = render_index(&versions, b"abc", "0123abc", None, None);
        assert!(text.ends_with('\n'));
        let index: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(index["format"], BUNDLE_FORMAT);
        assert_eq!(index["file"], BUNDLE);
        assert_eq!(index["size"], 3);
        assert_eq!(
            index["sha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(index["tool_commit"], "0123abc");
        assert_eq!(
            index["versions"],
            json!([
                { "id": "1.13.2", "data_version": 1631, "jar_sha1": "aaaa" },
                { "id": "26.3", "data_version": 4700, "jar_sha1": "bbbb" },
            ])
        );
        assert!(index.get("entities").is_none());
        assert!(index.get("names").is_none());

        let source = render_source(&versions, b"abc", 7, "0123abc", None, None);
        assert!(source.contains("`cargo xtask bake-blocks`"));
        assert!(source.contains("Distinct images: 7"));
        assert!(source.contains("| 26.3 | 4700 | `bbbb` |"));
        assert!(!source.contains("Entity geometry"));
        assert!(!source.contains("## Names"));

        let names = NameBake {
            bytes: b"abc".to_vec(),
            counts: vec![("en_us".to_owned(), 2900), ("zh_cn".to_owned(), 2899)],
        };
        let text = render_index(&versions, b"abc", "0123abc", None, Some(&names));
        let index: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            index["names"],
            json!({
                "format": lumilio_block_assets::NAMES_FORMAT,
                "file": "names.dat.z",
                "size": 3,
                "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                "languages": { "en_us": 2900, "zh_cn": 2899 },
            })
        );
        let source = render_source(&versions, b"abc", 7, "0123abc", None, Some(&names));
        assert!(source.contains("## Names"));
        assert!(source.contains("en_us 2900, zh_cn 2899"));

        let entities = EntityBake {
            bytes: b"abc".to_vec(),
            layers: 361,
            versions: vec![
                ("1.17".to_owned(), None),
                ("26.3".to_owned(), None),
                ("1.13.2".to_owned(), Some("1.17".to_owned())),
            ],
            failed: vec![("1.18".to_owned(), "no Java 21 or newer".to_owned())],
            runtimes: vec![("26.3".to_owned(), "Java 25 (/jdk/bin/java)".to_owned())],
            elapsed: std::time::Duration::from_secs(1),
        };
        let text = render_index(&versions, b"abc", "0123abc", Some(&entities), None);
        let index: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            index["entities"],
            json!({
                "format": lumilio_block_assets::ENTITY_FORMAT,
                "file": "entities.dat.z",
                "size": 3,
                "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                "layers": 361,
                "versions": [
                    { "id": "1.17" },
                    { "id": "26.3" },
                    { "id": "1.13.2", "geometry_from": "1.17" },
                ],
            })
        );
        let source = render_source(&versions, b"abc", 7, "0123abc", Some(&entities), None);
        assert!(source.contains("## Entity geometry"));
        assert!(source.contains("not redistributed"));
        assert!(source.contains("| 1.13.2 | same as 1.17 |"));
        assert!(source.contains("| 26.3 | dumped with Java 25 (/jdk/bin/java) |"));
        assert!(source.contains("| 1.18 | not included: no Java 21 or newer |"));
    }

    #[test]
    fn the_notice_names_mojangs_assets_and_the_mappings_left_out() {
        let notice = render_notice(true, true);
        assert!(notice.contains(BUNDLE));
        assert!(notice.contains("entities.dat.z"));
        assert!(notice.contains("names.dat.z"));
        assert!(notice.contains("language files"));
        assert!(notice.contains("Mojang"));
        assert!(notice.contains("NOT AN OFFICIAL MINECRAFT PRODUCT"));
        assert!(notice.contains("not redistributed"));
        let blocks_only = render_notice(false, false);
        assert!(!blocks_only.contains("entities.dat.z"));
        assert!(!blocks_only.contains("client_mappings"));
        assert!(!blocks_only.contains("names.dat.z"));
    }
}

mod entity_dump {
    use std::path::PathBuf;

    use serde_json::json;

    use crate::entity_dump::*;
    use crate::proguard::{Mappings, dumper_names};

    #[test]
    fn java_versions_read_in_both_numbering_schemes() {
        let modern = "openjdk version \"25.0.2\" 2026-01-20 LTS\nOpenJDK Runtime Environment";
        assert_eq!(java_major(modern), Some(25));
        assert_eq!(java_major("java version \"1.8.0_402\""), Some(8));
        assert_eq!(java_major("openjdk version \"21\" 2023-09-19"), Some(21));
        assert_eq!(java_major("openjdk version \"17-ea\""), Some(17));
        assert_eq!(java_major("no version here"), None);
    }

    #[test]
    fn the_smallest_runtime_that_is_new_enough_runs_the_dump() {
        let runtime = |major| Runtime {
            java: PathBuf::from(format!("/jdk{major}/bin/java")),
            major,
        };
        let runtimes = [runtime(25), runtime(17), runtime(21)];
        assert_eq!(pick_runtime(&runtimes, 21).unwrap().major, 21);
        assert_eq!(pick_runtime(&runtimes, 22).unwrap().major, 25);
        assert_eq!(pick_runtime(&runtimes, 26), None);
    }

    #[test]
    fn library_rules_follow_the_last_matching_rule() {
        let only_mac = json!({ "rules": [{ "action": "allow", "os": { "name": "osx" } }] });
        let not_mac = json!({ "rules": [
            { "action": "allow" },
            { "action": "disallow", "os": { "name": "osx" } },
        ]});
        let feature = json!({ "rules": [
            { "action": "allow", "features": { "is_demo_user": true } },
        ]});
        assert!(library_allowed(&json!({}), "osx"));
        assert!(library_allowed(&only_mac, "osx"));
        assert!(!library_allowed(&only_mac, "linux"));
        assert!(!library_allowed(&not_mac, "osx"));
        assert!(library_allowed(&not_mac, "windows"));
        assert!(!library_allowed(&feature, "osx"));
    }

    /// The shape of Mojang's `client_mappings`, trimmed to what the dumper reads.
    const MAPPINGS: &str = "\
# {\"fileName\":\"client.txt\",\"id\":\"sourceFile\"}
# comment line
net.minecraft.client.model.geom.LayerDefinitions -> eqp:
    net.minecraft.client.model.geom.builders.CubeDeformation FISH_PATTERN_DEFORMATION -> a
    23:23:void <init>() -> <init>
    27:312:java.util.Map createRoots() -> a
    313:315:java.util.Map createRoots(boolean) -> b
net.minecraft.client.model.geom.builders.LayerDefinition -> erk:
    net.minecraft.client.model.geom.builders.MeshDefinition mesh -> a
    net.minecraft.client.model.geom.builders.MaterialDefinition material -> b
net.minecraft.client.model.geom.builders.MaterialDefinition -> erl:
    int xTexSize -> a
    int yTexSize -> b
net.minecraft.client.model.geom.builders.MeshDefinition -> erm:
    net.minecraft.client.model.geom.builders.PartDefinition root -> a
net.minecraft.client.model.geom.builders.PartDefinition -> ern:
    java.util.List cubes -> a
    net.minecraft.client.model.geom.PartPose partPose -> b
    java.util.Map children -> c
    14:16:void <init>(java.util.List,net.minecraft.client.model.geom.PartPose):17:17 -> <init>
net.minecraft.client.model.geom.builders.CubeDefinition -> eri:
    java.lang.String comment -> a
    com.mojang.math.Vector3f origin -> b
    com.mojang.math.Vector3f dimensions -> c
    net.minecraft.client.model.geom.builders.CubeDeformation grow -> d
    boolean mirror -> e
    net.minecraft.client.model.geom.builders.UVPair texCoord -> f
    net.minecraft.client.model.geom.builders.UVPair texScale -> g
net.minecraft.client.model.geom.builders.CubeDeformation -> erj:
    float growX -> a
    float growY -> b
    float growZ -> c
net.minecraft.client.model.geom.builders.UVPair -> ers:
    float u -> a
    float v -> b
net.minecraft.client.model.geom.PartPose -> eqt:
    float x -> a
    float y -> b
    float z -> c
    float xRot -> d
    float yRot -> e
    float zRot -> f
net.minecraft.server.Bootstrap -> ab:
    95:110:void bootStrap() -> a
com.mojang.math.Vector3f -> d:
    float x -> a
    float y -> b
    float z -> c
";

    #[test]
    fn mappings_name_classes_fields_and_overloaded_methods() {
        let mappings = Mappings::parse(MAPPINGS).unwrap();
        let layers = mappings
            .class("net.minecraft.client.model.geom.LayerDefinitions")
            .unwrap();
        assert_eq!(layers.obfuscated, "eqp");
        assert_eq!(layers.method("createRoots", ""), Some("a"));
        assert_eq!(layers.method("createRoots", "boolean"), Some("b"));
        assert_eq!(layers.field("FISH_PATTERN_DEFORMATION"), Some("a"));
        let part = mappings
            .class("net.minecraft.client.model.geom.builders.PartDefinition")
            .unwrap();
        assert_eq!(
            part.method(
                "<init>",
                "java.util.List,net.minecraft.client.model.geom.PartPose"
            ),
            Some("<init>")
        );
        assert_eq!(part.field("children"), Some("c"));
        assert_eq!(part.field("missing"), None);
        assert!(mappings.class("net.minecraft.Missing").is_none());

        assert!(Mappings::parse("no arrow here").is_err());
        assert!(Mappings::parse("    float x -> a").is_err());
        assert!(Mappings::parse("a.B -> c").is_err());
    }

    #[test]
    fn the_names_file_maps_what_the_dumper_reads() {
        let names = dumper_names(&Mappings::parse(MAPPINGS).unwrap()).unwrap();
        let lines: Vec<&str> = names.lines().collect();
        for expected in [
            "class net.minecraft.client.model.geom.LayerDefinitions eqp",
            "method net.minecraft.client.model.geom.LayerDefinitions createRoots a",
            "field net.minecraft.client.model.geom.builders.PartDefinition partPose b",
            "field net.minecraft.client.model.geom.builders.CubeDefinition texScale g",
            "field net.minecraft.client.model.geom.PartPose zRot f",
            "class com.mojang.math.Vector3f d",
            "method net.minecraft.server.Bootstrap bootStrap a",
        ] {
            assert!(lines.contains(&expected), "{expected} in\n{names}");
        }
        // Fields only newer versions have are left out, not guessed.
        assert!(!names.contains("visibleFaces"));
        assert!(!names.contains(" xScale "));
        assert!(!names.contains("SharedConstants"));

        let broken = MAPPINGS.replace("    java.util.Map children -> c\n", "");
        let error = dumper_names(&Mappings::parse(&broken).unwrap()).unwrap_err();
        assert!(error.contains("PartDefinition.children"), "{error}");
    }

    /// The same dumper output through the bake's runner and the launcher's
    /// runtime runner (`lumilio_core::run_dumper`, `background_command`)
    /// is kept as the same bytes (T6).
    #[cfg(unix)]
    #[test]
    fn the_bake_and_the_launcher_keep_the_same_bytes_for_a_dump() {
        use std::os::unix::fs::PermissionsExt;

        use lumilio_block_assets::{EntityDump, install_dumper};

        const FIXTURE: &[u8] =
            include_bytes!("../../lumilio-block-assets/src/entity/fixture_26_3.json");
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.json");
        std::fs::write(&fixture, FIXTURE).unwrap();
        // A `java` that writes the dumper's own spelling of a 26.3 dump.
        let java = dir.path().join("java");
        std::fs::write(
            &java,
            format!("#!/bin/sh\ncp '{}' \"$5\"\n", fixture.display()),
        )
        .unwrap();
        std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dumper = install_dumper(&dir.path().join("dumper")).unwrap();
        let classpath = [dir.path().join("client.jar")];

        let bake_dir = dir.path().join("bake");
        std::fs::create_dir_all(&bake_dir).unwrap();
        let (dump, baked) = run_dumper(
            &java,
            &dumper,
            &classpath,
            None,
            &bake_dir.join("entities.json"),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(bake_dir.join("entities.json")).unwrap(),
            baked
        );

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let launcher = runtime
            .block_on(lumilio_core::run_dumper(
                &java,
                &dumper,
                &classpath,
                &dir.path().join("launcher/out.json"),
                &dir.path().join("launcher/work"),
                &dir.path().join("launcher/dump.log"),
            ))
            .unwrap();
        assert_eq!(launcher, baked);
        assert_ne!(baked, FIXTURE, "both rewrite the dumper's spelling");
        assert_eq!(dump, EntityDump::from_json(FIXTURE).unwrap());
    }
}
