use crate::account::AuthSession;
use crate::forge_install::ForgeError;
use crate::install::InstallError;
use crate::instance::{InstanceRecord, Loader};
use crate::java::JavaRuntime;
use crate::launch::{LaunchDirectories, LaunchError};
use crate::launch_session::LaunchSignal;
use crate::process::{LogStream, ProcessError};
use crate::release::ReleaseError;
use crate::tuning::{LaunchTuning, QuickPlay};
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

/// Everything a launch needs that is not part of the instance record.
#[derive(Clone, Debug)]
pub struct LaunchRequest {
    pub instance: InstanceRecord,
    pub directories: LaunchDirectories,
    /// Who plays: the identity values the game is started with.
    pub session: AuthSession,
    /// Where to fetch the release manifest when it is not on disk yet.
    pub manifest_url: Option<String>,
    /// Where to fetch the loader profile for a loader instance that is not
    /// installed yet (see `loader_profile_url`); for Forge and NeoForge, the
    /// address of their installer.
    pub loader_profile_url: Option<String>,
    pub runtimes: Vec<JavaRuntime>,
    /// Launcher-wide memory defaults; the instance's own settings win.
    pub default_max_memory_mb: Option<u32>,
    pub default_min_memory_mb: Option<u32>,
    /// How the game starts (launcher defaults; the instance's own JVM
    /// arguments are applied on top).
    pub tuning: LaunchTuning,
    /// Files downloaded at once; `None` leaves it to the source chain.
    pub download_concurrency: Option<u32>,
    /// Where to go once the game is up; `None` is the main menu.
    pub quick_play: Option<QuickPlay>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchUpdate {
    Signal(LaunchSignal),
    Log { stream: LogStream, text: String },
}

#[derive(Debug)]
pub enum LaunchServiceError {
    /// This loader cannot be installed or started yet.
    LoaderUnsupported(Loader),
    /// A loader instance without a loader version cannot name its release.
    LoaderVersionMissing,
    InvalidMemory,
    /// The manifest is neither on disk nor fetchable.
    ManifestUnavailable(String),
    Release(ReleaseError),
    Launch(LaunchError),
    Install(InstallError),
    /// No installed Java satisfies the release; carries the required major.
    NoJava {
        required: Option<u32>,
    },
    Process(ProcessError),
    /// This game version cannot start in a world or on a server directly.
    QuickPlayUnsupported {
        /// `"singleplayer"` or `"multiplayer"`.
        mode: &'static str,
    },
    /// The world to start in is not among the saves.
    QuickPlayWorldMissing(String),
    /// A command the user set up did not succeed.
    Hook {
        /// `"before launch"` or `"after exit"`.
        when: &'static str,
        code: Option<i32>,
        stopped: bool,
    },
    /// Preparing Forge or NeoForge from its installer failed.
    Loader(ForgeError),
    Cancelled,
}

impl Display for LaunchServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoaderUnsupported(loader) => {
                write!(f, "{loader:?} instances cannot be launched yet")
            }
            Self::LoaderVersionMissing => f.write_str("the instance has no loader version"),
            Self::InvalidMemory => f.write_str("effective memory values are out of range"),
            Self::ManifestUnavailable(reason) => {
                write!(f, "release manifest unavailable: {reason}")
            }
            Self::Release(error) => write!(f, "{error}"),
            Self::Launch(error) => write!(f, "{error}"),
            Self::Install(error) => write!(f, "{error}"),
            Self::NoJava {
                required: Some(major),
            } => write!(f, "Java {major} or newer is required but not installed"),
            Self::NoJava { required: None } => f.write_str("no Java installation found"),
            Self::Process(error) => write!(f, "{error}"),
            Self::QuickPlayUnsupported { mode } => write!(
                f,
                "this game version cannot start directly in {mode}; start it from the menu"
            ),
            Self::QuickPlayWorldMissing(name) => {
                write!(f, "there is no saved world {name:?} to start in")
            }
            Self::Hook {
                when,
                stopped: true,
                ..
            } => {
                write!(f, "the command {when} was stopped before it finished")
            }
            Self::Hook {
                when,
                code: Some(code),
                ..
            } => {
                write!(f, "the command {when} failed (exit code {code})")
            }
            Self::Hook { when, .. } => write!(f, "the command {when} failed"),
            Self::Loader(error) => write!(f, "{error}"),
            Self::Cancelled => f.write_str("launch cancelled"),
        }
    }
}

impl Error for LaunchServiceError {}
