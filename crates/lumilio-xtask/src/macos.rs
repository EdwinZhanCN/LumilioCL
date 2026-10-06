//! `LumilioCL.app` in a `.dmg`, in the order PACKAGING.md §2 fixes: resources
//! complete, sign inside out with the hardened runtime, verify, make and sign
//! the disk image, notarize, staple.
//!
//! Signing is configured from the environment; without it the bundle is
//! signed ad hoc and Gatekeeper asks the user to confirm the first launch.
//!
//! - `LUMILIO_MACOS_SIGNING_IDENTITY`: a Developer ID Application identity in
//!   the keychain.
//! - Notarization, either `LUMILIO_NOTARY_PROFILE` (a `notarytool
//!   store-credentials` keychain profile) or an App Store Connect API key:
//!   `LUMILIO_NOTARY_KEY` (path to the `.p8`), `LUMILIO_NOTARY_KEY_ID`,
//!   `LUMILIO_NOTARY_ISSUER`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::release::{
    APP_ID, APP_NAME, BINARY, Release, Result, copy, copy_tree, create_dir, fill, read, run,
    symlink, write,
};

/// Apple silicon's floor, and what rustc targets by default there.
pub const MIN_MACOS: &str = "11.0";

pub const INFO_PLIST: &str = include_str!("../packaging/Info.plist");

pub fn package(release: &Release, skip_build: bool) -> Result<Vec<PathBuf>> {
    if !skip_build {
        release.build(&[("MACOSX_DEPLOYMENT_TARGET", MIN_MACOS)])?;
    }
    let signing = Signing::from_env();
    let stage = release.stage("macos")?;
    let app = stage.join(format!("{APP_NAME}.app"));
    let contents = app.join("Contents");
    let resources = contents.join("Resources");
    create_dir(&resources)?;

    // 1. Resources, including the compiled icon.
    copy(&release.binary(), &contents.join("MacOS").join(BINARY))?;
    compile_icon(release, &stage, &resources)?;
    write(&contents.join("Info.plist"), info_plist(release)?)?;
    run(Command::new("plutil")
        .arg("-lint")
        .arg(contents.join("Info.plist")))?;
    write(&contents.join("PkgInfo"), "APPL????")?;

    // 2–3. Sign inside out, then verify.
    signing.sign(&contents.join("MacOS").join(BINARY))?;
    signing.sign(&app)?;
    run(Command::new("codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(&app))?;

    // 4. The disk image: the app, a link to /Applications, the licence.
    let image_root = stage.join("dmg");
    create_dir(&image_root)?;
    run(Command::new("ditto")
        .arg(&app)
        .arg(image_root.join(format!("{APP_NAME}.app"))))?;
    symlink(Path::new("/Applications"), &image_root.join("Applications"))?;
    copy(&release.license(), &image_root.join("LICENSE.txt"))?;
    write(
        &image_root.join("BUILD.txt"),
        release.build_info("macos", signing.describe()),
    )?;
    let dmg = release.dist_file(&format!("{}.dmg", release.stem("macos")))?;
    run(Command::new("hdiutil")
        .args([
            "create", "-volname", APP_NAME, "-fs", "HFS+", "-format", "UDZO",
        ])
        .arg("-srcfolder")
        .arg(&image_root)
        .arg(&dmg))?;
    if signing.identity.is_some() {
        signing.sign(&dmg)?;
    }

    // 5–6. Notarize and staple.
    match (&signing.identity, &signing.notary) {
        (Some(_), Some(notary)) => {
            notarize(&dmg, notary)?;
            run(Command::new("xcrun").args(["stapler", "staple"]).arg(&dmg))?;
            run(Command::new("xcrun")
                .args(["stapler", "validate"])
                .arg(&dmg))?;
        }
        (Some(_), None) => {
            eprintln!("warning: signed but not notarized; Gatekeeper will refuse it")
        }
        (None, _) => eprintln!("warning: ad-hoc signature; this build is for testing only"),
    }
    Ok(vec![dmg])
}

pub fn info_plist(release: &Release) -> Result<String> {
    let numeric = release.version.numeric();
    let version = release.version.to_string();
    fill(
        INFO_PLIST,
        &[
            ("APP_NAME", APP_NAME),
            ("BINARY", BINARY),
            ("APP_ID", APP_ID),
            ("SHORT_VERSION", &numeric),
            ("BUNDLE_VERSION", &numeric),
            ("VERSION", &version),
            ("COMMIT", &release.commit),
            ("MIN_MACOS", MIN_MACOS),
        ],
    )
}

/// Compiles `AppIcon.icon` (Icon Composer) into `Assets.car` for macOS 26+
/// and `AppIcon.icns` for older systems. Needs the full Xcode 26.
fn compile_icon(release: &Release, stage: &Path, resources: &Path) -> Result {
    let source = stage.join("AppIcon.icon");
    copy_tree(&release.icons().join("macos/AppIcon.icon"), &source)?;
    run(Command::new("xcrun")
        .arg("actool")
        .arg(&source)
        .arg("--compile")
        .arg(resources)
        .args(["--app-icon", "AppIcon", "--platform", "macosx"])
        .args([
            "--target-device",
            "mac",
            "--minimum-deployment-target",
            MIN_MACOS,
        ])
        .arg("--output-partial-info-plist")
        .arg(stage.join("AppIcon.partial.plist")))?;
    // actool can succeed without recognising its input, so check the output.
    for compiled in ["Assets.car", "AppIcon.icns"] {
        if !resources.join(compiled).is_file() {
            return Err(format!(
                "actool produced no {compiled}; the icon needs Xcode 26 or later"
            ));
        }
    }
    Ok(())
}

struct Signing {
    identity: Option<String>,
    notary: Option<Notary>,
}

enum Notary {
    Profile(String),
    ApiKey {
        key: String,
        key_id: String,
        issuer: String,
    },
}

impl Signing {
    fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let notary = match (
            var("LUMILIO_NOTARY_PROFILE"),
            var("LUMILIO_NOTARY_KEY"),
            var("LUMILIO_NOTARY_KEY_ID"),
            var("LUMILIO_NOTARY_ISSUER"),
        ) {
            (Some(profile), ..) => Some(Notary::Profile(profile)),
            (None, Some(key), Some(key_id), Some(issuer)) => Some(Notary::ApiKey {
                key,
                key_id,
                issuer,
            }),
            _ => None,
        };
        Self {
            identity: var("LUMILIO_MACOS_SIGNING_IDENTITY"),
            notary,
        }
    }

    fn describe(&self) -> &'static str {
        match (&self.identity, &self.notary) {
            (Some(_), Some(_)) => "Developer ID, notarized",
            (Some(_), None) => "Developer ID, not notarized",
            (None, _) => "ad hoc (unsigned)",
        }
    }

    fn sign(&self, path: &Path) -> Result {
        let mut command = Command::new("codesign");
        command.arg("--force");
        match &self.identity {
            Some(identity) => command
                .args(["--options", "runtime", "--timestamp", "--sign"])
                .arg(identity),
            None => command.args(["--sign", "-"]),
        };
        run(command.arg(path))
    }
}

fn notarize(dmg: &Path, notary: &Notary) -> Result {
    let mut command = Command::new("xcrun");
    command
        .args(["notarytool", "submit"])
        .arg(dmg)
        .args(["--wait", "--output-format", "json"]);
    notary.authenticate(&mut command);
    eprintln!("$ xcrun notarytool submit {} --wait", dmg.display());
    let output = read(&mut command)?;
    let result: serde_json::Value = serde_json::from_str(&output)
        .map_err(|error| format!("notarytool said something unreadable ({error}): {output}"))?;
    let status = result["status"].as_str().unwrap_or("unknown");
    if status == "Accepted" {
        return Ok(());
    }
    // The log says why; print it before failing.
    if let Some(id) = result["id"].as_str() {
        let mut log = Command::new("xcrun");
        log.args(["notarytool", "log", id]);
        notary.authenticate(&mut log);
        if let Ok(text) = read(&mut log) {
            eprintln!("{text}");
        }
    }
    Err(format!("notarization ended {status}, not Accepted"))
}

impl Notary {
    fn authenticate(&self, command: &mut Command) {
        match self {
            Self::Profile(profile) => command.arg("--keychain-profile").arg(profile),
            Self::ApiKey {
                key,
                key_id,
                issuer,
            } => command
                .arg("--key")
                .arg(key)
                .arg("--key-id")
                .arg(key_id)
                .arg("--issuer")
                .arg(issuer),
        };
    }
}
