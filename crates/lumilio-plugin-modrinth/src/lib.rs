//! Modrinth's API protocol. All network I/O is supplied by the plugin host;
//! installation, hashing, mirrors and proxies remain the launcher's concern.

mod project;
mod protocol;
mod query;
mod search;
mod tags;
mod text;
mod updates;
mod versions;

#[cfg(test)]
mod tests;

#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
use lumilio_plugin_api::content::{
    Compatibility, ContentSource, DependencyKind, Environment, FileIdentity, Fingerprint, Pick,
    ProjectKind, ReleaseChannel, SearchQuery, Sort, Stance, VersionRef,
};
#[cfg(test)]
use lumilio_plugin_api::{API_VERSION, FetchRequest, HostContext, Permission, Plugin, PluginError};
#[cfg(test)]
use modrinth::API_BASE;
pub use modrinth::{ID, Modrinth};
#[cfg(test)]
use query::Query;

mod modrinth;
