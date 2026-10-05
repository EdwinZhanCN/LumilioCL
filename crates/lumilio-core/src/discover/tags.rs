use super::kinds::ProjectKind;

/// A category a project can be filed under, for one project type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryTag {
    pub name: String,
    /// The source's grouping, e.g. `categories`, `features`, `resolutions`.
    pub header: String,
    pub kind: ProjectKind,
}

/// A loader (or shader loader, or platform) and the project types it serves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoaderTag {
    pub name: String,
    /// The source's own names (`mod`, `plugin`, `datapack`, …), not only ours.
    pub project_types: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameVersionTag {
    pub version: String,
    /// A full release, as opposed to a snapshot or pre-release.
    pub release: bool,
    /// A snapshot; what is neither this nor a release is a legacy version
    /// (alpha, beta, classic).
    pub snapshot: bool,
    pub published: String,
}

/// What an installed file's label needs to know about its project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub kind: Option<ProjectKind>,
    pub icon_url: Option<String>,
    pub author: Option<String>,
}
