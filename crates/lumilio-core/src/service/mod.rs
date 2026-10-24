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
mod diagnostics;
mod game_settings;
mod install;
mod instances;
mod java;
mod launch;
mod library;
mod modpacks;
mod packs;
mod plugins;
mod portability;
mod preferences;
mod snapshots;
mod tracking;
mod types;
mod worlds;

#[cfg(test)]
mod tests;

pub use launcher_service::LauncherService;
use launcher_service::{DIAGNOSED_LIMIT, loader_text};

pub use self::types::Library;

mod launcher_service;
