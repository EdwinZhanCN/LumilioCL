//! The multiplayer server list of an instance (`servers.dat`).
//!
//! The file is the game's own, so edits keep every field this module does not
//! understand (the cached icon, anything a newer game adds). A file that
//! cannot be read is reported and left alone, never overwritten.

mod ping;
mod protocol;

pub use self::ping::{PingError, ServerStatus, probe};
pub use self::protocol::ProtocolVersion;

use crate::nbt::{self, NbtError, Tag};
use crate::skin::Pixels;
use base64::Engine as _;
use image::ImageReader;
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
/// A server icon is a small PNG (the game writes 64×64); anything far bigger
/// is not one.
const ICON_LIMIT: usize = 1024 * 1024;
const MAX_ICON_SIDE: u32 = 512;

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
    /// The icon the game cached for this server, when it has one (the `icon`
    /// field of `servers.dat`), decoded for the preview. Read only: edits keep
    /// the game's copy, so the launcher never loses it.
    pub icon: Option<Pixels>,
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
        icon: raw.get("icon").and_then(Tag::as_str).and_then(stored_icon),
    }
}

/// Decodes a server icon (a small PNG) with limits. Anything unreadable,
/// oversized or not an image is `None`: an icon is a nicety, not a fact the
/// list depends on. Everything a server sends is untrusted, so the size and
/// pixel count are capped before memory is reserved.
fn decode_icon(bytes: &[u8]) -> Option<Pixels> {
    if bytes.is_empty() || bytes.len() > ICON_LIMIT {
        return None;
    }
    let mut reader = ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ICON_SIDE);
    limits.max_image_height = Some(MAX_ICON_SIDE);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?.to_rgba8();
    Some(Pixels {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

/// The game keeps a server's icon in `servers.dat` as base64 PNG.
fn stored_icon(text: &str) -> Option<Pixels> {
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .ok()?;
    decode_icon(&bytes)
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
        icon: entry.icon.clone(),
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

/// The protocol a server must be asked with for `game_version`: the table for
/// versions before `version.json` was added to the client jar, otherwise the
/// number the jar carries. `None` when neither is known, so the modern
/// handshake goes out without a version.
#[must_use]
pub fn protocol_version(game_version: &str, versions_dir: &Path) -> Option<ProtocolVersion> {
    if let Some(known) = protocol::OLD_PROTOCOL_VERSIONS.get(game_version) {
        return Some(*known);
    }
    let jar = versions_dir
        .join(game_version)
        .join(format!("{game_version}.jar"));
    protocol_from_jar(&jar).map(ProtocolVersion::modern)
}

/// The `protocolVersion` a client jar embeds in `version.json` (present from
/// snapshot 18w47b onward). A missing or unreadable jar is `None`.
fn protocol_from_jar(jar: &Path) -> Option<u32> {
    let mut archive = zip::ZipArchive::new(fs::File::open(jar).ok()?).ok()?;
    let mut entry = archive.by_name("version.json").ok()?;
    let mut text = String::new();
    io::Read::read_to_string(&mut entry, &mut text).ok()?;
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()?
        .get("protocolVersion")
        .and_then(serde_json::Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
}

#[cfg(test)]
mod tests;
