//! What changed in each launcher release, shown once on the first start after
//! an update and again from Settings › About.
//!
//! The notes are the files in `release-notes/` (one per version, Chinese with
//! an optional English one), embedded at build time so showing them needs no
//! network. The release workflow publishes the same file as the release
//! description (`assets/icons/PACKAGING.md` §5).

mod dialog;
mod text;

#[cfg(test)]
mod tests;

#[cfg(test)]
use notes::{RELEASES, pick};
pub use notes::{ReleaseNotes, current, running, since};

pub use self::dialog::{Occasion, show};

mod notes;
