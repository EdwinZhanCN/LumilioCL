//! Starting a game: what one launch keeps while it runs (how to stop it,
//! the world or server it goes into, what the game prints), and the checks
//! before it. The shell decides when to launch and asks the questions.

pub use launch_actions::{Launch, LaunchEvent, Readiness};

mod launch_actions;

#[cfg(test)]
mod tests;
