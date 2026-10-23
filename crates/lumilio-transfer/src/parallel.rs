//! A download over several connections (plan download-engine T3). The
//! attempt's own connection keeps reading its answer; free connections of
//! the budget join it, each asking the address that answer came from for the
//! back half of the largest range left. A joining connection lets go as soon
//! as another download waits for one, so a batch is never slowed by a large
//! file. Any failure fails the attempt as a whole, and what was written is
//! kept for the next one.

use super::attempt::{content_range, hash_prefix, status_failure, verify_digest};
use super::budget::{Connection, ConnectionBudget};
use super::engine::{Events, TransferEngine};
use super::files::{AttemptFailure, ContentHasher, file_system_error, publish_temporary};
use super::partial::{Note, Written};
use super::pieces::Pieces;
use super::request::{TransferEvent, TransferRequest};
use super::transport::{ByteStream, Transport};
use crate::CancellationToken;
use futures_util::stream::StreamExt;
use std::io::SeekFrom;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinSet;

/// The most connections one file downloads over at once, its own included.
pub const CONNECTIONS_PER_FILE: usize = 8;

/// When a free connection may join a file, and how finely it may be split.
#[derive(Clone, Copy, Debug)]
pub(super) struct Split {
    /// Files with less than this left use one connection.
    pub(super) threshold: u64,
    /// No connection is given less than this to write.
    pub(super) smallest: u64,
    /// Connections one file may use at once, its own included.
    pub(super) connections: usize,
}

impl Default for Split {
    fn default() -> Self {
        Self {
            threshold: 8 * 1024 * 1024,
            smallest: 1024 * 1024,
            connections: CONNECTIONS_PER_FILE,
        }
    }
}

/// How often the attempt looks for free connections to join.
pub(super) const JOIN_INTERVAL: Duration = Duration::from_millis(250);

/// What an attempt continues from, as its first answer showed.
pub(super) struct Plan {
    /// Ranges of the part file already written.
    pub(super) written: Vec<Range<u64>>,
    /// The first byte of the first answer.
    pub(super) start: u64,
    pub(super) length: u64,
    pub(super) validator: Option<String>,
}

/// What the connections of one attempt share.
struct Shared<T> {
    transport: Arc<T>,
    /// The first answer's address after redirects: every range of a file
    /// comes from one server.
    url: String,
    validator: Option<String>,
    part: PathBuf,
    length: u64,
    pieces: Mutex<Pieces>,
    stop: CancellationToken,
    /// Set once the source answers a range with the whole file: no more
    /// connections join.
    alone: AtomicBool,
    budget: ConnectionBudget,
    events: Events,
    id: String,
}

impl<T> Shared<T> {
    fn pieces(&self) -> MutexGuard<'_, Pieces> {
        self.pieces.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<T> TransferEngine<T>
where
    T: Transport,
{
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn download_pieces(
        &self,
        request: &TransferRequest,
        source: &str,
        part: &Path,
        note: &mut Option<Note>,
        (url, first): (String, ByteStream),
        plan: Plan,
        helper: Option<Connection>,
        cancellation: &CancellationToken,
    ) -> Result<u64, AttemptFailure> {
        allocate(part, plan.length).await.map_err(|error| {
            AttemptFailure::Fatal(file_system_error("open partial download", part, error))
        })?;
        let mut kept = Note {
            part: part.to_owned(),
            source: source.to_owned(),
            validator: plan.validator.clone(),
            digest: request.expected_digest.clone(),
            written: Some(Written {
                length: plan.length,
                ranges: plan.written.clone(),
            }),
        };
        if let Some(partials) = &self.partials {
            partials.write(&request.destination, &kept).await;
        }
        *note = Some(kept.clone());

        let mut pieces = Pieces::new(plan.length, plan.written, self.split.smallest);
        let mut next_worker = 0;
        // The first answer covers the first gap; claimed before any other
        // connection can take it.
        if pieces.claim(next_worker, plan.start).is_none() {
            *note = None;
            return Err(AttemptFailure::source(format!(
                "nothing is missing at byte {}",
                plan.start
            )));
        }
        let shared = Arc::new(Shared {
            transport: self.transport.clone(),
            url,
            validator: plan.validator,
            part: part.to_owned(),
            length: plan.length,
            pieces: Mutex::new(pieces),
            stop: CancellationToken::new(),
            alone: AtomicBool::new(false),
            budget: self.budget.clone(),
            events: self.events.clone(),
            id: request.id.clone(),
        });
        let mut workers = JoinSet::new();
        let mut own = Some(next_worker);
        workers.spawn(work(
            shared.clone(),
            next_worker,
            Some((first, plan.start)),
            None,
        ));
        if let Some(helper) = helper {
            next_worker += 1;
            workers.spawn(work(shared.clone(), next_worker, None, Some(helper)));
        }
        let mut connections = 0;
        let mut failure = None;
        let mut tick = tokio::time::interval(JOIN_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if connections != workers.len() {
                connections = workers.len();
                self.events.send(TransferEvent::Connections {
                    id: request.id.clone(),
                    count: connections,
                });
            }
            tokio::select! {
                biased;
                () = cancellation.paused(), if !shared.stop.is_cancelled() => shared.stop.cancel(),
                joined = workers.join_next() => match joined {
                    Some(Ok((worker, result))) => {
                        if own == Some(worker) {
                            own = None;
                        }
                        if let Err(error) = result {
                            failure.get_or_insert(error);
                            shared.stop.cancel();
                        }
                    }
                    Some(Err(error)) => {
                        failure.get_or_insert(AttemptFailure::source(format!(
                            "download connection failed: {error}"
                        )));
                        shared.stop.cancel();
                    }
                    None => {}
                },
                _ = tick.tick() => {
                    while !shared.stop.is_cancelled()
                        && !shared.alone.load(Ordering::Relaxed)
                        && workers.len() < self.split.connections
                        && shared.pieces().has_work()
                        && let Some(helper) = self.budget.try_acquire()
                    {
                        next_worker += 1;
                        workers.spawn(work(shared.clone(), next_worker, None, Some(helper)));
                    }
                }
            }
            // Ranges a joining connection let go of fall to the attempt's own.
            if own.is_none() && !shared.stop.is_cancelled() && shared.pieces().has_waiting() {
                next_worker += 1;
                own = Some(next_worker);
                workers.spawn(work(shared.clone(), next_worker, None, None));
            }
            if workers.is_empty() {
                break;
            }
        }

        let (ranges, complete) = {
            let pieces = shared.pieces();
            (pieces.written().to_vec(), pieces.is_complete())
        };
        if let Some(written) = &mut kept.written {
            written.ranges = ranges;
        }
        if !complete {
            if let Some(partials) = &self.partials {
                partials.write(&request.destination, &kept).await;
            }
            *note = Some(kept);
            if let Some(failure) = failure {
                return Err(failure);
            }
            return Err(if cancellation.is_cancelled() {
                AttemptFailure::Cancelled
            } else if cancellation.is_paused() {
                AttemptFailure::Paused
            } else {
                AttemptFailure::source("connections ended before the file was complete".to_owned())
            });
        }
        if let Err(reason) = self.verify_part(request, part, shared.length).await? {
            *note = None;
            return Err(AttemptFailure::source(reason));
        }
        publish_temporary(part, &request.destination)
            .await
            .map_err(AttemptFailure::Fatal)?;
        *note = None;
        Ok(shared.length)
    }

    /// Whether a part written over several connections is the file asked
    /// for; it is hashed whole, since its ranges arrived out of order.
    async fn verify_part(
        &self,
        request: &TransferRequest,
        part: &Path,
        length: u64,
    ) -> Result<Result<(), String>, AttemptFailure> {
        let file = File::open(part).await.map_err(|error| {
            AttemptFailure::Fatal(file_system_error("open partial download", part, error))
        })?;
        file.sync_all().await.map_err(|error| {
            AttemptFailure::Fatal(file_system_error("sync partial download", part, error))
        })?;
        drop(file);
        if let Some(expected) = request.expected_size
            && expected != length
        {
            return Ok(Err(format!("file size was {length}, expected {expected}")));
        }
        let mut digest = request
            .expected_digest
            .as_ref()
            .map(ContentHasher::for_digest);
        if let Some(digest) = &mut digest {
            hash_prefix(part, length, digest).await.map_err(|error| {
                AttemptFailure::Fatal(file_system_error("read partial download", part, error))
            })?;
        }
        Ok(verify_digest(request, digest))
    }
}

/// The part file, as long as the whole content.
async fn allocate(part: &Path, length: u64) -> std::io::Result<()> {
    if let Some(parent) = part.parent() {
        fs::create_dir_all(parent).await?;
    }
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(part)
        .await?;
    file.set_len(length).await?;
    file.sync_all().await
}

/// One connection's work: its first answer, or ranges it takes, until none
/// is left, it must stop, or (when it borrowed `helper`) another download
/// waits for a connection.
async fn work<T: Transport>(
    shared: Arc<Shared<T>>,
    worker: usize,
    first: Option<(ByteStream, u64)>,
    helper: Option<Connection>,
) -> (usize, Result<(), AttemptFailure>) {
    let result = match OpenOptions::new().write(true).open(&shared.part).await {
        Ok(mut file) => {
            let written = write_ranges(&shared, worker, first, helper.is_some(), &mut file).await;
            let flushed = file.flush().await.map_err(|error| {
                AttemptFailure::Fatal(file_system_error(
                    "flush partial download",
                    &shared.part,
                    error,
                ))
            });
            written.and(flushed)
        }
        Err(error) => Err(AttemptFailure::Fatal(file_system_error(
            "open partial download",
            &shared.part,
            error,
        ))),
    };
    shared.pieces().release(worker);
    drop(helper);
    (worker, result)
}

async fn write_ranges<T: Transport>(
    shared: &Shared<T>,
    worker: usize,
    mut first: Option<(ByteStream, u64)>,
    borrowed: bool,
    file: &mut File,
) -> Result<(), AttemptFailure> {
    loop {
        if shared.stop.is_cancelled() || (borrowed && shared.budget.waiting() > 0) {
            return Ok(());
        }
        // Bytes of the answer before the range, when it came whole.
        let mut skip = 0;
        let (body, start) = match first.take() {
            // Its range was claimed when the attempt began.
            Some((body, start)) => (body, start),
            None => {
                let Some(range) = shared.pieces().take(worker) else {
                    return Ok(());
                };
                // A borrowed connection asks for its range alone; the
                // attempt's own asks to the end, so it can read on into a
                // range let go of behind it.
                let end = borrowed.then_some(range.end);
                let response = tokio::select! {
                    () = shared.stop.cancelled() => return Ok(()),
                    response = shared.transport.get_range(
                        &shared.url,
                        range.start,
                        end,
                        shared.validator.as_deref(),
                    ) => response.map_err(|error| AttemptFailure::Source {
                        reason: error.message,
                        retryable: error.retryable,
                    })?,
                };
                let status = response.status();
                if !(200..300).contains(&status) {
                    return Err(status_failure(status));
                }
                let fits = status == 206
                    && content_range(&response).is_some_and(|(at, length)| {
                        at == range.start && length.is_none_or(|length| length == shared.length)
                    });
                if !fits {
                    let whole = status == 200
                        && response
                            .content_length()
                            .is_none_or(|length| length == shared.length);
                    if !whole {
                        return Err(AttemptFailure::source(format!(
                            "asked for bytes from {}, received {:?}",
                            range.start,
                            response.header("content-range")
                        )));
                    }
                    // The source will not split this file after all: no more
                    // connections join, and the attempt's own reads on from
                    // the whole answer.
                    shared.alone.store(true, Ordering::Relaxed);
                    if borrowed {
                        return Ok(());
                    }
                    skip = range.start;
                }
                (response.into_body(), range.start)
            }
        };
        file.seek(SeekFrom::Start(start)).await.map_err(|error| {
            AttemptFailure::Fatal(file_system_error(
                "seek partial download",
                &shared.part,
                error,
            ))
        })?;
        stream_share(shared, worker, body, borrowed, skip, file).await?;
        shared.pieces().release(worker);
    }
}

/// Writes `body`, after its first `skip` bytes, into the worker's share
/// until the share is done. The attempt's own connection asked to the end of
/// the file, so it reads on into a range let go of right behind its share.
async fn stream_share<T>(
    shared: &Shared<T>,
    worker: usize,
    mut body: ByteStream,
    borrowed: bool,
    mut skip: u64,
    file: &mut File,
) -> Result<(), AttemptFailure> {
    let open_ended = !borrowed;
    loop {
        if borrowed && shared.budget.waiting() > 0 {
            return Ok(());
        }
        let chunk = tokio::select! {
            () = shared.stop.cancelled() => return Ok(()),
            chunk = body.next() => chunk,
        };
        let chunk = match chunk {
            None => {
                return Err(AttemptFailure::source(
                    "the answer ended before its range".to_owned(),
                ));
            }
            Some(Err(error)) => {
                return Err(AttemptFailure::Source {
                    reason: error.message,
                    retryable: error.retryable,
                });
            }
            Some(Ok(chunk)) => chunk,
        };
        let skipped = usize::try_from(skip).map_or(chunk.len(), |skip| skip.min(chunk.len()));
        skip -= skipped as u64;
        let mut used = skipped;
        while used < chunk.len() {
            let (at, fits) = {
                let mut pieces = shared.pieces();
                let mut reserved = pieces.reserve(worker, chunk.len() - used);
                if reserved.1 == 0 && open_ended && pieces.extend(worker) {
                    reserved = pieces.reserve(worker, chunk.len() - used);
                }
                reserved
            };
            if fits == 0 {
                return Ok(());
            }
            file.write_all(&chunk[used..used + fits])
                .await
                .map_err(|error| {
                    AttemptFailure::Fatal(file_system_error(
                        "write partial download",
                        &shared.part,
                        error,
                    ))
                })?;
            let written = shared.pieces().commit(at..at + fits as u64);
            shared.budget.count(fits);
            shared.events.send(TransferEvent::Progress {
                id: shared.id.clone(),
                completed: written,
                total: Some(shared.length),
            });
            used += fits;
        }
        let done = {
            let mut pieces = shared.pieces();
            pieces.left(worker) == 0 && !(open_ended && pieces.extend(worker))
        };
        if done {
            return Ok(());
        }
    }
}
