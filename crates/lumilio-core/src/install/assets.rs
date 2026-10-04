use super::OFFICIAL_ASSET_ROOT;
use super::files::{validate_protocol_path, validate_sha1};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct AssetIndexWire {
    #[serde(default)]
    pub(super) objects: BTreeMap<String, AssetObjectWire>,
    #[serde(default, rename = "virtual")]
    pub(super) virtual_layout: bool,
    #[serde(default)]
    pub(super) map_to_resources: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct AssetObjectWire {
    pub(super) hash: String,
    pub(super) size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetObject {
    pub(super) hash: String,
    pub(super) size: u64,
}

impl AssetObject {
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn relative_path(&self) -> PathBuf {
        PathBuf::from(&self.hash[..2]).join(&self.hash)
    }

    #[must_use]
    pub fn official_url(&self) -> String {
        format!("{OFFICIAL_ASSET_ROOT}{}/{}", &self.hash[..2], self.hash)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetIndex {
    pub(super) objects: BTreeMap<String, AssetObject>,
    pub(super) virtual_layout: bool,
    pub(super) map_to_resources: bool,
}

impl AssetIndex {
    pub fn decode_json(json: &str) -> Result<Self, AssetIndexError> {
        let wire: AssetIndexWire = serde_json::from_str(json).map_err(AssetIndexError::Json)?;
        let mut objects = BTreeMap::new();
        let mut known_sizes = BTreeMap::<String, u64>::new();
        for (logical_name, object) in wire.objects {
            validate_protocol_path(&logical_name).map_err(|reason| {
                AssetIndexError::UnsafeLogicalPath {
                    path: logical_name.clone(),
                    reason,
                }
            })?;
            validate_sha1(&object.hash).map_err(|_| AssetIndexError::InvalidHash {
                logical_name: logical_name.clone(),
                hash: object.hash.clone(),
            })?;
            let hash = object.hash.to_ascii_lowercase();
            if let Some(previous_size) = known_sizes.insert(hash.clone(), object.size)
                && previous_size != object.size
            {
                return Err(AssetIndexError::ConflictingSize {
                    hash,
                    first: previous_size,
                    second: object.size,
                });
            }
            objects.insert(
                logical_name,
                AssetObject {
                    hash,
                    size: object.size,
                },
            );
        }
        Ok(Self {
            objects,
            virtual_layout: wire.virtual_layout,
            map_to_resources: wire.map_to_resources,
        })
    }

    #[must_use]
    pub fn objects(&self) -> &BTreeMap<String, AssetObject> {
        &self.objects
    }

    #[must_use]
    pub fn unique_objects(&self) -> Vec<&AssetObject> {
        let mut seen = BTreeSet::new();
        self.objects
            .values()
            .filter(|object| seen.insert(object.hash.clone()))
            .collect()
    }

    #[must_use]
    pub const fn uses_virtual_layout(&self) -> bool {
        self.virtual_layout
    }

    #[must_use]
    pub const fn maps_to_resources(&self) -> bool {
        self.map_to_resources
    }
}

#[derive(Debug)]
pub enum AssetIndexError {
    Json(serde_json::Error),
    UnsafeLogicalPath {
        path: String,
        reason: String,
    },
    InvalidHash {
        logical_name: String,
        hash: String,
    },
    ConflictingSize {
        hash: String,
        first: u64,
        second: u64,
    },
}

impl Display for AssetIndexError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid asset catalog JSON: {error}"),
            Self::UnsafeLogicalPath { path, reason } => {
                write!(formatter, "unsafe asset path {path:?}: {reason}")
            }
            Self::InvalidHash { logical_name, hash } => {
                write!(
                    formatter,
                    "invalid asset hash {hash:?} for {logical_name:?}"
                )
            }
            Self::ConflictingSize {
                hash,
                first,
                second,
            } => write!(
                formatter,
                "asset {hash} has conflicting sizes {first} and {second}"
            ),
        }
    }
}

impl Error for AssetIndexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}
