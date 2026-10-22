//! Java runtime discovery and selection.

mod choice;
mod locator;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::environment::HostProfile;

pub use self::choice::{
    JavaChoice, JavaNeeds, JavaRule, RuleStrength, choose, install_build, install_major,
};
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
            major,
            version,
            vendor: properties
                .get("IMPLEMENTOR")
                .and_then(|vendor| normalize_vendor(vendor)),
            // Mojang's runtimes other than Java 8 do not say.
            architecture: properties
                .get("OS_ARCH")
                .cloned()
                .or_else(|| executable_architecture(&executable).map(str::to_owned)),
            executable,
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

/// Rosetta 2's runtime, installed with it.
const ROSETTA_RUNTIME: &str = "/Library/Apple/usr/libexec/oah/libRosettaRuntime";

/// Whether `host` is Apple silicon without Rosetta 2, so an Intel Java
/// cannot start there.
#[must_use]
pub fn rosetta_missing(host: &HostProfile) -> bool {
    rosetta_missing_at(host, Path::new(ROSETTA_RUNTIME))
}

fn rosetta_missing_at(host: &HostProfile, runtime: &Path) -> bool {
    use crate::environment::{MachineArchitecture, PlatformFamily};
    *host.platform() == PlatformFamily::MacOs
        && *host.architecture() == MachineArchitecture::Arm64
        && !runtime.is_file()
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

/// The architecture a `java` executable is built for, from its Mach-O, PE or
/// ELF header: `x86_64`, `aarch64` or `x86`. `None` for a universal binary,
/// which runs natively, and for anything unreadable.
fn executable_architecture(executable: &Path) -> Option<&'static str> {
    use std::io::Read;
    let mut header = Vec::new();
    fs::File::open(executable)
        .ok()?
        .take(1024)
        .read_to_end(&mut header)
        .ok()?;
    let u16_at = |at: usize| Some(u16::from_le_bytes(header.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(header.get(at..at + 4)?.try_into().ok()?));
    match header.get(..4)? {
        // 64-bit Mach-O: the CPU type follows the magic.
        [0xcf, 0xfa, 0xed, 0xfe] => match u32_at(4)? {
            0x0100_0007 => Some("x86_64"),
            0x0100_000c => Some("aarch64"),
            _ => None,
        },
        // Little-endian ELF: the machine is at 18.
        [0x7f, b'E', b'L', b'F'] if header.get(5) == Some(&1) => match u16_at(18)? {
            0x03 => Some("x86"),
            0x3e => Some("x86_64"),
            0xb7 => Some("aarch64"),
            _ => None,
        },
        // PE: 0x3c holds where "PE\0\0" starts; the machine follows it.
        [b'M', b'Z', ..] => {
            let pe = usize::try_from(u32_at(0x3c)?).ok()?;
            if header.get(pe..pe + 4)? != b"PE\0\0" {
                return None;
            }
            match u16_at(pe + 4)? {
                0x014c => Some("x86"),
                0x8664 => Some("x86_64"),
                0xaa64 => Some("aarch64"),
                _ => None,
            }
        }
        _ => None,
    }
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
