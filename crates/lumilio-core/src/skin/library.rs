//! Local wardrobe index and immutable PNGs. The order, model and origin are
//! launcher-owned facts; no Mojang artwork is bundled (ADR 0024).

use super::{AppearanceError, SkinModel, mojang::normalize_png};
use crate::persist::{self, Versioned};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SkinSource {
    LocalFile(PathBuf),
    Mojang(String),
}

/// The cape a library skin is worn with on a Microsoft account. Entries
/// written before pairing existed keep the cape as it is.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum PairedCape {
    #[default]
    Keep,
    Hidden,
    Cape(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibrarySkin {
    pub id: String,
    pub name: String,
    pub model: SkinModel,
    pub source: SkinSource,
    #[serde(default)]
    pub cape: PairedCape,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Index {
    schema: u32,
    entries: Vec<LibrarySkin>,
}
impl Versioned for Index {
    fn schema(&self) -> u32 {
        self.schema
    }
    fn set_schema(&mut self, schema: u32) {
        self.schema = schema;
    }
}

pub struct SkinLibrary {
    directory: PathBuf,
    index: Index,
}

fn storage(error: impl std::fmt::Display) -> AppearanceError {
    AppearanceError::Storage(error.to_string())
}

impl SkinLibrary {
    /// Caller serializes writes and runs disk work off its UI thread.
    pub fn open(root: &Path) -> Result<Self, AppearanceError> {
        let directory = root.join("skins");
        let index = persist::load::<Index>(&directory.join("library.json"), 1)
            .map_err(storage)?
            .value;
        let mut ids = std::collections::BTreeSet::new();
        if index.entries.iter().any(|entry| {
            entry.id.len() != 64
                || !entry.id.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !ids.insert(&entry.id)
        }) {
            return Err(AppearanceError::Storage(
                "invalid or repeated skin id in index".into(),
            ));
        }
        Ok(Self { directory, index })
    }

    #[must_use]
    pub fn entries(&self) -> &[LibrarySkin] {
        &self.index.entries
    }

    pub fn path(&self, id: &str) -> Result<PathBuf, AppearanceError> {
        self.entry(id)?;
        Ok(self.directory.join(format!("{id}.png")))
    }

    fn entry(&self, id: &str) -> Result<&LibrarySkin, AppearanceError> {
        self.index
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(AppearanceError::UnknownSkin)
    }

    pub fn picture(&self, id: &str) -> Result<Vec<u8>, AppearanceError> {
        let path = self.path(id)?;
        if fs::metadata(&path).map_err(storage)?.len() > super::PICTURE_LIMIT {
            return Err(AppearanceError::Picture("skin exceeds size limit".into()));
        }
        normalize_png(&fs::read(path).map_err(storage)?)
    }

    /// Repeated pixels occupy one entry; import order is retained. The
    /// normalized copy survives moving or removing the source file.
    pub fn add(
        &mut self,
        name: String,
        bytes: &[u8],
        model: SkinModel,
        source: SkinSource,
    ) -> Result<LibrarySkin, AppearanceError> {
        let png = normalize_png(bytes)?;
        let id = Sha256::digest(&png)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if let Ok(entry) = self.entry(&id) {
            return Ok(entry.clone());
        }
        fs::create_dir_all(&self.directory).map_err(storage)?;
        let path = self.directory.join(format!("{id}.png"));
        let temporary = self.directory.join(format!("{id}.png.tmp"));
        fs::write(&temporary, &png).map_err(storage)?;
        fs::rename(&temporary, path).map_err(storage)?;
        let entry = LibrarySkin {
            id,
            name,
            model,
            source,
            cape: PairedCape::Keep,
        };
        let mut index = self.index.clone();
        index.entries.push(entry.clone());
        self.commit(index)?;
        Ok(entry)
    }

    pub fn set_model(&mut self, id: &str, model: SkinModel) -> Result<(), AppearanceError> {
        self.entry(id)?;
        let mut index = self.index.clone();
        index
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .expect("checked id")
            .model = model;
        self.commit(index)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), AppearanceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppearanceError::EmptyName);
        }
        self.update(id, |entry| entry.name = name.to_owned())
    }

    pub fn set_cape(&mut self, id: &str, cape: PairedCape) -> Result<(), AppearanceError> {
        self.update(id, |entry| entry.cape = cape)
    }

    /// Puts another picture in an entry's place, keeping its name, model,
    /// cape and position. The old file stays, like a removed entry's, so an
    /// offline account that chose it still launches. A picture already in
    /// the library takes the place and the duplicate row is dropped.
    pub fn replace(
        &mut self,
        id: &str,
        bytes: &[u8],
        source: SkinSource,
    ) -> Result<LibrarySkin, AppearanceError> {
        let old = self.entry(id)?.clone();
        let added = self.add(old.name.clone(), bytes, old.model, source)?;
        if added.id == old.id {
            return Ok(added);
        }
        let mut index = self.index.clone();
        index.entries.retain(|entry| entry.id != added.id);
        let at = index
            .entries
            .iter()
            .position(|entry| entry.id == old.id)
            .expect("checked id");
        let replaced = LibrarySkin {
            id: added.id,
            source: added.source,
            ..old
        };
        index.entries[at] = replaced.clone();
        self.commit(index)?;
        Ok(replaced)
    }

    fn update(
        &mut self,
        id: &str,
        change: impl FnOnce(&mut LibrarySkin),
    ) -> Result<(), AppearanceError> {
        self.entry(id)?;
        let mut index = self.index.clone();
        change(
            index
                .entries
                .iter_mut()
                .find(|entry| entry.id == id)
                .expect("checked id"),
        );
        self.commit(index)
    }

    /// Removes the wardrobe row. Its immutable file remains valid for an
    /// offline account that already selected it; removal never breaks launch.
    pub fn remove(&mut self, id: &str) -> Result<(), AppearanceError> {
        self.entry(id)?;
        let mut index = self.index.clone();
        index.entries.retain(|entry| entry.id != id);
        self.commit(index)
    }

    pub fn reorder(&mut self, ids: &[String]) -> Result<(), AppearanceError> {
        if ids.len() != self.index.entries.len()
            || ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len()
        {
            return Err(AppearanceError::InvalidOrder);
        }
        let mut index = self.index.clone();
        index.entries = ids
            .iter()
            .map(|id| self.entry(id).cloned())
            .collect::<Result<_, _>>()?;
        self.commit(index)
    }

    fn commit(&mut self, mut index: Index) -> Result<(), AppearanceError> {
        persist::save(&self.directory.join("library.json"), &mut index, 1).map_err(storage)?;
        self.index = index;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
