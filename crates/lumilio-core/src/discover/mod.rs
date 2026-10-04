//! Discover: finding mods, modpacks, resource packs and shaders on Modrinth.
//!
//! Search, project, and version shapes follow Modrinth's public API
//! documentation.

mod client;
mod error;
mod intent;
mod kinds;
mod project;
mod search;
mod tags;
mod versions;

#[cfg(test)]
mod tests;

pub use self::client::ModrinthClient;
pub use self::error::DiscoverError;
pub(crate) use self::intent::is_safe_file_name;
pub use self::intent::{IntentError, install_request};
pub use self::kinds::{ProjectKind, SortIndex, browse_page_url, project_page_url};
pub use self::project::{GalleryImage, Project, ProjectLinks};
pub use self::search::{Environment, SearchHit, SearchPage, SearchQuery, SideSupport, environment};
pub use self::tags::{CategoryTag, GameVersionTag, ProjectSummary};
#[cfg(test)]
pub(crate) use self::versions::decode_versions;
pub use self::versions::{
    Dependency, DependencyKind, ReleaseChannel, Version, VersionFile, fits, pick_version,
};

pub const API_BASE: &str = "https://api.modrinth.com";
pub const SITE_BASE: &str = "https://modrinth.com";
