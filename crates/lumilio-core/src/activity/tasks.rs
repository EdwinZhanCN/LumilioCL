use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::{Mutex, MutexGuard};

use tokio::sync::broadcast;

use super::{
    ActiveTask, ActivityLog, CancellationToken, FinishedTask, RetryAction, TaskBoard, TaskCategory,
    TaskLabel, TaskOutcome,
};
use crate::clock::now;
use crate::error::ServiceError;
use crate::launching::LaunchServiceError;
use crate::transfer::TransferEvent;

/// Running and recently finished work, for the Activity page and its badge.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivityView {
    pub active: Vec<ActiveTask>,
    /// Ids among `active` that
    /// [`LauncherService::cancel_task`](crate::LauncherService::cancel_task) can stop.
    pub cancellable: BTreeSet<u64>,
    /// Ids among `active` whose downloads are paused.
    pub paused: BTreeSet<u64>,
    /// Newest first.
    pub finished: Vec<FinishedTask>,
    pub traffic: Traffic,
}

/// How the connections all downloads share are used right now.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Traffic {
    /// Connections held.
    pub open: usize,
    /// The most that may be held at once.
    pub limit: usize,
    /// Bytes received since the launcher opened; two readings give the
    /// speed of all downloads.
    pub received: u64,
}

/// The one owner of the task board, the cancellation tokens of running tasks
/// and the durable activity log.
pub(crate) struct Tasks {
    board: Mutex<TaskBoard>,
    /// Cancellation tokens of running tasks, by task id.
    cancels: Mutex<BTreeMap<u64, CancellationToken>>,
    log: ActivityLog,
}

impl Tasks {
    pub(crate) fn new(log: ActivityLog) -> Self {
        Self {
            board: Mutex::default(),
            cancels: Mutex::default(),
            log,
        }
    }

    fn board(&self) -> MutexGuard<'_, TaskBoard> {
        self.board.lock().expect("activity board lock was poisoned")
    }

    fn cancels(&self) -> MutexGuard<'_, BTreeMap<u64, CancellationToken>> {
        self.cancels
            .lock()
            .expect("activity cancellation lock was poisoned")
    }

    /// Shows a new running task; `cancel` makes it stoppable from Activity.
    pub(crate) fn begin(
        &self,
        category: TaskCategory,
        label: TaskLabel,
        instance: Option<String>,
        cancel: Option<&CancellationToken>,
    ) -> TaskLease<'_> {
        let id = self.board().start(category, label, instance, now());
        if let Some(cancel) = cancel {
            self.cancels().insert(id, cancel.clone());
        }
        TaskLease {
            tasks: self,
            id,
            ended: false,
        }
    }

    /// Records how `task` ended from its result.
    pub(crate) fn end<V>(&self, task: TaskLease<'_>, result: &Result<V, ServiceError>) {
        let outcome = match result {
            Ok(_) => TaskOutcome::Succeeded,
            Err(ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled)) => {
                TaskOutcome::Cancelled
            }
            Err(error) => TaskOutcome::Failed(error.to_string()),
        };
        task.finish(outcome);
    }

    /// Shows `done` of `total` items as the progress of task `task`.
    pub(crate) fn progress(&self, task: u64, done: u64, total: u64) {
        self.board().progress(task, done, total);
    }

    /// Runs `work` while the byte progress of the engine behind `events` is
    /// shown as the progress of task `task`, with the files it downloads.
    pub(crate) async fn metered<R>(
        &self,
        task: u64,
        events: broadcast::Receiver<TransferEvent>,
        work: impl Future<Output = R>,
    ) -> R {
        self.watching(task, events, Meter::Bytes, work).await
    }

    /// Runs `work` while the transfers behind `events` are recorded as task
    /// `task`'s downloads.
    pub(crate) async fn watching<R>(
        &self,
        task: u64,
        events: broadcast::Receiver<TransferEvent>,
        meter: Meter,
        work: impl Future<Output = R>,
    ) -> R {
        let watch = self.watch_transfers(task, events, meter);
        tokio::pin!(work);
        tokio::pin!(watch);
        tokio::select! {
            result = &mut work => result,
            () = &mut watch => work.await,
        }
    }

    /// Records what task `task` downloads from the events of its engines.
    /// With [`Meter::Bytes`] the bytes of all its transfers are also its
    /// progress; a transfer of unknown length makes the whole total unknown
    /// (shown as "working", not as a percentage).
    async fn watch_transfers(
        &self,
        task: u64,
        mut events: broadcast::Receiver<TransferEvent>,
        meter: Meter,
    ) {
        use tokio::sync::broadcast::error::RecvError;

        let mut seen: BTreeMap<String, (u64, Option<u64>)> = BTreeMap::new();
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            let mut board = self.board();
            board.record_transfer(task, &event);
            if let (
                Meter::Bytes,
                TransferEvent::Progress {
                    id,
                    completed,
                    total,
                },
            ) = (meter, event)
            {
                seen.insert(id, (completed, total));
                let done: u64 = seen.values().map(|(done, _)| *done).sum();
                let total = seen
                    .values()
                    .map(|(_, total)| *total)
                    .try_fold(0_u64, |sum, total| total.map(|total| sum + total))
                    .unwrap_or(0);
                board.progress_bytes(task, done, total);
            }
        }
    }

    fn control(&self, id: u64) -> Option<CancellationToken> {
        self.cancels().get(&id).cloned()
    }

    /// Asks a running task to stop. `false` when the id is unknown, already
    /// finished, or not cancellable; the task's own outcome is still decided by
    /// the work itself.
    pub(crate) fn cancel(&self, id: u64) -> bool {
        self.control(id).map(|token| token.cancel()).is_some()
    }

    /// Holds the downloads of a running task until [`Self::resume`]: they let
    /// go of their connections and keep their bytes. `false` when the task is
    /// unknown, finished or cannot be stopped.
    pub(crate) fn pause(&self, id: u64) -> bool {
        self.control(id).map(|token| token.pause()).is_some()
    }

    /// Lets a paused task's downloads continue from the bytes they kept.
    pub(crate) fn resume(&self, id: u64) -> bool {
        self.control(id).map(|token| token.resume()).is_some()
    }

    /// Forgets every finished task; running ones are untouched.
    pub(crate) fn clear_finished(&self) -> std::io::Result<()> {
        let mut board = self.board();
        self.log.compact(0)?;
        board.clear_unrecorded();
        Ok(())
    }

    /// Running tasks and the most recent `finished_limit` finished ones.
    pub(crate) fn view(&self, finished_limit: usize, traffic: Traffic) -> ActivityView {
        let board = self.board();
        let active = board.active().to_vec();
        let mut finished = self.log.recent(None, finished_limit).unwrap_or_default();
        for terminal in board.unrecorded() {
            finished.push(terminal.clone());
        }
        finished.sort_by_key(|task| std::cmp::Reverse(task.finished));
        finished.truncate(finished_limit);
        let (cancellable, paused) = {
            let cancels = self.cancels();
            (
                cancels.keys().copied().collect(),
                cancels
                    .iter()
                    .filter(|(_, token)| token.is_paused())
                    .map(|(id, _)| *id)
                    .collect(),
            )
        };
        ActivityView {
            active,
            cancellable,
            paused,
            finished,
            traffic,
        }
    }
}

/// A running task on the board. Accounts for normal returns and futures
/// interrupted before reporting a result.
pub(crate) struct TaskLease<'a> {
    tasks: &'a Tasks,
    pub(crate) id: u64,
    ended: bool,
}

impl TaskLease<'_> {
    /// Remembers how to run this task again, should it fail.
    pub(crate) fn retrying(self, action: RetryAction) -> Self {
        self.tasks.board().set_retry(self.id, action);
        self
    }

    /// The game this task made, known only once it is done.
    pub(crate) fn about(&self, instance: &str) {
        self.tasks.board().set_instance(self.id, instance);
    }

    pub(crate) fn finish(mut self, outcome: TaskOutcome) {
        self.ended = true;
        self.record(outcome);
    }

    fn record(&self, outcome: TaskOutcome) {
        let mut board = self.tasks.board();
        // On append failure the terminal fact remains in the board's fallback.
        let _ = board.finish(self.id, outcome, now(), &self.tasks.log);
    }
}

impl Drop for TaskLease<'_> {
    fn drop(&mut self) {
        // A finished or interrupted task can no longer be cancelled.
        self.tasks.cancels().remove(&self.id);
        if !self.ended {
            self.record(TaskOutcome::Failed(
                "operation interrupted before its result was reported; changes may already exist"
                    .into(),
            ));
        }
    }
}

/// What a task's progress counts while its transfers are watched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Meter {
    /// The bytes of all its transfers, summed.
    Bytes,
    /// Something else (files, from the launcher's signals): only what it
    /// downloads is recorded.
    Files,
}

#[cfg(test)]
mod tests;
