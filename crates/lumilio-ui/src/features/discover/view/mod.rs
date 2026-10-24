//! The Discover page: the browse page of Modrinth App, drawn in this
//! launcher's own language (ADR 0022), over whichever content source is
//! chosen. Inside a game's Content tab it browses *for that game*: an install
//! header names it, its version and loader are provided to the search
//! (locked until released), and tabs that cannot apply are not offered.

pub(super) use context::{ChangeHandler, DiscoverCtx, DiscoverHandler, DiscoverIntent};
pub(super) use results::discover;

mod card;
mod context;
mod results;
mod sidebar;
