//! Connects the interface to the launcher service (plan 0007).
//!
//! The shell only renders and reports intents. Here each intent becomes work on
//! the worker runtime, and each result is handed back to the shell on the
//! interface thread. The shell is never touched from inside one of its own
//! updates: results always arrive from a spawned task.

mod context;
mod dispatch;
mod jobs;
mod open_game;
mod recovery_review;
mod settings;
mod terms;
mod update;

#[cfg(test)]
mod tests;

mod wiring;

#[cfg(test)]
use wiring::State;
pub use wiring::build;
use wiring::{ACTIVITY_LIMIT, Reload, Wiring};
