//! Each feature keeps its own state, the work it asks of the launcher, its
//! views and dialogs, and its tests. The shell composes them; features talk to
//! each other only through explicit inputs and events.

pub mod accounts;
pub mod activity;
pub mod discover;
pub mod game;
pub mod launch;
pub mod library;
pub mod new_game;
pub mod settings;
