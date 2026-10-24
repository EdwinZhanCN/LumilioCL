//! How features reach the launcher service. The application owns the Tokio
//! runtime and hands each feature a [`Worker`]: work runs there, and its
//! result comes back to the interface thread, where only GPUI state changes.

use std::future::Future;
use std::sync::Arc;

use gpui::AsyncApp;
use lumilio_core::{DefaultTransport, LauncherService, ServiceError};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

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
