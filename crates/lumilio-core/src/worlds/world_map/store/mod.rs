//! Manual seeds share launcher.db but survive the library's table replacement.

#[cfg(test)]
use rusqlite::Connection;
pub use seeds::{link, links, load, parse_seed, save, unlink};

pub mod annotations;

#[cfg(test)]
mod tests;

mod seeds;
