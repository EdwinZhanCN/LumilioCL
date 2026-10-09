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
    let mut names: Vec<_> = zip.file_names().collect();
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
        ])
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
        assert_eq!(table.len(), 8);
    }

    #[test]
    fn the_table_renders_the_same_way_every_time() {
        let table = build_table(&mut sample());
        let first = render(&table);
        assert_eq!(first, render(&build_table(&mut sample())));
        assert!(first.contains("    \"minecraft:stone\": [100, 120, 140, 0],\n"));
        let parsed: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(parsed["blocks"]["minecraft:water"][3], TINT_WATER);
        assert!(first.ends_with("  }\n}\n"));
        assert!(!first.contains(",\n  }"), "no trailing comma");
        let names: Vec<&str> = first
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
