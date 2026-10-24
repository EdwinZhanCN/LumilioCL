//! Page state shared by the live pages: which tab each page is on and the
//! small choices (sort, filter, settings tab) the person made.

pub use view_state::ViewState;

#[cfg(test)]
use crate::kit::ViewIntent;

pub mod live;
pub mod settings;

#[cfg(test)]
mod tests;

mod view_state;
