//! The headless launcher: one object that owns the stores and answers what the
//! interface asks (library, Home, create, launch, search, install).
//!
//! Every method is `async` and `Send`, so any front end can run it on a
//! runtime; nothing here knows about a window. Locks are never held across a
//! network wait.

mod accounts;
mod activity;
mod content;
mod content_install;
mod diagnostics;
mod discover;
mod error;
mod install;
mod instances;
mod java;
mod launch;
mod library;
mod maintenance;
mod modpacks;
mod packs;
mod portability;
mod preferences;
mod snapshots;
mod support;
mod tracking;
mod types;
mod worlds;

#[cfg(test)]
mod tests;

pub use self::error::ServiceError;
pub use self::types::{
    ActivityView, ContentEffect, ContentResult, DependencyNeed, DependencyReport, DiscoverFilters,
    GameLogs, InstalledProject, Library, ProjectDetail, now,
};

use self::support::InstanceLease;
use crate::activity::CancellationToken;
use crate::activity_log::{ActivityLog, TaskBoard};
use crate::credentials::{CredentialStore, SystemCredentials};
use crate::instance::{InstanceStore, Loader};
use crate::layout::Layout;
use crate::recovery;
use crate::recovery::RecoveryNote;
use crate::settings::SettingsStore;
use crate::transfer::Transport;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

/// `Fabric 0.16.0` / `Vanilla`, for a history line.
fn loader_text(loader: Loader, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{loader:?} {version}"),
        None => format!("{loader:?}"),
    }
}

/// How many recently played instances Home lists next to Continue.
const RECENT_LIMIT: usize = 4;
/// Instances diagnosed for Home: verifying files is not free, so only the
/// ones a person is likely to care about right now.
const DIAGNOSED_LIMIT: usize = 8;
const TRANSFER_CONCURRENCY: usize = 4;
/// A Minecraft token with less than this left is refreshed before a launch.
const TOKEN_MARGIN_SECONDS: u64 = 300;
/// A key that is never an account, used to see whether the store works.
const CREDENTIAL_PROBE: &str = "lumilio-store-check";

pub struct LauncherService<T> {
    layout: Layout,
    transport: T,
    store: Mutex<InstanceStore>,
    settings: Mutex<SettingsStore>,
    board: StdMutex<TaskBoard>,
    /// Cancellation tokens of running tasks, by task id.
    cancels: StdMutex<BTreeMap<u64, CancellationToken>>,
    /// What the start-up recovery found and did.
    startup: Vec<RecoveryNote>,
    operations: StdMutex<BTreeSet<String>>,
    log: ActivityLog,
    /// Replaces the conventional Java search locations (tests, portable setups).
    runtime_roots: Option<Vec<PathBuf>>,
    filters: Mutex<Option<DiscoverFilters>>,
    /// Where sign-in secrets live; never a file.
    credentials: Arc<dyn CredentialStore>,
    /// One refresh at a time: refresh tokens rotate, so two at once would
    /// invalidate each other.
    auth_lock: Mutex<()>,
    // Declared last so the stores close before ownership is released.
    _root_lock: crate::root_lock::RootLock,
}

impl<T: Transport + Clone> LauncherService<T> {
    /// Opens (creating on first use) everything under `root`.
    pub fn open(root: impl Into<PathBuf>, transport: T) -> Result<Self, ServiceError> {
        let layout = Layout::new(root);
        let root_lock = crate::root_lock::RootLock::acquire(layout.root())?;
        let store = InstanceStore::open(layout.root())?;
        let settings = SettingsStore::open(layout.root())?;
        let log = ActivityLog::open(layout.root());
        let startup = recovery::startup_notes(&layout, &store, &settings, &log);
        Ok(Self {
            layout,
            transport,
            store: Mutex::new(store),
            settings: Mutex::new(settings),
            board: StdMutex::new(TaskBoard::default()),
            cancels: StdMutex::default(),
            startup,
            operations: StdMutex::new(BTreeSet::new()),
            log,
            runtime_roots: None,
            filters: Mutex::new(None),
            credentials: Arc::new(SystemCredentials),
            auth_lock: Mutex::new(()),
            _root_lock: root_lock,
        })
    }

    /// Keeps sign-in secrets in this store instead of the system one (tests).
    #[must_use]
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialStore>) -> Self {
        self.credentials = credentials;
        self
    }

    /// What start-up recovery found and did, for Diagnostics and Activity.
    #[must_use]
    pub fn startup_notes(&self) -> &[RecoveryNote] {
        &self.startup
    }

    fn reserve_instance(&self, id: &str) -> Result<InstanceLease<'_>, ServiceError> {
        let mut targets = self
            .operations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !targets.insert(id.to_owned()) {
            return Err(ServiceError::InstanceBusy(id.to_owned()));
        }
        Ok(InstanceLease {
            targets: &self.operations,
            id: id.to_owned(),
        })
    }

    /// Looks for Java only in these folders instead of the conventional
    /// system locations.
    #[must_use]
    pub fn with_runtime_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.runtime_roots = Some(roots);
        self
    }

    #[must_use]
    pub const fn layout(&self) -> &Layout {
        &self.layout
    }
}
