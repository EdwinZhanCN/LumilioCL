use super::request::TransferError;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::{AcquireError, Semaphore, SemaphorePermit};

/// How many downloads may hold a connection at once, shared by every engine
/// made with it. A larger size applies at once; a smaller one as running
/// downloads finish.
#[derive(Clone, Debug)]
pub struct ConnectionBudget {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    permits: Semaphore,
    state: Mutex<State>,
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
                permits: Semaphore::new(size),
                state: Mutex::new(State { size, owed: 0 }),
            }),
        })
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.state().size
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

    pub(super) async fn acquire(&self) -> Result<Connection<'_>, AcquireError> {
        let permit = self.inner.permits.acquire().await?;
        Ok(Connection {
            permit: Some(permit),
            budget: self,
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// One connection's share of a [`ConnectionBudget`], held while it downloads.
pub(super) struct Connection<'a> {
    permit: Option<SemaphorePermit<'a>>,
    budget: &'a ConnectionBudget,
}

impl Drop for Connection<'_> {
    fn drop(&mut self) {
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
