//! What the Activity page shows: running tasks and a durable history.
//!
//! The [`TaskBoard`] holds running tasks in memory; finishing one appends it to
//! the [`ActivityLog`] (JSON lines), so history survives restarts.

#[cfg(test)]
use std::fs;

pub use task_board::{
    ActiveTask, ActivityLog, FinishedTask, ProgressUnit, RetryAction, TaskAction, TaskBoard,
    TaskCategory, TaskLabel, TaskOutcome,
};

pub use self::downloads::{FileDownload, SourceSwitch, TaskDownloads};

mod downloads;

#[cfg(test)]
mod tests;

mod task_board;
