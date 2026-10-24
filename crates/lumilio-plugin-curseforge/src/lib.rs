//! CurseForge's API protocol (`https://docs.curseforge.com/rest-api/`). All
//! network I/O is supplied by the plugin host; installation, hashing, mirrors
//! and proxies remain the launcher's concern.
//!
//! The API needs a key. It is read when the launcher is built, from the
//! `CF_API_KEY` environment variable, and is never part of the repository.
//! A request that carries it stays on CurseForge's own host: the plugin host
//! does not send credentials to mirrors.

mod convert;
mod protocol;
mod query;
mod text;

#[cfg(test)]
mod tests;

#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
use curseforge::API_BASE;
pub use curseforge::{CurseForge, ID};
#[cfg(test)]
use lumilio_plugin_api::content::{
    Catalog, Compatibility, ContentSource, DependencyKind, FileIdentity, Fingerprint,
    FingerprintKind, PackFileKind, Pick, ProjectKind, SearchQuery, Sort, Stance, TextFormat,
    VersionRef,
};
#[cfg(test)]
use lumilio_plugin_api::{API_VERSION, FetchRequest, HostContext, Permission, Plugin, PluginError};

mod curseforge;
