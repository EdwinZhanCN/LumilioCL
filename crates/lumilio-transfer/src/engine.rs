use super::files::{AttemptFailure, copy_verified_candidate, verify_file};
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

/// Downloads files under a [`ConnectionBudget`]. Engines made with one
/// budget share its connections; each reports only its own transfers.
pub struct TransferEngine<T> {
    pub(super) transport: Arc<T>,
    pub(super) budget: ConnectionBudget,
    pub(super) events: broadcast::Sender<TransferEvent>,
    /// Where notes on downloads kept part-way live; `None` keeps the bytes
    /// only between the attempts of one transfer.
    pub(super) partials: Option<Partials>,
}

impl<T> Clone for TransferEngine<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            budget: self.budget.clone(),
            events: self.events.clone(),
            partials: self.partials.clone(),
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
        let (events, _) = broadcast::channel(512);
        Self {
            transport: Arc::new(transport),
            budget,
            events,
            partials: None,
        }
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
        self.events.subscribe()
    }

    pub async fn transfer(
        &self,
        request: TransferRequest,
        cancellation: CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        let id = request.id.clone();
        let _ = self.events.send(TransferEvent::Started {
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

        let permit = tokio::select! {
            () = cancellation.cancelled() => return self.finish_error(&id, TransferError::Cancelled),
            permit = self.budget.acquire() => permit.map_err(|error| {
                TransferError::InvalidRequest(format!("transfer scheduler is closed: {error}"))
            })?,
        };
        let result = self.transfer_with_permit(&request, &cancellation).await;
        drop(permit);

        match result {
            Ok(outcome) => {
                let _ = self.events.send(TransferEvent::Completed {
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
        let _ = self.events.send(TransferEvent::Failed {
            id: id.to_owned(),
            message: error.to_string(),
        });
        Err(error)
    }

    pub(super) async fn transfer_with_permit(
        &self,
        request: &TransferRequest,
        cancellation: &CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        if cancellation.is_cancelled() {
            return Err(TransferError::Cancelled);
        }
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
            .download_from_sources(request, &part, &mut note, cancellation)
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
        cancellation: &CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        let mut failures = Vec::new();
        let mut total_attempts = 0;
        for source in &request.sources {
            for attempt in 1..=request.retry_policy.attempts_per_source {
                if cancellation.is_cancelled() {
                    return Err(TransferError::Cancelled);
                }
                total_attempts += 1;
                match self
                    .download_once(request, source, part, note, cancellation)
                    .await
                {
                    Ok(bytes) => {
                        return Ok(TransferOutcome::Downloaded {
                            source: source.clone(),
                            attempts: total_attempts,
                            bytes,
                        });
                    }
                    Err(AttemptFailure::Cancelled) => return Err(TransferError::Cancelled),
                    Err(AttemptFailure::Fatal(error)) => return Err(error),
                    Err(AttemptFailure::Source { reason, retryable }) => {
                        failures.push(SourceFailure {
                            source: source.clone(),
                            attempt,
                            reason,
                            retryable,
                        });
                        if !retryable || attempt == request.retry_policy.attempts_per_source {
                            break;
                        }
                        let next_attempt = attempt + 1;
                        let _ = self.events.send(TransferEvent::Retrying {
                            id: request.id.clone(),
                            source: source.clone(),
                            next_attempt,
                        });
                        let delay = request.retry_policy.delay_before_attempt(next_attempt);
                        if !delay.is_zero() {
                            tokio::select! {
                                () = cancellation.cancelled() => return Err(TransferError::Cancelled),
                                () = tokio::time::sleep(delay) => {}
                            }
                        }
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
