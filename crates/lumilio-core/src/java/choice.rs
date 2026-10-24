//! Which installed Java starts a game.
//!
//! The rules are adapted from HMCL
//! (`HMCLCore/src/main/java/org/jackhuang/hmcl/game/JavaVersionConstraint.java`,
//! Copyright (C) 2021 huangyuhui and contributors, GPL-3.0-or-later): a
//! required rule keeps a Java from being picked at all, a suggested one only
//! ranks it lower. Rules for loaders the launcher does not run (Cleanroom)
//! and for single Forge builds are left out.

use std::cmp::Reverse;
use std::path::Path;

use super::JavaRuntime;
use crate::instance::{InstanceRecord, Loader};
use crate::java_runtime::JavaBuild;
use lumilio_minecraft::LaunchPlan;
use lumilio_minecraft::{HostProfile, MachineArchitecture, PlatformFamily};
use lumilio_minecraft::{JavaRequirement, ReleaseManifest};

/// The class LaunchWrapper starts the game through.
const LAUNCH_WRAPPER_MAIN: &str = "net.minecraft.launchwrapper.Launch";

/// What about a game decides which Java can run it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JavaNeeds {
    /// The lowest major the release asks for, when it says.
    pub required: Option<u32>,
    /// The game version, e.g. `1.12.2`.
    pub game_version: String,
    /// Forge or NeoForge: each game version of theirs is tested on one Java.
    pub forge_like: bool,
    /// Starts through LaunchWrapper before 1.13 (old Forge, LiteLoader).
    pub launch_wrapper: bool,
    /// The variants of its native libraries, for every system (`osx`,
    /// `macos-arm64`, `windows-${arch}`…).
    pub natives: Vec<String>,
}

impl JavaNeeds {
    #[must_use]
    pub fn new(game_version: &str, loader: Loader, required: Option<u32>) -> Self {
        Self {
            required,
            game_version: game_version.to_owned(),
            forge_like: matches!(loader, Loader::Forge | Loader::NeoForge),
            launch_wrapper: false,
            natives: Vec::new(),
        }
    }

    /// What a launch plan for `record` needs: its requirement, main class
    /// and class path.
    #[must_use]
    pub fn of_plan(record: &InstanceRecord, plan: &LaunchPlan) -> Self {
        let classpath: Vec<String> = plan
            .classpath()
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let libraries: Vec<&str> = classpath.iter().map(String::as_str).collect();
        let archives: Vec<String> = plan
            .native_archives()
            .iter()
            .map(|archive| archive.path().to_string_lossy().into_owned())
            .collect();
        Self::new(
            &record.game_version,
            record.loader,
            plan.java_requirement().map(JavaRequirement::major),
        )
        .with_start(plan.main_class(), &libraries)
        .with_natives(
            libraries
                .iter()
                .copied()
                .chain(archives.iter().map(String::as_str)),
        )
    }

    /// What an installed release of `record` needs, before any launch.
    #[must_use]
    pub fn of_release(record: &InstanceRecord, release: &ReleaseManifest) -> Self {
        let coordinates: Vec<String> = release
            .libraries()
            .iter()
            .map(|library| library.coordinate().to_string())
            .collect();
        let libraries: Vec<&str> = coordinates.iter().map(String::as_str).collect();
        Self::new(
            &record.game_version,
            record.loader,
            release.java_requirement().map(JavaRequirement::major),
        )
        .with_start(release.main_class().unwrap_or_default(), &libraries)
        .with_natives(
            libraries.iter().copied().chain(
                release
                    .libraries()
                    .iter()
                    .flat_map(|library| library.native_classifiers()),
            ),
        )
    }

    /// Notes the native variants among library file names
    /// (`lwjgl-platform-2.9.0-natives-osx.jar`), coordinates
    /// (`org.lwjgl:lwjgl:3.3.1:natives-macos-arm64`) or classifiers
    /// (`natives-osx`).
    #[must_use]
    pub fn with_natives<'a>(mut self, names: impl IntoIterator<Item = &'a str>) -> Self {
        for name in names {
            let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
            if let Some((_, variant)) = file.rsplit_once("natives-") {
                let variant = variant.strip_suffix(".jar").unwrap_or(variant);
                if !self.natives.iter().any(|known| known == variant) {
                    self.natives.push(variant.to_owned());
                }
            }
        }
        self
    }

    /// Notes LaunchWrapper from the main class and the libraries (file
    /// names such as `launchwrapper-1.12.jar` or coordinates such as
    /// `net.minecraft:launchwrapper:1.12`). A LaunchWrapper of 1.13 or later
    /// works on newer Java.
    #[must_use]
    pub fn with_start(mut self, main_class: &str, libraries: &[&str]) -> Self {
        if main_class != LAUNCH_WRAPPER_MAIN {
            return self;
        }
        let version = libraries.iter().find_map(|library| {
            let name = library.rsplit(['/', '\\']).next().unwrap_or(library);
            name.strip_prefix("launchwrapper-")
                .and_then(|rest| rest.strip_suffix(".jar"))
                .or_else(|| {
                    let mut parts = library.split(':');
                    (parts.nth(1) == Some("launchwrapper"))
                        .then(|| parts.next())
                        .flatten()
                })
        });
        self.launch_wrapper = version.is_none_or(|version| version_parts(version) < vec![1, 13]);
        self
    }
}

/// How firmly a rule holds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuleStrength {
    /// Breaking it usually still starts; it ranks a Java lower.
    Suggested,
    /// Breaking it fails the game; such a Java is never picked by itself.
    Required,
}

/// One rule about the Java a game runs on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaRule {
    /// The release's own minimum.
    Minimum(u32),
    /// LaunchWrapper before 1.13 takes the system class loader for a
    /// `URLClassLoader`, which it stopped being in Java 9.
    LaunchWrapper,
    /// On 64-bit Linux, Java 9 and later refuse the 32-bit natives of 1.12
    /// and older.
    LegacyLinux,
    /// Forge and NeoForge are tested on this major for the game version.
    ModLoader(u32),
    /// Its native libraries are Intel builds only (before 1.19), which Apple
    /// silicon and Windows on Arm run under translation with an Intel Java.
    /// The major is the one to download.
    IntelNatives(u32),
}

impl JavaRule {
    #[must_use]
    pub const fn strength(self) -> RuleStrength {
        match self {
            Self::Minimum(_) | Self::LaunchWrapper | Self::LegacyLinux | Self::IntelNatives(_) => {
                RuleStrength::Required
            }
            Self::ModLoader(_) => RuleStrength::Suggested,
        }
    }

    #[must_use]
    pub fn allows(self, runtime: &JavaRuntime) -> bool {
        let major = runtime.major();
        match self {
            Self::Minimum(minimum) => major >= minimum,
            Self::LaunchWrapper | Self::LegacyLinux => major <= 8,
            Self::ModLoader(advised) => major == advised,
            Self::IntelNatives(_) => runtime
                .architecture()
                .is_some_and(|name| MachineArchitecture::X86_64.matches_name(name)),
        }
    }

    /// The major that satisfies it, to advise or to download.
    #[must_use]
    pub const fn advised_major(self) -> u32 {
        match self {
            Self::Minimum(major) | Self::ModLoader(major) | Self::IntelNatives(major) => major,
            Self::LaunchWrapper | Self::LegacyLinux => 8,
        }
    }

    /// The highest major it allows, if it caps them.
    const fn ceiling(self) -> Option<u32> {
        match self {
            Self::Minimum(_) | Self::IntelNatives(_) => None,
            Self::LaunchWrapper | Self::LegacyLinux => Some(8),
            Self::ModLoader(major) => Some(major),
        }
    }
}

/// Whether `natives` hold builds for `host`'s system and none of them for
/// Arm, on an Arm host that runs Intel code translated: Apple silicon
/// (Rosetta 2) and Windows on Arm. Mojang ships `natives-macos-arm64` and
/// `natives-windows-arm64` from 1.19 on.
fn intel_natives_only(natives: &[String], host: &HostProfile) -> bool {
    let systems: &[&str] = match (host.platform(), host.architecture()) {
        (PlatformFamily::MacOs, MachineArchitecture::Arm64) => &["osx", "macos"],
        (PlatformFamily::Windows, MachineArchitecture::Arm64) => &["windows"],
        _ => return false,
    };
    let here: Vec<&String> = natives
        .iter()
        .filter(|variant| systems.iter().any(|system| variant.starts_with(system)))
        .collect();
    !here.is_empty()
        && !here
            .iter()
            .any(|variant| variant.contains("arm64") || variant.contains("aarch64"))
}

/// `1.12.2` → `[1, 12, 2]`; `1.20.5-pre1` → `[1, 20, 5]`; alpha, beta and
/// classic versions → `[0]`, older than any release; snapshots (`24w14a`)
/// → `None`.
fn release_number(game_version: &str) -> Option<Vec<u32>> {
    let version = game_version.trim();
    if ["a1.", "b1.", "c0.", "rd-", "inf-"]
        .iter()
        .any(|prefix| version.starts_with(prefix))
    {
        return Some(vec![0]);
    }
    let release = version.split(['-', ' ']).next()?;
    let parts: Option<Vec<u32>> = release.split('.').map(|part| part.parse().ok()).collect();
    parts.filter(|parts| parts.len() >= 2)
}

/// Numbers in a version string, for ordering (`1.8.0_412` → 1, 8, 0, 412).
fn version_parts(version: &str) -> Vec<u32> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

/// The rules for a game on `host`. A cap the release's own minimum already
/// rules out does not apply (HMCL does the same): newer releases of an old
/// line may need more than the line once did.
#[must_use]
pub fn rules(needs: &JavaNeeds, host: &HostProfile) -> Vec<JavaRule> {
    let mut rules: Vec<JavaRule> = needs.required.map(JavaRule::Minimum).into_iter().collect();
    if intel_natives_only(&needs.natives, host) {
        // Without a requirement the game is old enough for Java 8.
        rules.push(JavaRule::IntelNatives(needs.required.unwrap_or(8)));
    }
    let Some(version) = release_number(&needs.game_version) else {
        return rules;
    };
    let at_most = |last: &[u32]| version.as_slice() < last;
    let at_least = |first: &[u32]| version.as_slice() >= first;
    if needs.launch_wrapper && at_most(&[1, 13]) {
        rules.push(JavaRule::LaunchWrapper);
    }
    if *host.platform() == PlatformFamily::Linux
        && *host.architecture() == MachineArchitecture::X86_64
        && at_most(&[1, 13])
    {
        rules.push(JavaRule::LegacyLinux);
    }
    if needs.forge_like {
        let advised = if at_least(&[1, 20, 5]) {
            Some(21)
        } else if at_least(&[1, 18]) {
            Some(17)
        } else if at_least(&[1, 17]) {
            Some(16)
        } else if at_least(&[1, 7, 10]) {
            Some(8)
        } else {
            None
        };
        rules.extend(advised.map(JavaRule::ModLoader));
    }
    rules.retain(|rule| {
        rule.ceiling()
            .zip(needs.required)
            .is_none_or(|(ceiling, minimum)| ceiling >= minimum)
    });
    rules
}

/// The Java picked for a game, with the rules it breaks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaChoice {
    pub runtime: JavaRuntime,
    /// Rules the runtime breaks. The launcher's own pick breaks no required
    /// rule; the person's pick may.
    pub broken: Vec<JavaRule>,
    /// The person chose it in the instance settings.
    pub chosen_by_user: bool,
}

impl JavaChoice {
    #[must_use]
    pub fn breaks_required(&self) -> bool {
        self.broken
            .iter()
            .any(|rule| rule.strength() == RuleStrength::Required)
    }

    /// The major the broken rules advise: a required one's first.
    #[must_use]
    pub fn advised_major(&self) -> Option<u32> {
        self.broken
            .iter()
            .max_by_key(|rule| rule.strength())
            .map(|rule| rule.advised_major())
    }
}

fn broken(rules: &[JavaRule], runtime: &JavaRuntime) -> Vec<JavaRule> {
    rules
        .iter()
        .copied()
        .filter(|rule| !rule.allows(runtime))
        .collect()
}

/// Picks the runtime to start a game with.
///
/// 1. A `preferred` executable or home the person chose always wins, even
///    when it breaks a rule: the person knows their setup. The rules it
///    breaks come with it, so the launch can ask once.
/// 2. Otherwise only runtimes that break no required rule: those that also
///    keep the suggestions first, then those that run natively, then the
///    lowest major from the requirement up (games rarely break on a slightly
///    newer Java and always on an older one), then the newest build.
/// 3. Without a requirement (very old metadata), Java 8 counts as the
///    requirement.
#[must_use]
pub fn choose(
    runtimes: &[JavaRuntime],
    needs: &JavaNeeds,
    preferred: Option<&Path>,
    host: &HostProfile,
) -> Option<JavaChoice> {
    let rules = rules(needs, host);
    if let Some(path) = preferred {
        let found = runtimes
            .iter()
            .find(|runtime| runtime.executable == path || runtime.home == path)
            .cloned()
            .or_else(|| JavaRuntime::at(path));
        if let Some(runtime) = found {
            return Some(JavaChoice {
                broken: broken(&rules, &runtime),
                runtime,
                chosen_by_user: true,
            });
        }
    }
    let anchor = needs.required.unwrap_or(8);
    runtimes
        .iter()
        .map(|runtime| (runtime, broken(&rules, runtime)))
        .filter(|(_, broken)| {
            broken
                .iter()
                .all(|rule| rule.strength() == RuleStrength::Suggested)
        })
        .min_by_key(|(runtime, broken)| {
            let distance = if runtime.major >= anchor {
                runtime.major - anchor
            } else {
                1000 + anchor - runtime.major
            };
            (
                broken.len(),
                !runtime.is_native(host),
                distance,
                Reverse(version_parts(&runtime.version)),
                runtime.home.clone(),
            )
        })
        .map(|(runtime, broken)| JavaChoice {
            runtime: runtime.clone(),
            broken,
            chosen_by_user: false,
        })
}

/// The major to download when no installed Java may run the game: Java 8
/// when a rule caps it there, else the release's minimum, else Java 8 for
/// Intel-only natives. `None` leaves it to the recommended one.
#[must_use]
pub fn install_major(needs: &JavaNeeds, host: &HostProfile) -> Option<u32> {
    let rules = rules(needs, host);
    rules
        .iter()
        .filter(|rule| rule.strength() == RuleStrength::Required)
        .filter_map(|rule| rule.ceiling())
        .min()
        .or(needs.required)
        .or_else(|| {
            rules.iter().find_map(|rule| match rule {
                JavaRule::IntelNatives(major) => Some(*major),
                _ => None,
            })
        })
}

/// Which build that download must be: Intel when the game's native
/// libraries have no other for this system.
#[must_use]
pub fn install_build(needs: &JavaNeeds, host: &HostProfile) -> JavaBuild {
    if intel_natives_only(&needs.natives, host) {
        JavaBuild::Intel
    } else {
        JavaBuild::Any
    }
}

#[cfg(test)]
mod tests;
