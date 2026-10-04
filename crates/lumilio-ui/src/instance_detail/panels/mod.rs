//! The real Instance sections: Overview actions, Content, Worlds and History.
//!
//! A child module of `instance_detail`, so it shares the view's private state.
//! Copy for domain facts is mapped here; core types never carry display text.

mod data;
mod helpers;
mod history;
mod labels;
mod overview;
mod worlds;

#[cfg(test)]
mod tests;

pub use self::data::{Arrived, Confirm, Data};
pub(super) use self::helpers::{act, act_index, clock};
pub use self::labels::{export_notice, size_label};
pub use self::overview::{ProblemAction, hint_text, problem_action, problem_text, problem_tone};

use lumilio_core::ProjectKind;

/// The three content kinds, in the order of their sub-tabs.
pub const CONTENT_KINDS: [ProjectKind; 3] = [
    ProjectKind::Mod,
    ProjectKind::ResourcePack,
    ProjectKind::Shader,
];
pub const CONTENT_LABELS: [&str; 3] = ["Mod", "资源包", "光影"];
pub const HISTORY_LABELS: [&str; 3] = ["变更", "游玩记录", "快照"];
pub const WORLD_SORTS: [&str; 2] = ["最近游玩", "名称"];
