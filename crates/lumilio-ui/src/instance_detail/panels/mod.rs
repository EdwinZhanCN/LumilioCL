//! The real Instance sections: Overview actions, Content, Worlds and History.
//!
//! A child module of `instance_detail`, so it shares the view's private state.
//! Copy for domain facts is mapped here; core types never carry display text.

mod data;
mod helpers;
mod history;
mod labels;
mod overview;
mod screenshots;
mod servers;
mod worlds;

#[cfg(test)]
mod tests;

pub use choices::{CONTENT_KINDS, CONTENT_LABELS, HISTORY_LABELS, WORLD_SORTS, WORLD_SUBS};

pub use self::data::{Arrived, Confirm, Data, ServerState, Thumb};
pub(super) use self::helpers::{act, act_index, clock};
pub use self::labels::{export_notice, size_label};
pub use self::overview::{ProblemAction, problem_action, problem_text, problem_tone};
pub(super) use self::screenshots::SHOTS_PAGE;

mod choices;
