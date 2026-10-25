//! The live pages not yet moved into `features`, and the small choices
//! (such as the account shown) made on them.

pub use view_state::ViewState;

#[cfg(test)]
use crate::kit::ViewIntent;

pub mod live;

#[cfg(test)]
mod tests;

mod view_state;
