//! Java runtimes the launcher installs itself (`ADR 0014`).
//!
//! Two sources. Mojang publishes the runtimes its own launcher uses as a
//! public index: per platform, per component, a manifest naming every file
//! with its size, SHA-1 and address; this module reads those documents.
//! Oracle GraalVM ships one archive per platform ([`graalvm`]), unpacked by
//! [`archive`]. The service does the downloading.

pub mod archive;
pub mod graalvm;

#[cfg(test)]
use std::path::Path;

#[cfg(test)]
use distribution::choose_component;
use distribution::link_stays_inside;
#[cfg(test)]
pub(crate) use distribution::platform_key;
pub use distribution::{
    Entry, INDEX_URL, JavaBuild, JavaRuntimeError, JavaSource, choose_for_host, is_trusted_source,
    offered_majors, parse_manifest, place, platform_keys, source_for,
};

#[cfg(test)]
use lumilio_minecraft::{HostProfile, MachineArchitecture, PlatformFamily};

#[cfg(test)]
mod tests;

mod distribution;
