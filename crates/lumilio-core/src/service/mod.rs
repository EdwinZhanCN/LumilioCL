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
mod skins;
mod snapshots;
mod third_party;
mod tracking;
mod types;
mod wardrobe;
mod worlds;

#[cfg(test)]
mod tests;

pub use launcher_service::LauncherService;
use launcher_service::{CREDENTIAL_PROBE, DIAGNOSED_LIMIT, TOKEN_MARGIN_SECONDS, loader_text};

pub use self::appearance::ProfileSnapshot;
pub use self::third_party::ThirdPartySignIn;
pub use self::types::Library;
pub use self::wardrobe::AppearanceApplyResult;

mod launcher_service;
