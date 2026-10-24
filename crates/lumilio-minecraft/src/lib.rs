//! Minecraft metadata: package coordinates, platform rules, the version
//! catalog, release manifests and the launch plan built from them.
//!
//! Everything here works on explicit input. Fetching documents, installing
//! files and running the game belong to `lumilio-core`.

pub use artifact::{CoordinateError, PackageCoordinate};
pub use catalog::{
    CatalogEntry, CatalogError, OFFICIAL_CATALOG_URL, VersionCatalog, VersionChannel, VersionKind,
};
pub use environment::{
    CompatibilityRule, HostProfile, MachineArchitecture, PlatformFamily, RuleDecision,
};
pub use launch::{
    LaunchContext, LaunchDirectories, LaunchError, LaunchPlan, NativeArchive, RequiredDownload,
};
pub use release::{
    ArgumentGroups, ArgumentTemplate, AssetCatalog, DownloadDescriptor, ExtractionPolicy,
    IdentifiedDownload, JavaRequirement, LibraryDependency, LoggingConfiguration, ReleaseError,
    ReleaseManifest, ReleaseSet, ResolvedLibraryFile,
};

mod artifact;
mod catalog;
mod environment;
mod launch;
mod release;
