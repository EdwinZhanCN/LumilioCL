use std::collections::BTreeSet;
use std::sync::Mutex as StdMutex;

use tokio::sync::{Mutex, MutexGuard};

use super::InstanceStore;

/// The one owner of the library and of which instances are in use.
///
/// A launch, an install, a world copy and a deletion of the same instance
/// exclude each other through [`Instances::reserve`]; the library itself is
/// read and written under its own lock, never held across network reads.
pub(crate) struct Instances {
    store: Mutex<InstanceStore>,
    leases: StdMutex<BTreeSet<String>>,
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

    /// The library lock itself, for work that takes it in steps.
    pub(crate) const fn store(&self) -> &Mutex<InstanceStore> {
        &self.store
    }

    /// Holds `id` until the lease drops; refuses while anything else holds it.
    pub(crate) fn reserve(&self, id: &str) -> Result<InstanceLease<'_>, InstanceBusy> {
        if !self.leased().insert(id.to_owned()) {
            return Err(InstanceBusy(id.to_owned()));
        }
        Ok(InstanceLease {
            leases: &self.leases,
            id: id.to_owned(),
        })
    }

    pub(crate) fn is_busy(&self, id: &str) -> bool {
        self.leased().contains(id)
    }

    /// Any instance currently in use.
    pub(crate) fn any_busy(&self) -> Option<String> {
        self.leased().iter().next().cloned()
    }

    fn leased(&self) -> std::sync::MutexGuard<'_, BTreeSet<String>> {
        self.leases
            .lock()
            .expect("instance lease lock was poisoned")
    }
}

/// Released on drop, including when the future holding it is cancelled.
pub(crate) struct InstanceLease<'a> {
    leases: &'a StdMutex<BTreeSet<String>>,
    id: String,
}

impl Drop for InstanceLease<'_> {
    fn drop(&mut self) {
        self.leases
            .lock()
            .expect("instance lease lock was poisoned")
            .remove(&self.id);
    }
}
