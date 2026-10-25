//! The launch service: from an instance record to a running game.
//!
//! It composes the catalog, release, install, repair-scan, Java, and process
//! pieces and reports everything as [`LaunchUpdate`]s, so a UI only has to
//! render a [`crate::LaunchSession`].

mod output;
mod phases;
mod progress;
mod types;

#[cfg(test)]
mod tests;

pub use runner::Launcher;
use runner::{LAUNCHER_NAME, QUICK_PLAY_LOG};

pub use self::output::GAME_OUTPUT_LOG;
pub use self::progress::runtimes_root;
pub use self::types::{JavaRequest, LaunchRequest, LaunchServiceError, LaunchUpdate};

mod runner;
