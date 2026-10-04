use super::TEMPORARY_FILE_SEQUENCE;
use super::request::{TransferError, TransferRequest};
use sha1::{Digest, Sha1};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tokio::fs;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncReadExt;

#[derive(Debug)]
pub(super) enum AttemptFailure {
    Source { reason: String, retryable: bool },
    Cancelled,
    Fatal(TransferError),
}

impl AttemptFailure {
    pub(super) fn source(reason: String) -> Self {
        Self::Source {
            reason,
            retryable: true,
        }
    }
}

pub(super) async fn verify_file(
    path: &Path,
    expected_size: Option<u64>,
    expected_sha1: Option<&str>,
) -> Result<bool, TransferError> {
    let metadata = match fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(file_system_error("inspect", path, error)),
    };
    if !metadata.is_file() {
        return Ok(false);
    }
    if expected_size.is_some_and(|expected| metadata.len() != expected) {
        return Ok(false);
    }
    let Some(expected_sha1) = expected_sha1 else {
        return Ok(true);
    };

    let mut file = File::open(path)
        .await
        .map_err(|error| file_system_error("open", path, error))?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| file_system_error("read", path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(encode_lower_hex(&hasher.finalize()).eq_ignore_ascii_case(expected_sha1))
}

pub(super) fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub(super) async fn copy_verified_candidate(
    candidate: &Path,
    request: &TransferRequest,
) -> Result<(), TransferError> {
    let (temporary_path, temporary_file) = create_temporary_sibling(&request.destination).await?;
    drop(temporary_file);
    let result = async {
        fs::copy(candidate, &temporary_path)
            .await
            .map_err(|error| file_system_error("copy cache candidate", candidate, error))?;
        if !verify_file(
            &temporary_path,
            request.expected_size,
            request.expected_sha1.as_deref(),
        )
        .await?
        {
            return Err(TransferError::InvalidRequest(format!(
                "cache copy failed verification: {}",
                candidate.display()
            )));
        }
        publish_temporary(&temporary_path, &request.destination).await
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path).await;
    }
    result
}

pub(super) async fn create_temporary_sibling(
    destination: &Path,
) -> Result<(PathBuf, File), TransferError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .map_err(|error| file_system_error("create destination directory", parent, error))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");

    for _ in 0..128 {
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{file_name}.lumilio.{}.{}.part",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(file_system_error("create temporary file", &path, error)),
        }
    }
    Err(TransferError::FileSystem {
        operation: "create unique temporary file",
        path: destination.to_owned(),
        message: "temporary name space exhausted".to_owned(),
    })
}

pub(super) async fn publish_temporary(
    temporary: &Path,
    destination: &Path,
) -> Result<(), TransferError> {
    fs::rename(temporary, destination)
        .await
        .map_err(|error| file_system_error("publish verified file", destination, error))
}

pub(super) fn file_system_error(
    operation: &'static str,
    path: &Path,
    error: std::io::Error,
) -> TransferError {
    TransferError::FileSystem {
        operation,
        path: path.to_owned(),
        message: error.to_string(),
    }
}
