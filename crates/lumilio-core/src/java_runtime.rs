//! Java runtimes the launcher installs itself (`ADR 0014`).
//!
//! Mojang publishes the runtimes its own launcher uses as a public index:
//! per platform, per component, a manifest naming every file with its size,
//! SHA-1 and address. This module reads those documents; the service does the
//! downloading. Behavior notes: `docs/behavior/java-runtime.md`.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::environment::{HostProfile, MachineArchitecture, PlatformFamily};
use crate::modpack::is_safe_relative;

/// Where the index of runtimes lives.
pub const INDEX_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Hosts a runtime file may be fetched from.
const TRUSTED_HOSTS: [&str; 3] = [
    "launchermeta.mojang.com",
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
];

/// Whether a runtime file may be downloaded from `url`: https and a Mojang host.
#[must_use]
pub fn is_trusted_source(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| {
        parsed.scheme() == "https"
            && parsed
                .host_str()
                .is_some_and(|host| TRUSTED_HOSTS.contains(&host))
    })
}

#[derive(Debug, Eq, PartialEq)]
pub enum JavaRuntimeError {
    /// The index or a manifest could not be understood.
    Invalid(String),
    /// The index has no runtime for this system.
    UnsupportedPlatform,
    /// The index has no runtime of the needed version for this system.
    NoSuchRuntime(Option<u32>),
    /// A path or address in a manifest is not acceptable.
    Unsafe(String),
}

impl Display for JavaRuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(why) => write!(f, "unusable Java runtime list: {why}"),
            Self::UnsupportedPlatform => f.write_str("no downloadable Java for this system"),
            Self::NoSuchRuntime(Some(major)) => {
                write!(f, "no downloadable Java {major} for this system")
            }
            Self::NoSuchRuntime(None) => f.write_str("no downloadable Java for this system"),
            Self::Unsafe(what) => write!(f, "refused {what:?} in the Java runtime list"),
        }
    }
}

impl Error for JavaRuntimeError {}

/// The index's name for this system, if it has one.
#[must_use]
pub fn platform_key(host: &HostProfile) -> Option<&'static str> {
    use MachineArchitecture::{Arm64, X86, X86_64};
    Some(match (host.platform(), host.architecture()) {
        (PlatformFamily::Linux, X86_64) => "linux",
        (PlatformFamily::Linux, X86) => "linux-i386",
        (PlatformFamily::MacOs, X86_64) => "mac-os",
        (PlatformFamily::MacOs, Arm64) => "mac-os-arm64",
        (PlatformFamily::Windows, X86_64) => "windows-x64",
        (PlatformFamily::Windows, X86) => "windows-x86",
        (PlatformFamily::Windows, Arm64) => "windows-arm64",
        _ => return None,
    })
}

/// A file to download: where, how big, and its SHA-1.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Download {
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub size: u64,
}

/// The runtime picked from the index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeChoice {
    /// Mojang's name for it, e.g. `java-runtime-gamma`.
    pub component: String,
    pub major: u32,
    /// The version it reports, e.g. `17.0.8`.
    pub version: String,
    pub manifest: Download,
}

#[derive(Deserialize)]
struct RawEntry {
    manifest: Download,
    version: RawVersion,
}

#[derive(Deserialize)]
struct RawVersion {
    name: String,
}

/// The major version in a runtime's version name (`17.0.8` → 17, `1.8.0_202` → 8).
fn major_of(name: &str) -> Option<u32> {
    crate::java::parse_major(name)
}

/// Picks the runtime for this system. With a wanted major, the newest runtime
/// of exactly that major; without one, Java 21 if offered, else the newest.
pub fn choose_component(
    index: &str,
    platform: &str,
    wanted: Option<u32>,
) -> Result<RuntimeChoice, JavaRuntimeError> {
    let all: BTreeMap<String, BTreeMap<String, Vec<RawEntry>>> = serde_json::from_str(index)
        .map_err(|error| JavaRuntimeError::Invalid(error.to_string()))?;
    let components = all
        .get(platform)
        .ok_or(JavaRuntimeError::UnsupportedPlatform)?;
    let mut offered: Vec<RuntimeChoice> = components
        .iter()
        .filter_map(|(component, entries)| {
            // Snapshot components are Mojang's own testing copies.
            if component.contains("snapshot") {
                return None;
            }
            let entry = entries.first()?;
            Some(RuntimeChoice {
                component: component.clone(),
                major: major_of(&entry.version.name)?,
                version: entry.version.name.clone(),
                manifest: entry.manifest.clone(),
            })
        })
        .collect();
    offered.sort_by(|a, b| {
        a.major
            .cmp(&b.major)
            .then_with(|| a.version.cmp(&b.version))
    });
    let pick = match wanted {
        Some(major) => offered
            .into_iter()
            .rev()
            .find(|choice| choice.major == major),
        None => {
            let newest = offered.last().cloned();
            offered
                .into_iter()
                .rev()
                .find(|choice| choice.major == 21)
                .or(newest)
        }
    };
    pick.ok_or(JavaRuntimeError::NoSuchRuntime(wanted))
}

/// One entry of a runtime's file list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Entry {
    Directory,
    File {
        download: Download,
        executable: bool,
    },
    /// A symbolic link to a path relative to the link's own folder.
    Link {
        target: String,
    },
}

#[derive(Deserialize)]
struct RawManifest {
    files: BTreeMap<String, RawFile>,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    downloads: BTreeMap<String, Download>,
    target: Option<String>,
}

/// A link may point to a sibling or below, or up within the runtime, but not
/// out of it.
fn link_stays_inside(path: &str, target: &str) -> bool {
    if target.is_empty() || target.starts_with('/') || target.contains(['\\', ':']) {
        return false;
    }
    let mut depth = path.matches('/').count() as i64;
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => depth -= 1,
            _ => depth += 1,
        }
        if depth < 0 {
            return false;
        }
    }
    true
}

/// Reads a component's file list. Every path must be a plain relative path,
/// every file needs a raw download from an address `allow` accepts, and every link must
/// stay inside the runtime.
pub fn parse_manifest(
    json: &str,
    allow: fn(&str) -> bool,
) -> Result<Vec<(String, Entry)>, JavaRuntimeError> {
    let raw: RawManifest =
        serde_json::from_str(json).map_err(|error| JavaRuntimeError::Invalid(error.to_string()))?;
    let mut entries = Vec::new();
    for (path, file) in raw.files {
        if !is_safe_relative(&path) {
            return Err(JavaRuntimeError::Unsafe(path));
        }
        let entry = match file.kind.as_str() {
            "directory" => Entry::Directory,
            "file" => {
                let download =
                    file.downloads.get("raw").cloned().ok_or_else(|| {
                        JavaRuntimeError::Invalid(format!("{path} has no raw file"))
                    })?;
                if !allow(&download.url) {
                    return Err(JavaRuntimeError::Unsafe(download.url));
                }
                Entry::File {
                    download,
                    executable: file.executable,
                }
            }
            "link" => {
                let target = file
                    .target
                    .ok_or_else(|| JavaRuntimeError::Invalid(format!("{path} has no target")))?;
                if !link_stays_inside(&path, &target) {
                    return Err(JavaRuntimeError::Unsafe(target));
                }
                Entry::Link { target }
            }
            other => {
                return Err(JavaRuntimeError::Invalid(format!(
                    "unknown entry type {other}"
                )));
            }
        };
        entries.push((path, entry));
    }
    Ok(entries)
}

/// Where a manifest path goes below the runtime's folder. A macOS runtime
/// ships as `jre.bundle/Contents/Home/…`; the bundle folder is dropped so the
/// result is `<folder>/Contents/Home/…`, which Java discovery already knows.
#[must_use]
pub fn place(folder: &Path, path: &str) -> PathBuf {
    if path == "jre.bundle" {
        return folder.to_owned();
    }
    folder.join(path.strip_prefix("jre.bundle/").unwrap_or(path))
}

#[cfg(test)]
mod tests;
