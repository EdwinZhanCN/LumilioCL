//! Release discovery, verified downloads, and platform installation for the
//! launcher. The crate has no UI dependency; the app owns polling and state.

//! Stable-release discovery, checksum verification and platform installation
//! for the launcher (ADR 0042).

mod client;
mod install;
mod manifest;

#[cfg(test)]
mod tests;

pub use self::client::{
    CheckResult, Relaunch, UpdateClient, UpdateError, UpdateRelease, UpdateRestriction,
};
