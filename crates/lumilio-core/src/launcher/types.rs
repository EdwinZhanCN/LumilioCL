use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

use crate::account::AuthSession;
use crate::forge_install::ForgeError;
use crate::install::InstallError;
use crate::instance::{InstanceRecord, Loader};
use crate::java::JavaRuntime;
use crate::launch::{LaunchDirectories, LaunchError};
use crate::launch_session::{LaunchFailure, LaunchProblem, LaunchSignal};
use crate::process::{LogStream, ProcessError};
use crate::release::ReleaseError;
use crate::tuning::{LaunchTuning, QuickPlay};

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
    /// Where to go once the game is up; `None` is the main menu.
    pub quick_play: Option<QuickPlay>,
}

/// A launch asking for a Java to be downloaded because none installed may
/// run the game: the major (`None` for the recommended one) and build,
/// answered with the installed runtime or why it could not be installed.
#[derive(Debug)]
pub struct JavaRequest {
    pub major: Option<u32>,
    pub build: crate::java_runtime::JavaBuild,
    pub reply: tokio::sync::oneshot::Sender<Result<JavaRuntime, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchUpdate {
    Signal(LaunchSignal),
    Log { stream: LogStream, text: String },
}

#[derive(Debug)]
pub enum LaunchServiceError {
    GameSettings(String),
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
    /// No installed Java fitted and downloading one failed.
    JavaInstall(String),
    /// The game needs an Intel Java and this Apple silicon Mac has no
    /// Rosetta 2 to run it.
    RosettaMissing,
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
            Self::GameSettings(error) => f.write_str(error),
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
            } => write!(
                f,
                "no installed Java can run this game; it needs Java {major}"
            ),
            Self::NoJava { required: None } => f.write_str("no Java installation found"),
            Self::JavaInstall(why) => {
                write!(f, "the Java this game needs could not be downloaded: {why}")
            }
            Self::RosettaMissing => f.write_str(
                "this game needs an Intel Java, which runs only under Rosetta 2; install it with softwareupdate --install-rosetta",
            ),
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

impl LaunchServiceError {
    /// What the person can act on, when this is something they can.
    pub fn problem(&self) -> Option<LaunchProblem> {
        match self {
            Self::NoJava { required } => Some(LaunchProblem::NoJava(*required)),
            Self::JavaInstall(_) => Some(LaunchProblem::JavaDownload),
            Self::RosettaMissing => Some(LaunchProblem::RosettaMissing),
            Self::Hook {
                when: "before launch",
                code,
                ..
            } => Some(LaunchProblem::BeforeLaunch(*code)),
            Self::QuickPlayWorldMissing(world) => Some(LaunchProblem::WorldMissing(world.clone())),
            _ => None,
        }
    }

    /// This error as the failed step of a launch.
    pub fn failure(&self) -> LaunchFailure {
        LaunchFailure::Step {
            message: self.to_string(),
            problem: self.problem(),
        }
    }
}
