//! Forge and NeoForge version lists: which loader versions exist for a game
//! version, newest first, and which of them their publishers call stable.
//!
//! Installing them is separate (their installers run processors); this module
//! only answers "what can I choose".
//!
//! The NeoForge version → game version rule is adapted from HMCL
//! (`HMCLCore/src/main/java/org/jackhuang/hmcl/download/neoforge/NeoForgeOfficialVersionList.java`),
//! Copyright (C) 2021 huangyuhui and contributors, GPL-3.0-or-later.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::loader::{LoaderError, LoaderVersion};

/// Every Forge build, keyed by game version (`"1.20.1": ["1.20.1-47.2.0", …]`, oldest first).
pub const FORGE_METADATA_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
/// Forge's own "recommended" and "latest" pointers per game version.
pub const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
/// Every NeoForge build, oldest first.
pub const NEOFORGE_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
/// NeoForge's first builds for 1.20.1, published under the old Forge coordinates.
pub const NEOFORGE_LEGACY_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge";

/// The only game version NeoForge published under its legacy coordinates.
pub const NEOFORGE_LEGACY_GAME: &str = "1.20.1";

#[derive(Deserialize)]
struct Promotions {
    promos: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct MavenVersions {
    versions: Vec<String>,
}

fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, LoaderError> {
    serde_json::from_slice(bytes).map_err(|error| LoaderError::Decode(error.to_string()))
}

/// Forge builds for `game`, newest first. A build is stable when Forge
/// promotes it as recommended; `promotions` may be missing (then none is).
/// Versions are what follows `<game>-` in the build name, branch suffix
/// included (`10.13.4.1614-1.7.10`).
pub fn forge_versions(
    metadata: &[u8],
    promotions: Option<&[u8]>,
    game: &str,
) -> Result<Vec<LoaderVersion>, LoaderError> {
    let builds: BTreeMap<String, Vec<String>> = decode(metadata)?;
    let recommended = match promotions {
        Some(bytes) => decode::<Promotions>(bytes)?
            .promos
            .remove(&format!("{game}-recommended")),
        None => None,
    };
    let prefix = format!("{game}-");
    let Some(list) = builds.get(game) else {
        return Ok(Vec::new());
    };
    Ok(list
        .iter()
        .rev()
        .filter_map(|build| build.strip_prefix(&prefix))
        .filter(|version| !version.is_empty())
        .map(|version| {
            let number = version.split('-').next().unwrap_or(version);
            LoaderVersion {
                version: version.to_owned(),
                stable: recommended.as_deref() == Some(number),
            }
        })
        .collect())
}

/// The game version a NeoForge build is for, or `None` when its number does
/// not follow a known scheme:
///
/// - `0.<snapshot>.<n>` — a build for a game snapshot (`0.25w14craftmine.3-beta`);
/// - `<year>.<drop>.<patch>.<n>` from 26 on (`26.3.0.31-beta` → `26.3`,
///   `26.1.2.5` → `26.1.2`); a `+<suffix>` names a game pre-release
///   (`…+pre-2` → `26.3-pre-2`);
/// - `<minor>.<patch>.<n>` before that (`21.1.77` → `1.21.1`, `21.0.10` → `1.21`).
#[must_use]
pub fn neoforge_game_version(version: &str) -> Option<String> {
    let mut parts = version.splitn(4, '.');
    let major = parts.next()?;
    let second = parts.next()?;
    let third = parts.next()?;
    let major: u32 = major.parse().ok()?;
    if major == 0 {
        return Some(second.to_owned());
    }
    if major >= 26 {
        // A fourth component must exist: `<year>.<drop>.<patch>.<build>`.
        parts.next()?;
        let patch: u32 = third.parse().ok()?;
        let base = if patch == 0 {
            format!("{major}.{second}")
        } else {
            format!("{major}.{second}.{patch}")
        };
        return Some(match version.split_once('+') {
            Some((_, suffix)) => format!("{base}-{suffix}"),
            None => base,
        });
    }
    let minor: u32 = second.parse().ok()?;
    Some(if minor == 0 {
        format!("1.{major}")
    } else {
        format!("1.{major}.{minor}")
    })
}

/// A NeoForge build is stable unless its number says otherwise.
fn neoforge_stable(version: &str) -> bool {
    !(version.contains("-beta") || version.contains("-alpha") || version.contains("-rc"))
}

/// NeoForge builds for `game`, newest first. `legacy` is the list published
/// under the old coordinates, used only for 1.20.1, whose versions are
/// normalized to the plain build number (`1.20.1-47.1.5` → `47.1.5`).
pub fn neoforge_versions(
    modern: &[u8],
    legacy: Option<&[u8]>,
    game: &str,
) -> Result<Vec<LoaderVersion>, LoaderError> {
    let mut versions: Vec<LoaderVersion> = decode::<MavenVersions>(modern)?
        .versions
        .into_iter()
        .filter(|version| neoforge_game_version(version).as_deref() == Some(game))
        .map(|version| LoaderVersion {
            stable: neoforge_stable(&version),
            version,
        })
        .collect();
    if game == NEOFORGE_LEGACY_GAME
        && let Some(bytes) = legacy
    {
        let prefix = format!("{NEOFORGE_LEGACY_GAME}-");
        versions.extend(
            decode::<MavenVersions>(bytes)?
                .versions
                .into_iter()
                .map(|version| {
                    let plain = version.strip_prefix(&prefix).unwrap_or(&version);
                    let plain = plain.strip_prefix("forge-").unwrap_or(plain).to_owned();
                    LoaderVersion {
                        stable: neoforge_stable(&plain),
                        version: plain,
                    }
                }),
        );
    }
    versions.reverse();
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_lists_one_game_newest_first_and_marks_the_recommended_build() {
        let metadata = br#"{"1.7.10":["1.7.10-10.13.4.1614-1.7.10"],
            "26.2":["26.2-65.0.0","26.2-65.1.0","26.2-65.1.3"],
            "26.3":["26.3-66.0.0"]}"#;
        let promotions =
            br#"{"homepage":"x","promos":{"26.2-recommended":"65.1.0","26.2-latest":"65.1.3"}}"#;
        let versions = forge_versions(metadata, Some(promotions), "26.2").unwrap();
        let names: Vec<_> = versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(names, ["65.1.3", "65.1.0", "65.0.0"]);
        assert_eq!(
            versions.iter().map(|v| v.stable).collect::<Vec<_>>(),
            [false, true, false]
        );
        // The branch suffix stays part of the version; its number is what is promoted.
        let old = forge_versions(
            metadata,
            Some(br#"{"promos":{"1.7.10-recommended":"10.13.4.1614"}}"#),
            "1.7.10",
        )
        .unwrap();
        assert_eq!(old[0].version, "10.13.4.1614-1.7.10");
        assert!(old[0].stable);
        // No promotions: nothing is marked stable; unknown game: empty.
        assert!(
            forge_versions(metadata, None, "26.2")
                .unwrap()
                .iter()
                .all(|v| !v.stable)
        );
        assert!(forge_versions(metadata, None, "1.0").unwrap().is_empty());
        assert!(forge_versions(b"nope", None, "26.2").is_err());
    }

    #[test]
    fn neoforge_numbers_map_to_their_game_versions() {
        for (version, game) in [
            ("21.1.77", Some("1.21.1")),
            ("21.0.10-beta", Some("1.21")),
            ("20.2.3-beta", Some("1.20.2")),
            ("20.4.237", Some("1.20.4")),
            ("26.3.0.31-beta", Some("26.3")),
            ("26.1.2.5", Some("26.1.2")),
            ("26.3.0.1-beta+pre-2", Some("26.3-pre-2")),
            ("0.25w14craftmine.3-beta", Some("25w14craftmine")),
            ("26.3.0", None),
            ("x.y.z", None),
            ("21", None),
        ] {
            assert_eq!(neoforge_game_version(version).as_deref(), game, "{version}");
        }
    }

    #[test]
    fn neoforge_lists_one_game_newest_first_with_legacy_builds_for_1_20_1() {
        let modern = br#"{"isSnapshot":false,"versions":
            ["20.2.3-beta","21.1.1","21.1.2-beta","21.1.3","26.3.0.1-beta"]}"#;
        let versions = neoforge_versions(modern, None, "1.21.1").unwrap();
        let names: Vec<_> = versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(names, ["21.1.3", "21.1.2-beta", "21.1.1"]);
        assert_eq!(
            versions.iter().map(|v| v.stable).collect::<Vec<_>>(),
            [true, false, true]
        );
        let legacy = br#"{"versions":["1.20.1-47.1.5","1.20.1-forge-47.1.7"]}"#;
        let old = neoforge_versions(modern, Some(legacy), "1.20.1").unwrap();
        let names: Vec<_> = old.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(names, ["47.1.7", "47.1.5"]);
        // The legacy list is ignored for every other game version.
        assert_eq!(
            neoforge_versions(modern, Some(legacy), "26.3")
                .unwrap()
                .len(),
            1
        );
    }
}
