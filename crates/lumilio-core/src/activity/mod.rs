//! Background work the person can see: the task board and its durable log,
//! cancellation, and the dependency graph some tasks run on.

pub use graph::{
    ActivityContext, ActivityEvent, ActivityGraph, ActivityGraphError, ActivityNode,
    ActivityRecord, ActivityReport, ActivityScheduler, ActivityState, CancellationToken,
    ProgressSnapshot,
};
pub use log::{
    ActiveTask, ActivityLog, FileDownload, FinishedTask, ProgressUnit, RetryAction, SourceSwitch,
    TaskAction, TaskBoard, TaskCategory, TaskDownloads, TaskLabel, TaskOutcome,
};
pub use tasks::{ActivityView, Traffic};
pub(crate) use tasks::{Meter, Tasks};

mod graph;
mod log;
mod tasks;
