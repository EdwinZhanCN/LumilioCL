//! `blocks.json`, the bundle's index: which game versions it holds, by data
//! version, and the file(s) to fetch with their size and SHA-256. Written by
//! `cargo xtask bake-blocks` (plan schematic-studio, T2).

use serde::Deserialize;

use super::SchematicAssetsError;
use lumilio_block_assets::BUNDLE_FORMAT;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BundleIndex {
    pub format: u32,
    /// The single bundle file; per-version slices name their own (D2).
    pub file: String,
    pub size: u64,
    pub sha256: String,
    pub versions: Vec<BundleVersion>,
    /// `entities.dat.z`, the entity geometry (T6); absent from a bake
    /// without it.
    #[serde(default)]
    pub entities: Option<EntityIndex>,
    /// `names.dat.z`, the game's names for blocks, items and entities
    /// (T26); absent from a bake without it.
    #[serde(default)]
    pub names: Option<NamesIndex>,
}

/// The names file.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct NamesIndex {
    pub file: String,
    pub size: u64,
    pub sha256: String,
}

/// The entity geometry file and the versions it holds.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct EntityIndex {
    pub file: String,
    pub size: u64,
    pub sha256: String,
    pub versions: Vec<EntityIndexVersion>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct EntityIndexVersion {
    pub id: String,
    /// Set for a version that borrows another's geometry (before 1.17).
    #[serde(default)]
    pub geometry_from: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BundleVersion {
    pub id: String,
    pub data_version: u32,
    /// Set when the bundle is split into one file per version (D2: above
    /// 40 MB); the index stays the same otherwise.
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
}

/// A file to fetch: its name beside the index, size and SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleFile {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

/// A name that stays inside the bundle folder.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn sha256_hex(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())
}

impl BundleIndex {
    /// Reads and checks an index. A newer format is refused by number, so
    /// the interface can say why rather than fail on a field.
    pub fn parse(bytes: &[u8]) -> Result<Self, SchematicAssetsError> {
        #[derive(Deserialize)]
        struct Format {
            format: u32,
        }
        let format: Format = serde_json::from_slice(bytes)
            .map_err(|error| SchematicAssetsError::Index(error.to_string()))?;
        if format.format != BUNDLE_FORMAT {
            return Err(SchematicAssetsError::UnsupportedIndex(format.format));
        }
        let index: Self = serde_json::from_slice(bytes)
            .map_err(|error| SchematicAssetsError::Index(error.to_string()))?;
        let files = std::iter::once((Some(&index.file), Some(&index.sha256)))
            .chain(
                index
                    .versions
                    .iter()
                    .filter(|version| version.file.is_some())
                    .map(|version| (version.file.as_ref(), version.sha256.as_ref())),
            )
            .chain(
                index
                    .entities
                    .iter()
                    .map(|entities| (Some(&entities.file), Some(&entities.sha256))),
            )
            .chain(
                index
                    .names
                    .iter()
                    .map(|names| (Some(&names.file), Some(&names.sha256))),
            );
        for (file, sha256) in files {
            let (Some(file), Some(sha256)) = (file, sha256) else {
                return Err(SchematicAssetsError::Index(
                    "a slice has no SHA-256".to_owned(),
                ));
            };
            if !plain_name(file) || !sha256_hex(sha256) {
                return Err(SchematicAssetsError::Index(format!(
                    "{file} is not a plain file name with a SHA-256"
                )));
            }
        }
        if index.versions.is_empty() {
            return Err(SchematicAssetsError::Index("no versions".to_owned()));
        }
        Ok(index)
    }

    /// The version whose assets draw a schematic of `data_version`: the same
    /// data version, else the oldest newer one, else the newest. Without a
    /// data version, the instance's own `game_version` if the bundle has it,
    /// else the newest.
    #[must_use]
    pub fn version_for(&self, data_version: Option<u32>, game_version: &str) -> &BundleVersion {
        let newest = self
            .versions
            .iter()
            .max_by_key(|version| version.data_version)
            .expect("parse refuses an index without versions");
        match data_version {
            Some(wanted) => self
                .versions
                .iter()
                .filter(|version| version.data_version >= wanted)
                .min_by_key(|version| version.data_version)
                .unwrap_or(newest),
            None => self
                .versions
                .iter()
                .find(|version| version.id == game_version)
                .unwrap_or(newest),
        }
    }

    #[must_use]
    pub fn version(&self, id: &str) -> Option<&BundleVersion> {
        self.versions.iter().find(|version| version.id == id)
    }

    /// Whether `entities.dat.z` has geometry for game version `id`.
    #[must_use]
    pub fn covers_entities(&self, id: &str) -> bool {
        self.entities
            .as_ref()
            .is_some_and(|entities| entities.versions.iter().any(|version| version.id == id))
    }

    /// The entity geometry file, when the bundle has one.
    #[must_use]
    pub fn entities_file(&self) -> Option<BundleFile> {
        self.entities.as_ref().map(|entities| BundleFile {
            name: entities.file.clone(),
            size: entities.size,
            sha256: entities.sha256.clone(),
        })
    }

    /// The names file, when the bundle has one.
    #[must_use]
    pub fn names_file(&self) -> Option<BundleFile> {
        self.names.as_ref().map(|names| BundleFile {
            name: names.file.clone(),
            size: names.size,
            sha256: names.sha256.clone(),
        })
    }

    /// The file that holds `version`'s pack.
    #[must_use]
    pub fn file_of(&self, version: &BundleVersion) -> BundleFile {
        match (&version.file, &version.sha256) {
            (Some(name), Some(sha256)) => BundleFile {
                name: name.clone(),
                size: version.size.unwrap_or_default(),
                sha256: sha256.clone(),
            },
            _ => BundleFile {
                name: self.file.clone(),
                size: self.size,
                sha256: self.sha256.clone(),
            },
        }
    }
}
