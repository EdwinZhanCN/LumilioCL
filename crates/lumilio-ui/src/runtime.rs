//! How features reach the launcher service. The application owns the Tokio
//! runtime and hands each feature a [`Worker`]: work runs there, and its
//! result comes back to the interface thread, where only GPUI state changes.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use gpui::{AsyncApp, Context, EventEmitter};
use lumilio_core::{DefaultTransport, LauncherService, ServiceError};
use tokio::runtime::Handle;
use tokio::task::{JoinError, JoinHandle};

use crate::toast::Toast;

/// The service the launcher runs.
pub type Service = LauncherService<DefaultTransport>;

/// The service and the runtime its work runs on.
#[derive(Clone)]
pub struct Worker {
    service: Arc<Service>,
    runtime: Handle,
}

impl Worker {
    #[must_use]
    pub fn new(service: Arc<Service>, runtime: Handle) -> Self {
        Self { service, runtime }
    }

    #[must_use]
    pub fn service(&self) -> &Arc<Service> {
        &self.service
    }

    /// Runs `work` on the runtime. The handle can be awaited from any
    /// executor, including GPUI's.
    pub fn spawn<F>(&self, work: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(work)
    }
}

/// How often work that shows in Activity tells the feature's listeners so.
pub const POLL: Duration = Duration::from_millis(600);

/// Runs `work` on the worker's runtime, then hands its result to `done` on
/// the interface thread. `started` floats up when it begins. With `tasks`,
/// the work shows in Activity: the feature emits that event while it runs
/// and once more when it ends. Work that panics fails like any other, so
/// `done` still runs and says so.
pub fn job<V, Ev, T, E>(
    worker: &Worker,
    cx: &mut Context<V>,
    tasks: Option<Ev>,
    started: Option<Toast>,
    work: impl Future<Output = Result<T, E>> + Send + 'static,
    done: impl FnOnce(&mut V, Result<T, E>, &mut Context<V>) + 'static,
) where
    V: EventEmitter<Ev> + 'static,
    Ev: Clone + 'static,
    T: Send + 'static,
    E: From<JoinError> + Send + 'static,
{
    let handle = worker.spawn(work);
    cx.spawn(async move |view, cx| {
        if let Some(toast) = started {
            cx.update(|cx| crate::toast::post(toast, cx));
        }
        while let Some(event) = tasks.as_ref().filter(|_| !handle.is_finished()) {
            cx.background_executor().timer(POLL).await;
            let event = event.clone();
            let _ = view.update(cx, |_, cx| cx.emit(event));
        }
        let output = handle.await.unwrap_or_else(|error| Err(E::from(error)));
        let _ = view.update(cx, |view, cx| {
            done(view, output, cx);
            if let Some(event) = tasks {
                cx.emit(event);
            }
        });
    })
    .detach();
}

/// Waits for work on the runtime. Work that panicked is told in a toast,
/// whatever page is open; work aborted on purpose says nothing. Either way
/// there is no answer.
pub async fn answer<T>(handle: JoinHandle<T>, cx: &mut AsyncApp) -> Option<T> {
    match handle.await {
        Ok(output) => Some(output),
        Err(error) => {
            let error = ServiceError::from(error);
            if matches!(error, ServiceError::Panicked(_)) {
                let toast =
                    Toast::error(crate::tr!("common-work-stopped")).technical(error.to_string());
                cx.update(|cx| crate::toast::post(toast, cx));
            }
            None
        }
    }
}
