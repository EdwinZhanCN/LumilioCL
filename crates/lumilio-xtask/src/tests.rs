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
