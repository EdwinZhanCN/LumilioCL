use crate::activity::CancellationToken;
use crate::content::ContentError;
use crate::injector::InjectorError;
use crate::instance::StoreError;
use crate::launcher::LaunchServiceError;
use crate::microsoft::AuthError;
use crate::modpack;
use crate::screenshots::ScreenshotError;
use crate::servers::{PingError, ServerError};
use crate::settings::SettingsError;
use crate::skin::SkinError;
use crate::snapshots::SnapshotError;
use crate::worlds::WorldError;
use crate::yggdrasil::YggdrasilError;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ServiceError {
    Plugin(lumilio_plugin_api::PluginError),
    Store(StoreError),
    Settings(SettingsError),
    /// A network document could not be fetched or understood.
    Remote(String),
    /// No content source plugin is enabled (or the one that was has stopped).
    NoContentSource,
    NoSuchInstance(String),
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
    /// The sign-in the person is choosing a character for is gone.
    NoPendingSignIn,
    Export(crate::pack_export::ExportError),
    Snapshot(SnapshotError),
    Io(std::io::Error),
}

impl Display for ServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plugin(error) => write!(f, "plugin: {error}"),
            Self::Store(error) => write!(f, "{error}"),
            Self::Settings(error) => write!(f, "{error}"),
            Self::Remote(message) => write!(f, "could not reach the service: {message}"),
            Self::NoContentSource => f.write_str("no content source is available"),
            Self::RootBusy(root) => write!(
                f,
                "launcher data at {} is already in use; close the other launcher and retry",
                root.display()
            ),
            Self::InstanceBusy(id) => write!(f, "instance {id:?} has an operation in progress"),
            Self::NoSuchInstance(id) => write!(f, "no instance {id:?}"),
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
            Self::NoPendingSignIn => f.write_str("this sign-in is no longer waiting for a choice"),
            Self::Export(error) => write!(f, "{error}"),
            Self::Snapshot(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ServiceError {}

impl From<crate::discover::DiscoverError> for ServiceError {
    fn from(error: crate::discover::DiscoverError) -> Self {
        match error {
            crate::discover::DiscoverError::NoSource => Self::NoContentSource,
            other => Self::Remote(other.to_string()),
        }
    }
}

impl From<SnapshotError> for ServiceError {
    fn from(error: SnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

impl From<crate::pack_export::ExportError> for ServiceError {
    fn from(error: crate::pack_export::ExportError) -> Self {
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
pub(super) async fn read_unless_cancelled<V>(
    cancel: &CancellationToken,
    read: impl Future<Output = Result<V, ServiceError>>,
) -> Result<V, ServiceError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(ServiceError::Cancelled),
        result = read => result,
    }
}
