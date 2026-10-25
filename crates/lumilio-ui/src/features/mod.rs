//! Each feature keeps its own state, the work it asks of the launcher, its
//! views and dialogs, and its tests. The shell composes them; features talk to
//! each other only through explicit inputs and events.

pub mod activity;
pub mod discover;
pub mod library;
pub mod new_game;
