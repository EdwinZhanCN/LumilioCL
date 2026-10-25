use gpui::{Context, EventEmitter};
use lumilio_core::{CancellationToken, RetryAction};

use super::page::{ActivityInputs, ActivityView};
use crate::live::LiveHandler;
use crate::runtime::{self, Worker};
use crate::toast::{self, Toast};
use crate::tr;

/// What Activity tells the application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityEvent {
    /// The tasks changed: the lists are read again.
    ListsChanged,
    /// A task ran again; what a game has may have changed with it.
    Retried,
}

/// The Activity page: the tasks it lists, which tab and which task's files
/// are open, and what can be done to a task.
pub struct Activity {
    pub(super) worker: Option<Worker>,
    /// What the page shows of the rest of the launcher.
    pub(super) inputs: ActivityInputs,
    pub(super) view: ActivityView,
    /// Where the page sends what other features answer.
    pub(super) handler: Option<LiveHandler>,
}

impl EventEmitter<ActivityEvent> for Activity {}

impl Activity {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            inputs: ActivityInputs::default(),
            view: ActivityView::default(),
            handler: None,
        }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// Stops a running task. It finishes on its own terms; the next reading
    /// shows the outcome.
    pub fn cancel(&mut self, task: u64) {
        if let Some(worker) = &self.worker {
            worker.service().cancel_task(task);
        }
    }

    /// Holds a task's downloads, keeping what arrived.
    pub fn pause(&mut self, task: u64, cx: &mut Context<Self>) {
        if let Some(worker) = &self.worker {
            worker.service().pause_task(task);
            cx.emit(ActivityEvent::ListsChanged);
        }
    }

    /// Lets a held task's downloads go on from where they stopped.
    pub fn resume(&mut self, task: u64, cx: &mut Context<Self>) {
        if let Some(worker) = &self.worker {
            worker.service().resume_task(task);
            cx.emit(ActivityEvent::ListsChanged);
        }
    }

    /// Runs a failed or cancelled task again from its original input. It is a
    /// task of its own; the old entry stays.
    pub fn retry(&mut self, action: RetryAction, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            Some(ActivityEvent::ListsChanged),
            Some(Toast::info(tr!("activity-retry-started"))),
            async move {
                // Progress lives in Activity; the channel only has to drain.
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                tokio::spawn(async move { while rx.recv().await.is_some() {} });
                service
                    .retry_task(action, tx, CancellationToken::new())
                    .await
            },
            |_, result, cx| {
                toast::post(
                    toast::outcome(
                        result,
                        |()| tr!("activity-retry-ok").to_owned(),
                        tr!("activity-retry-failed").to_owned(),
                    ),
                    cx,
                );
                cx.emit(ActivityEvent::Retried);
            },
        );
    }

    /// Forgets the tasks that ended. The shorter list shows by itself: only
    /// a failure is told.
    pub fn clear_finished(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            None::<ActivityEvent>,
            None,
            async move { service.clear_finished() },
            |_, result, cx| {
                if let Err(error) = result {
                    toast::post(
                        Toast::error(tr!("activity-clear-failed")).technical(error.to_string()),
                        cx,
                    );
                }
                cx.emit(ActivityEvent::ListsChanged);
            },
        );
    }
}
