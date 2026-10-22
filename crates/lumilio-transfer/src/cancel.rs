use std::sync::Arc;

use tokio::sync::watch;

/// Stops the work it is handed to, or holds its downloads. A paused download
/// lets go of its connection and keeps its bytes; it continues from them
/// once resumed. Cancelling wins over a pause.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<watch::Sender<bool>>,
    paused: Arc<watch::Sender<bool>>,
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        let (cancelled, _receiver) = watch::channel(false);
        let (paused, _receiver) = watch::channel(false);
        Self {
            cancelled: Arc::new(cancelled),
            paused: Arc::new(paused),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    pub async fn cancelled(&self) {
        wait_for(&self.cancelled, true).await;
    }

    pub fn pause(&self) {
        self.paused.send_replace(true);
    }

    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        *self.paused.borrow()
    }

    /// Resolves once paused or cancelled.
    pub(super) async fn paused(&self) {
        tokio::select! {
            () = wait_for(&self.paused, true) => {}
            () = self.cancelled() => {}
        }
    }

    /// Resolves once not paused, or cancelled.
    pub(super) async fn resumed(&self) {
        tokio::select! {
            () = wait_for(&self.paused, false) => {}
            () = self.cancelled() => {}
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

async fn wait_for(sender: &watch::Sender<bool>, wanted: bool) {
    let mut receiver = sender.subscribe();
    // The sender lives as long as the token, so this never fails.
    let _ = receiver.wait_for(|value| *value == wanted).await;
}
