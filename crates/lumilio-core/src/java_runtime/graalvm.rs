//! Oracle GraalVM, fetched from Oracle's stable `latest` addresses.
//!
//! Oracle publishes one archive per Java release and platform at
//! `https://download.oracle.com/graalvm/<major>/latest/graalvm-jdk-<major>_<platform>_bin.<ext>`
//! and its SHA-256 next to it (the same address plus `.sha256`). Only
//! releases with a `latest` address can be downloaded this way: Java 21 and
//! 25 (Java 17 and 26 have none). There is no Windows on Arm build.
//!
//! Oracle GraalVM is distributed under the GraalVM Free Terms and Conditions
//! (<https://www.oracle.com/downloads/licenses/graal-free-license.html>);
//! the launcher only downloads it on the person's request and does not
//! redistribute it.

use super::JavaRuntimeError;
use super::archive::ArchiveKind;
use crate::environment::{HostProfile, MachineArchitecture, PlatformFamily};

/// The Java majors with a `latest` address, ascending.
pub const MAJORS: [u32; 2] = [21, 25];

/// The only host an archive or checksum may come from.
const HOST: &str = "download.oracle.com";

/// Where the license is published, for the interface to link to.
pub const LICENSE_URL: &str = "https://www.oracle.com/downloads/licenses/graal-free-license.html";

/// One archive to download: the file, its checksum file and how to unpack it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Archive {
    pub major: u32,
    pub url: String,
    pub checksum_url: String,
    pub kind: ArchiveKind,
}

/// Oracle's name for this system in an archive's file name.
#[must_use]
pub fn platform_key(host: &HostProfile) -> Option<&'static str> {
    use MachineArchitecture::{Arm64, X86_64};
    Some(match (host.platform(), host.architecture()) {
        (PlatformFamily::MacOs, Arm64) => "macos-aarch64",
        (PlatformFamily::MacOs, X86_64) => "macos-x64",
        (PlatformFamily::Linux, Arm64) => "linux-aarch64",
        (PlatformFamily::Linux, X86_64) => "linux-x64",
        (PlatformFamily::Windows, X86_64) => "windows-x64",
        _ => return None,
    })
}

/// The majors offered to this system, ascending; empty when Oracle builds
/// nothing for it.
#[must_use]
pub fn offered_majors(host: &HostProfile) -> Vec<u32> {
    if platform_key(host).is_some() {
        MAJORS.to_vec()
    } else {
        Vec::new()
    }
}

/// The archive of `major` for this system.
pub fn archive(host: &HostProfile, major: u32) -> Result<Archive, JavaRuntimeError> {
    let platform = platform_key(host).ok_or(JavaRuntimeError::UnsupportedPlatform)?;
    if !MAJORS.contains(&major) {
        return Err(JavaRuntimeError::NoSuchRuntime(Some(major)));
    }
    let kind = if *host.platform() == PlatformFamily::Windows {
        ArchiveKind::Zip
    } else {
        ArchiveKind::TarGz
    };
    let url = format!(
        "https://{HOST}/graalvm/{major}/latest/graalvm-jdk-{major}_{platform}_bin.{}",
        kind.extension()
    );
    Ok(Archive {
        major,
        checksum_url: format!("{url}.sha256"),
        url,
        kind,
    })
}

/// Whether an archive or checksum may be downloaded from `url`: https and
/// Oracle's download host.
#[must_use]
pub fn is_trusted_source(url: &str) -> bool {
    url::Url::parse(url)
        .is_ok_and(|parsed| parsed.scheme() == "https" && parsed.host_str() == Some(HOST))
}

/// The digest in a `.sha256` file: its first word, 64 hex digits.
pub fn parse_checksum(text: &str) -> Result<String, JavaRuntimeError> {
    let digest = text.split_whitespace().next().unwrap_or_default();
    if digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(digest.to_ascii_lowercase())
    } else {
        Err(JavaRuntimeError::Invalid(format!(
            "not a SHA-256 checksum: {:?}",
            text.chars().take(80).collect::<String>()
        )))
    }
}

#[cfg(test)]
mod tests;
