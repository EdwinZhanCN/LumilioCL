use crate::activity_log::{ActiveTask, FinishedTask};
use crate::content::ContentError;
use crate::discover::{CategoryTag, GameVersionTag, KindAbilities, LoaderTag, Project, Version};
use crate::instance::{Collection, InstanceRecord};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// What the library page shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Library {
    pub instances: Vec<InstanceRecord>,
    pub collections: Vec<Collection>,
}

/// Running and recently finished work, for the Activity page and its badge.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivityView {
    pub active: Vec<ActiveTask>,
    /// Ids among `active` that [`LauncherService::cancel_task`] can stop.
    pub cancellable: BTreeSet<u64>,
    /// Newest first.
    pub finished: Vec<FinishedTask>,
}

/// Filter choices for Discover.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoverFilters {
    /// The content source these choices belong to.
    pub source: String,
    /// What that source can filter and sort by, per kind of project.
    pub abilities: Vec<KindAbilities>,
    pub categories: Vec<CategoryTag>,
    pub game_versions: Vec<GameVersionTag>,
    /// Empty when Modrinth could not say; the page then falls back to the
    /// usual loaders.
    pub loaders: Vec<LoaderTag>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDetail {
    pub project: Project,
    /// Newest first.
    pub versions: Vec<Version>,
    pub owner: Option<String>,
}

/// Seconds since the Unix epoch.
#[must_use]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// What happened to one file of a content change.
#[derive(Debug)]
pub struct ContentResult {
    /// The file name as asked for.
    pub file_name: String,
    pub outcome: Result<ContentEffect, ContentError>,
    /// False when the file changed but the history entry could not be written.
    pub recorded: bool,
}

/// What a mod's declared relations to other mods mean for one game.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyReport {
    /// Required and missing, nearest first.
    pub needs: Vec<DependencyNeed>,
    /// Suggested by the mod, not installed.
    pub optional: Vec<DependencyNeed>,
    /// Titles of installed mods the new one says it does not work with.
    pub conflicts: Vec<String>,
}

/// A project the game already has.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledProject {
    pub file_name: String,
    pub version_id: String,
    /// A newer version that fits this game, if there is one.
    pub update: Option<String>,
}

/// A required dependency the game does not have.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyNeed {
    pub project_id: String,
    pub title: String,
    /// The version that would be installed; `None` when none fits this game.
    pub version: Option<Version>,
}

/// How much of the log's end `logs` returns.
pub const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// How much of a log an export carries.
pub(super) const EXPORT_LOG_BYTES: u64 = 8 * 1024 * 1024;
/// How much of a crash report `crash_report` returns.
pub const REPORT_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameLogs {
    pub latest: Option<String>,
    pub crashes: Vec<crate::diagnostics::CrashReport>,
    pub files: Vec<crate::diagnostics::FileEntry>,
}

/// An instance-local source; file names are validated before any disk access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameLogSource {
    Live,
    Latest,
    File(String),
    Crash(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentEffect {
    /// Already in the requested state; nothing was touched or recorded.
    Unchanged,
    /// Renamed; this is the file name now.
    Renamed(String),
    Removed,
}
