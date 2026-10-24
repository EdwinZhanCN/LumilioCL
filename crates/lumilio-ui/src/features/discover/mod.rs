//! Discover: browsing the content sources, a project's detail, and installing
//! what is chosen. [`Discover`] owns the page's search, the details it opens
//! and the installs it starts; the shell gives it the library and the current
//! game, shows the details it opens, and passes its events on.

pub use page::{Choices, Discover, DiscoverEvent, OpenedProject};
pub use state::DiscoverState;

pub mod dependencies;
pub mod detail;
pub mod model;

mod controls;
mod install;
mod page;
mod queries;
mod search;
mod state;
mod translation;
mod view;

#[cfg(test)]
mod tests;
