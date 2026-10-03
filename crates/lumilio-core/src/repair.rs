use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;

use crate::activity::CancellationToken;
use crate::install::{
    ArtifactKind, AssetIndex, InstallError, InstallationPlan, NativeBundle, NativeError,
    NativePublisher, PlannedArtifact,
};
use crate::transfer::{TransferBatchReport, TransferEngine, TransferError, Transport};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntegrityIssue {
    Missing,
    NotRegularFile,
    WrongSize { expected: u64, actual: u64 },
    WrongChecksum { expected: String, actual: String },
    MalformedAssetCatalog(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairFinding {
    id: String,
    kind: ArtifactKind,
    path: PathBuf,
    issue: IntegrityIssue,
}

impl RepairFinding {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn issue(&self) -> &IntegrityIssue {
        &self.issue
    }
}

#[derive(Clone, Debug, Default)]
pub struct RepairPlan {
    findings: Vec<RepairFinding>,
    artifacts: Vec<PlannedArtifact>,
    native_bundles: Vec<NativeBundle>,
    native_destination: Option<PathBuf>,
}

impl RepairPlan {
    #[must_use]
    pub fn findings(&self) -> &[RepairFinding] {
        &self.findings
    }

    #[must_use]
    pub fn artifacts(&self) -> &[PlannedArtifact] {
        &self.artifacts
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    #[must_use]
    pub fn republishes_natives(&self) -> bool {
        self.native_destination.is_some()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InstallationVerifier;

impl InstallationVerifier {
    pub async fn scan(
        plan: &InstallationPlan,
        supplied_index: Option<&AssetIndex>,
    ) -> Result<RepairPlan, RepairError> {
        let mut repair = RepairPlan::default();
        for artifact in plan.initial_artifacts() {
            if let Some(issue) = inspect_artifact(artifact).await? {
                push_repair(&mut repair, artifact, issue);
            }
        }

        let decoded_index;
        let index = if let Some(index) = supplied_index {
            Some(index)
        } else if let Some(catalog) = plan.asset_catalog_artifact() {
            let catalog_already_invalid = repair
                .artifacts
                .iter()
                .any(|artifact| artifact.request().id() == catalog.request().id());
            if catalog_already_invalid {
                None
            } else {
                let json = fs::read_to_string(catalog.request().destination())
                    .await
                    .map_err(|error| RepairError::FileSystem {
                        path: catalog.request().destination().to_owned(),
                        message: error.to_string(),
                    })?;
                match AssetIndex::decode_json(&json) {
                    Ok(index) => {
                        decoded_index = index;
                        Some(&decoded_index)
                    }
                    Err(error) => {
                        push_repair(
                            &mut repair,
                            catalog,
                            IntegrityIssue::MalformedAssetCatalog(error.to_string()),
                        );
                        None
                    }
                }
            }
        } else {
            None
        };

        if let Some(index) = index {
            for artifact in plan.asset_artifacts(index)? {
                if let Some(issue) = inspect_artifact(&artifact).await? {
                    push_repair(&mut repair, &artifact, issue);
                }
            }
        }
        if repair
            .artifacts
            .iter()
            .any(|artifact| artifact.kind() == ArtifactKind::NativeArchive)
        {
            repair.native_bundles = plan.native_bundles().to_vec();
            repair.native_destination = Some(plan.native_destination().to_owned());
        }
        Ok(repair)
    }
}

pub struct RepairExecutor<T> {
    transfers: TransferEngine<T>,
    native_publisher: NativePublisher,
}

impl<T> RepairExecutor<T>
where
    T: Transport,
{
    #[must_use]
    pub fn new(transfers: TransferEngine<T>) -> Self {
        Self {
            transfers,
            native_publisher: NativePublisher::default(),
        }
    }

    #[must_use]
    pub fn with_native_publisher(mut self, publisher: NativePublisher) -> Self {
        self.native_publisher = publisher;
        self
    }

    pub async fn execute(
        &self,
        plan: &RepairPlan,
        cancellation: CancellationToken,
    ) -> Result<RepairReport, RepairError> {
        let transfers = self
            .transfers
            .transfer_batch(
                plan.artifacts
                    .iter()
                    .map(|artifact| artifact.request().clone())
                    .collect(),
                cancellation.clone(),
            )
            .await;
        if transfers.failed() > 0 {
            let failures = transfers
                .results()
                .iter()
                .filter_map(|(id, result)| {
                    result
                        .as_ref()
                        .err()
                        .map(|error| (id.clone(), error.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            if failures
                .values()
                .any(|error| matches!(error, TransferError::Cancelled))
            {
                return Err(RepairError::Cancelled);
            }
            return Err(RepairError::Transfers(failures));
        }
        let native_files = match &plan.native_destination {
            Some(destination) => self
                .native_publisher
                .publish(&plan.native_bundles, destination, cancellation)
                .await?
                .published_files(),
            None => 0,
        };
        Ok(RepairReport {
            transfers,
            native_files,
        })
    }
}

#[derive(Clone, Debug)]
pub struct RepairReport {
    transfers: TransferBatchReport,
    native_files: usize,
}

impl RepairReport {
    #[must_use]
    pub const fn transfers(&self) -> &TransferBatchReport {
        &self.transfers
    }

    #[must_use]
    pub const fn native_files(&self) -> usize {
        self.native_files
    }
}

fn push_repair(repair: &mut RepairPlan, artifact: &PlannedArtifact, issue: IntegrityIssue) {
    if repair
        .artifacts
        .iter()
        .any(|existing| existing.request().id() == artifact.request().id())
    {
        return;
    }
    repair.findings.push(RepairFinding {
        id: artifact.request().id().to_owned(),
        kind: artifact.kind(),
        path: artifact.request().destination().to_owned(),
        issue,
    });
    repair.artifacts.push(artifact.clone());
}

async fn inspect_artifact(
    artifact: &PlannedArtifact,
) -> Result<Option<IntegrityIssue>, RepairError> {
    let request = artifact.request();
    let metadata = match fs::metadata(request.destination()).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(IntegrityIssue::Missing));
        }
        Err(error) => {
            return Err(RepairError::FileSystem {
                path: request.destination().to_owned(),
                message: error.to_string(),
            });
        }
    };
    if !metadata.is_file() {
        return Ok(Some(IntegrityIssue::NotRegularFile));
    }
    if let Some(expected) = request.expected_size()
        && metadata.len() != expected
    {
        return Ok(Some(IntegrityIssue::WrongSize {
            expected,
            actual: metadata.len(),
        }));
    }
    if let Some(expected) = request.expected_sha1() {
        let actual = sha1_file(request.destination()).await?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Ok(Some(IntegrityIssue::WrongChecksum {
                expected: expected.to_owned(),
                actual,
            }));
        }
    }
    Ok(None)
}

async fn sha1_file(path: &Path) -> Result<String, RepairError> {
    let mut file = File::open(path)
        .await
        .map_err(|error| RepairError::FileSystem {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    let mut digest = Sha1::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| RepairError::FileSystem {
                path: path.to_owned(),
                message: error.to_string(),
            })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[derive(Debug)]
pub enum RepairError {
    Installation(InstallError),
    Native(NativeError),
    Transfers(BTreeMap<String, TransferError>),
    Cancelled,
    FileSystem { path: PathBuf, message: String },
}

impl Display for RepairError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Installation(error) => Display::fmt(error, formatter),
            Self::Native(error) => Display::fmt(error, formatter),
            Self::Transfers(failures) => {
                write!(formatter, "{} repair transfers failed", failures.len())
            }
            Self::Cancelled => formatter.write_str("repair was cancelled"),
            Self::FileSystem { path, message } => {
                write!(formatter, "cannot inspect {}: {message}", path.display())
            }
        }
    }
}

impl Error for RepairError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Installation(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Transfers(_) | Self::Cancelled => None,
            Self::FileSystem { .. } => None,
        }
    }
}

impl From<InstallError> for RepairError {
    fn from(error: InstallError) -> Self {
        Self::Installation(error)
    }
}

impl From<NativeError> for RepairError {
    fn from(error: NativeError) -> Self {
        Self::Native(error)
    }
}
