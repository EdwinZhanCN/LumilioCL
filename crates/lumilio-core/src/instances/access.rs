use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex as StdMutex;

use tokio::sync::{Mutex, MutexGuard};

use super::{InstanceRecord, InstanceStore};
use crate::error::ServiceError;

/// The one owner of the library and of which instances are in use.
///
/// A launch, an install, a world copy and a deletion of the same instance
/// exclude each other through [`Instances::reserve`]; the library itself is
/// read and written under its own lock, never held across network reads.
///
/// A game that runs holds its instance differently: its launch hands the
/// lease over with [`InstanceLease::playing`] once the process exists. Work
/// that leaves the game's files alone (a world the game does not have open)
/// goes on beside it through [`Instances::reserve_beside_games`].
pub(crate) struct Instances {
    store: Mutex<InstanceStore>,
    leases: StdMutex<Leases>,
}

#[derive(Default)]
struct Leases {
    /// Instances some work holds on its own.
    held: BTreeSet<String>,
    /// When each running game of an instance started, in Unix milliseconds.
    games: BTreeMap<String, Vec<i64>>,
}

/// Something else is using the instance.
#[derive(Debug)]
pub(crate) struct InstanceBusy(pub(crate) String);

impl Instances {
    pub(crate) fn new(store: InstanceStore) -> Self {
        Self {
            store: Mutex::new(store),
            leases: StdMutex::default(),
        }
    }

    pub(crate) async fn lock(&self) -> MutexGuard<'_, InstanceStore> {
        self.store.lock().await
    }

    /// The registered instance, even when its profile folder is missing.
    pub(crate) async fn record(&self, id: &str) -> Result<InstanceRecord, ServiceError> {
        self.store
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))
    }

    /// The library lock itself, for work that takes it in steps.
    pub(crate) const fn store(&self) -> &Mutex<InstanceStore> {
        &self.store
    }

    /// Holds `id` until the lease drops; refuses while anything else holds
    /// it or one of its games runs.
    pub(crate) fn reserve(&self, id: &str) -> Result<InstanceLease<'_>, InstanceBusy> {
        let mut leases = self.leased();
        if leases.games.contains_key(id) || !leases.held.insert(id.to_owned()) {
            return Err(InstanceBusy(id.to_owned()));
        }
        Ok(InstanceLease {
            leases: &self.leases,
            id: Some(id.to_owned()),
        })
    }

    /// Holds `id` like [`Self::reserve`], but beside its running games: for
    /// work that keeps away from what those games have open. While the lease
    /// is held no game of `id` starts, so [`Self::games_started`] only
    /// shrinks.
    pub(crate) fn reserve_beside_games(&self, id: &str) -> Result<InstanceLease<'_>, InstanceBusy> {
        if !self.leased().held.insert(id.to_owned()) {
            return Err(InstanceBusy(id.to_owned()));
        }
        Ok(InstanceLease {
            leases: &self.leases,
            id: Some(id.to_owned()),
        })
    }

    /// When each running game of `id` started, in Unix milliseconds.
    pub(crate) fn games_started(&self, id: &str) -> Vec<i64> {
        self.leased().games.get(id).cloned().unwrap_or_default()
    }

    pub(crate) fn is_busy(&self, id: &str) -> bool {
        let leases = self.leased();
        leases.held.contains(id) || leases.games.contains_key(id)
    }

    /// Any instance currently in use.
    pub(crate) fn any_busy(&self) -> Option<String> {
        let leases = self.leased();
        leases
            .held
            .iter()
            .chain(leases.games.keys())
            .next()
            .cloned()
    }

    fn leased(&self) -> std::sync::MutexGuard<'_, Leases> {
        self.leases
            .lock()
            .expect("instance lease lock was poisoned")
    }
}

/// Released on drop, including when the future holding it is cancelled.
pub(crate) struct InstanceLease<'a> {
    leases: &'a StdMutex<Leases>,
    /// `None` once the lease is handed to a game.
    id: Option<String>,
}

impl<'a> InstanceLease<'a> {
    /// The launch's game process exists: from now on the instance is held
    /// by a game that started at `started_ms`, until the returned hold drops.
    pub(crate) fn playing(mut self, started_ms: i64) -> GameHold<'a> {
        let id = self.id.take().expect("a lease is handed to a game once");
        let mut leases = self
            .leases
            .lock()
            .expect("instance lease lock was poisoned");
        leases.held.remove(&id);
        leases.games.entry(id.clone()).or_default().push(started_ms);
        GameHold {
            leases: self.leases,
            id,
            started_ms,
        }
    }
}

impl Drop for InstanceLease<'_> {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            self.leases
                .lock()
                .expect("instance lease lock was poisoned")
                .held
                .remove(id);
        }
    }
}

/// One running game's hold on its instance, released on drop.
pub(crate) struct GameHold<'a> {
    leases: &'a StdMutex<Leases>,
    id: String,
    started_ms: i64,
}

impl Drop for GameHold<'_> {
    fn drop(&mut self) {
        let mut leases = self
            .leases
            .lock()
            .expect("instance lease lock was poisoned");
        if let Some(games) = leases.games.get_mut(&self.id) {
            if let Some(at) = games.iter().position(|started| *started == self.started_ms) {
                games.swap_remove(at);
            }
            if games.is_empty() {
                leases.games.remove(&self.id);
            }
        }
    }
}
