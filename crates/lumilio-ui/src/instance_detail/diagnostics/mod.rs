//! The Instance page's Diagnostics tab (IA `instance/diagnostics.md`):
//! problems, the game log with a level filter and search, crash reports, and
//! a read-only browser of the game directory.
//!
//! A child module of `instance_detail`, so it shares the view's private state.

mod analysis;
mod files;
mod helpers;
mod logs;
mod selects;

#[cfg(test)]
mod tests;

mod panel;
