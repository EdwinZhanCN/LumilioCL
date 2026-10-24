//! Host-owned settings and failure isolation for synchronous plugin code.

mod access;
mod analysis;
mod content;
mod context;
mod launch;
mod map;
mod model;
mod native;
mod network;
mod tabs;
pub(crate) mod write;

#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use std::time::Duration;

pub(crate) use access::{read_info, resolve_write};
pub use analysis::PluginFinding;
pub use content::PluginContentSource;
use context::Context;
#[cfg(test)]
use lumilio_plugin_api::{API_VERSION, HostContext, Manifest, Plugin, PluginError, PluginState};
pub(crate) use map::SourceStamp;
pub use map::{MapFailure, MapProviders};
use plugin_host::{Entry, Preferences, enabled, isolated};
pub use plugin_host::{PluginHost, PluginInfo, PluginStatus};
pub use tabs::{PluginEffect, PluginTab};

#[cfg(test)]
mod tests;

mod plugin_host;
