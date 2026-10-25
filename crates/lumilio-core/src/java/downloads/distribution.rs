use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::graalvm;
use crate::instances::modpack::is_safe_relative;
use lumilio_minecraft::{HostProfile, MachineArchitecture, PlatformFamily};

/// Where the launcher downloads a Java from.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum JavaSource {
    /// The runtimes Mojang publishes for its own launcher.
    #[default]
    Mojang,
    /// Oracle GraalVM, under the GraalVM Free Terms and Conditions.
    OracleGraalVm,
}

impl JavaSource {
    pub const ALL: [Self; 2] = [Self::Mojang, Self::OracleGraalVm];

    /// The product name, the same in every language.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mojang => "Mojang",
            Self::OracleGraalVm => "Oracle GraalVM",
        }
    }

    /// The folder below `runtimes/` a download of `major` lands in, for
    /// sources whose folder does not come from their index.
    #[must_use]
    pub fn folder_name(self, major: u32) -> Option<String> {
        match self {
            Self::Mojang => None,
            Self::OracleGraalVm => Some(format!("graalvm-{major}")),
        }
    }
}

/// Which builds a download may be.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JavaBuild {
    /// Any that runs here, native first.
    #[default]
    Any,
    /// Intel only, for a game whose native libraries have no other; Apple
    /// silicon and Windows on Arm run it translated.
    Intel,
}

/// The source a download of `major` uses: the preferred one when it offers
/// that major to this system, else Mojang, which has every major a game
/// needs. Without a wanted major any source with something to offer fits.
#[must_use]
pub fn source_for(preferred: JavaSource, host: &HostProfile, major: Option<u32>) -> JavaSource {
    let offered = match preferred {
        JavaSource::Mojang => return JavaSource::Mojang,
        JavaSource::OracleGraalVm => graalvm::offered_majors(host),
    };
    let fits = major.map_or(!offered.is_empty(), |major| offered.contains(&major));
    if fits { preferred } else { JavaSource::Mojang }
}

/// Whether a manifest component holds a Java runtime: the index also lists
/// helpers such as `minecraft-java-exe`.
pub(super) fn is_runtime_component(component: &str) -> bool {
    (component.starts_with("java-runtime-") || component.starts_with("jre-"))
        && !component.contains("snapshot")
}

/// Where the index of runtimes lives.
pub const INDEX_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Hosts a runtime file may be fetched from.
pub(super) const TRUSTED_HOSTS: [&str; 3] = [
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

/// The index's names whose runtimes run here, native first. Apple silicon
/// runs Intel runtimes under Rosetta 2 and Windows on Arm runs x64 ones
/// under emulation; those only fill majors without a native build (Mojang
/// has no Java 8 for either).
#[must_use]
pub fn platform_keys(host: &HostProfile) -> Vec<&'static str> {
    use MachineArchitecture::Arm64;
    let mut keys: Vec<_> = platform_key(host).into_iter().collect();
    match (host.platform(), host.architecture()) {
        (PlatformFamily::MacOs, Arm64) => keys.push("mac-os"),
        (PlatformFamily::Windows, Arm64) => keys.push("windows-x64"),
        _ => {}
    }
    keys
}

/// The index's name for this system's Intel runtimes, if it runs them.
pub(super) fn intel_platform_key(host: &HostProfile) -> Option<&'static str> {
    use MachineArchitecture::{Arm64, X86_64};
    Some(match (host.platform(), host.architecture()) {
        (PlatformFamily::Linux, X86_64) => "linux",
        (PlatformFamily::MacOs, X86_64 | Arm64) => "mac-os",
        (PlatformFamily::Windows, X86_64 | Arm64) => "windows-x64",
        _ => return None,
    })
}

/// [`choose_component`] over every platform that runs here and gives
/// `build`, native first.
pub fn choose_for_host(
    index: &str,
    host: &HostProfile,
    wanted: Option<u32>,
    build: JavaBuild,
) -> Result<RuntimeChoice, JavaRuntimeError> {
    let keys = match build {
        JavaBuild::Any => platform_keys(host),
        JavaBuild::Intel => intel_platform_key(host).into_iter().collect(),
    };
    if keys.is_empty() {
        return Err(JavaRuntimeError::UnsupportedPlatform);
    }
    let mut last = JavaRuntimeError::UnsupportedPlatform;
    for (position, platform) in keys.iter().enumerate() {
        // Without a wanted major only the native default is offered.
        if wanted.is_none() && position > 0 {
            break;
        }
        match choose_component(index, platform, wanted) {
            Ok(choice) => return Ok(choice),
            Err(error @ JavaRuntimeError::Invalid(_)) => return Err(error),
            Err(error) => last = error,
        }
    }
    Err(match last {
        JavaRuntimeError::UnsupportedPlatform => JavaRuntimeError::NoSuchRuntime(wanted),
        other => other,
    })
}

/// Every Java major the index offers to this system, ascending.
pub fn offered_majors(index: &str, host: &HostProfile) -> Result<Vec<u32>, JavaRuntimeError> {
    let all: BTreeMap<String, BTreeMap<String, Vec<RawEntry>>> = serde_json::from_str(index)
        .map_err(|error| JavaRuntimeError::Invalid(error.to_string()))?;
    let mut majors: Vec<u32> = platform_keys(host)
        .into_iter()
        .filter_map(|platform| all.get(platform))
        .flat_map(|components| {
            components
                .iter()
                .filter(|(component, _)| is_runtime_component(component))
                .filter_map(|(_, entries)| major_of(&entries.first()?.version.name))
        })
        .collect();
    majors.sort_unstable();
    majors.dedup();
    Ok(majors)
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
    /// The index's name for the system it is built for, e.g. `mac-os`.
    pub platform: String,
    pub major: u32,
    /// The version it reports, e.g. `17.0.8`.
    pub version: String,
    pub manifest: Download,
}

impl RuntimeChoice {
    /// The folder below `runtimes/` it is installed in: the component, with
    /// the platform added when it is not `host`'s own, so an Intel
    /// `java-runtime-gamma` does not take the native one's place.
    #[must_use]
    pub fn folder(&self, host: &HostProfile) -> String {
        if platform_key(host) == Some(self.platform.as_str()) {
            self.component.clone()
        } else {
            format!("{}-{}", self.component, self.platform)
        }
    }
}

#[derive(Deserialize)]
pub(super) struct RawEntry {
    pub(super) manifest: Download,
    pub(super) version: RawVersion,
}

#[derive(Deserialize)]
pub(super) struct RawVersion {
    pub(super) name: String,
}

/// The major version in a runtime's version name (`17.0.8` → 17, `1.8.0_202` → 8).
pub(super) fn major_of(name: &str) -> Option<u32> {
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
            if !is_runtime_component(component) {
                return None;
            }
            let entry = entries.first()?;
            Some(RuntimeChoice {
                component: component.clone(),
                platform: platform.to_owned(),
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
pub(super) struct RawManifest {
    pub(super) files: BTreeMap<String, RawFile>,
}

#[derive(Deserialize)]
pub(super) struct RawFile {
    #[serde(rename = "type")]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) executable: bool,
    #[serde(default)]
    pub(super) downloads: BTreeMap<String, Download>,
    pub(super) target: Option<String>,
}

/// A link may point to a sibling or below, or up within the runtime, but not
/// out of it.
pub(super) fn link_stays_inside(path: &str, target: &str) -> bool {
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

/// The manifest path of the file a link at `path` names, following links to
/// links. `None` when the chain loops or ends at a folder or at a path the
/// manifest does not list. Mojang's links name files (license texts), and
/// Windows ones have none.
#[cfg(any(not(unix), test))]
#[must_use]
pub fn linked_file(entries: &[(String, Entry)], path: &str) -> Option<String> {
    let mut current = path.to_owned();
    // Each step reaches another entry, so a longer chain loops.
    for _ in 0..entries.len() {
        let (_, entry) = entries.iter().find(|(name, _)| *name == current)?;
        let Entry::Link { target } = entry else {
            return matches!(entry, Entry::File { .. }).then_some(current);
        };
        let mut parts: Vec<&str> = current.split('/').collect();
        parts.pop();
        for part in target.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                _ => parts.push(part),
            }
        }
        current = parts.join("/");
    }
    None
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
