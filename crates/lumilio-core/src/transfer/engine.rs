use super::files::{
    AttemptFailure, copy_verified_candidate, create_temporary_sibling, encode_lower_hex,
    file_system_error, publish_temporary, verify_file,
};
use super::request::{
    SourceFailure, TransferError, TransferEvent, TransferOutcome, TransferRequest,
};
use super::transport::Transport;
use crate::activity::CancellationToken;
use futures_util::stream::StreamExt;
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Semaphore, broadcast};
use tokio::task::JoinSet;

pub struct TransferEngine<T> {
    pub(super) transport: Arc<T>,
    pub(super) concurrency: Arc<Semaphore>,
    pub(super) events: broadcast::Sender<TransferEvent>,
}

impl<T> Clone for TransferEngine<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            concurrency: self.concurrency.clone(),
            events: self.events.clone(),
        }
    }
}

impl<T> TransferEngine<T>
where
    T: Transport,
{
    pub fn new(transport: T, concurrency: usize) -> Result<Self, TransferError> {
        if concurrency == 0 {
            return Err(TransferError::InvalidRequest(
                "transfer concurrency must be positive".to_owned(),
            ));
        }
        let (events, _) = broadcast::channel(512);
        Ok(Self {
            transport: Arc::new(transport),
            concurrency: Arc::new(Semaphore::new(concurrency)),
            events,
        })
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
            permit = self.concurrency.acquire() => permit.map_err(|error| {
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
                request.expected_sha1.as_deref(),
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
                request.expected_sha1.as_deref(),
            )
            .await
            .unwrap_or(false);
            if candidate_is_valid && copy_verified_candidate(candidate, request).await.is_ok() {
                return Ok(TransferOutcome::ReusedCache {
                    source: candidate.clone(),
                });
            }
        }

        let mut failures = Vec::new();
        let mut total_attempts = 0;
        for source in &request.sources {
            for attempt in 1..=request.retry_policy.attempts_per_source {
                if cancellation.is_cancelled() {
                    return Err(TransferError::Cancelled);
                }
                total_attempts += 1;
                match self.download_once(request, source, cancellation).await {
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

    pub(super) async fn download_once(
        &self,
        request: &TransferRequest,
        source: &str,
        cancellation: &CancellationToken,
    ) -> Result<u64, AttemptFailure> {
        let response = tokio::select! {
            () = cancellation.cancelled() => return Err(AttemptFailure::Cancelled),
            response = self.transport.get(source) => response.map_err(|error| AttemptFailure::Source {
                reason: error.message,
                retryable: error.retryable,
            })?,
        };
        let status = response.status();
        if !(200..300).contains(&status) {
            let retryable = status == 408 || status == 429 || status >= 500;
            return Err(AttemptFailure::Source {
                reason: format!("HTTP status {status}"),
                retryable,
            });
        }

        let response_length = response.content_length();
        let (temporary_path, mut temporary_file) = create_temporary_sibling(&request.destination)
            .await
            .map_err(AttemptFailure::Fatal)?;
        let result = async {
            let mut body = response.into_body();
            let mut bytes_written = 0_u64;
            let mut digest = Sha1::new();
            loop {
                let chunk = tokio::select! {
                    () = cancellation.cancelled() => {
                        return Err(AttemptFailure::Cancelled);
                    }
                    chunk = body.next() => chunk,
                };
                let Some(chunk) = chunk else {
                    break;
                };
                let chunk = chunk.map_err(|error| AttemptFailure::Source {
                    reason: error.message,
                    retryable: error.retryable,
                })?;
                temporary_file.write_all(&chunk).await.map_err(|error| {
                    AttemptFailure::Fatal(file_system_error(
                        "write temporary file",
                        &temporary_path,
                        error,
                    ))
                })?;
                digest.update(&chunk);
                bytes_written = bytes_written.saturating_add(chunk.len() as u64);
                let _ = self.events.send(TransferEvent::Progress {
                    id: request.id.clone(),
                    completed: bytes_written,
                    total: request
                        .expected_size
                        .or(response_length)
                        .filter(|total| bytes_written <= *total),
                });
            }

            if let Some(response_length) = response_length
                && bytes_written != response_length
            {
                return Err(AttemptFailure::source(format!(
                    "response length was {bytes_written}, expected {response_length}"
                )));
            }
            if let Some(expected_size) = request.expected_size
                && bytes_written != expected_size
            {
                return Err(AttemptFailure::source(format!(
                    "file size was {bytes_written}, expected {expected_size}"
                )));
            }
            if let Some(expected) = &request.expected_sha1 {
                let actual = encode_lower_hex(&digest.finalize());
                if !actual.eq_ignore_ascii_case(expected) {
                    return Err(AttemptFailure::source(format!(
                        "SHA-1 was {actual}, expected {expected}"
                    )));
                }
            }

            temporary_file.flush().await.map_err(|error| {
                AttemptFailure::Fatal(file_system_error(
                    "flush temporary file",
                    &temporary_path,
                    error,
                ))
            })?;
            temporary_file.sync_all().await.map_err(|error| {
                AttemptFailure::Fatal(file_system_error(
                    "sync temporary file",
                    &temporary_path,
                    error,
                ))
            })?;
            Ok(bytes_written)
        }
        .await;

        drop(temporary_file);
        match result {
            Ok(bytes_written) => {
                if let Err(error) = publish_temporary(&temporary_path, &request.destination).await {
                    let _ = fs::remove_file(&temporary_path).await;
                    Err(AttemptFailure::Fatal(error))
                } else {
                    Ok(bytes_written)
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path).await;
                Err(error)
            }
        }
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
        let mut join_set = JoinSet::new();
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
            let id = request.id.clone();
            let engine = self.clone();
            let cancellation = cancellation.clone();
            join_set.spawn(async move { (id, engine.transfer(request, cancellation).await) });
        }

        while let Some(joined) = join_set.join_next().await {
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
