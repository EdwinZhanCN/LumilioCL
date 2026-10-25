//! A game's page: what it asks of the launcher (the game's record, its
//! sections, writes, options, logs and plugin tabs) and the map and studio
//! windows it opens. [`Game`] does the work; the page is the shell's
//! [`InstanceDetailView`](crate::instance_detail::InstanceDetailView), whose
//! handler the shell builds: it keeps what moves the window for itself, passes
//! playing on to the application and gives the rest to [`GameWork`].

pub use game_actions::{Game, GameEvent, GameWindows, GameWork};

mod answer;
mod content;
mod files;
mod game_actions;
mod load;
mod logs;
mod options;
mod plugins;
mod write;

#[cfg(test)]
mod tests;
