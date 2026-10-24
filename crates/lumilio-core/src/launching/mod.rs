//! From an instance record to a running game and back: preparing and running
//! the launch, the game process, the session it publishes and what a launch
//! that never finished leaves for the next start.

pub(crate) use coordinator::{LaunchParts, LaunchSteps, Launching, serving_java};
pub(crate) use launcher::JavaRequest;
pub use launcher::{
    GAME_OUTPUT_LOG, LaunchRequest, LaunchServiceError, LaunchUpdate, Launcher, runtimes_root,
};
pub(crate) use marker::{SessionMarker, settle_sessions};
pub(crate) use process::background_command;
pub use process::{
    DEFAULT_SETTLE, GameEvent, GameExit, GameOptions, LogStream, ProcessError, command_line, run,
};
pub use readiness::{LaunchReadiness, LaunchRequirement};
pub use session::{
    LaunchFailure, LaunchPhase, LaunchProblem, LaunchSession, LaunchSignal, LaunchStatus,
};

mod coordinator;
mod launcher;
mod marker;
mod process;
mod readiness;
mod session;
