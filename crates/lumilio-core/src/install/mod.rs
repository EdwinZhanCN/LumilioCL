mod assets;
mod files;
mod installer;
mod native;
mod plan;

pub use self::assets::{AssetIndex, AssetIndexError, AssetObject};
pub use self::installer::{InstallError, InstallEvent, InstallReport, InstallStage, Installer};
pub use self::native::{ArchiveLimits, NativeError, NativePublication, NativePublisher};
pub use self::plan::{ArtifactKind, InstallationPlan, NativeBundle, PlannedArtifact};

use std::sync::atomic::AtomicU64;

const OFFICIAL_ASSET_ROOT: &str = "https://resources.download.minecraft.net/";
const MAX_ARCHIVE_ENTRIES: usize = 65_536;
const MAX_ARCHIVE_FILE_SIZE: u64 = 1024 * 1024 * 1024;
const MAX_ARCHIVE_EXPANDED_SIZE: u64 = 4 * 1024 * 1024 * 1024;

static PUBLICATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);
