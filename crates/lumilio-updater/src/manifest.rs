use semver::Version;

use crate::client::{UpdateError, UpdateRelease};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AssetTarget {
    pub os: &'static str,
    pub arch: &'static str,
    pub suffix: &'static str,
}

impl AssetTarget {
    pub fn current() -> Result<Self, UpdateError> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => Ok(Self {
                os: "macos",
                arch: "arm64",
                suffix: "-macos-arm64.dmg",
            }),
            ("windows", "x86_64") => Ok(Self {
                os: "windows",
                arch: "x64",
                suffix: "-windows-x64-setup.exe",
            }),
            ("linux", "x86_64") => Ok(Self {
                os: "linux",
                arch: "x64",
                suffix: "-linux-x64.tar.gz",
            }),
            _ => Err(UpdateError::UnsupportedPlatform(format!(
                "{} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ))),
        }
    }
}

pub(crate) fn find_asset(
    manifest: &str,
    target: &AssetTarget,
) -> Result<UpdateRelease, UpdateError> {
    let mut matching = Vec::new();
    for line in manifest.lines() {
        let mut fields = line.split_whitespace();
        let (Some(hash), Some(name)) = (fields.next(), fields.next()) else {
            continue;
        };
        if fields.next().is_some() || !name.starts_with("LumilioCL-") {
            continue;
        }
        if !name.ends_with(target.suffix) {
            continue;
        }
        if !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) || hash.len() != 64 {
            return Err(UpdateError::Manifest(format!(
                "invalid checksum for {name}"
            )));
        }
        let version_text = name
            .strip_prefix("LumilioCL-")
            .and_then(|name| name.strip_suffix(target.suffix))
            .ok_or_else(|| UpdateError::Manifest(format!("unexpected asset name {name}")))?;
        let version = Version::parse(version_text).map_err(|error| {
            UpdateError::Manifest(format!("invalid version in {name}: {error}"))
        })?;
        matching.push(UpdateRelease {
            version,
            file_name: name.to_owned(),
            sha256: hash.to_ascii_lowercase(),
        });
    }
    match matching.len() {
        0 => Err(UpdateError::Manifest(format!(
            "no {} {} package in SHA256SUMS.txt",
            target.os, target.arch
        ))),
        1 => Ok(matching.remove(0)),
        count => Err(UpdateError::Manifest(format!(
            "found {count} matching platform packages"
        ))),
    }
}
