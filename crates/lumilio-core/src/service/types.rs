use crate::content::ContentError;
use crate::discover::{
    Catalog, CategoryTag, GameVersionTag, KindAbilities, LoaderTag, Project, Version,
};
use crate::instances::{Collection, InstanceRecord};

/// What the library page shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Library {
    pub instances: Vec<InstanceRecord>,
    pub collections: Vec<Collection>,
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
    /// The content source (plugin ID) the project is from, and its name.
    pub source: String,
    pub source_name: String,
}

/// A content source that is on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSourceInfo {
    /// The plugin ID; what project references name.
    pub id: String,
    pub name: String,
    pub catalog: Option<Catalog>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentEffect {
    /// Already in the requested state; nothing was touched or recorded.
    Unchanged,
    /// Renamed; this is the file name now.
    Renamed(String),
    Removed,
}
