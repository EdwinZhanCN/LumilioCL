use super::assets::{AssetIndex, AssetIndexError};
use super::files::{atomic_write, ensure_not_cancelled, publish_asset_views};
use super::native::{NativeError, NativePublisher};
use super::plan::InstallationPlan;
use crate::activity::CancellationToken;
use crate::transfer::{TransferBatchReport, TransferEngine, TransferError, Transport};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::broadcast;

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
    pub(super) transfers: TransferEngine<T>,
    pub(super) native_publisher: NativePublisher,
    pub(super) events: broadcast::Sender<InstallEvent>,
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

    pub(super) fn begin(&self, stage: InstallStage) {
        let _ = self.events.send(InstallEvent::StageStarted(stage));
    }

    pub(super) fn complete(&self, stage: InstallStage, items: usize) {
        let _ = self
            .events
            .send(InstallEvent::StageCompleted { stage, items });
    }

    pub(super) fn require_batch(
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

    pub(super) fn record_failure(&self, stage: InstallStage, error: InstallError) -> InstallError {
        let _ = self.events.send(InstallEvent::StageFailed {
            stage,
            message: error.to_string(),
        });
        error
    }
}

#[derive(Clone, Debug)]
pub struct InstallReport {
    pub(super) initial: TransferBatchReport,
    pub(super) assets: TransferBatchReport,
    pub(super) asset_objects: usize,
    pub(super) asset_views: usize,
    pub(super) native_files: usize,
    pub(super) manifest: PathBuf,
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
