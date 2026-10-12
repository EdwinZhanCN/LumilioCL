//! Where Java installations are looked for.
//!
//! The places beyond the usual install folders (the official launcher's
//! runtimes, `PATH`, more vendor folders) follow HMCL's search
//! (`HMCL/src/main/java/org/jackhuang/hmcl/java/JavaManager.java`,
//! Copyright (C) 2024 huangyuhui and contributors, GPL-3.0-or-later); only
//! the list of places is taken, and each one is still read from its
//! `release` file.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use super::JavaRuntime;
use crate::environment::{HostProfile, PlatformFamily};

/// Folders below Program Files (and the local app data) vendors install into.
const WINDOWS_VENDORS: [&str; 11] = [
    "Java",
    "Eclipse Adoptium",
    "Eclipse Foundation",
    "AdoptOpenJDK",
    "BellSoft",
    "Microsoft",
    "Zulu",
    "Semeru",
    "Amazon Corretto",
    "GraalVM",
    "Programs",
];

/// Where to look for Java installations.
///
/// Each root is either an installation itself (it has a `release` file) or a
/// folder of installations. macOS bundles (`<name>/Contents/Home`) are looked
/// into automatically. Runtime trees are laid out like the official
/// launcher's `runtime` folder, and executables (from `PATH`) lead to the
/// installation they belong to.
#[derive(Clone, Debug, Default)]
pub struct JavaLocator {
    roots: Vec<PathBuf>,
    runtime_trees: Vec<PathBuf>,
    executables: Vec<PathBuf>,
}

impl JavaLocator {
    #[must_use]
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
            ..Self::default()
        }
    }

    /// The conventional install locations for a host, plus `JAVA_HOME`,
    /// `PATH`, the official launcher's runtimes and the launcher's own
    /// `runtimes` folder.
    #[must_use]
    pub fn standard(host: &HostProfile, user_home: Option<&Path>, launcher_root: &Path) -> Self {
        let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
        let mut roots = vec![launcher_root.join("runtimes")];
        let mut trees = Vec::new();
        roots.extend(env("JAVA_HOME"));
        match host.platform() {
            PlatformFamily::MacOs => {
                roots.push("/Library/Java/JavaVirtualMachines".into());
                roots.push("/opt/homebrew/opt".into());
                roots.push("/usr/local/opt".into());
                if let Some(home) = user_home {
                    roots.push(home.join("Library/Java/JavaVirtualMachines"));
                    roots.push(home.join(".sdkman/candidates/java"));
                    trees.push(home.join("Library/Application Support/minecraft/runtime"));
                }
            }
            PlatformFamily::Windows => {
                for base in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
                    let Some(base) = env(base) else {
                        continue;
                    };
                    roots.extend(WINDOWS_VENDORS.iter().map(|vendor| base.join(vendor)));
                }
                if let Some(base) = env("ProgramFiles(x86)") {
                    trees.push(base.join("Minecraft Launcher/runtime"));
                }
                if let Some(base) = env("LOCALAPPDATA") {
                    // The Microsoft Store edition of the official launcher.
                    trees.push(base.join(
                        "Packages/Microsoft.4297127D64EC6_8wekyb3d8bbwe/LocalCache/Local/runtime",
                    ));
                }
                if let Some(base) = env("APPDATA") {
                    trees.push(base.join(".minecraft/runtime"));
                }
            }
            _ => {
                for system in [
                    "/usr/lib/jvm",
                    "/usr/lib32/jvm",
                    "/usr/lib64/jvm",
                    "/usr/java",
                ] {
                    roots.push(system.into());
                }
                if let Some(home) = user_home {
                    roots.push(home.join(".sdkman/candidates/java"));
                    roots.push(home.join(".jdks"));
                    trees.push(home.join(".minecraft/runtime"));
                }
            }
        }
        let locator = Self {
            roots,
            runtime_trees: trees,
            executables: Vec::new(),
        };
        match std::env::var_os("PATH") {
            Some(path) => locator.with_path_variable(&path, host),
            None => locator,
        }
    }

    /// Searches these folders too, after the ones already listed.
    #[must_use]
    pub fn with_roots(mut self, extra: impl IntoIterator<Item = PathBuf>) -> Self {
        self.roots.extend(extra);
        self
    }

    /// Searches folders laid out like the official launcher's `runtime`:
    /// `<component>/<platform>/<component>`, a macOS one inside
    /// `jre.bundle/Contents/Home`.
    #[must_use]
    pub fn with_runtime_trees(mut self, trees: impl IntoIterator<Item = PathBuf>) -> Self {
        self.runtime_trees.extend(trees);
        self
    }

    /// Follows every `java` on a `PATH`-style list of folders to the
    /// installation it belongs to.
    #[must_use]
    pub fn with_path_variable(mut self, path: &OsStr, host: &HostProfile) -> Self {
        let name = if *host.platform() == PlatformFamily::Windows {
            "java.exe"
        } else {
            "java"
        };
        for folder in std::env::split_paths(path) {
            // Oracle's installer puts a launcher stub here that is no JDK
            // (HMCL issue 4079).
            if folder
                .to_string_lossy()
                .to_lowercase()
                .contains("common files\\oracle\\java")
            {
                continue;
            }
            self.executables.push(folder.join(name));
        }
        self
    }

    /// Every installation found, deduplicated by real location and ordered
    /// newest major first, then by path so the order is stable.
    #[must_use]
    pub fn discover(&self) -> Vec<JavaRuntime> {
        let mut found: Vec<JavaRuntime> = Vec::new();
        let mut seen = Vec::new();
        let mut consider = |home: PathBuf| {
            let identity = fs::canonicalize(&home).unwrap_or_else(|_| home.clone());
            if seen.contains(&identity) {
                return;
            }
            if let Some(runtime) = JavaRuntime::inspect(&home) {
                seen.push(identity);
                found.push(runtime);
            }
        };
        for root in &self.roots {
            candidate_homes(root).into_iter().for_each(&mut consider);
        }
        for tree in &self.runtime_trees {
            official_homes(tree).into_iter().for_each(&mut consider);
        }
        for executable in &self.executables {
            // `/usr/bin/java` is usually a chain of links into the real home.
            let Ok(real) = fs::canonicalize(executable) else {
                continue;
            };
            if let Some(home) = real.parent().and_then(Path::parent) {
                consider(home.to_owned());
            }
        }
        found.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.home.cmp(&b.home)));
        found
    }
}

fn sorted_children(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut children: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    children.sort();
    children
}

fn candidate_homes(root: &Path) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    push_installation(root, &mut homes);
    for child in sorted_children(root) {
        push_installation(&child, &mut homes);
        // Homebrew keeps the JDK a few folders down inside the formula.
        push_installation(&child.join("libexec/openjdk.jdk/Contents/Home"), &mut homes);
    }
    homes
}

/// `<tree>/<component>/<platform>/<component>`, or its `jre.bundle`.
fn official_homes(tree: &Path) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    for component in sorted_children(tree) {
        let Some(name) = component.file_name() else {
            continue;
        };
        for platform in sorted_children(&component) {
            let home = platform.join(name);
            push_installation(&home, &mut homes);
            push_installation(&home.join("jre.bundle"), &mut homes);
        }
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
