use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use lumilio_transfer::{
    CancellationToken, ConnectionBudget, HttpTransport, RetryPolicy, TransferEngine, TransferError,
    TransferRequest,
};
use semver::Version;

use crate::manifest::{AssetTarget, find_asset};

const WORKER: &str = "https://launcher.lumilio.org";
const GITHUB: &str = "https://github.com/EdwinZhanCN/LumilioCL";
/// The release manifest lists one line per package; more is not one.
const MANIFEST_LIMIT: usize = 1024 * 1024;

/// A package that does not match its checksum is fetched once more before
/// the next source: downloading it again and again would not change it.
static UPDATE_RETRY: LazyLock<RetryPolicy> = LazyLock::new(|| {
    RetryPolicy::fixed(2, Duration::from_millis(500), Duration::from_secs(2))
        .expect("two attempts with a short delay is a valid policy")
});

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

impl From<TransferError> for UpdateError {
    fn from(error: TransferError) -> Self {
        match error {
            TransferError::FileSystem { .. } => Self::Io(std::io::Error::other(error.to_string())),
            other => Self::Request(other.to_string()),
        }
    }
}

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
    /// Downloads on the launcher's shared connections, kept part-way like
    /// any download (plan download-engine T4).
    engine: TransferEngine<HttpTransport>,
    updates_dir: PathBuf,
    current: Version,
    /// Read at startup: on Linux, once the file is replaced, the running
    /// process's `current_exe` ends in " (deleted)".
    executable: Option<PathBuf>,
    restriction: Option<UpdateRestriction>,
    /// The launcher site, then GitHub.
    bases: [String; 2],
}

impl UpdateClient {
    /// A client for this build, downloading over `budget`. Fails only when no
    /// HTTP client can be made (no TLS backend).
    pub fn from_build(
        data_dir: impl Into<PathBuf>,
        budget: ConnectionBudget,
    ) -> Result<Self, UpdateError> {
        let transport =
            HttpTransport::new().map_err(|error| UpdateError::Request(error.to_string()))?;
        let executable = std::env::current_exe().ok();
        let restriction = if cfg!(debug_assertions) {
            Some(UpdateRestriction::DebugBuild)
        } else {
            option_env!("LUMILIO_UPDATE_EXPLANATION")
                .map(UpdateRestriction::from_build_value)
                .or_else(|| runtime_restriction(executable.as_deref()))
        };
        Ok(Self {
            engine: TransferEngine::with_budget(transport, budget),
            updates_dir: data_dir.into().join("updates"),
            current: Version::parse(env!("CARGO_PKG_VERSION"))
                .expect("workspace package version is SemVer"),
            executable,
            restriction,
            bases: [WORKER.to_owned(), GITHUB.to_owned()],
        })
    }

    /// A client that may update, asking `bases` instead of the launcher site
    /// and GitHub.
    #[cfg(test)]
    pub(crate) fn for_test(updates_dir: PathBuf, bases: [String; 2]) -> Self {
        Self {
            engine: TransferEngine::new(HttpTransport::new().unwrap(), 2).unwrap(),
            updates_dir,
            current: Version::new(0, 0, 1),
            executable: None,
            restriction: None,
            bases,
        }
    }

    /// Keeps a download stopped part-way for the next try to continue,
    /// noted in `folder` as the launcher notes its own.
    #[must_use]
    pub fn keeping_partials(mut self, folder: impl Into<PathBuf>) -> Self {
        self.engine = self.engine.keeping_partials(folder);
        self
    }

    pub fn restriction(&self) -> Option<UpdateRestriction> {
        self.restriction
    }

    pub async fn check(&self) -> Result<CheckResult, UpdateError> {
        if let Some(reason) = self.restriction() {
            return Err(UpdateError::Restricted(reason));
        }
        let target = AssetTarget::current()?;
        let [site, github] = &self.bases;
        let primary = self.fetch_manifest(site).await;
        let manifest = with_fallback(primary, self.fetch_manifest(github)).await?;
        let release = find_asset(&manifest, &target)?;
        if release.version <= self.current {
            Ok(CheckResult::UpToDate)
        } else {
            Ok(CheckResult::Available(release))
        }
    }

    /// Downloads `release` from the launcher site, else from GitHub, and
    /// checks it against the release's SHA256SUMS. A package already here
    /// that matches is used as it is.
    pub async fn download(&self, release: &UpdateRelease) -> Result<PathBuf, UpdateError> {
        if let Some(reason) = self.restriction() {
            return Err(UpdateError::Restricted(reason));
        }
        tokio::fs::create_dir_all(&self.updates_dir).await?;
        let destination = self.updates_dir.join(&release.file_name);
        let sources = self.bases.clone().map(|base| {
            format!(
                "{base}/releases/download/v{}/{}",
                release.version, release.file_name
            )
        });
        let request = TransferRequest::new(
            format!("update:{}", release.file_name),
            sources,
            &destination,
        )
        .and_then(|request| request.expect_sha256(&release.sha256))
        .map_err(|error| UpdateError::Manifest(error.to_string()))?
        .with_retry_policy(UPDATE_RETRY.clone());
        self.engine
            .transfer(request, CancellationToken::new())
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
        let bytes = self
            .engine
            .fetch(&url, MANIFEST_LIMIT, &UPDATE_RETRY)
            .await
            .map_err(|failure| UpdateError::Request(format!("{url}: {failure}")))?;
        String::from_utf8(bytes).map_err(|error| UpdateError::Manifest(error.to_string()))
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

#[cfg(test)]
pub(crate) async fn fallback_for_test<T>(
    primary: Result<T, UpdateError>,
    fallback: impl std::future::Future<Output = Result<T, UpdateError>>,
) -> Result<T, UpdateError> {
    with_fallback(primary, fallback).await
}
