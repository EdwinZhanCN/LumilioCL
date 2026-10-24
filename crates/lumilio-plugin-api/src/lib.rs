//! Synchronous, UI-independent contracts for the launcher's built-in plugins.
//! The host supplies all I/O and invokes plugin code on a worker thread.

mod analysis;
pub mod content;
mod launch;
pub mod map;
mod network;
#[cfg(test)]
mod tests;
mod view;

pub use analysis::{
    AnalysisInput, AnalysisSource, Analyzer, Finding, GameFacts, ModFact, Severity,
};
pub use content::ContentSource;
pub use launch::{DiscordActivity, LaunchEvent, LaunchObserver, LaunchOutcome, LaunchTarget};
pub use network::{FetchMethod, FetchRequest};
pub use plugin::{
    API_VERSION, DirEntry, DirPage, FetchResponse, FileInfo, FileStat, HostContext, MAX_PAGE,
    MAX_RANGE, Manifest, NativeCapability, Permission, Plugin, PluginError, PluginState,
    SettingField, SettingKind, SettingValue, Words,
};
pub use view::{ActionId, Effect, ImageData, InstanceTab, KeyKind, ListItem, TabState, Tone, View};

mod plugin;
