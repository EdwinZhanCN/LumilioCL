//! Blocking work moved off the async threads.

use crate::error::ServiceError;

/// Runs blocking work that is not file I/O of its own off the async threads.
pub(crate) async fn blocking_value<V: Send + 'static>(
    work: impl FnOnce() -> V + Send + 'static,
) -> Result<V, ServiceError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ServiceError::Io(std::io::Error::other(error)))
}

/// Runs blocking file work off the async threads.
pub(crate) async fn blocking<V: Send + 'static>(
    work: impl FnOnce() -> std::io::Result<V> + Send + 'static,
) -> std::io::Result<V> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(std::io::Error::other)?
}
