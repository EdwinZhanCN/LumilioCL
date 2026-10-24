//! The headless launcher: one object that owns the stores and answers what the
//! interface asks (library, Home, create, launch, search, install).
//!
//! Every method is `async` and `Send`, so any front end can run it on a
//! runtime; nothing here knows about a window. Resource-specific profile and
//! texture locks can span their own network reads to coalesce requests; global
//! settings and library locks do not.

mod accounts;
mod activity;
mod appearance;
mod content;
mod content_install;
mod diagnostics;
mod discover;
mod game_settings;
mod install;
mod instances;
mod java;
mod launch;
mod library;
mod maintenance;
mod models;
mod modpacks;
mod packs;
mod pending;
mod plugins;
mod portability;
mod preferences;
mod recognize;
mod schematic_ops;
mod schematic_packs;
mod screenshots;
mod servers;
mod skins;
mod snapshots;
mod third_party;
mod tracking;
mod translation;
mod types;
mod wardrobe;
mod world_map;
mod worlds;

#[cfg(test)]
mod tests;

pub use launcher_service::LauncherService;
use launcher_service::{CREDENTIAL_PROBE, DIAGNOSED_LIMIT, TOKEN_MARGIN_SECONDS, loader_text};

pub use self::appearance::ProfileSnapshot;
pub use self::java::InstanceJava;
pub use self::models::ModelPreview;
pub use self::pending::AddedFile;
pub use self::third_party::ThirdPartySignIn;
pub use self::types::{
    ContentEffect, ContentResult, ContentSourceInfo, DependencyNeed, DependencyReport,
    DiscoverFilters, GameLogSource, GameLogs, InstalledProject, Library, ProjectDetail,
};
pub use self::wardrobe::AppearanceApplyResult;

mod launcher_service;
