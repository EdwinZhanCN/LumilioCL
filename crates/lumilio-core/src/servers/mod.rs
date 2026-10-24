//! The multiplayer server list of an instance (`servers.dat`).
//!
//! The file is the game's own, so edits keep every field this module does not
//! understand (the cached icon, anything a newer game adds). A file that
//! cannot be read is reported and left alone, never overwritten.

mod motd;
mod ping;
mod protocol;

#[cfg(test)]
use std::fs;

#[cfg(test)]
use base64::Engine as _;
use server_list::decode_icon;
pub use server_list::{
    PackPolicy, ServerEntry, ServerError, add, list, move_to, protocol_version, remove, update,
};
#[cfg(test)]
use server_list::{Raw, entry_of, load, store};

pub use self::motd::{Motd, MotdSpan};
pub use self::ping::{PingError, ServerStatus, probe};
pub use self::protocol::ProtocolVersion;
#[cfg(test)]
use crate::nbt::Tag;

#[cfg(test)]
mod tests;

mod server_list;
