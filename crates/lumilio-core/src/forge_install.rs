//! Installing Forge and NeoForge from their official installers.
//!
//! Behavior notes: `docs/behavior/loader.md`. An installer jar carries
//! `version.json` (a release manifest inheriting the vanilla one, used like a
//! Fabric profile) and `install_profile.json`: libraries the installer tools
//! need, named data values, and "processors" — small Java programs that turn
//! the vanilla client into the patched one the loader starts from. This module
//! reads the installer, prepares those values and runs the client processors.
//!
//! Adapted from HMCL (`HMCLCore/src/main/java/org/jackhuang/hmcl/download/forge/ForgeNewInstallTask.java`,
//! `ForgeNewInstallProfile.java` and `neoforge/NeoForgeInstallTask.java`),
//! Copyright (C) 2021 huangyuhui and contributors, GPL-3.0-or-later.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::activity::CancellationToken;
use crate::artifact::PackageCoordinate;
use crate::environment::HostProfile;
use crate::forge_meta::NEOFORGE_LEGACY_GAME;
use crate::instance::Loader;
use crate::release::LibraryDependency;

#[derive(Debug)]
pub enum ForgeError {
    /// The installer predates processors (Forge 1.12.2 and older).
    LegacyInstaller,
    /// The installer is not a readable archive or lacks its profile.
    Unreadable(String),
    /// A data value or argument names something that cannot be resolved.
    Profile(String),
    /// A processor failed: its tool, and the end of what it printed.
    Processor {
        tool: String,
        detail: String,
    },
    /// A processor ran but did not produce what the profile promises.
    Output(String),
    Io(io::Error),
    Cancelled,
}

impl fmt::Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LegacyInstaller => write!(f, "this installer uses the pre-1.13 format"),
            Self::Unreadable(message) => write!(f, "installer unreadable: {message}"),
            Self::Profile(message) => write!(f, "install profile: {message}"),
            Self::Processor { tool, detail } => write!(f, "processor {tool} failed: {detail}"),
            Self::Output(message) => write!(f, "processor output: {message}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for ForgeError {}

impl From<io::Error> for ForgeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Where the official installer for a build lives.
#[must_use]
pub fn installer_url(loader: Loader, game: &str, version: &str) -> Option<String> {
    match loader {
        Loader::Forge => Some(format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{game}-{version}/forge-{game}-{version}-installer.jar"
        )),
        Loader::NeoForge if game == NEOFORGE_LEGACY_GAME => Some(format!(
            "https://maven.neoforged.net/releases/net/neoforged/forge/{game}-{version}/forge-{game}-{version}-installer.jar"
        )),
        Loader::NeoForge => Some(format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
        )),
        _ => None,
    }
}

/// Where an instance keeps its installer, so processors can run again later.
#[must_use]
pub fn installer_path(versions: &Path, release_id: &str) -> PathBuf {
    versions
        .join(release_id)
        .join(format!("{release_id}-installer.jar"))
}

#[derive(Clone, Debug, Deserialize)]
pub struct Sided {
    pub client: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Processor {
    pub jar: PackageCoordinate,
    #[serde(default)]
    pub classpath: Vec<PackageCoordinate>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
    /// `None` means both sides.
    pub sides: Option<Vec<String>>,
}

impl Processor {
    fn runs_on_client(&self) -> bool {
        self.sides
            .as_ref()
            .is_none_or(|sides| sides.iter().any(|side| side == "client"))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct InstallProfile {
    #[serde(default)]
    pub data: BTreeMap<String, Sided>,
    #[serde(default)]
    pub processors: Vec<Processor>,
    #[serde(default)]
    pub libraries: Vec<LibraryDependency>,
    /// Where `version.json` sits inside the installer.
    #[serde(default = "default_json")]
    pub json: String,
}

fn default_json() -> String {
    "/version.json".to_owned()
}

impl InstallProfile {
    /// The processors this launcher runs: the client's, in order.
    pub fn client_processors(&self) -> impl Iterator<Item = &Processor> {
        self.processors
            .iter()
            .filter(|processor| processor.runs_on_client())
    }
}

/// What an installer carries.
pub struct Installer {
    pub version_json: Vec<u8>,
    pub profile: InstallProfile,
}

fn entry_name(path: &str) -> &str {
    path.trim_start_matches('/')
}

fn read_entry(archive: &mut zip::ZipArchive<File>, name: &str) -> Result<Vec<u8>, ForgeError> {
    let mut entry = archive
        .by_name(entry_name(name))
        .map_err(|error| ForgeError::Unreadable(format!("{name}: {error}")))?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn open(path: &Path) -> Result<zip::ZipArchive<File>, ForgeError> {
    zip::ZipArchive::new(File::open(path)?)
        .map_err(|error| ForgeError::Unreadable(error.to_string()))
}

/// Reads `install_profile.json` and the `version.json` it points to.
pub fn read_installer(path: &Path) -> Result<Installer, ForgeError> {
    let mut archive = open(path)?;
    let profile_bytes = read_entry(&mut archive, "install_profile.json")?;
    let raw: serde_json::Value = serde_json::from_slice(&profile_bytes)
        .map_err(|error| ForgeError::Unreadable(error.to_string()))?;
    // The old format nests everything under "install"/"versionInfo".
    if raw.get("install").is_some() || raw.get("versionInfo").is_some() {
        return Err(ForgeError::LegacyInstaller);
    }
    let profile: InstallProfile =
        serde_json::from_value(raw).map_err(|error| ForgeError::Unreadable(error.to_string()))?;
    let version_json = read_entry(&mut archive, &profile.json)?;
    Ok(Installer {
        version_json,
        profile,
    })
}

/// Copies one installer entry to `destination`.
pub fn extract(installer: &Path, entry: &str, destination: &Path) -> Result<(), ForgeError> {
    let mut archive = open(installer)?;
    let bytes = read_entry(&mut archive, entry)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(destination, bytes)?;
    Ok(())
}

/// Whether the installer carries a library itself (older builds embed theirs
/// under `maven/`); the library is extracted when it does.
pub fn extract_embedded(installer: &Path, relative: &str, destination: &Path) -> bool {
    extract(installer, &format!("maven/{relative}"), destination).is_ok()
}

fn artifact_path(libraries: &Path, coordinate: &str) -> Result<PathBuf, ForgeError> {
    let coordinate: PackageCoordinate = coordinate
        .parse()
        .map_err(|_| ForgeError::Profile(format!("not an artifact: {coordinate}")))?;
    Ok(libraries.join(coordinate.repository_path()))
}

fn surrounded(text: &str, open: char, close: char) -> Option<&str> {
    text.strip_prefix(open)?.strip_suffix(close)
}

/// The places processor values refer to.
pub struct Paths<'a> {
    pub installer: &'a Path,
    pub libraries: &'a Path,
    /// The vanilla client jar.
    pub minecraft_jar: &'a Path,
    /// The launcher's data root.
    pub root: &'a Path,
    /// Scratch space for files taken out of the installer.
    pub work: &'a Path,
}

/// The named values processor arguments use: the profile's client data plus
/// the standard ones. `/path` data is extracted from the installer into `work`.
pub fn variables(
    profile: &InstallProfile,
    game: &str,
    paths: &Paths<'_>,
) -> Result<BTreeMap<String, String>, ForgeError> {
    let mut vars = BTreeMap::new();
    for (key, value) in &profile.data {
        let value = &value.client;
        let resolved = if let Some(coordinate) = surrounded(value, '[', ']') {
            artifact_path(paths.libraries, coordinate)?
                .to_string_lossy()
                .into_owned()
        } else if let Some(literal) = surrounded(value, '\'', '\'') {
            literal.to_owned()
        } else if value.starts_with('/') {
            let destination = paths.work.join(entry_name(value));
            extract(paths.installer, value, &destination)?;
            destination.to_string_lossy().into_owned()
        } else {
            value.clone()
        };
        vars.insert(key.clone(), resolved);
    }
    let path = |path: &Path| path.to_string_lossy().into_owned();
    vars.insert("SIDE".to_owned(), "client".to_owned());
    vars.insert("MINECRAFT_JAR".to_owned(), path(paths.minecraft_jar));
    vars.insert("MINECRAFT_VERSION".to_owned(), game.to_owned());
    vars.insert("ROOT".to_owned(), path(paths.root));
    vars.insert("INSTALLER".to_owned(), path(paths.installer));
    vars.insert("LIBRARY_DIR".to_owned(), path(paths.libraries));
    Ok(vars)
}

/// One argument or output with its references filled in: `{NAME}` is a
/// variable, `[artifact]` a library path, `'text'` a literal.
pub fn expand(
    text: &str,
    vars: &BTreeMap<String, String>,
    libraries: &Path,
) -> Result<String, ForgeError> {
    if let Some(name) = surrounded(text, '{', '}') {
        return vars
            .get(name)
            .cloned()
            .ok_or_else(|| ForgeError::Profile(format!("unknown value {{{name}}}")));
    }
    if let Some(coordinate) = surrounded(text, '[', ']') {
        return Ok(artifact_path(libraries, coordinate)?
            .to_string_lossy()
            .into_owned());
    }
    if let Some(literal) = surrounded(text, '\'', '\'') {
        return Ok(literal.to_owned());
    }
    // Inline references such as `{ROOT}/run.sh`.
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        match tail.find('}') {
            Some(end) if vars.contains_key(&tail[..end]) => {
                out.push_str(&vars[&tail[..end]]);
                rest = &tail[end + 1..];
            }
            _ => {
                out.push('{');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn sha1_of(path: &Path) -> io::Result<String> {
    crate::content::sha1_hex(path)
}

/// The files a processor promises and their SHA-1, expanded.
fn expected_outputs(
    processor: &Processor,
    vars: &BTreeMap<String, String>,
    libraries: &Path,
) -> Result<Vec<(PathBuf, String)>, ForgeError> {
    processor
        .outputs
        .iter()
        .map(|(file, digest)| {
            Ok((
                PathBuf::from(expand(file, vars, libraries)?),
                expand(digest, vars, libraries)?.to_ascii_lowercase(),
            ))
        })
        .collect()
}

/// Whether a processor's promised outputs are all present and intact, so it
/// need not run again. A processor that promises nothing always runs.
pub fn outputs_present(
    processor: &Processor,
    vars: &BTreeMap<String, String>,
    libraries: &Path,
) -> Result<bool, ForgeError> {
    let outputs = expected_outputs(processor, vars, libraries)?;
    Ok(!outputs.is_empty()
        && outputs
            .iter()
            .all(|(file, digest)| sha1_of(file).is_ok_and(|actual| &actual == digest)))
}

/// The main class named in a jar's manifest.
pub fn main_class(jar: &Path) -> Result<String, ForgeError> {
    let mut archive = open(jar)?;
    let manifest =
        String::from_utf8_lossy(&read_entry(&mut archive, "META-INF/MANIFEST.MF")?).into_owned();
    manifest
        .lines()
        .find_map(|line| line.strip_prefix("Main-Class:"))
        .map(|class| class.trim().to_owned())
        .filter(|class| !class.is_empty())
        .ok_or_else(|| ForgeError::Profile(format!("{} names no main class", jar.display())))
}

/// The command that runs one processor: `java -cp <jar + classpath> <main> <args>`.
pub fn command(
    processor: &Processor,
    vars: &BTreeMap<String, String>,
    libraries: &Path,
    java: &Path,
    host: &HostProfile,
) -> Result<Vec<String>, ForgeError> {
    let jar = libraries.join(processor.jar.repository_path());
    let main = main_class(&jar)?;
    let separator = if matches!(host.platform(), crate::environment::PlatformFamily::Windows) {
        ";"
    } else {
        ":"
    };
    let classpath = std::iter::once(jar)
        .chain(
            processor
                .classpath
                .iter()
                .map(|coordinate| libraries.join(coordinate.repository_path())),
        )
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(separator);
    let mut line = vec![
        java.to_string_lossy().into_owned(),
        "-cp".to_owned(),
        classpath,
        main,
    ];
    for arg in &processor.args {
        line.push(expand(arg, vars, libraries)?);
    }
    Ok(line)
}

/// Runs the client processors in order. A processor whose outputs are already
/// in place is skipped; after each run its outputs are checked.
pub async fn run_processors(
    profile: &InstallProfile,
    game: &str,
    paths: &Paths<'_>,
    java: &Path,
    log: &(dyn Fn(String) + Send + Sync),
    cancel: &CancellationToken,
) -> Result<(), ForgeError> {
    std::fs::create_dir_all(paths.work)?;
    let vars = variables(profile, game, paths)?;
    let host = HostProfile::current();
    for processor in profile.client_processors() {
        if cancel.is_cancelled() {
            return Err(ForgeError::Cancelled);
        }
        if outputs_present(processor, &vars, paths.libraries)? {
            continue;
        }
        let line = command(processor, &vars, paths.libraries, java, &host)?;
        let tool = processor.jar.to_string();
        log(format!("running {tool}"));
        let mut child = tokio::process::Command::new(&line[0]);
        child
            .args(&line[1..])
            .current_dir(paths.work)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        let output = tokio::select! {
            () = cancel.cancelled() => return Err(ForgeError::Cancelled),
            output = child.output() => output?,
        };
        if !output.status.success() {
            let printed = String::from_utf8_lossy(&output.stderr);
            let tail: Vec<&str> = printed.lines().rev().take(12).collect();
            return Err(ForgeError::Processor {
                tool,
                detail: format!(
                    "exit {:?}: {}",
                    output.status.code(),
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
                ),
            });
        }
        for (file, digest) in expected_outputs(processor, &vars, paths.libraries)? {
            let actual = sha1_of(&file)
                .map_err(|_| ForgeError::Output(format!("{} was not produced", file.display())))?;
            if actual != digest {
                return Err(ForgeError::Output(format!(
                    "{} has SHA-1 {actual}, expected {digest}",
                    file.display()
                )));
            }
        }
    }
    Ok(())
}

/// Whether the patched client the loader starts from is in place: the
/// `PATCHED` output exists (and matches `PATCHED_SHA` when the profile has it).
pub fn patched(profile: &InstallProfile, libraries: &Path) -> bool {
    let Some(target) = profile.data.get("PATCHED") else {
        return true;
    };
    let Some(coordinate) = surrounded(&target.client, '[', ']') else {
        return true;
    };
    let Ok(path) = artifact_path(libraries, coordinate) else {
        return false;
    };
    match profile
        .data
        .get("PATCHED_SHA")
        .and_then(|sha| surrounded(&sha.client, '\'', '\''))
    {
        Some(digest) => sha1_of(&path).is_ok_and(|actual| actual.eq_ignore_ascii_case(digest)),
        None => path.is_file(),
    }
}

#[cfg(test)]
mod tests;
