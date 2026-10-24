use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use lumilio_block_assets::{BakeError, ContainerError};

use super::entities::EntityGeometry;
use super::index::BundleIndex;
use super::provided::ProvidedSource;

/// What an instance offers to draw with: its overlays and which of them the
/// studio folder switched on. Its installed clients only lend their
/// language files for names ([`super::installed_names`]); their block assets are
/// never baked (D23).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstanceAssets {
    /// The game version the instance plays (`26.3`, `1.20.1`).
    pub game_version: String,
    /// The vanilla client jar the instance launches with, when installed.
    pub client: Option<InstalledClient>,
    /// Every other installed client jar, most recently changed first.
    pub other_clients: Vec<InstalledClient>,
    /// `mods/*.jar`, the enabled resource packs and the packs added in the
    /// studio that exist, lowest priority first in the order the studio
    /// keeps (T36): every overlay on offer.
    pub overlays: Vec<PathBuf>,
    /// Every pack added in the studio, whether or not its file is there.
    pub added: Vec<PathBuf>,
    /// The overlays the studio folder switched on; the rest are off (T21).
    pub switched_on: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledClient {
    pub version: String,
    pub jar: PathBuf,
}

/// Whether the bundle's index could be had when the plan was made.
#[derive(Clone, Copy, Debug)]
pub enum BundleReach<'a> {
    Index(&'a BundleIndex),
    /// No network (or the host could not be reached) and nothing cached.
    Offline,
    /// The host answered but has no usable index there (not published yet,
    /// or a wrong address), and nothing cached.
    Unavailable,
    /// Automatic download is turned off in settings and nothing is cached.
    Disabled,
}

/// The sources a preview is drawn with, lowest priority first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetPlan {
    pub sources: Vec<PlannedSource>,
    /// Why there is no base to draw with; `None` when there is one.
    pub notice: Option<AssetNotice>,
    /// Where the entity geometry comes from (T6). After loading, the source
    /// actually used: a failed runtime dump becomes its fallback.
    pub entities: EntityGeometry,
    /// After loading, what each source in effect provides (T17).
    pub provided: Vec<ProvidedSource>,
}

impl AssetPlan {
    /// The vanilla assets everything else is layered on.
    #[must_use]
    pub fn base(&self) -> Option<&PlannedSource> {
        self.sources.first().filter(|source| source.is_base())
    }

    #[must_use]
    pub fn overlays(&self) -> Vec<&Path> {
        self.sources
            .iter()
            .filter_map(|source| match source {
                PlannedSource::Overlay(path) => Some(path.as_path()),
                _ => None,
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedSource {
    /// The pre-baked pack of this game version from the bundle.
    Bundle { version: String },
    /// A mod jar, a resource-pack zip or an unpacked resource pack.
    Overlay(PathBuf),
}

impl PlannedSource {
    #[must_use]
    pub const fn is_base(&self) -> bool {
        !matches!(self, Self::Overlay(_))
    }
}

/// A calm reason a preview has no block assets; the interface words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetNotice {
    /// No network, and the bundle was never downloaded.
    Offline,
    /// The host answered without the bundle, and it was never downloaded:
    /// the network is fine.
    Unavailable,
    /// Automatic download is off, and the bundle was never downloaded.
    DownloadDisabled,
    /// The sources were there but could not be read.
    Unreadable,
}

#[derive(Debug)]
pub enum SchematicAssetsError {
    /// A source could not be read as a resource archive.
    Bake(BakeError),
    /// A baked file (the bundle, a cache entry) could not be read.
    Container(ContainerError),
    /// The downloaded bundle is not the file its index names.
    Mismatch {
        file: String,
    },
    /// The index names a format this launcher cannot read.
    UnsupportedIndex(u32),
    /// The index is not the expected JSON.
    Index(String),
    /// The bundle could not be fetched.
    Download(String),
    /// The host answered without the bundle (an error status, or an index
    /// that is not one).
    NotServed(String),
    /// The plan has no base source.
    NoBase,
    /// The entity dumper could not run or failed (the bake's test runner).
    EntityDump(String),
    Io(std::io::Error),
}

impl Display for SchematicAssetsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bake(error) => write!(f, "{error}"),
            Self::Container(error) => write!(f, "{error}"),
            Self::Mismatch { file } => {
                write!(f, "{file} does not match the SHA-256 its index names")
            }
            Self::UnsupportedIndex(format) => write!(
                f,
                "the block-asset index uses format {format}, which this launcher cannot read"
            ),
            Self::Index(reason) => write!(f, "the block-asset index is unreadable: {reason}"),
            Self::Download(reason) => write!(f, "the block assets could not be fetched: {reason}"),
            Self::NotServed(reason) => {
                write!(f, "the block assets are not on the server: {reason}")
            }
            Self::NoBase => f.write_str("there are no block assets to draw with"),
            Self::EntityDump(reason) => write!(f, "the entity geometry dump failed: {reason}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SchematicAssetsError {}

impl From<BakeError> for SchematicAssetsError {
    fn from(error: BakeError) -> Self {
        Self::Bake(error)
    }
}

impl From<ContainerError> for SchematicAssetsError {
    fn from(error: ContainerError) -> Self {
        Self::Container(error)
    }
}

impl From<std::io::Error> for SchematicAssetsError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
