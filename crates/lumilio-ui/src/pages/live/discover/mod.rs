//! The Discover page: the browse page of Modrinth App, drawn in this
//! launcher's own language (ADR 0022), over whichever content source is
//! chosen. Inside a game's Content tab it browses *for that game*: an install
//! header names it, its version and loader are provided to the search
//! (locked until released), and tabs that cannot apply are not offered.

mod card;
mod sidebar;

pub use results::{discover, kind_label};

pub use self::card::project_icon;

mod results;
