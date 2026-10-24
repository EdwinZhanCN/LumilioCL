#[cfg(test)]
mod tests;

mod library;

pub(crate) use arguments::expand_variables;
pub use arguments::{ArgumentGroups, ArgumentTemplate};
pub use library::{LibraryDependency, ResolvedLibraryFile};
use release_manifest::DEFAULT_LIBRARY_REPOSITORY;
pub use release_manifest::{
    AssetCatalog, DownloadDescriptor, ExtractionPolicy, IdentifiedDownload, JavaRequirement,
    LoggingConfiguration, ReleaseError, ReleaseManifest, ReleaseSet,
};

mod arguments;

mod release_manifest;
