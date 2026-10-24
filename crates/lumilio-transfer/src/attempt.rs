//! One attempt at one source: continue the bytes kept from an earlier
//! attempt when they may be trusted, else start over, and publish the file
//! once it is verified. A large file whose source serves ranges is written
//! over several connections when one is free to help ([`super::parallel`]).

use std::io::SeekFrom;
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use futures_util::stream::StreamExt;
use tokio::fs;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use super::engine::TransferEngine;
use super::files::{AttemptFailure, ContentHasher, file_system_error, publish_temporary};
use super::parallel::{JOIN_INTERVAL, Plan};
use super::partial::Note;
use super::pieces::gaps;
use super::request::{TransferEvent, TransferRequest};
use super::transport::{Transport, TransportResponse};
use crate::CancellationToken;

impl<T> TransferEngine<T>
where
    T: Transport,
{
    /// Downloads `source` into `part` and publishes it. `note` describes the
    /// bytes in `part`: it is read to continue them and replaced by what this
    /// attempt wrote, or cleared when they must not be continued.
    pub(super) async fn download_once(
        &self,
        request: &TransferRequest,
        source: &str,
        part: &Path,
        note: &mut Option<Note>,
        cancellation: &CancellationToken,
    ) -> Result<u64, AttemptFailure> {
        let kept = kept_bytes(request, source, part, note.as_ref()).await;
        let offset = kept.first_missing();
        let validator = note
            .as_ref()
            .filter(|note| note.source == source)
            .and_then(|note| note.validator.clone());
        let response = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(AttemptFailure::Cancelled),
            () = cancellation.paused() => return Err(AttemptFailure::Paused),
            response = async {
                if kept.ranges.is_empty() {
                    self.transport.get(source).await
                } else {
                    self.transport.get_range(source, offset, None, validator.as_deref()).await
                }
            } => response.map_err(|error| AttemptFailure::Source {
                reason: error.message,
                retryable: error.retryable,
            })?,
        };
        let status = response.status();
        if status == 416 {
            // The kept bytes do not fit what the source has now.
            *note = None;
            return Err(AttemptFailure::source(format!("HTTP status {status}")));
        }
        if !(200..300).contains(&status) {
            return Err(status_failure(status));
        }
        let continued = !kept.ranges.is_empty() && status == 206;
        if continued {
            let fits = content_range(&response).is_some_and(|(start, length)| {
                start == offset
                    && match (kept.length, length) {
                        (Some(kept), Some(length)) => kept == length,
                        _ => true,
                    }
            });
            if !fits {
                *note = None;
                return Err(AttemptFailure::source(format!(
                    "asked for bytes from {offset}, received {:?}",
                    response.header("content-range")
                )));
            }
        }
        let (written, start) = if continued {
            (kept.ranges, offset)
        } else {
            (Vec::new(), 0)
        };
        let validator = range_validator(&response).or(if continued { validator } else { None });

        let response_length = response.content_length();
        let length = request
            .expected_size
            .or(response_length.map(|length| start + length));
        let scattered = !(written.is_empty() || is_only(&written, 0..start));
        let serves_ranges = status == 206
            || response
                .header("accept-ranges")
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("bytes"));
        // The file's length, when its source would split it.
        let splittable = length.filter(|length| {
            serves_ranges
                && response_length.is_none_or(|response_length| start + response_length == *length)
        });
        if let Some(length) = splittable {
            let left = length
                - written
                    .iter()
                    .map(|range| range.end - range.start)
                    .sum::<u64>();
            let helper = if left >= self.split.threshold {
                self.budget.try_acquire()
            } else {
                None
            };
            if scattered || helper.is_some() {
                let plan = Plan {
                    written,
                    start,
                    length,
                    validator,
                };
                let url = response.url().unwrap_or(source).to_owned();
                return self
                    .download_pieces(
                        request,
                        source,
                        part,
                        note,
                        (url, response.into_body()),
                        plan,
                        helper,
                        cancellation,
                    )
                    .await;
            }
        } else if scattered {
            *note = None;
            return Err(AttemptFailure::source(
                "the source no longer answers with the rest of the file".to_owned(),
            ));
        }
        self.download_sequentially(
            request,
            source,
            part,
            note,
            Sequential {
                response,
                start,
                validator,
                splittable,
            },
            cancellation,
        )
        .await
    }

    /// Writes the answer into `part` after its first `start` bytes. When a
    /// connection comes free and enough of a splittable file is left, the
    /// answer is handed on to be read beside others.
    async fn download_sequentially(
        &self,
        request: &TransferRequest,
        source: &str,
        part: &Path,
        note: &mut Option<Note>,
        Sequential {
            response,
            start,
            validator,
            splittable,
        }: Sequential,
        cancellation: &CancellationToken,
    ) -> Result<u64, AttemptFailure> {
        let url = response.url().unwrap_or(source).to_owned();
        let response_length = response.content_length();
        let mut file = open_part(part, start).await.map_err(|error| {
            AttemptFailure::Fatal(file_system_error("open partial download", part, error))
        })?;
        let mut digest = request
            .expected_digest
            .as_ref()
            .map(ContentHasher::for_digest);
        if let Some(digest) = &mut digest {
            hash_prefix(part, start, digest).await.map_err(|error| {
                AttemptFailure::Fatal(file_system_error("read partial download", part, error))
            })?;
        }
        let kept = Note {
            part: part.to_owned(),
            source: source.to_owned(),
            validator,
            digest: request.expected_digest.clone(),
            written: None,
        };
        if let Some(partials) = &self.partials {
            partials.write(&request.destination, &kept).await;
        }
        *note = Some(kept);

        let total = request
            .expected_size
            .or(response_length.map(|length| start + length));
        let mut body = response.into_body();
        let mut received = 0_u64;
        let mut looked = Instant::now();
        let streamed = loop {
            let chunk = tokio::select! {
                biased;
                () = cancellation.cancelled() => break Err(AttemptFailure::Cancelled),
                () = cancellation.paused() => break Err(AttemptFailure::Paused),
                chunk = body.next() => chunk,
            };
            let Some(chunk) = chunk else {
                break Ok(None);
            };
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    break Err(AttemptFailure::Source {
                        reason: error.message,
                        retryable: error.retryable,
                    });
                }
            };
            if let Err(error) = file.write_all(&chunk).await {
                break Err(AttemptFailure::Fatal(file_system_error(
                    "write partial download",
                    part,
                    error,
                )));
            }
            if let Some(digest) = &mut digest {
                digest.update(&chunk);
            }
            self.budget.count(chunk.len());
            received = received.saturating_add(chunk.len() as u64);
            let completed = start + received;
            self.events.send(TransferEvent::Progress {
                id: request.id.clone(),
                completed,
                total: total.filter(|total| completed <= *total),
            });
            if let Some(length) = splittable
                && looked.elapsed() >= JOIN_INTERVAL
            {
                looked = Instant::now();
                if length.saturating_sub(completed) >= self.split.threshold
                    && let Some(helper) = self.budget.try_acquire()
                {
                    break Ok(Some((helper, length)));
                }
            }
        };
        // What was written stays a prefix the next attempt can measure.
        let flushed = file.flush().await;
        let handover = streamed?;
        flushed.map_err(|error| {
            AttemptFailure::Fatal(file_system_error("flush partial download", part, error))
        })?;
        if let Some((helper, length)) = handover {
            drop(file);
            let at = start + received;
            let plan = Plan {
                written: std::iter::once(0..at)
                    .filter(|range| !range.is_empty())
                    .collect(),
                start: at,
                length,
                validator: note.as_ref().and_then(|note| note.validator.clone()),
            };
            return self
                .download_pieces(
                    request,
                    source,
                    part,
                    note,
                    (url, body),
                    plan,
                    Some(helper),
                    cancellation,
                )
                .await;
        }

        let size = start + received;
        if let Some(response_length) = response_length
            && received < response_length
        {
            // Cut short: what arrived is still a prefix worth continuing.
            return Err(AttemptFailure::source(format!(
                "response ended after {received} of {response_length} bytes"
            )));
        }
        let checked = check(request, response_length, received, size, digest);
        if let Err(reason) = checked {
            *note = None;
            return Err(AttemptFailure::source(reason));
        }
        file.sync_all().await.map_err(|error| {
            AttemptFailure::Fatal(file_system_error("sync partial download", part, error))
        })?;
        drop(file);
        publish_temporary(part, &request.destination)
            .await
            .map_err(AttemptFailure::Fatal)?;
        *note = None;
        Ok(size)
    }
}

/// A single connection's answer, where it starts and what it may become.
struct Sequential {
    response: TransportResponse,
    /// Bytes of the part file already written before the answer.
    start: u64,
    validator: Option<String>,
    /// The file's length, when its source would split it.
    splittable: Option<u64>,
}

/// A source's answer that is no file: worth asking again when the server
/// was busy or failing.
pub(super) fn status_failure(status: u16) -> AttemptFailure {
    AttemptFailure::Source {
        reason: format!("HTTP status {status}"),
        retryable: status == 408 || status == 429 || status >= 500,
    }
}

/// The bytes of `part` this attempt may continue.
#[derive(Debug, Default)]
struct Kept {
    /// Written ranges, sorted; empty to start over.
    ranges: Vec<Range<u64>>,
    /// The content's length, when `part` is that long with only `ranges`
    /// written; `None` when `ranges` is a prefix.
    length: Option<u64>,
}

impl Kept {
    /// Where the request for the rest begins.
    fn first_missing(&self) -> u64 {
        gaps(&self.ranges, self.length.unwrap_or(u64::MAX))
            .first()
            .map_or(0, |gap| gap.start)
    }
}

async fn kept_bytes(
    request: &TransferRequest,
    source: &str,
    part: &Path,
    note: Option<&Note>,
) -> Kept {
    let Some(note) = note else {
        return Kept::default();
    };
    if !note.continues(source, request.expected_digest.as_ref()) {
        return Kept::default();
    }
    let Ok(metadata) = fs::metadata(part).await else {
        return Kept::default();
    };
    let size = metadata.len();
    match &note.written {
        None => {
            if size == 0
                || request
                    .expected_size
                    .is_some_and(|expected| size >= expected)
            {
                return Kept::default();
            }
            Kept {
                ranges: std::iter::once(0..size).collect(),
                length: None,
            }
        }
        Some(written) => {
            let whole = is_only(&written.ranges, 0..written.length);
            let fits = size == written.length
                && request
                    .expected_size
                    .is_none_or(|expected| expected == written.length);
            if !fits || whole || written.ranges.is_empty() {
                return Kept::default();
            }
            Kept {
                ranges: written.ranges.clone(),
                length: Some(written.length),
            }
        }
    }
}

/// What `If-Range` may carry for this answer: a strong `ETag`, else its
/// `Last-Modified`. A weak or unquoted tag is no tag to `If-Range`: a server
/// reads it as a date it cannot parse and answers a range with the whole
/// file (piston-data sends `etag: 0x8DD…` without quotes).
pub(super) fn range_validator(response: &TransportResponse) -> Option<String> {
    let strong = response
        .header("etag")
        .map(str::trim)
        .filter(|tag| tag.len() >= 2 && tag.starts_with('"') && tag.ends_with('"'));
    strong
        .or_else(|| response.header("last-modified").map(str::trim))
        .filter(|validator| !validator.is_empty())
        .map(str::to_owned)
}

/// Whether `ranges` is exactly `range`.
fn is_only(ranges: &[Range<u64>], range: Range<u64>) -> bool {
    matches!(ranges, [only] if *only == range)
}

/// The first byte and the full length of a `206` answer, from
/// `Content-Range: bytes 10-99/100` (`*` for an unknown length).
pub(super) fn content_range(response: &TransportResponse) -> Option<(u64, Option<u64>)> {
    let range = response.header("content-range")?.trim();
    let range = range.strip_prefix("bytes")?.trim_start();
    let (span, length) = range.split_once('/')?;
    let start = span.split_once('-')?.0.trim().parse().ok()?;
    Some((start, length.trim().parse().ok()))
}

/// The part file, cut to `start` bytes and positioned after them.
async fn open_part(part: &Path, start: u64) -> std::io::Result<File> {
    if let Some(parent) = part.parent() {
        fs::create_dir_all(parent).await?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(start == 0)
        .open(part)
        .await?;
    if start > 0 {
        file.set_len(start).await?;
        file.seek(SeekFrom::Start(start)).await?;
    }
    Ok(file)
}

/// Feeds the first `length` bytes of `part` to `digest`.
pub(super) async fn hash_prefix(
    part: &Path,
    length: u64,
    digest: &mut ContentHasher,
) -> std::io::Result<()> {
    if length == 0 {
        return Ok(());
    }
    let mut file = File::open(part).await?.take(length);
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        digest.update(&buffer[..read]);
    }
}

/// Whether the finished file is the one asked for.
fn check(
    request: &TransferRequest,
    response_length: Option<u64>,
    received: u64,
    size: u64,
    digest: Option<ContentHasher>,
) -> Result<(), String> {
    if let Some(response_length) = response_length
        && received != response_length
    {
        return Err(format!(
            "response length was {received}, expected {response_length}"
        ));
    }
    if let Some(expected_size) = request.expected_size
        && size != expected_size
    {
        return Err(format!("file size was {size}, expected {expected_size}"));
    }
    verify_digest(request, digest)
}

/// Whether hashed content matches the request's digest.
pub(super) fn verify_digest(
    request: &TransferRequest,
    digest: Option<ContentHasher>,
) -> Result<(), String> {
    if let (Some(expected), Some(digest)) = (&request.expected_digest, digest) {
        let actual = digest.finish_hex();
        if !actual.eq_ignore_ascii_case(expected.hex()) {
            return Err(format!(
                "{} was {actual}, expected {}",
                expected.algorithm(),
                expected.hex()
            ));
        }
    }
    Ok(())
}
