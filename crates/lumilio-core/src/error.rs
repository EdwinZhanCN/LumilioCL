use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::path::PathBuf;

use crate::accounts::injector::InjectorError;
use crate::accounts::microsoft::AuthError;
use crate::accounts::yggdrasil::YggdrasilError;
use crate::activity::CancellationToken;
use crate::appearance::skin::SkinError;
use crate::content::ContentError;
use crate::instances::modpack;
use crate::instances::snapshot_files::SnapshotError;
use crate::instances::{InstanceBusy, StoreError};
use crate::launching::LaunchServiceError;
use crate::settings::SettingsError;
use crate::worlds::WorldError;
use crate::worlds::multiplayer::{PingError, ServerError};
use crate::worlds::screenshot_files::ScreenshotError;

#[derive(Debug)]
pub enum ServiceError {
    GameSettings(crate::GameSettingsError),
    Plugin(lumilio_plugin_api::PluginError),
    Store(StoreError),
    Settings(SettingsError),
    /// A network document could not be fetched or understood.
    Remote(String),
    /// No content source plugin is enabled (or the one that was has stopped).
    NoContentSource,
    /// No enabled content source reads this catalog's IDs (a CurseForge
    /// pack with the CurseForge source off).
    NoCatalogSource(crate::content::discover::Catalog),
    /// The author lets people download this file only on the source's site:
    /// the file's name and its page there.
    ManualDownload {
        file: String,
        page: Option<String>,
    },
    NoSuchInstance(String),
    /// The game's own folder is not on disk (the path it should be at).
    GameFolderMissing(PathBuf),
    InstanceBusy(String),
    RootBusy(PathBuf),
    /// The project has no file that fits the instance.
    NoCompatibleVersion,
    Launch(LaunchServiceError),
    Install(String),
    Cancelled,
    /// Nothing can start until an account is added and selected.
    NoAccount,
    /// The selected Microsoft account must sign in again (its name).
    SignInRequired(String),
    /// Signing in or refreshing failed.
    Auth(AuthError),
    /// The chosen file or folder is not (part of) a Java installation.
    NoJavaAt(PathBuf),
    Content(ContentError),
    World(WorldError),
    Server(ServerError),
    Screenshot(ScreenshotError),
    Ping(PingError),
    /// An authlib-injector server refused or could not be reached.
    Yggdrasil(YggdrasilError),
    /// The authlib-injector agent could not be fetched or stored.
    Injector(InjectorError),
    /// A skin could not be used.
    Skin(SkinError),
    Appearance(crate::appearance::skin::AppearanceError),
    /// The sign-in the person is choosing a character for is gone.
    NoPendingSignIn,
    Export(crate::instances::pack_export::ExportError),
    Snapshot(SnapshotError),
    Io(std::io::Error),
    /// The work stopped without an answer because it panicked: a launcher
    /// bug, with the panic's message.
    Panicked(String),
}

impl Display for ServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameSettings(error) => error.fmt(f),
            Self::Plugin(error) => write!(f, "plugin: {error}"),
            Self::Store(error) => write!(f, "{error}"),
            Self::Settings(error) => write!(f, "{error}"),
            Self::Remote(message) => write!(f, "could not reach the service: {message}"),
            Self::NoContentSource => f.write_str("no content source is available"),
            Self::NoCatalogSource(catalog) => {
                write!(f, "no enabled content source reads {catalog:?} IDs")
            }
            Self::ManualDownload { file, .. } => {
                write!(f, "{file} can only be downloaded on its source's website")
            }
            Self::RootBusy(root) => write!(
                f,
                "launcher data at {} is already in use; close the other launcher and retry",
                root.display()
            ),
            Self::InstanceBusy(id) => write!(f, "instance {id:?} has an operation in progress"),
            Self::NoSuchInstance(id) => write!(f, "no instance {id:?}"),
            Self::GameFolderMissing(path) => {
                write!(f, "the game's folder is missing: {}", path.display())
            }
            Self::NoCompatibleVersion => f.write_str("no version fits this instance"),
            Self::Launch(error) => write!(f, "{error}"),
            Self::Cancelled => f.write_str("operation was cancelled"),
            Self::NoAccount => f.write_str("add an account before starting the game"),
            Self::SignInRequired(name) => {
                write!(f, "the Microsoft account {name} must sign in again")
            }
            Self::Auth(error) => write!(f, "{error}"),
            Self::NoJavaAt(path) => write!(f, "no Java installation found at {}", path.display()),
            Self::Install(message) => write!(f, "install failed: {message}"),
            Self::Content(error) => write!(f, "{error}"),
            Self::World(error) => write!(f, "{error}"),
            Self::Server(error) => write!(f, "{error}"),
            Self::Screenshot(error) => write!(f, "{error}"),
            Self::Ping(error) => write!(f, "{error}"),
            Self::Yggdrasil(error) => write!(f, "{error}"),
            Self::Injector(error) => write!(f, "{error}"),
            Self::Skin(error) => write!(f, "{error}"),
            Self::Appearance(error) => write!(f, "{error}"),
            Self::NoPendingSignIn => f.write_str("this sign-in is no longer waiting for a choice"),
            Self::Export(error) => write!(f, "{error}"),
            Self::Snapshot(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Panicked(message) => write!(f, "the work stopped unexpectedly: {message}"),
        }
    }
}

impl From<crate::GameSettingsError> for ServiceError {
    fn from(error: crate::GameSettingsError) -> Self {
        Self::GameSettings(error)
    }
}

impl From<lumilio_plugin_api::PluginError> for ServiceError {
    fn from(error: lumilio_plugin_api::PluginError) -> Self {
        Self::Plugin(error)
    }
}

impl Error for ServiceError {}

impl ServiceError {
    /// This error as the failed step of a launch, with what the person can
    /// act on when there is something.
    pub fn launch_failure(&self) -> crate::LaunchFailure {
        let problem = match self {
            Self::SignInRequired(name) => Some(crate::LaunchProblem::SignInRequired(name.clone())),
            Self::Auth(error) => Some(crate::LaunchProblem::Account(error.clone())),
            Self::GameFolderMissing(_) => Some(crate::LaunchProblem::GameFolderMissing),
            Self::Launch(error) => error.problem(),
            _ => None,
        };
        crate::LaunchFailure::Step {
            message: self.to_string(),
            problem,
        }
    }
}

impl From<crate::appearance::skin::AppearanceError> for ServiceError {
    fn from(error: crate::appearance::skin::AppearanceError) -> Self {
        Self::Appearance(error)
    }
}

impl From<crate::content::discover::DiscoverError> for ServiceError {
    fn from(error: crate::content::discover::DiscoverError) -> Self {
        match error {
            crate::content::discover::DiscoverError::NoSource => Self::NoContentSource,
            other => Self::Remote(other.to_string()),
        }
    }
}

impl From<SnapshotError> for ServiceError {
    fn from(error: SnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

impl From<crate::instances::pack_export::ExportError> for ServiceError {
    fn from(error: crate::instances::pack_export::ExportError) -> Self {
        Self::Export(error)
    }
}

impl From<WorldError> for ServiceError {
    fn from(error: WorldError) -> Self {
        Self::World(error)
    }
}

impl From<ServerError> for ServiceError {
    fn from(error: ServerError) -> Self {
        Self::Server(error)
    }
}

impl From<YggdrasilError> for ServiceError {
    fn from(error: YggdrasilError) -> Self {
        Self::Yggdrasil(error)
    }
}

impl From<InjectorError> for ServiceError {
    fn from(error: InjectorError) -> Self {
        Self::Injector(error)
    }
}

impl From<ScreenshotError> for ServiceError {
    fn from(error: ScreenshotError) -> Self {
        Self::Screenshot(error)
    }
}

impl From<PingError> for ServiceError {
    fn from(error: PingError) -> Self {
        Self::Ping(error)
    }
}

impl From<ContentError> for ServiceError {
    fn from(error: ContentError) -> Self {
        Self::Content(error)
    }
}

impl From<InstanceBusy> for ServiceError {
    fn from(InstanceBusy(id): InstanceBusy) -> Self {
        Self::InstanceBusy(id)
    }
}

impl From<StoreError> for ServiceError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<AuthError> for ServiceError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
}

impl From<SettingsError> for ServiceError {
    fn from(error: SettingsError) -> Self {
        Self::Settings(error)
    }
}

impl From<LaunchServiceError> for ServiceError {
    fn from(error: LaunchServiceError) -> Self {
        Self::Launch(error)
    }
}

impl From<std::io::Error> for ServiceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Awaited work that did not finish: aborting it was a cancellation, a panic
/// is a failure of its own that the person must hear about.
impl From<tokio::task::JoinError> for ServiceError {
    fn from(error: tokio::task::JoinError) -> Self {
        if error.is_cancelled() {
            return Self::Cancelled;
        }
        let panic = error.into_panic();
        let message = panic
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "no message".to_owned());
        Self::Panicked(message)
    }
}

impl From<crate::transfer::TransferError> for ServiceError {
    fn from(error: crate::transfer::TransferError) -> Self {
        match error {
            crate::transfer::TransferError::Cancelled => Self::Cancelled,
            error => Self::Install(error.to_string()),
        }
    }
}

impl From<modpack::ModpackError> for ServiceError {
    fn from(error: modpack::ModpackError) -> Self {
        match error {
            modpack::ModpackError::Cancelled
            | modpack::ModpackError::Transfer(crate::transfer::TransferError::Cancelled) => {
                Self::Cancelled
            }
            error => Self::Install(error.to_string()),
        }
    }
}

/// Read-only work can be dropped safely; do not wrap file commits in this helper.
pub(crate) async fn read_unless_cancelled<V>(
    cancel: &CancellationToken,
    read: impl Future<Output = Result<V, ServiceError>>,
) -> Result<V, ServiceError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(ServiceError::Cancelled),
        result = read => result,
    }
}
