use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::File as StandardFile;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use zip::ZipArchive;

use crate::activity::CancellationToken;
use crate::launch::{LaunchDirectories, LaunchPlan};
use crate::release::{DownloadDescriptor, ReleaseManifest};
use crate::transfer::{
    SourceChain, TransferBatchReport, TransferEngine, TransferError, TransferRequest, Transport,
};

const OFFICIAL_ASSET_ROOT: &str = "https://resources.download.minecraft.net/";
const MAX_ARCHIVE_ENTRIES: usize = 65_536;
const MAX_ARCHIVE_FILE_SIZE: u64 = 1024 * 1024 * 1024;
const MAX_ARCHIVE_EXPANDED_SIZE: u64 = 4 * 1024 * 1024 * 1024;

static PUBLICATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize)]
struct AssetIndexWire {
    #[serde(default)]
    objects: BTreeMap<String, AssetObjectWire>,
    #[serde(default, rename = "virtual")]
    virtual_layout: bool,
    #[serde(default)]
    map_to_resources: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct AssetObjectWire {
    hash: String,
    size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetObject {
    hash: String,
    size: u64,
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
    objects: BTreeMap<String, AssetObject>,
    virtual_layout: bool,
    map_to_resources: bool,
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ArtifactKind {
    Client,
    Library,
    NativeArchive,
    LoggingConfiguration,
    AssetCatalog,
    AssetObject,
}

impl ArtifactKind {
    const fn identifier(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Library => "library",
            Self::NativeArchive => "native",
            Self::LoggingConfiguration => "logging",
            Self::AssetCatalog => "catalog",
            Self::AssetObject => "asset",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedArtifact {
    kind: ArtifactKind,
    request: TransferRequest,
}

impl PlannedArtifact {
    #[must_use]
    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }

    #[must_use]
    pub const fn request(&self) -> &TransferRequest {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeBundle {
    archive: PathBuf,
    exclusions: Vec<String>,
}

impl NativeBundle {
    #[must_use]
    pub fn new(archive: impl Into<PathBuf>) -> Self {
        Self {
            archive: archive.into(),
            exclusions: Vec::new(),
        }
    }

    #[must_use]
    pub fn excluding<I, S>(mut self, prefixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.exclusions = prefixes
            .into_iter()
            .map(Into::into)
            .filter(|prefix| !prefix.is_empty())
            .collect();
        self
    }

    #[must_use]
    pub fn archive(&self) -> &Path {
        &self.archive
    }

    #[must_use]
    pub fn exclusions(&self) -> &[String] {
        &self.exclusions
    }
}

#[derive(Clone)]
pub struct InstallationPlan {
    release_id: String,
    manifest_json: String,
    manifest_destination: PathBuf,
    directories: LaunchDirectories,
    initial_artifacts: Vec<PlannedArtifact>,
    native_bundles: Vec<NativeBundle>,
    asset_catalog_id: Option<String>,
    sources: SourceChain,
    shared_cache: Option<PathBuf>,
}

impl fmt::Debug for InstallationPlan {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstallationPlan")
            .field("release_id", &self.release_id)
            .field("manifest_destination", &self.manifest_destination)
            .field("directories", &self.directories)
            .field("initial_artifacts", &self.initial_artifacts)
            .field("native_bundles", &self.native_bundles)
            .field("asset_catalog_id", &self.asset_catalog_id)
            .field("shared_cache", &self.shared_cache)
            .finish_non_exhaustive()
    }
}

impl InstallationPlan {
    pub fn build(
        release: &ReleaseManifest,
        launch: &LaunchPlan,
        directories: LaunchDirectories,
        sources: SourceChain,
        shared_cache: Option<PathBuf>,
    ) -> Result<Self, InstallError> {
        if release.id() != launch.release_id() {
            return Err(InstallError::Plan(format!(
                "release identity {} does not match launch identity {}",
                release.id(),
                launch.release_id()
            )));
        }
        let release_component = validate_single_component(release.id()).map_err(|reason| {
            InstallError::Plan(format!(
                "unsafe release identity {:?}: {reason}",
                release.id()
            ))
        })?;
        let manifest_destination = directories
            .versions()
            .join(&release_component)
            .join(format!("{}.json", release.id()));
        let native_paths = launch
            .native_archives()
            .iter()
            .map(|archive| archive.path())
            .collect::<BTreeSet<_>>();
        let mut initial_artifacts = Vec::new();
        for required in launch.required_downloads() {
            let kind = if native_paths.contains(required.destination()) {
                ArtifactKind::NativeArchive
            } else if required.destination().starts_with(directories.versions()) {
                ArtifactKind::Client
            } else {
                ArtifactKind::Library
            };
            let id = format!("{}:{}", kind.identifier(), initial_artifacts.len());
            let cache_candidate = cache_candidate_for(
                shared_cache.as_deref(),
                kind,
                required.destination(),
                &directories,
                required.source(),
            );
            let request = request_from_descriptor(
                id,
                required.source(),
                required.destination().to_owned(),
                &sources,
                cache_candidate,
            )?;
            initial_artifacts.push(PlannedArtifact { kind, request });
        }

        if let Some(logging) = launch.logging() {
            for configuration in logging.values() {
                let relative =
                    validate_protocol_path(configuration.file().id()).map_err(|reason| {
                        InstallError::Plan(format!(
                            "unsafe logging file identity {:?}: {reason}",
                            configuration.file().id()
                        ))
                    })?;
                let destination = directories.assets().join("log_configs").join(relative);
                let kind = ArtifactKind::LoggingConfiguration;
                let id = format!("{}:{}", kind.identifier(), initial_artifacts.len());
                let descriptor = configuration.file().download();
                let cache_candidate = cache_candidate_for(
                    shared_cache.as_deref(),
                    kind,
                    &destination,
                    &directories,
                    descriptor,
                );
                let request = request_from_descriptor(
                    id,
                    descriptor,
                    destination,
                    &sources,
                    cache_candidate,
                )?;
                initial_artifacts.push(PlannedArtifact { kind, request });
            }
        }

        let asset_catalog_id = if let Some(catalog) = launch.asset_catalog() {
            let catalog_component = validate_single_component(catalog.id()).map_err(|reason| {
                InstallError::Plan(format!(
                    "unsafe asset catalog identity {:?}: {reason}",
                    catalog.id()
                ))
            })?;
            let destination = directories
                .assets()
                .join("indexes")
                .join(format!("{}.json", catalog_component.to_string_lossy()));
            let candidates = sources.candidates(catalog.url());
            let mut request =
                TransferRequest::new(format!("catalog:{}", catalog.id()), candidates, destination)?
                    .force_refresh();
            if let Some(hash) = catalog.sha1().filter(|hash| catalog.url().contains(*hash)) {
                request = request.expect_sha1(hash)?;
            }
            initial_artifacts.push(PlannedArtifact {
                kind: ArtifactKind::AssetCatalog,
                request,
            });
            Some(catalog.id().to_owned())
        } else {
            None
        };

        let native_bundles = launch
            .native_archives()
            .iter()
            .map(|archive| {
                NativeBundle::new(archive.path())
                    .excluding(archive.extraction_policy().exclusions().iter().cloned())
            })
            .collect();

        Ok(Self {
            release_id: release.id().to_owned(),
            manifest_json: release.encode_json().map_err(|error| {
                InstallError::Plan(format!("cannot encode release manifest: {error}"))
            })?,
            manifest_destination,
            directories,
            initial_artifacts,
            native_bundles,
            asset_catalog_id,
            sources,
            shared_cache,
        })
    }

    #[must_use]
    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    #[must_use]
    pub fn manifest_destination(&self) -> &Path {
        &self.manifest_destination
    }

    #[must_use]
    pub fn initial_artifacts(&self) -> &[PlannedArtifact] {
        &self.initial_artifacts
    }

    #[must_use]
    pub fn native_bundles(&self) -> &[NativeBundle] {
        &self.native_bundles
    }

    #[must_use]
    pub fn native_destination(&self) -> &Path {
        self.directories.natives()
    }

    #[must_use]
    pub fn asset_catalog_artifact(&self) -> Option<&PlannedArtifact> {
        self.initial_artifacts
            .iter()
            .find(|artifact| artifact.kind == ArtifactKind::AssetCatalog)
    }

    pub fn asset_artifacts(
        &self,
        index: &AssetIndex,
    ) -> Result<Vec<PlannedArtifact>, InstallError> {
        let mut artifacts = Vec::new();
        for object in index.unique_objects() {
            let destination = self
                .directories
                .assets()
                .join("objects")
                .join(object.relative_path());
            let cache_candidate = self
                .shared_cache
                .as_ref()
                .map(|root| root.join("assets/objects").join(object.relative_path()));
            let mut request = TransferRequest::new(
                format!("asset:{}", object.hash()),
                self.sources.candidates(&object.official_url()),
                destination,
            )?
            .expect_size(object.size())
            .expect_sha1(object.hash())?;
            if let Some(candidate) = cache_candidate {
                request = request.with_cache_candidate(candidate);
            }
            artifacts.push(PlannedArtifact {
                kind: ArtifactKind::AssetObject,
                request,
            });
        }
        Ok(artifacts)
    }
}

fn request_from_descriptor(
    id: String,
    descriptor: &DownloadDescriptor,
    destination: PathBuf,
    sources: &SourceChain,
    cache_candidate: Option<PathBuf>,
) -> Result<TransferRequest, InstallError> {
    let url = descriptor
        .url()
        .ok_or_else(|| InstallError::Plan(format!("download {id} has no usable source URL")))?;
    let mut request = TransferRequest::new(id, sources.candidates(url), destination)?;
    if descriptor.size() > 0 {
        request = request.expect_size(descriptor.size());
    }
    if let Some(hash) = descriptor.sha1() {
        request = request.expect_sha1(hash)?;
    }
    if let Some(candidate) = cache_candidate {
        request = request.with_cache_candidate(candidate);
    }
    Ok(request)
}

fn cache_candidate_for(
    root: Option<&Path>,
    kind: ArtifactKind,
    destination: &Path,
    directories: &LaunchDirectories,
    descriptor: &DownloadDescriptor,
) -> Option<PathBuf> {
    let root = root?;
    match kind {
        ArtifactKind::Library | ArtifactKind::NativeArchive => destination
            .strip_prefix(directories.libraries())
            .ok()
            .map(|relative| root.join("libraries").join(relative)),
        _ => descriptor
            .sha1()
            .map(|hash| root.join("content").join(hash)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLimits {
    maximum_entries: usize,
    maximum_file_size: u64,
    maximum_expanded_size: u64,
}

impl ArchiveLimits {
    pub fn new(
        maximum_entries: usize,
        maximum_file_size: u64,
        maximum_expanded_size: u64,
    ) -> Result<Self, NativeError> {
        if maximum_entries == 0 || maximum_file_size == 0 || maximum_expanded_size == 0 {
            return Err(NativeError::InvalidLimits);
        }
        Ok(Self {
            maximum_entries,
            maximum_file_size,
            maximum_expanded_size,
        })
    }
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            maximum_entries: MAX_ARCHIVE_ENTRIES,
            maximum_file_size: MAX_ARCHIVE_FILE_SIZE,
            maximum_expanded_size: MAX_ARCHIVE_EXPANDED_SIZE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePublication {
    published_files: usize,
}

impl NativePublication {
    #[must_use]
    pub const fn published_files(self) -> usize {
        self.published_files
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativePublisher {
    limits: ArchiveLimits,
}

impl NativePublisher {
    #[must_use]
    pub const fn with_limits(limits: ArchiveLimits) -> Self {
        Self { limits }
    }

    pub async fn publish(
        &self,
        bundles: &[NativeBundle],
        destination: impl AsRef<Path>,
        cancellation: CancellationToken,
    ) -> Result<NativePublication, NativeError> {
        if bundles.is_empty() {
            return Ok(NativePublication { published_files: 0 });
        }
        let bundles = bundles.to_vec();
        let destination = destination.as_ref().to_owned();
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            publish_native_blocking(&bundles, &destination, limits, &cancellation)
        })
        .await
        .map_err(|error| NativeError::Worker(error.to_string()))?
    }
}

#[derive(Debug)]
pub enum NativeError {
    InvalidLimits,
    Cancelled,
    UnsafePath { archive: PathBuf, entry: String },
    UnsupportedEntry { archive: PathBuf, entry: String },
    LimitExceeded { archive: PathBuf, detail: String },
    Archive { archive: PathBuf, message: String },
    FileSystem { path: PathBuf, message: String },
    Worker(String),
}

impl Display for NativeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("archive limits must be positive"),
            Self::Cancelled => formatter.write_str("native publication was cancelled"),
            Self::UnsafePath { archive, entry } => write!(
                formatter,
                "unsafe archive path {entry:?} in {}",
                archive.display()
            ),
            Self::UnsupportedEntry { archive, entry } => write!(
                formatter,
                "unsupported archive entry {entry:?} in {}",
                archive.display()
            ),
            Self::LimitExceeded { archive, detail } => {
                write!(
                    formatter,
                    "archive limit exceeded in {}: {detail}",
                    archive.display()
                )
            }
            Self::Archive { archive, message } => {
                write!(
                    formatter,
                    "cannot read archive {}: {message}",
                    archive.display()
                )
            }
            Self::FileSystem { path, message } => {
                write!(
                    formatter,
                    "filesystem operation failed at {}: {message}",
                    path.display()
                )
            }
            Self::Worker(message) => write!(formatter, "native worker failed: {message}"),
        }
    }
}

impl Error for NativeError {}

fn publish_native_blocking(
    bundles: &[NativeBundle],
    destination: &Path,
    limits: ArchiveLimits,
    cancellation: &CancellationToken,
) -> Result<NativePublication, NativeError> {
    if cancellation.is_cancelled() {
        return Err(NativeError::Cancelled);
    }
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| native_fs_error(parent, error))?;
    let stage = unique_sibling(destination, "stage");
    let backup = unique_sibling(destination, "backup");
    std::fs::create_dir(&stage).map_err(|error| native_fs_error(&stage, error))?;

    let result = extract_all(bundles, &stage, limits, cancellation);
    let published_files = match result {
        Ok(files) => files,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(error);
        }
    };
    if cancellation.is_cancelled() {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(NativeError::Cancelled);
    }

    let had_destination = destination.exists();
    if had_destination {
        std::fs::rename(destination, &backup)
            .map_err(|error| native_fs_error(destination, error))?;
    }
    if let Err(error) = std::fs::rename(&stage, destination) {
        if had_destination {
            let _ = std::fs::rename(&backup, destination);
        }
        let _ = std::fs::remove_dir_all(&stage);
        return Err(native_fs_error(destination, error));
    }
    if had_destination {
        remove_any_path(&backup).map_err(|error| native_fs_error(&backup, error))?;
    }
    Ok(NativePublication { published_files })
}

fn extract_all(
    bundles: &[NativeBundle],
    stage: &Path,
    limits: ArchiveLimits,
    cancellation: &CancellationToken,
) -> Result<usize, NativeError> {
    let mut expanded_size = 0_u64;
    let mut entry_count = 0_usize;
    let mut published_files = BTreeSet::new();
    for bundle in bundles {
        let file = StandardFile::open(&bundle.archive)
            .map_err(|error| native_fs_error(&bundle.archive, error))?;
        let mut archive = ZipArchive::new(file).map_err(|error| NativeError::Archive {
            archive: bundle.archive.clone(),
            message: error.to_string(),
        })?;
        entry_count = entry_count.saturating_add(archive.len());
        if entry_count > limits.maximum_entries {
            return Err(NativeError::LimitExceeded {
                archive: bundle.archive.clone(),
                detail: format!("more than {} entries", limits.maximum_entries),
            });
        }
        for index in 0..archive.len() {
            if cancellation.is_cancelled() {
                return Err(NativeError::Cancelled);
            }
            let mut entry = archive
                .by_index(index)
                .map_err(|error| NativeError::Archive {
                    archive: bundle.archive.clone(),
                    message: error.to_string(),
                })?;
            let raw_name = entry.name().to_owned();
            let relative =
                strict_archive_path(&raw_name).ok_or_else(|| NativeError::UnsafePath {
                    archive: bundle.archive.clone(),
                    entry: raw_name.clone(),
                })?;
            validate_entry_type(
                &bundle.archive,
                &raw_name,
                entry.is_dir(),
                entry.unix_mode(),
            )?;
            if bundle
                .exclusions
                .iter()
                .any(|prefix| raw_name.starts_with(prefix))
            {
                continue;
            }
            if entry.is_dir() {
                let directory = stage.join(relative);
                std::fs::create_dir_all(&directory)
                    .map_err(|error| native_fs_error(&directory, error))?;
                continue;
            }
            if entry.size() > limits.maximum_file_size {
                return Err(NativeError::LimitExceeded {
                    archive: bundle.archive.clone(),
                    detail: format!("entry {raw_name:?} is {} bytes", entry.size()),
                });
            }
            expanded_size = expanded_size.checked_add(entry.size()).ok_or_else(|| {
                NativeError::LimitExceeded {
                    archive: bundle.archive.clone(),
                    detail: "expanded size overflowed".to_owned(),
                }
            })?;
            if expanded_size > limits.maximum_expanded_size {
                return Err(NativeError::LimitExceeded {
                    archive: bundle.archive.clone(),
                    detail: format!(
                        "expanded content exceeds {} bytes",
                        limits.maximum_expanded_size
                    ),
                });
            }
            let output_path = stage.join(&relative);
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| native_fs_error(parent, error))?;
            }
            let mut output = StandardFile::create(&output_path)
                .map_err(|error| native_fs_error(&output_path, error))?;
            let copied = io::copy(&mut entry, &mut output)
                .map_err(|error| native_fs_error(&output_path, error))?;
            if copied != entry.size() {
                return Err(NativeError::Archive {
                    archive: bundle.archive.clone(),
                    message: format!(
                        "entry {raw_name:?} produced {copied} bytes, expected {}",
                        entry.size()
                    ),
                });
            }
            output
                .flush()
                .map_err(|error| native_fs_error(&output_path, error))?;
            published_files.insert(relative);
        }
    }
    Ok(published_files.len())
}

fn validate_entry_type(
    archive: &Path,
    entry: &str,
    is_directory: bool,
    unix_mode: Option<u32>,
) -> Result<(), NativeError> {
    let Some(mode) = unix_mode else {
        return Ok(());
    };
    let file_type = mode & 0o170_000;
    let supported = file_type == 0
        || (!is_directory && file_type == 0o100_000)
        || (is_directory && file_type == 0o040_000);
    if supported {
        Ok(())
    } else {
        Err(NativeError::UnsupportedEntry {
            archive: archive.to_owned(),
            entry: entry.to_owned(),
        })
    }
}

fn strict_archive_path(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() || raw.contains('\0') || raw.contains('\\') || raw.contains(':') {
        return None;
    }
    let trimmed = raw.strip_suffix('/').unwrap_or(raw);
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Some(path.to_owned())
    } else {
        None
    }
}

fn unique_sibling(destination: &Path, suffix: &str) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("publication");
    let sequence = PUBLICATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".{name}.lumilio.{}.{}.{suffix}",
        std::process::id(),
        sequence
    ))
}

fn remove_any_path(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn native_fs_error(path: &Path, error: io::Error) -> NativeError {
    NativeError::FileSystem {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallStage {
    InitialTransfers,
    AssetCatalogDecode,
    AssetTransfers,
    AssetViews,
    NativePublication,
    ManifestPublication,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallEvent {
    StageStarted(InstallStage),
    StageCompleted {
        stage: InstallStage,
        items: usize,
    },
    StageFailed {
        stage: InstallStage,
        message: String,
    },
    Completed {
        manifest: PathBuf,
    },
}

pub struct Installer<T> {
    transfers: TransferEngine<T>,
    native_publisher: NativePublisher,
    events: broadcast::Sender<InstallEvent>,
}

impl<T> Installer<T>
where
    T: Transport,
{
    #[must_use]
    pub fn new(transfers: TransferEngine<T>) -> Self {
        let (events, _) = broadcast::channel(128);
        Self {
            transfers,
            native_publisher: NativePublisher::default(),
            events,
        }
    }

    #[must_use]
    pub fn with_native_publisher(mut self, publisher: NativePublisher) -> Self {
        self.native_publisher = publisher;
        self
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<InstallEvent> {
        self.events.subscribe()
    }

    pub async fn execute(
        &self,
        plan: &InstallationPlan,
        cancellation: CancellationToken,
    ) -> Result<InstallReport, InstallError> {
        let publication = [
            plan.directories.natives().to_path_buf(),
            plan.manifest_destination.clone(),
        ];
        let _resources = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(InstallError::Cancelled),
            acquired = crate::resource_lock::acquire(&publication) => acquired.map_err(|error| InstallError::FileSystem {
                operation: "lock installation publication", path: plan.manifest_destination.clone(), message: error.to_string(),
            })?,
        };
        self.begin(InstallStage::InitialTransfers);
        let initial = self
            .transfers
            .transfer_batch(
                plan.initial_artifacts
                    .iter()
                    .map(|artifact| artifact.request.clone())
                    .collect(),
                cancellation.clone(),
            )
            .await;
        self.require_batch(InstallStage::InitialTransfers, &initial)?;
        self.complete(InstallStage::InitialTransfers, initial.succeeded());
        ensure_not_cancelled(&cancellation)?;

        self.begin(InstallStage::AssetCatalogDecode);
        let asset_index = match plan.asset_catalog_artifact() {
            Some(catalog) => {
                let json = fs::read_to_string(catalog.request.destination())
                    .await
                    .map_err(|error| InstallError::FileSystem {
                        operation: "read asset catalog",
                        path: catalog.request.destination().to_owned(),
                        message: error.to_string(),
                    })
                    .map_err(|error| {
                        self.record_failure(InstallStage::AssetCatalogDecode, error)
                    })?;
                Some(
                    AssetIndex::decode_json(&json)
                        .map_err(InstallError::from)
                        .map_err(|error| {
                            self.record_failure(InstallStage::AssetCatalogDecode, error)
                        })?,
                )
            }
            None => None,
        };
        self.complete(
            InstallStage::AssetCatalogDecode,
            usize::from(asset_index.is_some()),
        );

        self.begin(InstallStage::AssetTransfers);
        let asset_artifacts = asset_index
            .as_ref()
            .map(|index| plan.asset_artifacts(index))
            .transpose()
            .map_err(|error| self.record_failure(InstallStage::AssetTransfers, error))?
            .unwrap_or_default();
        let assets = self
            .transfers
            .transfer_batch(
                asset_artifacts
                    .iter()
                    .map(|artifact| artifact.request.clone())
                    .collect(),
                cancellation.clone(),
            )
            .await;
        self.require_batch(InstallStage::AssetTransfers, &assets)?;
        self.complete(InstallStage::AssetTransfers, assets.succeeded());
        ensure_not_cancelled(&cancellation)?;

        self.begin(InstallStage::AssetViews);
        let asset_views = match &asset_index {
            Some(index) => publish_asset_views(plan, index, &cancellation)
                .await
                .map_err(|error| self.record_failure(InstallStage::AssetViews, error))?,
            None => 0,
        };
        self.complete(InstallStage::AssetViews, asset_views);

        self.begin(InstallStage::NativePublication);
        let natives = self
            .native_publisher
            .publish(
                &plan.native_bundles,
                plan.directories.natives(),
                cancellation.clone(),
            )
            .await
            .map_err(InstallError::from)
            .map_err(|error| self.record_failure(InstallStage::NativePublication, error))?;
        self.complete(InstallStage::NativePublication, natives.published_files());

        self.begin(InstallStage::ManifestPublication);
        atomic_write(&plan.manifest_destination, plan.manifest_json.as_bytes())
            .await
            .map_err(|error| self.record_failure(InstallStage::ManifestPublication, error))?;
        self.complete(InstallStage::ManifestPublication, 1);
        let _ = self.events.send(InstallEvent::Completed {
            manifest: plan.manifest_destination.clone(),
        });

        Ok(InstallReport {
            initial,
            assets,
            asset_objects: asset_artifacts.len(),
            asset_views,
            native_files: natives.published_files(),
            manifest: plan.manifest_destination.clone(),
        })
    }

    fn begin(&self, stage: InstallStage) {
        let _ = self.events.send(InstallEvent::StageStarted(stage));
    }

    fn complete(&self, stage: InstallStage, items: usize) {
        let _ = self
            .events
            .send(InstallEvent::StageCompleted { stage, items });
    }

    fn require_batch(
        &self,
        stage: InstallStage,
        report: &TransferBatchReport,
    ) -> Result<(), InstallError> {
        if report.failed() == 0 {
            return Ok(());
        }
        let failures = report
            .results()
            .iter()
            .filter_map(|(id, result)| {
                result
                    .as_ref()
                    .err()
                    .map(|error| (id.clone(), error.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let error = if failures
            .values()
            .any(|error| matches!(error, TransferError::Cancelled))
        {
            InstallError::Cancelled
        } else {
            InstallError::Transfers { stage, failures }
        };
        Err(self.record_failure(stage, error))
    }

    fn record_failure(&self, stage: InstallStage, error: InstallError) -> InstallError {
        let _ = self.events.send(InstallEvent::StageFailed {
            stage,
            message: error.to_string(),
        });
        error
    }
}

#[derive(Clone, Debug)]
pub struct InstallReport {
    initial: TransferBatchReport,
    assets: TransferBatchReport,
    asset_objects: usize,
    asset_views: usize,
    native_files: usize,
    manifest: PathBuf,
}

impl InstallReport {
    #[must_use]
    pub const fn initial_transfers(&self) -> &TransferBatchReport {
        &self.initial
    }

    #[must_use]
    pub const fn asset_transfers(&self) -> &TransferBatchReport {
        &self.assets
    }

    #[must_use]
    pub const fn asset_objects(&self) -> usize {
        self.asset_objects
    }

    #[must_use]
    pub const fn asset_views(&self) -> usize {
        self.asset_views
    }

    #[must_use]
    pub const fn native_files(&self) -> usize {
        self.native_files
    }

    #[must_use]
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }
}

#[derive(Debug)]
pub enum InstallError {
    Plan(String),
    Transfer(TransferError),
    Transfers {
        stage: InstallStage,
        failures: BTreeMap<String, TransferError>,
    },
    AssetIndex(AssetIndexError),
    Native(NativeError),
    Cancelled,
    FileSystem {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
}

impl Display for InstallError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(message) => write!(formatter, "invalid installation plan: {message}"),
            Self::Transfer(error) => Display::fmt(error, formatter),
            Self::Transfers { stage, failures } => {
                write!(
                    formatter,
                    "{} transfers failed during {stage:?}",
                    failures.len()
                )?;
                // Name the first few, so 技术详情 says what could not be fetched.
                for (id, error) in failures.iter().take(3) {
                    write!(formatter, "\n{id}: {error}")?;
                }
                Ok(())
            }
            Self::AssetIndex(error) => Display::fmt(error, formatter),
            Self::Native(error) => Display::fmt(error, formatter),
            Self::Cancelled => formatter.write_str("installation was cancelled"),
            Self::FileSystem {
                operation,
                path,
                message,
            } => write!(
                formatter,
                "failed to {operation} {}: {message}",
                path.display()
            ),
        }
    }
}

impl Error for InstallError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transfer(error) => Some(error),
            Self::AssetIndex(error) => Some(error),
            Self::Native(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TransferError> for InstallError {
    fn from(error: TransferError) -> Self {
        Self::Transfer(error)
    }
}

impl From<AssetIndexError> for InstallError {
    fn from(error: AssetIndexError) -> Self {
        Self::AssetIndex(error)
    }
}

impl From<NativeError> for InstallError {
    fn from(error: NativeError) -> Self {
        Self::Native(error)
    }
}

async fn publish_asset_views(
    plan: &InstallationPlan,
    index: &AssetIndex,
    cancellation: &CancellationToken,
) -> Result<usize, InstallError> {
    let Some(catalog_id) = plan.asset_catalog_id.as_deref() else {
        return Ok(0);
    };
    let mut published = 0_usize;
    for (logical_name, object) in index.objects() {
        ensure_not_cancelled(cancellation)?;
        let relative = validate_protocol_path(logical_name).map_err(InstallError::Plan)?;
        let object_path = plan
            .directories
            .assets()
            .join("objects")
            .join(object.relative_path());
        if index.uses_virtual_layout() {
            let destination = plan
                .directories
                .assets()
                .join("virtual")
                .join(catalog_id)
                .join(&relative);
            publish_file_view(&object_path, &destination).await?;
            published += 1;
        }
        if index.maps_to_resources() {
            let destination = plan.directories.game().join("resources").join(&relative);
            publish_file_view(&object_path, &destination).await?;
            published += 1;
        }
    }
    Ok(published)
}

async fn publish_file_view(source: &Path, destination: &Path) -> Result<(), InstallError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .map_err(|error| install_fs_error("create asset view directory", parent, error))?;
    let temporary = unique_sibling(destination, "asset");
    match fs::hard_link(source, &temporary).await {
        Ok(()) => {}
        Err(_) => {
            fs::copy(source, &temporary)
                .await
                .map_err(|error| install_fs_error("copy asset view", source, error))?;
        }
    }
    replace_path(&temporary, destination).await
}

async fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .map_err(|error| install_fs_error("create manifest directory", parent, error))?;
    let temporary = unique_sibling(destination, "manifest");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(|error| install_fs_error("create manifest staging file", &temporary, error))?;
    if let Err(error) = file.write_all(bytes).await {
        let _ = fs::remove_file(&temporary).await;
        return Err(install_fs_error(
            "write manifest staging file",
            &temporary,
            error,
        ));
    }
    if let Err(error) = file.flush().await {
        let _ = fs::remove_file(&temporary).await;
        return Err(install_fs_error(
            "flush manifest staging file",
            &temporary,
            error,
        ));
    }
    if let Err(error) = file.sync_all().await {
        let _ = fs::remove_file(&temporary).await;
        return Err(install_fs_error(
            "sync manifest staging file",
            &temporary,
            error,
        ));
    }
    drop(file);
    replace_path(&temporary, destination).await
}

async fn replace_path(temporary: &Path, destination: &Path) -> Result<(), InstallError> {
    let backup = unique_sibling(destination, "backup");
    let had_destination = fs::symlink_metadata(destination).await.is_ok();
    if had_destination {
        fs::rename(destination, &backup)
            .await
            .map_err(|error| install_fs_error("stage previous destination", destination, error))?;
    }
    if let Err(error) = fs::rename(temporary, destination).await {
        if had_destination {
            let _ = fs::rename(&backup, destination).await;
        }
        let _ = fs::remove_file(temporary).await;
        return Err(install_fs_error(
            "publish staged destination",
            destination,
            error,
        ));
    }
    if had_destination {
        remove_async_path(&backup).await?;
    }
    Ok(())
}

async fn remove_async_path(path: &Path) -> Result<(), InstallError> {
    let metadata = match fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(install_fs_error("inspect old destination", path, error)),
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .await
            .map_err(|error| install_fs_error("remove old directory", path, error))
    } else {
        fs::remove_file(path)
            .await
            .map_err(|error| install_fs_error("remove old file", path, error))
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), InstallError> {
    if cancellation.is_cancelled() {
        Err(InstallError::Cancelled)
    } else {
        Ok(())
    }
}

fn install_fs_error(operation: &'static str, path: &Path, error: io::Error) -> InstallError {
    InstallError::FileSystem {
        operation,
        path: path.to_owned(),
        message: error.to_string(),
    }
}

fn validate_sha1(hash: &str) -> Result<(), ()> {
    if hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(())
    }
}

fn validate_single_component(value: &str) -> Result<PathBuf, String> {
    let path = validate_protocol_path(value)?;
    if path.components().count() == 1 {
        Ok(path)
    } else {
        Err("must contain exactly one path component".to_owned())
    }
}

fn validate_protocol_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() {
        return Err("path is empty".to_owned());
    }
    if raw.contains('\0') {
        return Err("path contains a NUL byte".to_owned());
    }
    if raw.contains('\\') {
        return Err("backslash path aliases are not accepted".to_owned());
    }
    if raw.contains(':') {
        return Err("platform path prefixes are not accepted".to_owned());
    }
    let path = Path::new(raw);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(path.to_owned())
    } else {
        Err("path must contain only normal relative components".to_owned())
    }
}
