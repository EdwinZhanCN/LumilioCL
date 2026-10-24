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

pub use origin::{DEFAULT_SOURCE, ProjectOrigin};

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

mod origin;
