//! Builds the shell on the launcher service and adapts what only the
//! application owns: saved preferences and how they look, the terms, the
//! updater, the clipboard and the review hooks. The shell and its features
//! answer the pages themselves (plan workspace-architecture, D16).

mod dispatch;
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
use wiring::Wiring;
pub use wiring::build;
