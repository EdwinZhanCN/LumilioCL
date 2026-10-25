//! Connects the interface to the launcher service (plan 0007).
//!
//! The shell only renders and reports intents. Here each intent becomes work on
//! the worker runtime, and each result is handed back to the shell on the
//! interface thread. The shell is never touched from inside one of its own
//! updates: results always arrive from a spawned task.

mod context;
mod dispatch;
mod game_settings;
mod instance;
mod instance_content;
mod instance_plugins;
mod instance_write;
mod jobs;
mod launch;
mod logs;
mod settings;
mod terms;
mod update;

#[cfg(test)]
mod tests;

mod wiring;

#[cfg(test)]
use wiring::State;
pub use wiring::build;
use wiring::{ACTIVITY_LIMIT, GAME_LOG_FLUSH, GAME_LOG_LINES, Reload, Wiring};
