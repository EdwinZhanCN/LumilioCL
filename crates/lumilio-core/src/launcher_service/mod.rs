//! The headless launcher: one object that composes the capabilities and
//! answers what the interface asks (library, Home, create, launch, search,
//! install). Each method here hands the request to the capability that owns
//! it; Home and retrying a task, which span several, are composed here.
//!
//! Every method is `async` and `Send`, so any front end can run it on a
//! runtime; nothing here knows about a window. Resource-specific profile and
//! texture locks can span their own network reads to coalesce requests; global
//! settings and library locks do not.

mod accounts;
mod activity;
mod appearance;
mod composition;
mod content;
mod diagnostics;
mod game_settings;
mod home;
mod instances;
mod java;
mod launch;
mod plugins;
mod preferences;
mod worlds;

#[cfg(test)]
mod tests;

pub use composition::LauncherService;
