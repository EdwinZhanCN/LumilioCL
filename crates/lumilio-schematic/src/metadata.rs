//! Reading and editing a schematic's descriptive metadata (schematic-studio T15).
//!
//! Edits touch only name, author, description and the modified time; block
//! data stays as it was.

use std::fmt;

use crate::UniversalSchematic;

/// What the inspector's metadata page shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub name: String,
    pub author: String,
    pub description: String,
    /// The box enclosing every region.
    pub size: (i32, i32, i32),
    /// Non-air blocks, each cell once.
    pub block_count: u64,
    pub region_count: usize,
    /// Milliseconds since the Unix epoch, as Litematica stores them.
    pub created: Option<u64>,
    pub modified: Option<u64>,
    /// The Minecraft data version the content is at; `None` when the file
    /// names none and the version is unknown.
    pub data_version: Option<i32>,
}

pub fn read(schematic: &UniversalSchematic) -> Info {
    let stats = crate::stats::stats(schematic);
    let metadata = &schematic.metadata;
    Info {
        name: metadata.name.clone().unwrap_or_default(),
        author: metadata.author.clone().unwrap_or_default(),
        description: metadata.description.clone().unwrap_or_default(),
        size: stats.size,
        block_count: stats.block_count,
        region_count: stats.regions.len(),
        created: metadata.created,
        modified: metadata.modified,
        data_version: crate::convert::known_data_version(schematic),
    }
}

/// The editable fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub name: String,
    pub author: String,
    pub description: String,
}

/// Why an edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    EmptyName,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::EmptyName => f.write_str("the name must not be empty"),
        }
    }
}

impl std::error::Error for EditError {}

/// Applies an edit and sets the modified time to `now_millis`. A blank name is
/// refused and leaves the schematic unchanged. Name and author are trimmed.
pub fn apply(
    schematic: &mut UniversalSchematic,
    edit: &Edit,
    now_millis: u64,
) -> Result<(), EditError> {
    let name = edit.name.trim();
    if name.is_empty() {
        return Err(EditError::EmptyName);
    }
    let metadata = &mut schematic.metadata;
    metadata.name = Some(name.to_owned());
    metadata.author = Some(edit.author.trim().to_owned());
    metadata.description = Some(edit.description.clone());
    // Litematica writes TimeCreated from `created`, or from now when missing;
    // pin it so a metadata edit never moves the creation time.
    metadata.created.get_or_insert(now_millis);
    metadata.modified = Some(now_millis);
    Ok(())
}
