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

pub use self::data::{Arrived, Confirm, Data, ServerState, Thumb};
pub(super) use self::helpers::{act, act_index, clock};
pub use self::labels::{export_notice, size_label};
pub use self::overview::{ProblemAction, problem_action, problem_text, problem_tone};
pub(super) use self::screenshots::SHOTS_PAGE;
#[cfg(test)]
pub(super) use self::servers::server_detail;

use crate::tr_all;
use lumilio_core::ProjectKind;

/// The three content kinds, in the order of their sub-tabs.
pub const CONTENT_KINDS: [ProjectKind; 3] = [
    ProjectKind::Mod,
    ProjectKind::ResourcePack,
    ProjectKind::Shader,
];

/// Label lists read through `Deref`, so each read takes the words again and a
/// language switch reaches the segment bars and dropdowns.
pub static CONTENT_LABELS: ContentLabels = ContentLabels;

pub struct ContentLabels;

impl std::ops::Deref for ContentLabels {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "instance-content-kind-mod",
            "instance-content-kind-resource-pack",
            "instance-content-kind-shader",
        ]
    }
}

pub static HISTORY_LABELS: HistoryLabels = HistoryLabels;

pub struct HistoryLabels;

impl std::ops::Deref for HistoryLabels {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "instance-history-changes",
            "instance-history-sessions",
            "instance-history-snapshots",
        ]
    }
}

pub static WORLD_SORTS: WorldSorts = WorldSorts;

pub struct WorldSorts;

impl std::ops::Deref for WorldSorts {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all!["library-sort-recent", "library-sort-name"]
    }
}

pub static WORLD_SUBS: WorldSubs = WorldSubs;

pub struct WorldSubs;

impl std::ops::Deref for WorldSubs {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all!["instance-worlds-world", "instance-worlds-server"]
    }
}
