use super::request::TransferError;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore};

/// How many downloads may hold a connection at once, shared by every engine
/// made with it. A larger size applies at once; a smaller one as running
/// downloads finish.
#[derive(Clone, Debug)]
pub struct ConnectionBudget {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    permits: Arc<Semaphore>,
    state: Mutex<State>,
    /// Connections held right now.
    open: AtomicUsize,
    /// Downloads waiting for a connection.
    waiting: AtomicUsize,
    /// Bytes received over all connections since the budget was made.
    received: AtomicU64,
}

#[derive(Debug)]
struct State {
    size: usize,
    /// Permits in use that are retired instead of returned, after a shrink.
    owed: usize,
}

impl ConnectionBudget {
    pub fn new(size: usize) -> Result<Self, TransferError> {
        positive(size)?;
        Ok(Self {
            inner: Arc::new(Inner {
                permits: Arc::new(Semaphore::new(size)),
                state: Mutex::new(State { size, owed: 0 }),
                open: AtomicUsize::new(0),
                waiting: AtomicUsize::new(0),
                received: AtomicU64::new(0),
            }),
        })
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.state().size
    }

    /// Connections held right now, by every engine on this budget.
    #[must_use]
    pub fn open(&self) -> usize {
        self.inner.open.load(Ordering::Relaxed)
    }

    /// Bytes received over this budget's connections so far; two readings a
    /// moment apart give the speed of all downloads together.
    #[must_use]
    pub fn received(&self) -> u64 {
        self.inner.received.load(Ordering::Relaxed)
    }

    pub fn resize(&self, size: usize) -> Result<(), TransferError> {
        positive(size)?;
        let mut state = self.state();
        if size > state.size {
            let mut added = size - state.size;
            let settled = added.min(state.owed);
            state.owed -= settled;
            added -= settled;
            self.inner.permits.add_permits(added);
        } else {
            let surplus = state.size - size;
            let forgotten = self.inner.permits.forget_permits(surplus);
            state.owed += surplus - forgotten;
        }
        state.size = size;
        Ok(())
    }

    pub(super) async fn acquire(&self) -> Result<Connection, AcquireError> {
        self.inner.waiting.fetch_add(1, Ordering::Relaxed);
        let _waiting = Waiting(&self.inner.waiting);
        let permit = self.inner.permits.clone().acquire_owned().await?;
        Ok(self.connection(permit))
    }

    /// A connection nobody is waiting for, if one is free now. A queued
    /// download is always served first.
    pub(super) fn try_acquire(&self) -> Option<Connection> {
        if self.waiting() > 0 {
            return None;
        }
        let permit = self.inner.permits.clone().try_acquire_owned().ok()?;
        Some(self.connection(permit))
    }

    /// Downloads waiting for a connection right now.
    pub(super) fn waiting(&self) -> usize {
        self.inner.waiting.load(Ordering::Relaxed)
    }

    pub(super) fn count(&self, bytes: usize) {
        self.inner
            .received
            .fetch_add(bytes as u64, Ordering::Relaxed);
    }

    fn connection(&self, permit: OwnedSemaphorePermit) -> Connection {
        self.inner.open.fetch_add(1, Ordering::Relaxed);
        Connection {
            permit: Some(permit),
            budget: self.clone(),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Counts a download as waiting until its connection is granted or it gives
/// up.
struct Waiting<'a>(&'a AtomicUsize);

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// One connection's share of a [`ConnectionBudget`], held while it downloads.
pub(super) struct Connection {
    permit: Option<OwnedSemaphorePermit>,
    budget: ConnectionBudget,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.budget.inner.open.fetch_sub(1, Ordering::Relaxed);
        let mut state = self.budget.state();
        if state.owed > 0
            && let Some(permit) = self.permit.take()
        {
            state.owed -= 1;
            permit.forget();
        }
    }
}

fn positive(size: usize) -> Result<(), TransferError> {
    if size == 0 {
        return Err(TransferError::InvalidRequest(
            "transfer concurrency must be positive".to_owned(),
        ));
    }
    Ok(())
}
