use std::path::{Path, PathBuf};

use futures_util::StreamExt as _;
use reqwest::Client;
use semver::Version;
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncWriteExt as _;

use crate::manifest::{AssetTarget, find_asset};

const WORKER: &str = "https://launcher.lumilio.org";
const GITHUB: &str = "https://github.com/EdwinZhanCN/LumilioCL";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateRestriction {
    DebugBuild,
    PortableWindows,
    PackageManagedLinux,
    UnsupportedInstall,
}

impl UpdateRestriction {
    pub(crate) fn from_build_value(value: &str) -> Self {
        match value {
            "portable-windows" => Self::PortableWindows,
            "package-managed-linux" => Self::PackageManagedLinux,
            _ => Self::UnsupportedInstall,
        }
    }
}

#[derive(Debug)]
pub enum UpdateError {
    Restricted(UpdateRestriction),
    UnsupportedPlatform(String),
    Manifest(String),
    Request(String),
    Io(std::io::Error),
    Install(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Restricted(reason) => write!(f, "updates are unavailable: {reason:?}"),
            Self::UnsupportedPlatform(platform) => {
                write!(f, "updates are not available for {platform}")
            }
            Self::Manifest(reason) => write!(f, "release manifest: {reason}"),
            Self::Request(reason) => write!(f, "update request: {reason}"),
            Self::Io(error) => write!(f, "update file: {error}"),
            Self::Install(reason) => write!(f, "install update: {reason}"),
        }
    }
}

impl std::error::Error for UpdateError {}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateRelease {
    pub(crate) version: Version,
    pub(crate) file_name: String,
    pub(crate) sha256: String,
}

impl UpdateRelease {
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckResult {
    UpToDate,
    Available(UpdateRelease),
}

/// How the launcher finishes after `UpdateClient::install`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Relaunch {
    /// Start this path again once the running process has exited
    /// (GPUI's `App::restart`): the `.app` bundle on macOS, the executable on
    /// Linux.
    Restart(PathBuf),
    /// Quit; the installer that `install` started opens the new version.
    Quit,
}

#[derive(Clone)]
pub struct UpdateClient {
    http: Client,
    updates_dir: PathBuf,
    current: Version,
    /// Read at startup: on Linux, once the file is replaced, the running
    /// process's `current_exe` ends in " (deleted)".
    executable: Option<PathBuf>,
    restriction: Option<UpdateRestriction>,
}

impl UpdateClient {
    pub fn from_build(data_dir: impl Into<PathBuf>) -> Self {
        let executable = std::env::current_exe().ok();
        let restriction = if cfg!(debug_assertions) {
            Some(UpdateRestriction::DebugBuild)
        } else {
            option_env!("LUMILIO_UPDATE_EXPLANATION")
                .map(UpdateRestriction::from_build_value)
                .or_else(|| runtime_restriction(executable.as_deref()))
        };
        Self {
            http: Client::new(),
            updates_dir: data_dir.into().join("updates"),
            current: Version::parse(env!("CARGO_PKG_VERSION"))
                .expect("workspace package version is SemVer"),
            executable,
            restriction,
        }
    }

    pub fn restriction(&self) -> Option<UpdateRestriction> {
        self.restriction
    }

    pub async fn check(&self) -> Result<CheckResult, UpdateError> {
        if let Some(reason) = self.restriction() {
            return Err(UpdateError::Restricted(reason));
        }
        let target = AssetTarget::current()?;
        let primary = self.fetch_manifest(WORKER).await;
        let manifest = with_fallback(primary, self.fetch_manifest(GITHUB)).await?;
        let release = find_asset(&manifest, &target)?;
        if release.version <= self.current {
            Ok(CheckResult::UpToDate)
        } else {
            Ok(CheckResult::Available(release))
        }
    }

    pub async fn download(&self, release: &UpdateRelease) -> Result<PathBuf, UpdateError> {
        if let Some(reason) = self.restriction() {
            return Err(UpdateError::Restricted(reason));
        }
        tokio::fs::create_dir_all(&self.updates_dir).await?;
        let destination = self.updates_dir.join(&release.file_name);
        if destination.is_file() {
            if hash_file(&destination)
                .await
                .is_ok_and(|hash| hash == release.sha256)
            {
                return Ok(destination);
            }
            let _ = tokio::fs::remove_file(&destination).await;
        }
        let partial = destination.with_extension(format!(
            "{}.part",
            destination
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("update")
        ));
        let primary = self
            .download_from(WORKER, release, &partial, &destination)
            .await;
        with_fallback(
            primary,
            self.download_from(GITHUB, release, &partial, &destination),
        )
        .await?;
        Ok(destination)
    }

    /// Whether `install` replaces the launcher while it keeps running
    /// (macOS, Linux). The Windows installer needs the launcher closed, so
    /// there it runs only when the user asks to restart.
    pub fn installs_while_running(&self) -> bool {
        crate::install::WHILE_RUNNING
    }

    /// Installs a verified update. It blocks on external tools: call it off
    /// the async runtime.
    pub fn install(&self, update: &Path) -> Result<Relaunch, UpdateError> {
        if let Some(reason) = self.restriction() {
            return Err(UpdateError::Restricted(reason));
        }
        let executable = self
            .executable
            .as_deref()
            .ok_or_else(|| UpdateError::Install("cannot locate the running launcher".to_owned()))?;
        crate::install::install(executable, update).map_err(UpdateError::Install)
    }

    async fn fetch_manifest(&self, base: &str) -> Result<String, UpdateError> {
        let url = format!("{base}/releases/latest/download/SHA256SUMS.txt");
        self.http
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| UpdateError::Request(error.to_string()))?
            .text()
            .await
            .map_err(|error| UpdateError::Request(error.to_string()))
    }

    async fn download_from(
        &self,
        base: &str,
        release: &UpdateRelease,
        partial: &Path,
        destination: &Path,
    ) -> Result<(), UpdateError> {
        let url = format!(
            "{base}/releases/download/v{}/{}",
            release.version, release.file_name
        );
        let result = async {
            let response = self
                .http
                .get(url)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .map_err(|error| UpdateError::Request(error.to_string()))?;
            let mut stream = response.bytes_stream();
            let mut file = tokio::fs::File::create(partial).await?;
            let mut hasher = Sha256::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| UpdateError::Request(error.to_string()))?;
                hasher.update(&chunk);
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            file.sync_all().await?;
            let digest = hex_digest(&hasher.finalize());
            if digest != release.sha256 {
                return Err(UpdateError::Manifest(format!(
                    "checksum mismatch for {}",
                    release.file_name
                )));
            }
            drop(file);
            tokio::fs::rename(partial, destination).await?;
            Ok(())
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(partial).await;
        }
        result
    }
}

fn runtime_restriction(executable: Option<&Path>) -> Option<UpdateRestriction> {
    let Some(running) = executable else {
        return Some(UpdateRestriction::UnsupportedInstall);
    };
    #[cfg(target_os = "macos")]
    {
        // The installer replaces the whole bundle.
        if !running
            .ancestors()
            .any(|path| path.extension().is_some_and(|extension| extension == "app"))
        {
            return Some(UpdateRestriction::UnsupportedInstall);
        }
    }
    #[cfg(target_os = "linux")]
    {
        let Some(home) = std::env::var_os("HOME") else {
            return Some(UpdateRestriction::UnsupportedInstall);
        };
        let Ok(expected) = PathBuf::from(home)
            .join(".local/lib/lumiliocl/lumiliocl")
            .canonicalize()
        else {
            return Some(UpdateRestriction::UnsupportedInstall);
        };
        let Ok(running) = running.canonicalize() else {
            return Some(UpdateRestriction::UnsupportedInstall);
        };
        if running != expected {
            return Some(UpdateRestriction::UnsupportedInstall);
        }
    }
    #[cfg(target_os = "windows")]
    {
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            let Some(root) = std::env::var_os(variable) else {
                continue;
            };
            let root = PathBuf::from(root);
            if running.starts_with(root) {
                return Some(UpdateRestriction::UnsupportedInstall);
            }
        }
    }
    None
}

async fn with_fallback<T>(
    primary: Result<T, UpdateError>,
    fallback: impl std::future::Future<Output = Result<T, UpdateError>>,
) -> Result<T, UpdateError> {
    match primary {
        Ok(value) => Ok(value),
        Err(primary_error) => fallback.await.map_err(|fallback_error| {
            UpdateError::Request(format!(
                "{primary_error}; fallback failed: {fallback_error}"
            ))
        }),
    }
}

async fn hash_file(path: &Path) -> Result<String, UpdateError> {
    use tokio::io::AsyncReadExt as _;
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
pub(crate) async fn fallback_for_test<T>(
    primary: Result<T, UpdateError>,
    fallback: impl std::future::Future<Output = Result<T, UpdateError>>,
) -> Result<T, UpdateError> {
    with_fallback(primary, fallback).await
}
