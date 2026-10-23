use super::budget::Connection;
use super::files::{AttemptFailure, copy_verified_candidate, verify_file};
use super::parallel::Split;
use super::partial::{Note, Partials, part_path};
use super::request::{
    SourceFailure, TransferError, TransferEvent, TransferOutcome, TransferRequest,
};
use super::transport::Transport;
use crate::CancellationToken;
use crate::budget::ConnectionBudget;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::broadcast;
use tokio::task::JoinSet;

/// Where an engine's events go: its own subscribers, and whoever watches
/// the work it belongs to.
#[derive(Clone, Debug)]
pub(super) struct Events {
    own: broadcast::Sender<TransferEvent>,
    watcher: Option<broadcast::Sender<TransferEvent>>,
}

impl Events {
    pub(super) fn send(&self, event: TransferEvent) {
        if let Some(watcher) = &self.watcher {
            let _ = watcher.send(event.clone());
        }
        let _ = self.own.send(event);
    }
}

/// Downloads files under a [`ConnectionBudget`]. Engines made with one
/// budget share its connections; each reports only its own transfers.
pub struct TransferEngine<T> {
    pub(super) transport: Arc<T>,
    pub(super) budget: ConnectionBudget,
    pub(super) events: Events,
    /// Where notes on downloads kept part-way live; `None` keeps the bytes
    /// only between the attempts of one transfer.
    pub(super) partials: Option<Partials>,
    pub(super) split: Split,
}

impl<T> Clone for TransferEngine<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            budget: self.budget.clone(),
            events: self.events.clone(),
            partials: self.partials.clone(),
            split: self.split,
        }
    }
}

impl<T> TransferEngine<T>
where
    T: Transport,
{
    /// An engine with a budget of its own.
    pub fn new(transport: T, concurrency: usize) -> Result<Self, TransferError> {
        Ok(Self::with_budget(
            transport,
            ConnectionBudget::new(concurrency)?,
        ))
    }

    #[must_use]
    pub fn with_budget(transport: T, budget: ConnectionBudget) -> Self {
        let (own, _) = broadcast::channel(512);
        Self {
            transport: Arc::new(transport),
            budget,
            events: Events { own, watcher: None },
            partials: None,
            split: Split::default(),
        }
    }

    /// The same engine, also reporting every event to `watcher`, so one
    /// watcher follows all the engines a piece of work makes. Its own
    /// subscribers still see only its own transfers.
    #[must_use]
    pub fn watched_by(mut self, watcher: broadcast::Sender<TransferEvent>) -> Self {
        self.events.watcher = Some(watcher);
        self
    }

    /// The connections this engine downloads over.
    #[must_use]
    pub const fn budget(&self) -> &ConnectionBudget {
        &self.budget
    }

    #[cfg(test)]
    pub(super) fn splitting(mut self, threshold: u64, smallest: u64, connections: usize) -> Self {
        self.split = Split {
            threshold,
            smallest,
            connections,
        };
        self
    }

    /// Keeps the bytes of a cancelled or failed download, noted in `folder`,
    /// so the next transfer to the same destination continues them. Clear
    /// old ones with [`crate::sweep_partials`].
    #[must_use]
    pub fn keeping_partials(mut self, folder: impl Into<PathBuf>) -> Self {
        self.partials = Some(Partials {
            folder: folder.into(),
        });
        self
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<TransferEvent> {
        self.events.own.subscribe()
    }

    pub async fn transfer(
        &self,
        request: TransferRequest,
        cancellation: CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        let id = request.id.clone();
        self.events.send(TransferEvent::Started {
            id: id.clone(),
            destination: request.destination.clone(),
        });

        let paths = [request.destination.clone()];
        let _resource = tokio::select! {
            biased;
            () = cancellation.cancelled() => return self.finish_error(&id, TransferError::Cancelled),
            acquired = crate::resource_lock::acquire(&paths) => match acquired {
                Ok(guard) => guard,
                Err(error) => return self.finish_error(&id, TransferError::FileSystem {
                    operation: "lock download destination", path: request.destination.clone(), message: error.to_string(),
                }),
            },
        };

        let mut connection = None;
        let result = self
            .transfer_locked(&request, &mut connection, &cancellation)
            .await;
        drop(connection);

        match result {
            Ok(outcome) => {
                self.events.send(TransferEvent::Completed {
                    id,
                    outcome: outcome.clone(),
                });
                Ok(outcome)
            }
            Err(error) => self.finish_error(&id, error),
        }
    }

    pub(super) fn finish_error<R>(
        &self,
        id: &str,
        error: TransferError,
    ) -> Result<R, TransferError> {
        self.events.send(TransferEvent::Failed {
            id: id.to_owned(),
            message: error.to_string(),
        });
        Err(error)
    }

    /// Holds a connection in `connection`, waiting for one unless it is
    /// held already. While paused it waits for the pause to end instead.
    pub(super) async fn connect(
        &self,
        connection: &mut Option<Connection>,
        cancellation: &CancellationToken,
    ) -> Result<(), TransferError> {
        loop {
            if cancellation.is_cancelled() {
                return Err(TransferError::Cancelled);
            }
            if cancellation.is_paused() {
                *connection = None;
                cancellation.resumed().await;
                continue;
            }
            if connection.is_some() {
                return Ok(());
            }
            tokio::select! {
                biased;
                () = cancellation.paused() => {}
                acquired = self.budget.acquire() => {
                    *connection = Some(acquired.map_err(|error| {
                        TransferError::InvalidRequest(format!("transfer scheduler is closed: {error}"))
                    })?);
                }
            }
        }
    }

    /// The transfer once its destination is locked.
    async fn transfer_locked(
        &self,
        request: &TransferRequest,
        connection: &mut Option<Connection>,
        cancellation: &CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        self.connect(connection, cancellation).await?;
        if request.reuse_existing
            && verify_file(
                &request.destination,
                request.expected_size,
                request.expected_digest.as_ref(),
            )
            .await?
        {
            return Ok(TransferOutcome::ReusedExisting);
        }

        for candidate in &request.cache_candidates {
            if cancellation.is_cancelled() {
                return Err(TransferError::Cancelled);
            }
            let candidate_is_valid = verify_file(
                candidate,
                request.expected_size,
                request.expected_digest.as_ref(),
            )
            .await
            .unwrap_or(false);
            if candidate_is_valid && copy_verified_candidate(candidate, request).await.is_ok() {
                return Ok(TransferOutcome::ReusedCache {
                    source: candidate.clone(),
                });
            }
        }

        let part = part_path(&request.destination);
        let mut note = match &self.partials {
            Some(partials) => partials.read(&request.destination).await,
            None => None,
        };
        let result = self
            .download_from_sources(request, &part, &mut note, connection, cancellation)
            .await;
        // Bytes a later attempt may continue are kept only where a note can
        // find them again; a failed write or publication keeps nothing.
        let keep = self.partials.is_some()
            && note.is_some()
            && matches!(
                result,
                Err(TransferError::Cancelled | TransferError::AllSourcesFailed { .. })
            );
        if !keep {
            if result.is_err() {
                let _ = fs::remove_file(&part).await;
            }
            if let Some(partials) = &self.partials {
                partials.forget(&request.destination).await;
            }
        }
        result
    }

    async fn download_from_sources(
        &self,
        request: &TransferRequest,
        part: &Path,
        note: &mut Option<Note>,
        connection: &mut Option<Connection>,
        cancellation: &CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        let mut failures = Vec::new();
        let mut total_attempts = 0;
        for (index, source) in request.sources.iter().enumerate() {
            let mut attempt = 1;
            while attempt <= request.retry_policy.attempts_per_source {
                self.connect(connection, cancellation).await?;
                self.events.send(TransferEvent::Downloading {
                    id: request.id.clone(),
                    source: source.clone(),
                    attempt,
                });
                let result = self
                    .download_once(request, source, part, note, cancellation)
                    .await;
                if result.is_err() {
                    // Between attempts, and while paused, it holds none.
                    self.events.send(TransferEvent::Connections {
                        id: request.id.clone(),
                        count: 0,
                    });
                }
                let failure = match result {
                    Ok(bytes) => {
                        return Ok(TransferOutcome::Downloaded {
                            source: source.clone(),
                            attempts: total_attempts + 1,
                            bytes,
                        });
                    }
                    Err(AttemptFailure::Cancelled) => return Err(TransferError::Cancelled),
                    // The same attempt again once resumed, from the bytes kept.
                    Err(AttemptFailure::Paused) => continue,
                    Err(AttemptFailure::Fatal(error)) => return Err(error),
                    Err(AttemptFailure::Source { reason, retryable }) => SourceFailure {
                        source: source.clone(),
                        attempt,
                        reason,
                        retryable,
                    },
                };
                total_attempts += 1;
                let last =
                    !failure.retryable || attempt == request.retry_policy.attempts_per_source;
                if last {
                    self.events.send(TransferEvent::SourceFailed {
                        id: request.id.clone(),
                        source: source.clone(),
                        reason: failure.reason.clone(),
                        next: request.sources.get(index + 1).cloned(),
                    });
                    failures.push(failure);
                    break;
                }
                failures.push(failure);
                attempt += 1;
                self.events.send(TransferEvent::Retrying {
                    id: request.id.clone(),
                    source: source.clone(),
                    next_attempt: attempt,
                });
                let delay = request.retry_policy.delay_before_attempt(attempt);
                if !delay.is_zero() {
                    tokio::select! {
                        biased;
                        () = cancellation.cancelled() => return Err(TransferError::Cancelled),
                        // Waiting out a pause is wait enough.
                        () = cancellation.paused() => {}
                        () = tokio::time::sleep(delay) => {}
                    }
                }
            }
        }

        Err(TransferError::AllSourcesFailed {
            id: request.id.clone(),
            failures,
        })
    }

    pub async fn transfer_batch(
        &self,
        requests: Vec<TransferRequest>,
        cancellation: CancellationToken,
    ) -> TransferBatchReport {
        let mut duplicate_ids = BTreeSet::new();
        let mut seen_ids = BTreeSet::new();
        for request in &requests {
            if !seen_ids.insert(request.id.clone()) {
                duplicate_ids.insert(request.id.clone());
            }
        }

        let mut results = BTreeMap::new();
        let mut pending = VecDeque::new();
        for request in requests {
            if duplicate_ids.contains(&request.id) {
                results.insert(
                    request.id.clone(),
                    Err(TransferError::InvalidRequest(format!(
                        "duplicate transfer id: {}",
                        request.id
                    ))),
                );
                continue;
            }
            pending.push_back(request);
        }

        // A batch queues no more transfers than the budget has connections,
        // so batches sharing a budget take turns instead of one waiting for
        // all of another's files.
        let mut join_set = JoinSet::new();
        loop {
            while join_set.len() < self.budget.size()
                && let Some(request) = pending.pop_front()
            {
                let id = request.id.clone();
                let engine = self.clone();
                let cancellation = cancellation.clone();
                join_set.spawn(async move { (id, engine.transfer(request, cancellation).await) });
            }
            let Some(joined) = join_set.join_next().await else {
                break;
            };
            match joined {
                Ok((id, result)) => {
                    results.insert(id, result);
                }
                Err(error) => {
                    results.insert(
                        format!("worker-{}", results.len()),
                        Err(TransferError::InvalidRequest(format!(
                            "transfer worker failed: {error}"
                        ))),
                    );
                }
            }
        }
        TransferBatchReport { results }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TransferBatchReport {
    pub(super) results: BTreeMap<String, Result<TransferOutcome, TransferError>>,
}

impl TransferBatchReport {
    #[must_use]
    pub fn results(&self) -> &BTreeMap<String, Result<TransferOutcome, TransferError>> {
        &self.results
    }

    #[must_use]
    pub fn result(&self, id: &str) -> Option<&Result<TransferOutcome, TransferError>> {
        self.results.get(id)
    }

    #[must_use]
    pub fn succeeded(&self) -> usize {
        self.results
            .values()
            .filter(|result| result.is_ok())
            .count()
    }

    #[must_use]
    pub fn failed(&self) -> usize {
        self.results
            .values()
            .filter(|result| result.is_err())
            .count()
    }
}
