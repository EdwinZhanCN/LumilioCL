use semver::Version;

use crate::i18n;
use crate::i18n::Locale;

/// One release's notes as written for the launcher.
#[derive(Debug)]
pub struct ReleaseNotes {
    pub version: &'static str,
    pub(super) chinese: &'static str,
    pub(super) english: Option<&'static str>,
}

impl ReleaseNotes {
    /// The notes in the launcher's language; the Chinese ones when no
    /// English was written.
    #[must_use]
    pub fn text(&self) -> &'static str {
        match (i18n::locale(), self.english) {
            (Locale::English, Some(english)) => english,
            _ => self.chinese,
        }
    }
}

pub(super) static RELEASES: &[ReleaseNotes] =
    include!(concat!(env!("OUT_DIR"), "/release_notes.rs"));

/// The running launcher's version.
#[must_use]
pub fn running() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The notes to show when the launcher starts as `running` after it last
/// showed those of `seen`: every release after `seen` up to `running`,
/// newest first. `None` (or a version that cannot be read) is a launcher from
/// before release notes, which gets only the running release's notes.
#[must_use]
pub fn since(seen: Option<&str>, running: &str) -> Vec<&'static ReleaseNotes> {
    pick(RELEASES, seen, running)
}

/// The running release's notes, if it has any.
#[must_use]
pub fn current() -> Option<&'static ReleaseNotes> {
    RELEASES.iter().find(|notes| notes.version == running())
}

pub(super) fn pick<'a>(
    releases: &'a [ReleaseNotes],
    seen: Option<&str>,
    running: &str,
) -> Vec<&'a ReleaseNotes> {
    let Ok(running) = Version::parse(running) else {
        return Vec::new();
    };
    let seen = seen.and_then(|seen| Version::parse(seen).ok());
    let mut picked: Vec<(Version, &ReleaseNotes)> = releases
        .iter()
        .filter_map(|notes| Some((Version::parse(notes.version).ok()?, notes)))
        .filter(|(version, _)| match &seen {
            Some(seen) => version > seen && *version <= running,
            None => *version == running,
        })
        .collect();
    picked.sort_by(|(a, _), (b, _)| b.cmp(a));
    picked.into_iter().map(|(_, notes)| notes).collect()
}
