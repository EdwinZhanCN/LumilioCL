//! Java runtime discovery and selection.

mod choice;
mod locator;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::environment::HostProfile;

pub use self::choice::{JavaChoice, JavaNeeds, JavaRule, RuleStrength, choose, install_major};
pub use self::locator::JavaLocator;

/// A Java installation the launcher can start the game with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaRuntime {
    home: PathBuf,
    executable: PathBuf,
    major: u32,
    version: String,
    vendor: Option<String>,
    architecture: Option<String>,
}

impl JavaRuntime {
    /// Reads the installation whose `release` file sits directly in `home`.
    ///
    /// Returns `None` when the file is missing or unreadable, its version is
    /// absent or unparseable, or the directory has no `bin/java` launcher.
    #[must_use]
    pub fn inspect(home: &Path) -> Option<Self> {
        let text = fs::read_to_string(home.join("release")).ok()?;
        let properties = parse_release(&text);
        let version = properties.get("JAVA_VERSION")?.clone();
        let major = parse_major(&version)?;
        let executable = ["bin/java", "bin/java.exe"]
            .into_iter()
            .map(|relative| home.join(relative))
            .find(|path| path.is_file())?;
        Some(Self {
            home: home.to_owned(),
            executable,
            major,
            version,
            vendor: properties
                .get("IMPLEMENTOR")
                .and_then(|vendor| normalize_vendor(vendor)),
            architecture: properties.get("OS_ARCH").cloned(),
        })
    }

    /// The installation at `path`, which may be its home, its `java`
    /// executable or a macOS bundle.
    #[must_use]
    pub fn at(path: &Path) -> Option<Self> {
        let mut homes = vec![path.to_owned(), path.join("Contents/Home")];
        if let Some(home) = path.parent().and_then(Path::parent) {
            homes.insert(0, home.to_owned());
        }
        homes.iter().find_map(|home| Self::inspect(home))
    }

    #[must_use]
    pub fn home(&self) -> &Path {
        &self.home
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub const fn major(&self) -> u32 {
        self.major
    }

    /// The full version string as the runtime reports it, e.g. `21.0.3`.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn vendor(&self) -> Option<&str> {
        self.vendor.as_deref()
    }

    #[must_use]
    pub fn architecture(&self) -> Option<&str> {
        self.architecture.as_deref()
    }

    /// Whether it runs on `host` without translation (Rosetta 2, Windows on
    /// Arm's emulation). A runtime that does not say is taken as native.
    #[must_use]
    pub fn is_native(&self, host: &HostProfile) -> bool {
        self.architecture
            .as_deref()
            .is_none_or(|name| host.architecture().matches_name(name))
    }

    /// Whether the launcher installed it into `runtimes_root`, so it may
    /// also remove it.
    #[must_use]
    pub fn installed_under(&self, runtimes_root: &Path) -> bool {
        self.home.starts_with(runtimes_root)
    }
}

/// The feature-release number of a Java version string.
///
/// Versions before 9 are written `1.<major>.…` (`1.8.0_412` is 8); later ones
/// start with the major (`21.0.3`, `17`, `25-ea`). Returns `None` when no
/// leading number can be read.
#[must_use]
pub fn parse_major(version: &str) -> Option<u32> {
    let version = version.trim();
    let rest = version.strip_prefix("1.").unwrap_or(version);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Parses the `KEY="value"` lines of a JDK `release` file.
#[must_use]
pub fn parse_release(text: &str) -> BTreeMap<String, String> {
    let mut properties = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|inner| inner.strip_suffix('"'))
            .unwrap_or(value);
        properties.insert(key.trim().to_owned(), value.to_owned());
    }
    properties
}

/// Shortens well-known vendor strings; unknown ones pass through, and the
/// placeholder `N/A` means "no vendor".
#[must_use]
pub fn normalize_vendor(vendor: &str) -> Option<String> {
    let vendor = vendor.trim();
    match vendor {
        "" | "N/A" => None,
        "Oracle Corporation" => Some("Oracle".to_owned()),
        "Azul Systems, Inc." => Some("Azul".to_owned()),
        other => Some(other.to_owned()),
    }
}

#[cfg(test)]
mod tests;
