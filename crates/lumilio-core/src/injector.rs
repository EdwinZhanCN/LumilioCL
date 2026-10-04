//! The authlib-injector agent: the jar that makes the game talk to another
//! authentication server (or to the launcher's own skin server).
//!
//! HMCL ships the jar inside its package and also carries the downloader
//! (`HMCLCore/.../auth/authlibinjector/AuthlibInjectorDownloader.java`,
//! Copyright (C) 2020 huangyuhui and contributors, GPL-3.0-or-later; ADR
//! 0011). We fetch it the first time it is needed, from the
//! official artifact index through the configured source chain, and only keep
//! a file whose SHA-256 matches the index and whose manifest says it is
//! authlib-injector: it runs inside the game, so it is never taken on trust.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::fetch::fetch_document;
use crate::transfer::{SourceChain, Transport};

/// The index of the latest build.
pub const LATEST_URL: &str = "https://authlib-injector.yushi.moe/artifact/latest.json";
const FILE_NAME: &str = "authlib-injector.jar";
/// The agent is a few hundred kilobytes; a larger answer is not it.
const JAR_LIMIT: usize = 8 * 1024 * 1024;

/// An authlib-injector jar on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Injector {
    pub path: PathBuf,
    pub build_number: u32,
    pub version: String,
}

#[derive(Debug)]
pub enum InjectorError {
    Io(io::Error),
    /// The index or the jar could not be fetched or did not check out.
    Unavailable(String),
}

impl Display for InjectorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "authlib-injector storage failed: {error}"),
            Self::Unavailable(why) => write!(f, "could not get authlib-injector: {why}"),
        }
    }
}

impl Error for InjectorError {}

impl From<io::Error> for InjectorError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Deserialize)]
struct LatestWire {
    build_number: u32,
    download_url: String,
    #[serde(default)]
    checksums: std::collections::BTreeMap<String, String>,
}

/// Reads what the jar says about itself, or `None` when it is not
/// authlib-injector.
fn inspect(bytes: &[u8]) -> Option<(u32, String)> {
    let mut archive = zip::ZipArchive::new(io::Cursor::new(bytes)).ok()?;
    let mut manifest = String::new();
    archive
        .by_name("META-INF/MANIFEST.MF")
        .ok()?
        .take(64 * 1024)
        .read_to_string(&mut manifest)
        .ok()?;
    let attribute = |name: &str| {
        manifest.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == name).then(|| value.trim().to_owned())
        })
    };
    if attribute("Implementation-Title")? != "authlib-injector" {
        return None;
    }
    Some((
        attribute("Build-Number")?.parse().ok()?,
        attribute("Implementation-Version")?,
    ))
}

/// The jar already kept in `dir`, if it is a real one.
#[must_use]
pub fn installed(dir: &Path) -> Option<Injector> {
    let path = dir.join(FILE_NAME);
    let bytes = fs::read(&path)
        .ok()
        .filter(|bytes| bytes.len() <= JAR_LIMIT)?;
    let (build_number, version) = inspect(&bytes)?;
    Some(Injector {
        path,
        build_number,
        version,
    })
}

async fn latest(
    transport: &(impl Transport + ?Sized),
    chain: &SourceChain,
) -> Result<LatestWire, InjectorError> {
    let bytes = fetch_document(transport, &chain.candidates(LATEST_URL))
        .await
        .map_err(|error| InjectorError::Unavailable(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        InjectorError::Unavailable(format!("the index is not understood: {error}"))
    })
}

/// Downloads the latest build into `dir`, unless `dir` already has it or a
/// newer one.
async fn update(
    transport: &(impl Transport + ?Sized),
    chain: &SourceChain,
    dir: &Path,
) -> Result<Injector, InjectorError> {
    let latest = latest(transport, chain).await?;
    if let Some(local) = installed(dir)
        && local.build_number >= latest.build_number
    {
        return Ok(local);
    }
    let expected = latest
        .checksums
        .get("sha256")
        .ok_or_else(|| InjectorError::Unavailable("the index has no checksum".to_owned()))?;
    let bytes = fetch_document(transport, &chain.candidates(&latest.download_url))
        .await
        .map_err(|error| InjectorError::Unavailable(error.to_string()))?;
    if bytes.len() > JAR_LIMIT {
        return Err(InjectorError::Unavailable(
            "the file is too large".to_owned(),
        ));
    }
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if !actual.eq_ignore_ascii_case(expected.trim()) {
        return Err(InjectorError::Unavailable(
            "the file does not match its checksum".to_owned(),
        ));
    }
    let (build_number, version) = inspect(&bytes)
        .ok_or_else(|| InjectorError::Unavailable("the file is not authlib-injector".to_owned()))?;
    fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    // Written whole, then renamed in, so a launch never sees half a jar.
    let temporary = dir.join(format!(".{FILE_NAME}.part"));
    fs::write(&temporary, &bytes)?;
    fs::rename(&temporary, &path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })?;
    Ok(Injector {
        path,
        build_number,
        version,
    })
}

/// The jar to launch with: the one on disk, or the latest, downloaded now.
pub async fn ensure(
    transport: &(impl Transport + ?Sized),
    chain: &SourceChain,
    dir: &Path,
) -> Result<Injector, InjectorError> {
    match installed(dir) {
        Some(local) => Ok(local),
        None => update(transport, chain, dir).await,
    }
}

/// Looks for a newer build and keeps it. A failure changes nothing: the jar on
/// disk keeps working.
pub async fn refresh(
    transport: &(impl Transport + ?Sized),
    chain: &SourceChain,
    dir: &Path,
) -> Result<Injector, InjectorError> {
    update(transport, chain, dir).await
}

/// The JVM arguments that load the agent against `api_root`; `prefetched` is
/// the server's metadata, so the game does not ask for it again.
#[must_use]
pub fn jvm_arguments(jar: &Path, api_root: &str, prefetched: Option<&str>) -> Vec<String> {
    let mut arguments = vec![
        format!("-javaagent:{}={api_root}", jar.display()),
        "-Dauthlibinjector.side=client".to_owned(),
    ];
    if let Some(metadata) = prefetched {
        arguments.push(format!(
            "-Dauthlibinjector.yggdrasil.prefetched={}",
            base64::engine::general_purpose::STANDARD.encode(metadata)
        ));
    }
    arguments
}

#[cfg(test)]
mod tests;
