//! Launcher-wide settings and the accounts the user has added.
//!
//! Per-instance overrides live on the instance record; these are the defaults
//! they fall back to.

pub(crate) use current::{settle_current, settle_current_in};
pub use memory::{recommended_memory_mb, total_memory_mb};
pub(crate) use operations::Settings;
pub(crate) use store::validate_memory;
pub use store::{
    AccountEntry, AccountKind, AuthServerEntry, DownloadSourcePreference, LauncherSettings,
    MAX_MEMORY_MB, MirrorRule, SettingsError, SettingsStore, TERMS_VERSION,
};

mod current;
mod memory;
mod operations;
mod store;
