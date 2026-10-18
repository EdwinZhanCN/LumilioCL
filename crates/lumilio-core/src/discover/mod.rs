//! Discover: finding mods, modpacks, resource packs and shaders from the
//! enabled content source (a plugin; Modrinth ships with the launcher).
//!
//! The data shapes here are the launcher's own. A source speaks its service's
//! protocol; `source` asks it through the plugin host and `convert` maps its
//! answers to these types.

mod convert;
mod error;
mod intent;
mod kinds;
mod project;
mod query;
mod search;
mod source;
mod tags;
mod version_groups;
mod versions;

#[cfg(test)]
mod tests;

pub use self::error::DiscoverError;
pub(crate) use self::intent::is_safe_file_name;
pub use self::intent::{IntentError, install_request};
pub use self::kinds::{Catalog, ProjectKind, SortIndex, TextFormat};
pub use self::project::{GalleryImage, Project, ProjectLinks};
pub use self::query::{Pick, SearchQuery, Stance};
pub use self::search::{Environment, SearchHit, SearchPage, SideSupport, environment};
pub use self::source::{ContentClient, FileHashes, KindAbilities, SourceFilters};
pub use self::tags::{CategoryTag, GameVersionTag, LoaderTag, ProjectSummary};
pub use self::version_groups::{GroupLabel, VersionGroup, version_groups};
pub use self::versions::{
    Dependency, DependencyKind, PackFileKind, ReleaseChannel, ResolvedFile, Version, VersionFile,
    fits, pick_version,
};

/// The source a project reference without one means: everything recorded
/// before there was more than one source came from Modrinth.
pub const DEFAULT_SOURCE: &str = "lumilio.modrinth";

/// A project in one content source, such as the pack an instance was made
/// from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOrigin {
    pub source: String,
    pub project: String,
}

impl ProjectOrigin {
    /// As stored: the bare ID for [`DEFAULT_SOURCE`] (as before there were
    /// several sources), `<source>/<id>` otherwise. Plugin and project IDs
    /// never contain `/`.
    #[must_use]
    pub fn stored(&self) -> String {
        if self.source == DEFAULT_SOURCE {
            self.project.clone()
        } else {
            format!("{}/{}", self.source, self.project)
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Self {
        match text.split_once('/') {
            Some((source, project)) => Self {
                source: source.to_owned(),
                project: project.to_owned(),
            },
            None => Self {
                source: DEFAULT_SOURCE.to_owned(),
                project: text.to_owned(),
            },
        }
    }
}
