//! Java runtime discovery and selection.
//!
//! Behavior notes: `docs/behavior/launcher-spine.md`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::environment::{HostProfile, PlatformFamily};

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

/// Where to look for Java installations.
///
/// Each root is either an installation itself (it has a `release` file) or a
/// folder of installations. macOS bundles (`<name>/Contents/Home`) are looked
/// into automatically.
#[derive(Clone, Debug, Default)]
pub struct JavaLocator {
    roots: Vec<PathBuf>,
}

impl JavaLocator {
    #[must_use]
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
        }
    }

    /// The conventional install locations for a host, plus `JAVA_HOME` and
    /// the launcher's own `runtimes` folder.
    #[must_use]
    pub fn standard(host: &HostProfile, user_home: Option<&Path>, launcher_root: &Path) -> Self {
        let mut roots = vec![launcher_root.join("runtimes")];
        if let Some(java_home) = std::env::var_os("JAVA_HOME") {
            roots.push(PathBuf::from(java_home));
        }
        match host.platform() {
            PlatformFamily::MacOs => {
                roots.push("/Library/Java/JavaVirtualMachines".into());
                roots.push("/opt/homebrew/opt".into());
                roots.push("/usr/local/opt".into());
                if let Some(home) = user_home {
                    roots.push(home.join("Library/Java/JavaVirtualMachines"));
                    roots.push(home.join(".sdkman/candidates/java"));
                }
            }
            PlatformFamily::Windows => {
                for base in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
                    let Some(base) = std::env::var_os(base).map(PathBuf::from) else {
                        continue;
                    };
                    for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "Programs"] {
                        roots.push(base.join(vendor));
                    }
                }
            }
            _ => {
                roots.push("/usr/lib/jvm".into());
                roots.push("/usr/java".into());
                if let Some(home) = user_home {
                    roots.push(home.join(".sdkman/candidates/java"));
                    roots.push(home.join(".jdks"));
                }
            }
        }
        Self { roots }
    }

    /// Searches these folders too, after the ones already listed.
    #[must_use]
    pub fn with_roots(mut self, extra: impl IntoIterator<Item = PathBuf>) -> Self {
        self.roots.extend(extra);
        self
    }

    /// Every installation found, deduplicated by real location and ordered
    /// newest major first, then by path so the order is stable.
    #[must_use]
    pub fn discover(&self) -> Vec<JavaRuntime> {
        let mut found: Vec<JavaRuntime> = Vec::new();
        let mut seen = Vec::new();
        for root in &self.roots {
            for home in candidate_homes(root) {
                let identity = fs::canonicalize(&home).unwrap_or_else(|_| home.clone());
                if seen.contains(&identity) {
                    continue;
                }
                if let Some(runtime) = JavaRuntime::inspect(&home) {
                    seen.push(identity);
                    found.push(runtime);
                }
            }
        }
        found.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.home.cmp(&b.home)));
        found
    }
}

fn candidate_homes(root: &Path) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    push_installation(root, &mut homes);
    let Ok(entries) = fs::read_dir(root) else {
        return homes;
    };
    let mut children: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    children.sort();
    for child in children {
        push_installation(&child, &mut homes);
        // Homebrew keeps the JDK a few folders down inside the formula.
        push_installation(&child.join("libexec/openjdk.jdk/Contents/Home"), &mut homes);
    }
    homes
}

/// Adds `dir` (or, for a macOS bundle, its `Contents/Home`) if it looks like
/// an installation.
fn push_installation(dir: &Path, homes: &mut Vec<PathBuf>) {
    if dir.join("release").is_file() {
        homes.push(dir.to_owned());
        return;
    }
    let bundled = dir.join("Contents/Home");
    if bundled.join("release").is_file() {
        homes.push(bundled);
    }
}

/// Picks the runtime to start a game with.
///
/// 1. An explicit `preferred` executable or home the user chose always wins,
///    even when its major version is unusual: the user knows their setup.
/// 2. Otherwise the runtime whose major equals `required`; failing that the
///    lowest newer major, since games rarely break on a slightly newer Java
///    and always break on an older one.
/// 3. Without a requirement (very old metadata), Java 8 if present, else the
///    lowest installed.
#[must_use]
pub fn choose<'a>(
    runtimes: &'a [JavaRuntime],
    required: Option<u32>,
    preferred: Option<&Path>,
) -> Option<&'a JavaRuntime> {
    if let Some(path) = preferred
        && let Some(runtime) = runtimes
            .iter()
            .find(|runtime| runtime.executable == path || runtime.home == path)
    {
        return Some(runtime);
    }
    match required {
        Some(required) => runtimes
            .iter()
            .filter(|runtime| runtime.major >= required)
            .min_by_key(|runtime| runtime.major),
        None => runtimes
            .iter()
            .find(|runtime| runtime.major == 8)
            .or_else(|| runtimes.iter().min_by_key(|runtime| runtime.major)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(root: &Path, name: &str, version: &str, vendor: &str) -> PathBuf {
        let home = root.join(name);
        fs::create_dir_all(home.join("bin")).unwrap();
        fs::write(home.join("bin/java"), "").unwrap();
        fs::write(
            home.join("release"),
            format!("IMPLEMENTOR=\"{vendor}\"\nJAVA_VERSION=\"{version}\"\nOS_ARCH=\"aarch64\"\n"),
        )
        .unwrap();
        home
    }

    #[test]
    fn parses_legacy_and_modern_versions() {
        assert_eq!(parse_major("1.8.0_412"), Some(8));
        assert_eq!(parse_major("17"), Some(17));
        assert_eq!(parse_major("21.0.3"), Some(21));
        assert_eq!(parse_major("25-ea"), Some(25));
        assert_eq!(parse_major("1.7"), Some(7));
        assert_eq!(parse_major(""), None);
        assert_eq!(parse_major("abc"), None);
        assert_eq!(parse_major("99999999999999999999"), None);
    }

    #[test]
    fn parses_release_files() {
        let props = parse_release("# c\n\nJAVA_VERSION=\"21.0.3\"\nBARE=yes\nnot a pair\n");
        assert_eq!(props["JAVA_VERSION"], "21.0.3");
        assert_eq!(props["BARE"], "yes");
        assert_eq!(props.len(), 2);
    }

    #[test]
    fn normalizes_vendors() {
        assert_eq!(
            normalize_vendor("Oracle Corporation").as_deref(),
            Some("Oracle")
        );
        assert_eq!(normalize_vendor("N/A"), None);
        assert_eq!(
            normalize_vendor("Eclipse Adoptium").as_deref(),
            Some("Eclipse Adoptium")
        );
    }

    #[test]
    fn discovers_installations_newest_first_without_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), "jdk-8", "1.8.0_412", "Azul Systems, Inc.");
        let home21 = install(dir.path(), "jdk-21", "21.0.3", "Eclipse Adoptium");
        install(dir.path(), "jdk-17", "17.0.11", "N/A");
        // A folder without a usable launcher is ignored.
        fs::create_dir_all(dir.path().join("not-java")).unwrap();
        let locator = JavaLocator::new([dir.path().to_owned(), home21.clone()]);
        let runtimes = locator.discover();
        let majors: Vec<_> = runtimes.iter().map(JavaRuntime::major).collect();
        assert_eq!(majors, [21, 17, 8]);
        assert_eq!(runtimes[0].vendor(), Some("Eclipse Adoptium"));
        assert_eq!(runtimes[1].vendor(), None);
        assert_eq!(runtimes[2].vendor(), Some("Azul"));
        assert_eq!(runtimes[0].architecture(), Some("aarch64"));
    }

    #[test]
    fn finds_macos_bundles() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("temurin-21.jdk/Contents");
        install(&bundle, "Home", "21.0.1", "Eclipse Adoptium");
        let runtimes = JavaLocator::new([dir.path().to_owned()]).discover();
        assert_eq!(runtimes.len(), 1);
        assert!(runtimes[0].home().ends_with("Contents/Home"));
    }

    #[test]
    fn a_release_without_a_launcher_is_not_an_installation() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("broken");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join("release"), "JAVA_VERSION=\"17\"").unwrap();
        assert!(JavaRuntime::inspect(&home).is_none());
    }

    #[test]
    fn chooses_by_requirement_then_preference() {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), "a", "1.8.0_1", "x");
        install(dir.path(), "b", "17.0.1", "x");
        install(dir.path(), "c", "21.0.1", "x");
        let runtimes = JavaLocator::new([dir.path().to_owned()]).discover();

        assert_eq!(choose(&runtimes, Some(17), None).unwrap().major(), 17);
        // No exact match: the lowest newer major.
        assert_eq!(choose(&runtimes, Some(18), None).unwrap().major(), 21);
        // Nothing new enough.
        assert!(choose(&runtimes, Some(25), None).is_none());
        // No requirement: Java 8 when present.
        assert_eq!(choose(&runtimes, None, None).unwrap().major(), 8);
        // An explicit choice wins over the requirement.
        let preferred = runtimes.iter().find(|r| r.major() == 8).unwrap();
        assert_eq!(
            choose(&runtimes, Some(21), Some(preferred.executable()))
                .unwrap()
                .major(),
            8
        );
        // An unknown preference falls back to the requirement.
        assert_eq!(
            choose(&runtimes, Some(21), Some(Path::new("/nowhere/java")))
                .unwrap()
                .major(),
            21
        );
    }
}
