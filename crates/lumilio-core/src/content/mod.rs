//! What is installed in a game (mods, resource packs, shaders), where it came
//! from, and the [`Content`] capability that finds, installs and recognizes it.

pub use items::{
    ContentError, ContentItem, ModMetadata, import, read_mod_metadata, remove, scan, set_enabled,
    sha1_hex,
};
pub(crate) use items::{DISABLED_SUFFIX, folder};
pub use pending::AddedFile;
pub use sources::{ContentEntry, ContentList, ContentSource};
pub(crate) use sources::{Recognized, Summaries, assemble};
pub(crate) use state::Content;
pub use types::{
    ContentEffect, ContentResult, ContentSourceInfo, DependencyNeed, DependencyReport,
    DiscoverFilters, InstalledProject, ProjectDetail,
};

mod changes;
pub(crate) mod discover;
pub(crate) mod fingerprint;
mod install;
mod items;
mod pending;
mod recognize;
mod search;
mod sources;
mod state;
mod translate;
pub(crate) mod translation;
mod types;
pub(crate) mod updates;
