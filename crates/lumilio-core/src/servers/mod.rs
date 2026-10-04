//! The multiplayer server list of an instance (`servers.dat`).
//!
//! The file is the game's own, so edits keep every field this module does not
//! understand (the cached icon, anything a newer game adds). A file that
//! cannot be read is reported and left alone, never overwritten.

mod ping;

pub use self::ping::{PingError, ServerStatus, probe};

use crate::nbt::{self, NbtError, Tag};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::Path;

const FILE: &str = "servers.dat";
/// The game keeps the previous list beside the new one under this name.
const BACKUP: &str = "servers.dat_old";
const LIST_LIMIT: u64 = 8 * 1024 * 1024;
const MAX_FIELD: usize = 255;

/// What the game does when the server offers a resource pack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackPolicy {
    Ask,
    Allow,
    Deny,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerEntry {
    pub name: String,
    /// `host` or `host:port`, as the game stores it (`ip`).
    pub address: String,
    pub packs: PackPolicy,
}

#[derive(Debug)]
pub enum ServerError {
    Io(io::Error),
    /// `servers.dat` exists but is not a valid list.
    Damaged(NbtError),
    /// The name or address is empty, has spaces, or is too long.
    InvalidField(&'static str),
    /// The list is not what the caller was looking at (it changed meanwhile).
    Changed,
    NotFound(usize),
}

impl Display for ServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "server list storage failed: {error}"),
            Self::Damaged(error) => write!(f, "servers.dat is damaged: {error}"),
            Self::InvalidField(which) => write!(f, "invalid server {which}"),
            Self::Changed => f.write_str("the server list changed; read it again"),
            Self::NotFound(index) => write!(f, "no server at position {index}"),
        }
    }
}

impl Error for ServerError {}

impl From<io::Error> for ServerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

type Raw = BTreeMap<String, Tag>;

fn entry_of(raw: &Raw) -> ServerEntry {
    let text = |key: &str| {
        raw.get(key)
            .and_then(Tag::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    ServerEntry {
        name: text("name"),
        address: text("ip"),
        packs: match raw.get("acceptTextures").and_then(Tag::as_i64) {
            Some(1) => PackPolicy::Allow,
            Some(_) => PackPolicy::Deny,
            None => PackPolicy::Ask,
        },
    }
}

fn apply(raw: &mut Raw, entry: &ServerEntry) {
    raw.insert("name".into(), Tag::String(entry.name.clone()));
    raw.insert("ip".into(), Tag::String(entry.address.clone()));
    match entry.packs {
        PackPolicy::Ask => {
            raw.remove("acceptTextures");
        }
        PackPolicy::Allow => {
            raw.insert("acceptTextures".into(), Tag::Byte(1));
        }
        PackPolicy::Deny => {
            raw.insert("acceptTextures".into(), Tag::Byte(0));
        }
    }
}

fn validated(entry: &ServerEntry) -> Result<ServerEntry, ServerError> {
    let name = entry.name.trim();
    let address = entry.address.trim();
    if name.is_empty() || name.chars().count() > MAX_FIELD {
        return Err(ServerError::InvalidField("name"));
    }
    if address.is_empty()
        || address.chars().count() > MAX_FIELD
        || address.chars().any(char::is_whitespace)
    {
        return Err(ServerError::InvalidField("address"));
    }
    Ok(ServerEntry {
        name: name.to_owned(),
        address: address.to_owned(),
        packs: entry.packs,
    })
}

/// The whole document and its server compounds; a missing file is an empty list.
fn load(game_dir: &Path) -> Result<(Raw, Vec<Raw>), ServerError> {
    let path = game_dir.join(FILE);
    let data = match fs::metadata(&path) {
        Ok(metadata) if metadata.len() > LIST_LIMIT => {
            return Err(ServerError::Damaged(NbtError::TooLarge));
        }
        Ok(_) => fs::read(&path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((Raw::new(), Vec::new()));
        }
        Err(error) => return Err(error.into()),
    };
    let Tag::Compound(root) = nbt::parse_maybe_gzip(&data).map_err(ServerError::Damaged)? else {
        return Err(ServerError::Damaged(NbtError::NotACompound));
    };
    let servers = match root.get("servers") {
        Some(Tag::List(items)) => items
            .iter()
            .filter_map(|item| match item {
                Tag::Compound(raw) => Some(raw.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Ok((root, servers))
}

fn store(game_dir: &Path, mut root: Raw, servers: Vec<Raw>) -> Result<(), ServerError> {
    root.insert(
        "servers".into(),
        Tag::List(servers.into_iter().map(Tag::Compound).collect()),
    );
    fs::create_dir_all(game_dir)?;
    let path = game_dir.join(FILE);
    if path.is_file() {
        // Best effort: losing the courtesy copy must not block the edit.
        let _ = fs::copy(&path, game_dir.join(BACKUP));
    }
    let temporary = game_dir.join(".servers.dat.tmp");
    fs::write(&temporary, nbt::to_bytes(&Tag::Compound(root)))?;
    fs::rename(&temporary, &path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })?;
    Ok(())
}

/// The saved servers, in the order the game shows them.
pub fn list(game_dir: &Path) -> Result<Vec<ServerEntry>, ServerError> {
    Ok(load(game_dir)?.1.iter().map(entry_of).collect())
}

/// Adds a server at the end.
pub fn add(game_dir: &Path, entry: &ServerEntry) -> Result<(), ServerError> {
    let entry = validated(entry)?;
    let (root, mut servers) = load(game_dir)?;
    let mut raw = Raw::new();
    apply(&mut raw, &entry);
    servers.push(raw);
    store(game_dir, root, servers)
}

/// The server at `index`, checked to still be `expected` (the list is
/// addressed by position, so a changed file must not edit the wrong row).
fn locate(servers: &[Raw], index: usize, expected: &ServerEntry) -> Result<(), ServerError> {
    let raw = servers.get(index).ok_or(ServerError::NotFound(index))?;
    let found = entry_of(raw);
    if found.name == expected.name && found.address == expected.address {
        Ok(())
    } else {
        Err(ServerError::Changed)
    }
}

/// Replaces the server at `index` (which must still be `expected`), keeping
/// the fields this module does not manage.
pub fn update(
    game_dir: &Path,
    index: usize,
    expected: &ServerEntry,
    entry: &ServerEntry,
) -> Result<(), ServerError> {
    let entry = validated(entry)?;
    let (root, mut servers) = load(game_dir)?;
    locate(&servers, index, expected)?;
    apply(&mut servers[index], &entry);
    store(game_dir, root, servers)
}

pub fn remove(game_dir: &Path, index: usize, expected: &ServerEntry) -> Result<(), ServerError> {
    let (root, mut servers) = load(game_dir)?;
    locate(&servers, index, expected)?;
    servers.remove(index);
    store(game_dir, root, servers)
}

/// Moves the server at `index` to position `to` (clamped to the list).
pub fn move_to(
    game_dir: &Path,
    index: usize,
    expected: &ServerEntry,
    to: usize,
) -> Result<(), ServerError> {
    let (root, mut servers) = load(game_dir)?;
    locate(&servers, index, expected)?;
    let moved = servers.remove(index);
    servers.insert(to.min(servers.len()), moved);
    store(game_dir, root, servers)
}

#[cfg(test)]
mod tests;
