//! Java runtime discovery and selection.

mod choice;
mod installations;
mod locator;

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::{Path, PathBuf};

pub use runtime::{JavaRuntime, parse_major, rosetta_missing};
#[cfg(test)]
use runtime::{normalize_vendor, parse_release, rosetta_missing_at};

pub use self::choice::{
    JavaChoice, JavaNeeds, JavaRule, RuleStrength, choose, install_build, install_major,
};
pub use self::installations::InstanceJava;
pub(crate) use self::installations::Java;
pub use self::locator::JavaLocator;
#[cfg(test)]
use lumilio_minecraft::HostProfile;

#[cfg(test)]
mod tests;

pub(crate) mod downloads;
mod runtime;
