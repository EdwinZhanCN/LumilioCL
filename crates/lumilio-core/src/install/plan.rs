use super::assets::AssetIndex;
use super::files::{validate_protocol_path, validate_single_component};
use super::installer::InstallError;
use crate::launch::{LaunchDirectories, LaunchPlan};
use crate::release::{DownloadDescriptor, ReleaseManifest};
use crate::transfer::{SourceChain, TransferRequest};
use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Formatter;
use std::path::{Path, PathBuf};

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
    pub(super) const fn identifier(self) -> &'static str {
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
    pub(super) kind: ArtifactKind,
    pub(super) request: TransferRequest,
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
    pub(super) archive: PathBuf,
    pub(super) exclusions: Vec<String>,
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
    pub(super) release_id: String,
    pub(super) manifest_json: String,
    pub(super) manifest_destination: PathBuf,
    pub(super) directories: LaunchDirectories,
    pub(super) initial_artifacts: Vec<PlannedArtifact>,
    pub(super) native_bundles: Vec<NativeBundle>,
    pub(super) asset_catalog_id: Option<String>,
    pub(super) sources: SourceChain,
    pub(super) shared_cache: Option<PathBuf>,
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

pub(super) fn request_from_descriptor(
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

pub(super) fn cache_candidate_for(
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
