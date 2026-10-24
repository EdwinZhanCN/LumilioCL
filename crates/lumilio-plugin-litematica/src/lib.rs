//! Lists the `.litematic` files in a game's `schematics` folder, with their
//! details and material lists. All file access goes through the host.

mod format;
mod overlay;
mod tab;
#[cfg(test)]
mod tests;
mod text;

mod litematica;

use litematica::FOLDER;
pub use litematica::{ID, Litematica};
