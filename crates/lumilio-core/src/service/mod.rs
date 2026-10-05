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
mod plugins;
mod portability;
mod preferences;
mod screenshots;
mod servers;
mod skins;
mod snapshots;
mod support;
mod third_party;
mod tracking;
mod types;
mod worlds;

#[cfg(test)]
mod tests;

pub use self::error::ServiceError;
pub use self::third_party::ThirdPartySignIn;
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
    pub(crate) plugins: crate::plugins::PluginHost,
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
    /// Third-party sign-ins waiting for the person to choose a character:
    /// the session stays in memory only, until chosen or abandoned.
    pending_sign_ins: StdMutex<BTreeMap<u64, third_party::PendingSignIn>>,
    next_pending: std::sync::atomic::AtomicU64,
    /// A client token to use instead of a random one (tests only: a scripted
    /// server cannot echo what it was sent).
    client_token_override: Option<String>,
    /// The agent was looked up for a newer build in this run already.
    injector_checked: std::sync::atomic::AtomicBool,
    /// The key the local skin server signs with, made on first use.
    skin_signer: tokio::sync::OnceCell<Arc<crate::skin::Signer>>,
    /// Size of that key (smaller in tests, where generating one is slow).
    skin_key_bits: usize,
    // Declared last so the stores close before ownership is released.
    _root_lock: crate::root_lock::RootLock,
}

impl<T: Transport + Clone> LauncherService<T> {
    /// Opens (creating on first use) everything under `root`.
    pub fn open(
        root: impl Into<PathBuf>,
        transport: T,
        plugins: Vec<Arc<dyn lumilio_plugin_api::Plugin>>,
    ) -> Result<Self, ServiceError> {
        let layout = Layout::new(root);
        let root_lock = crate::root_lock::RootLock::acquire(layout.root())?;
        let store = InstanceStore::open(layout.root())?;
        let settings = SettingsStore::open(layout.root())?;
        let log = ActivityLog::open(layout.root());
        let startup = recovery::startup_notes(&layout, &store, &settings, &log);
        let plugins = crate::plugins::PluginHost::new(plugins, settings.get().plugins.clone())
            // Invalid hand-edited mirrors must not make plugin initialization
            // prevent the launcher from opening. Network calls fail locally
            // until corrected; the rest of the service remains available.
            .with_network_config(transport.clone(), settings.source_chain().ok());
        Ok(Self {
            plugins,
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
            pending_sign_ins: StdMutex::default(),
            next_pending: std::sync::atomic::AtomicU64::new(1),
            client_token_override: None,
            injector_checked: std::sync::atomic::AtomicBool::new(false),
            skin_signer: tokio::sync::OnceCell::new(),
            skin_key_bits: crate::skin::KEY_BITS,
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
