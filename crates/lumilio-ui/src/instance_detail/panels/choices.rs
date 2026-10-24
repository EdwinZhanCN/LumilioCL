use lumilio_core::ProjectKind;

use crate::tr_all;

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
