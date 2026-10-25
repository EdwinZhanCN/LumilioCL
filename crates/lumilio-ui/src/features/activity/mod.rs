//! The Activity page: the launcher's tasks as rows, the downloads of a running
//! task, and what can be done to one (pause, cancel, retry, clear the ended
//! ones). [`Activity`] draws the page and does the work; the shell owns it,
//! gives it what it shows and passes its events on.

pub use activity_actions::{Activity, ActivityEvent};
pub use page::{ActivityInputs, ActivityView};
pub use view::ACTIVITY_TABS;

mod activity_actions;
mod page;
mod view;

#[cfg(test)]
mod tests;
