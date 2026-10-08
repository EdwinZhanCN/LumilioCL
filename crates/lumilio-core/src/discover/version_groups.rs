//! Game versions as a short list of ranges (`1.20.1–1.20.6`, `1.21.x`), the
//! way Modrinth App's project pages show them.
//!
//! Ported from `getVersionGroupsForDisplay` in `packages/utils/projects.ts`
//! (GPL-3.0-only; ADR 0022).

use super::tags::GameVersionTag;
use std::borrow::Cow;
use std::collections::HashMap;

/// What a group of game versions is called, as a type rather than a sentence:
/// the interface turns the named ranges into words and shows a `Range` as is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupLabel {
    Ancient,
    AlphaBeta,
    Alpha,
    Beta,
    PreAlpha,
    Range(String),
}

impl GroupLabel {
    /// The text of the label, for a caller with no catalog.
    #[must_use]
    pub fn text(&self) -> Cow<'_, str> {
        match self {
            Self::Ancient => Cow::Borrowed("all ancient versions"),
            Self::AlphaBeta => Cow::Borrowed("all Alpha and Beta versions"),
            Self::Alpha => Cow::Borrowed("all Alpha versions"),
            Self::Beta => Cow::Borrowed("all Beta versions"),
            Self::PreAlpha => Cow::Borrowed("all Pre-alpha versions"),
            Self::Range(range) => Cow::Borrowed(range),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionGroup {
    pub label: GroupLabel,
    pub versions: Vec<String>,
}

/// `major.minor[.patch]` as numbers, for versions that look like releases.
fn parse(version: &str) -> Option<(String, u32)> {
    let mut parts = version.split('.');
    let (a, b) = (parts.next()?, parts.next()?);
    if a.is_empty() || b.is_empty() || !a.chars().chain(b.chars()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    let patch = match parts.next() {
        None => 0,
        Some(patch) => patch.parse().ok()?,
    };
    parts.next().is_none().then(|| (format!("{a}.{b}"), patch))
}

struct Range {
    major: String,
    minor: Vec<u32>,
}

/// Groups into per-major ranges; `consecutive` splits a run where a patch is
/// missing.
fn group(versions: &[&str], consecutive: bool) -> Vec<Range> {
    let mut ranges: Vec<Range> = Vec::new();
    for version in versions.iter().rev() {
        let Some((major, patch)) = parse(version) else {
            continue;
        };
        let found = ranges.iter_mut().find(|range| {
            range.major == major
                && (!consecutive || range.minor.last().is_some_and(|last| *last + 1 == patch))
        });
        match found {
            Some(range) => range.minor.push(patch),
            None => ranges.push(Range {
                major,
                minor: vec![patch],
            }),
        }
    }
    ranges.reverse();
    ranges
}

fn name(major: &str, minor: u32) -> String {
    if minor == 0 {
        major.to_owned()
    } else {
        format!("{major}.{minor}")
    }
}

fn consecutive_ranges(versions: &[String], reference: &[&GameVersionTag]) -> Vec<VersionGroup> {
    if versions.is_empty() {
        return Vec::new();
    }
    let index: HashMap<&str, usize> = reference
        .iter()
        .enumerate()
        .map(|(at, tag)| (tag.version.as_str(), at))
        .collect();
    let mut sorted: Vec<&String> = versions.iter().collect();
    sorted.sort_by_key(|version| index.get(version.as_str()).copied().unwrap_or(usize::MAX));
    let mut ranges = Vec::new();
    let mut start = 0;
    for at in 1..=sorted.len() {
        let breaks = at == sorted.len()
            || index.get(sorted[at].as_str()).copied()
                != index.get(sorted[at - 1].as_str()).map(|i| i + 1);
        if breaks {
            let members: Vec<String> = sorted[start..at].iter().map(|v| (*v).clone()).collect();
            let last = members.last().cloned().unwrap_or_default();
            let first = members.first().cloned().unwrap_or_default();
            ranges.push(VersionGroup {
                label: range_label(&format!("{last}–{first}")),
                versions: members,
            });
            start = at;
        }
    }
    ranges
}

fn range_label(range: &str) -> GroupLabel {
    match range {
        "rd-132211–b1.8.1" => return GroupLabel::Ancient,
        "a1.0.4–b1.8.1" => return GroupLabel::AlphaBeta,
        "a1.0.4–a1.2.6" => return GroupLabel::Alpha,
        "b1.0–b1.8.1" => return GroupLabel::Beta,
        "rd-132211–inf20100618" => return GroupLabel::PreAlpha,
        _ => {}
    }
    GroupLabel::Range(match range.split_once('–') {
        Some((a, b)) if a == b => a.to_owned(),
        _ => range.to_owned(),
    })
}

/// `versions` (a project's) as display groups against Modrinth's whole list
/// (`all`, newest first).
#[must_use]
pub fn version_groups(versions: &[String], all: &[GameVersionTag]) -> Vec<VersionGroup> {
    let index: HashMap<&str, usize> = all
        .iter()
        .enumerate()
        .map(|(at, tag)| (tag.version.as_str(), at))
        .collect();
    let mut input: Vec<&String> = versions.iter().collect();
    input.sort_by_key(|version| index.get(version.as_str()).copied().unwrap_or(usize::MAX));
    let releases: Vec<&GameVersionTag> = all.iter().filter(|tag| tag.release).collect();
    let snapshots: Vec<&GameVersionTag> = all
        .iter()
        .filter(|tag| !tag.release && tag.snapshot)
        .collect();
    let legacy: Vec<&GameVersionTag> = all
        .iter()
        .filter(|tag| !tag.release && !tag.snapshot)
        .collect();
    let is_in =
        |list: &[&GameVersionTag], version: &str| list.iter().any(|tag| tag.version == version);

    let release_versions: Vec<&str> = input
        .iter()
        .filter(|version| is_in(&releases, version))
        .map(|version| version.as_str())
        .collect();
    let latest_release_date = release_versions
        .first()
        .and_then(|first| releases.iter().find(|tag| tag.version == *first))
        .map(|tag| tag.published.as_str())
        .unwrap_or("");
    // A snapshot newer than every release the project supports is worth a mention.
    let latest_snapshot = input.iter().find(|version| {
        snapshots.iter().any(|tag| {
            tag.version == ***version
                && (latest_release_date.is_empty() || latest_release_date < tag.published.as_str())
        })
    });

    let all_releases: Vec<&str> = releases.iter().map(|tag| tag.version.as_str()).collect();
    let all_grouped = group(&all_releases, false);
    let mut as_ranges: Vec<VersionGroup> = group(&release_versions, true)
        .into_iter()
        .map(|Range { major, minor }| {
            let versions: Vec<String> = minor.iter().map(|m| name(&major, *m)).collect();
            if minor.len() == 1 {
                return VersionGroup {
                    label: GroupLabel::Range(versions[0].clone()),
                    versions,
                };
            }
            if all_grouped
                .iter()
                .find(|range| range.major == major)
                .is_some_and(|range| range.minor == minor)
            {
                return VersionGroup {
                    label: GroupLabel::Range(format!("{major}.x")),
                    versions,
                };
            }
            VersionGroup {
                label: GroupLabel::Range(format!(
                    "{}–{}",
                    name(&major, minor[0]),
                    name(&major, minor[minor.len() - 1])
                )),
                versions,
            }
        })
        .collect();

    let legacy_versions: Vec<String> = input
        .iter()
        .filter(|version| is_in(&legacy, version))
        .map(|version| (*version).clone())
        .collect();
    let mut output = consecutive_ranges(&legacy_versions, &legacy);
    if as_ranges.is_empty() {
        let shown: Vec<String> = input
            .iter()
            .filter(|version| is_in(&snapshots, version))
            .map(|version| (*version).clone())
            .collect();
        let mut head: Vec<VersionGroup> = if shown.len() > 3 {
            consecutive_ranges(&shown, &snapshots)
        } else {
            shown
                .into_iter()
                .map(|version| VersionGroup {
                    label: GroupLabel::Range(version.clone()),
                    versions: vec![version],
                })
                .collect()
        };
        head.extend(output);
        output = head;
    } else {
        as_ranges.extend(output);
        output = as_ranges;
        if let Some(snapshot) = latest_snapshot {
            output.insert(
                0,
                VersionGroup {
                    label: GroupLabel::Range((*snapshot).clone()),
                    versions: vec![(*snapshot).clone()],
                },
            );
        }
    }
    output
}
