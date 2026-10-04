use super::error::ServiceError;
use super::types::now;
use crate::activity::CancellationToken;
use crate::activity_log::{ActivityLog, RetryAction, TaskBoard, TaskOutcome};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex as StdMutex;

/// Runs blocking work that is not file I/O of its own off the async threads.
pub(super) async fn blocking_value<V: Send + 'static>(
    work: impl FnOnce() -> V + Send + 'static,
) -> Result<V, ServiceError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ServiceError::Io(std::io::Error::other(error)))
}

/// Runs blocking file work off the async threads.
pub(super) async fn blocking<V: Send + 'static>(
    work: impl FnOnce() -> std::io::Result<V> + Send + 'static,
) -> std::io::Result<V> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(std::io::Error::other)?
}

/// Keeps a target reserved without holding a mutex across asynchronous work.
pub(super) struct InstanceLease<'a> {
    pub(super) targets: &'a StdMutex<BTreeSet<String>>,
    pub(super) id: String,
}

impl Drop for InstanceLease<'_> {
    fn drop(&mut self) {
        self.targets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

/// Accounts for normal returns and futures interrupted before reporting a result.
pub(super) struct TaskLease<'a> {
    pub(super) board: &'a StdMutex<TaskBoard>,
    pub(super) log: &'a ActivityLog,
    pub(super) cancels: &'a StdMutex<BTreeMap<u64, CancellationToken>>,
    pub(super) id: u64,
    pub(super) ended: bool,
}

impl TaskLease<'_> {
    /// Remembers how to run this task again, should it fail.
    pub(super) fn retrying(self, action: RetryAction) -> Self {
        self.board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_retry(self.id, action);
        self
    }

    /// The game this task made, known only once it is done.
    pub(super) fn about(&self, instance: &str) {
        self.board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_instance(self.id, instance);
    }

    pub(super) fn finish(mut self, outcome: TaskOutcome) {
        self.ended = true;
        self.record(outcome);
    }

    pub(super) fn record(&self, outcome: TaskOutcome) {
        let mut board = self
            .board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // On append failure the terminal fact remains in the board's fallback.
        let _ = board.finish(self.id, outcome, now(), self.log);
    }
}

impl Drop for TaskLease<'_> {
    fn drop(&mut self) {
        // A finished or interrupted task can no longer be cancelled.
        self.cancels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
        if !self.ended {
            self.record(TaskOutcome::Failed(
                "operation interrupted before its result was reported; changes may already exist"
                    .into(),
            ));
        }
    }
}

/// Sums the bytes of all the engine's transfers into one task's progress.
/// A transfer of unknown length makes the whole total unknown (shown as
/// "working", not as a percentage).
pub(super) async fn meter_bytes(
    board: &StdMutex<TaskBoard>,
    task: u64,
    mut events: tokio::sync::broadcast::Receiver<crate::transfer::TransferEvent>,
) {
    use crate::transfer::TransferEvent;
    use tokio::sync::broadcast::error::RecvError;
    let mut seen: BTreeMap<String, (u64, Option<u64>)> = BTreeMap::new();
    loop {
        match events.recv().await {
            Ok(TransferEvent::Progress {
                id,
                completed,
                total,
            }) => {
                seen.insert(id, (completed, total));
                let done: u64 = seen.values().map(|(done, _)| *done).sum();
                let total = seen
                    .values()
                    .map(|(_, total)| *total)
                    .try_fold(0_u64, |sum, total| total.map(|total| sum + total))
                    .unwrap_or(0);
                board
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .progress_bytes(task, done, total);
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}
