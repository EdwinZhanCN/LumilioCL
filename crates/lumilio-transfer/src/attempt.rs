//! One attempt at one source: continue the bytes kept from an earlier
//! attempt when they may be trusted, else start over, and publish the file
//! once it is verified.

use super::engine::TransferEngine;
use super::files::{AttemptFailure, ContentHasher, file_system_error, publish_temporary};
use super::partial::Note;
use super::request::{TransferEvent, TransferRequest};
use super::transport::{Transport, TransportResponse};
use crate::CancellationToken;
use futures_util::stream::StreamExt;
use std::io::SeekFrom;
use std::path::Path;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

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
        let offset = resumable_offset(request, source, part, note.as_ref()).await;
        let validator = note
            .as_ref()
            .filter(|note| note.source == source)
            .and_then(|note| note.validator.clone());
        let response = tokio::select! {
            () = cancellation.cancelled() => return Err(AttemptFailure::Cancelled),
            response = async {
                if offset > 0 {
                    self.transport.get_from(source, offset, validator.as_deref()).await
                } else {
                    self.transport.get(source).await
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
            let retryable = status == 408 || status == 429 || status >= 500;
            return Err(AttemptFailure::Source {
                reason: format!("HTTP status {status}"),
                retryable,
            });
        }
        let start = if offset > 0 && status == 206 {
            if range_start(&response) != Some(offset) {
                *note = None;
                return Err(AttemptFailure::source(format!(
                    "asked for bytes from {offset}, received {:?}",
                    response.header("content-range")
                )));
            }
            offset
        } else {
            0
        };

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
            validator: response
                .header("etag")
                .or_else(|| response.header("last-modified"))
                .map(str::to_owned)
                .or(if start > 0 { validator } else { None }),
            digest: request.expected_digest.clone(),
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
        let streamed = loop {
            let chunk = tokio::select! {
                () = cancellation.cancelled() => break Err(AttemptFailure::Cancelled),
                chunk = body.next() => chunk,
            };
            let Some(chunk) = chunk else {
                break Ok(());
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
            received = received.saturating_add(chunk.len() as u64);
            let completed = start + received;
            let _ = self.events.send(TransferEvent::Progress {
                id: request.id.clone(),
                completed,
                total: total.filter(|total| completed <= *total),
            });
        };
        // What was written stays a prefix the next attempt can measure.
        let flushed = file.flush().await;
        streamed?;
        flushed.map_err(|error| {
            AttemptFailure::Fatal(file_system_error("flush partial download", part, error))
        })?;

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

/// How many kept bytes this attempt may continue from.
async fn resumable_offset(
    request: &TransferRequest,
    source: &str,
    part: &Path,
    note: Option<&Note>,
) -> u64 {
    let Some(note) = note else {
        return 0;
    };
    if !note.continues(source, request.expected_digest.as_ref()) {
        return 0;
    }
    let Ok(metadata) = fs::metadata(part).await else {
        return 0;
    };
    let size = metadata.len();
    if request
        .expected_size
        .is_some_and(|expected| size >= expected)
    {
        return 0;
    }
    size
}

/// The first byte of a `206` answer, from `Content-Range: bytes 10-99/100`.
fn range_start(response: &TransportResponse) -> Option<u64> {
    let range = response.header("content-range")?.trim();
    let range = range.strip_prefix("bytes")?.trim_start();
    range.split_once('-')?.0.trim().parse().ok()
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

async fn hash_prefix(part: &Path, length: u64, digest: &mut ContentHasher) -> std::io::Result<()> {
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
