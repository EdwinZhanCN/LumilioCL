//! Release tooling, run as `cargo xtask <command>`.
//!
//! - `package [--skip-build]`: build the launcher in release mode and pack it
//!   for the platform this runs on. Each platform packs natively; there is no
//!   cross packaging (assets/icons/PACKAGING.md §5). Artifacts land in
//!   `dist/`, each with a `.sha256` beside it.
//! - `block-colors VERSION`: build the block colour table of a game version from
//!   its client jar (see `block_colors.rs`).
//! - `release-check [TAG]`: print `version=` and `prerelease=` lines (for
//!   `$GITHUB_OUTPUT`), failing if TAG is not `v<workspace version>`.

mod block_colors;
mod linux;
mod macos;
mod release;
mod version;
mod windows;

use std::process::ExitCode;

use release::{Release, Result};
use version::Version;

const USAGE: &str = "usage: cargo xtask package [--skip-build]\n       cargo xtask release-check [TAG]\n       cargo xtask block-colors VERSION";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["package"] => package(false),
        ["package", "--skip-build"] => package(true),
        ["release-check"] => release_check(None),
        ["release-check", tag] => release_check(Some(tag)),
        ["block-colors", version] => Release::detect()
            .and_then(|release| block_colors::run(version, &release.root, &release.target)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn package(skip_build: bool) -> Result {
    let release = Release::detect()?;
    eprintln!(
        "packaging LumilioCL {} for {} {} at {}",
        release.version,
        std::env::consts::OS,
        release.arch,
        release.commit
    );
    let artifacts = match std::env::consts::OS {
        "macos" => macos::package(&release, skip_build)?,
        "windows" => windows::package(&release, skip_build)?,
        "linux" => linux::package(&release, skip_build)?,
        other => return Err(format!("no package is built on {other}")),
    };
    for artifact in &artifacts {
        release::write_checksum(artifact)?;
        println!("{}", artifact.display());
    }
    Ok(())
}

fn release_check(tag: Option<&str>) -> Result {
    let version = Version::workspace();
    println!("{}", release_outputs(&version, tag)?);
    Ok(())
}

/// A release is cut by pushing `v<version>`; any other tag means the version in
/// Cargo.toml and the tag disagree, and the release would lie about itself.
fn release_outputs(version: &Version, tag: Option<&str>) -> Result<String> {
    if let Some(tag) = tag
        && tag != format!("v{version}")
    {
        return Err(format!(
            "tag {tag} does not match the workspace version {version}; expected v{version}"
        ));
    }
    Ok(format!(
        "version={version}\nprerelease={}",
        version.is_prerelease()
    ))
}

#[cfg(test)]
mod tests;
