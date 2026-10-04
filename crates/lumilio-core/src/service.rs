//! The headless launcher: one object that owns the stores and answers what the
//! interface asks (library, Home, create, launch, search, install).
//!
//! Behavior notes: `docs/behavior/service.md`. Every method is `async` and
//! `Send`, so any front end can run it on a runtime; nothing here knows about
//! a window. Locks are never held across a network wait.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{Mutex, mpsc};

use crate::account::AuthSession;
use crate::activity::CancellationToken;
use crate::activity_log::{
    ActiveTask, ActivityLog, FinishedTask, RetryAction, TaskBoard, TaskCategory, TaskOutcome,
};
use crate::attention::{HomeSummary, summarize};
use crate::catalog::VersionCatalog;
use crate::content::{ContentError, ContentItem, scan as scan_content};
use crate::credentials::{CredentialStore, StoredLogin, SystemCredentials};
use crate::deletion::Deletion;
use crate::diagnostics::Problem;
use crate::discover::{
    CategoryTag, GameVersionTag, ModrinthClient, Project, ProjectKind, SearchPage, SearchQuery,
    Version, fits, install_request, pick_version,
};
use crate::environment::HostProfile;
use crate::fetch::fetch_document;
use crate::forge_meta;
use crate::history::{
    ChangeKind, HistoryEvent, HistoryLog, SessionOutcome, finish_session, record_attempt,
};
use crate::inspect::inspect_instance;
use crate::instance::Collection;
use crate::instance::{
    InstanceRecord, InstanceSettings, InstanceStore, Loader, NewInstance, StoreError,
};
use crate::java::JavaLocator;
use crate::launcher::{LaunchRequest, LaunchServiceError, LaunchUpdate, Launcher};
use crate::layout::Layout;
use crate::loader::{self, LAUNCHABLE_LOADERS};
use crate::microsoft::{AuthError, DeviceCode, MicrosoftClient, Secret};
use crate::modpack;
use crate::process::GameExit;
use crate::recovery::{self, RecoveryNote, SessionMarker};
use crate::settings::{
    AccountKind, LauncherSettings, SettingsError, SettingsStore, validate_memory,
};
use crate::snapshots::{self, SnapshotError, SnapshotInfo, SnapshotScope};
use crate::staged::Staged;
use crate::transfer::{SourceChain, TransferEngine, TransferRequest, Transport};
use crate::tuning::QuickPlay;
use crate::worlds::{self, WorldError, WorldInfo};

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

#[derive(Debug)]
pub enum ServiceError {
    Store(StoreError),
    Settings(SettingsError),
    /// A network document could not be fetched or understood.
    Remote(String),
    NoSuchInstance(String),
    InstanceBusy(String),
    RootBusy(PathBuf),
    /// The project has no file that fits the instance.
    NoCompatibleVersion,
    Launch(LaunchServiceError),
    Install(String),
    Cancelled,
    /// Nothing can start until an account is added and selected.
    NoAccount,
    /// The selected Microsoft account must sign in again (its name).
    SignInRequired(String),
    /// Signing in or refreshing failed.
    Auth(AuthError),
    /// The chosen file or folder is not (part of) a Java installation.
    NoJavaAt(PathBuf),
    Content(ContentError),
    World(WorldError),
    Export(crate::pack_export::ExportError),
    Snapshot(SnapshotError),
    Io(std::io::Error),
}

impl Display for ServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "{error}"),
            Self::Settings(error) => write!(f, "{error}"),
            Self::Remote(message) => write!(f, "could not reach the service: {message}"),
            Self::RootBusy(root) => write!(
                f,
                "launcher data at {} is already in use; close the other launcher and retry",
                root.display()
            ),
            Self::InstanceBusy(id) => write!(f, "instance {id:?} has an operation in progress"),
            Self::NoSuchInstance(id) => write!(f, "no instance {id:?}"),
            Self::NoCompatibleVersion => f.write_str("no version fits this instance"),
            Self::Launch(error) => write!(f, "{error}"),
            Self::Cancelled => f.write_str("operation was cancelled"),
            Self::NoAccount => f.write_str("add an account before starting the game"),
            Self::SignInRequired(name) => {
                write!(f, "the Microsoft account {name} must sign in again")
            }
            Self::Auth(error) => write!(f, "{error}"),
            Self::NoJavaAt(path) => write!(f, "no Java installation found at {}", path.display()),
            Self::Install(message) => write!(f, "install failed: {message}"),
            Self::Content(error) => write!(f, "{error}"),
            Self::World(error) => write!(f, "{error}"),
            Self::Export(error) => write!(f, "{error}"),
            Self::Snapshot(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ServiceError {}

impl From<SnapshotError> for ServiceError {
    fn from(error: SnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

impl From<crate::pack_export::ExportError> for ServiceError {
    fn from(error: crate::pack_export::ExportError) -> Self {
        Self::Export(error)
    }
}

impl From<WorldError> for ServiceError {
    fn from(error: WorldError) -> Self {
        Self::World(error)
    }
}

impl From<ContentError> for ServiceError {
    fn from(error: ContentError) -> Self {
        Self::Content(error)
    }
}

impl From<StoreError> for ServiceError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<AuthError> for ServiceError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
}

impl From<SettingsError> for ServiceError {
    fn from(error: SettingsError) -> Self {
        Self::Settings(error)
    }
}

impl From<LaunchServiceError> for ServiceError {
    fn from(error: LaunchServiceError) -> Self {
        Self::Launch(error)
    }
}

impl From<std::io::Error> for ServiceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<crate::transfer::TransferError> for ServiceError {
    fn from(error: crate::transfer::TransferError) -> Self {
        match error {
            crate::transfer::TransferError::Cancelled => Self::Cancelled,
            error => Self::Install(error.to_string()),
        }
    }
}

impl From<modpack::ModpackError> for ServiceError {
    fn from(error: modpack::ModpackError) -> Self {
        match error {
            modpack::ModpackError::Cancelled
            | modpack::ModpackError::Transfer(crate::transfer::TransferError::Cancelled) => {
                Self::Cancelled
            }
            error => Self::Install(error.to_string()),
        }
    }
}

/// Read-only work can be dropped safely; do not wrap file commits in this helper.
async fn read_unless_cancelled<V>(
    cancel: &CancellationToken,
    read: impl Future<Output = Result<V, ServiceError>>,
) -> Result<V, ServiceError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(ServiceError::Cancelled),
        result = read => result,
    }
}

/// What the library page shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Library {
    pub instances: Vec<InstanceRecord>,
    pub collections: Vec<Collection>,
}

/// Running and recently finished work, for the Activity page and its badge.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivityView {
    pub active: Vec<ActiveTask>,
    /// Ids among `active` that [`LauncherService::cancel_task`] can stop.
    pub cancellable: BTreeSet<u64>,
    /// Newest first.
    pub finished: Vec<FinishedTask>,
}

/// Filter choices for Discover.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoverFilters {
    pub categories: Vec<CategoryTag>,
    pub game_versions: Vec<GameVersionTag>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDetail {
    pub project: Project,
    /// Newest first.
    pub versions: Vec<Version>,
    pub owner: Option<String>,
}

/// Seconds since the Unix epoch.
#[must_use]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// What happened to one file of a content change.
#[derive(Debug)]
pub struct ContentResult {
    /// The file name as asked for.
    pub file_name: String,
    pub outcome: Result<ContentEffect, ContentError>,
    /// False when the file changed but the history entry could not be written.
    pub recorded: bool,
}

/// What a mod's declared relations to other mods mean for one game.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyReport {
    /// Required and missing, nearest first.
    pub needs: Vec<DependencyNeed>,
    /// Suggested by the mod, not installed.
    pub optional: Vec<DependencyNeed>,
    /// Titles of installed mods the new one says it does not work with.
    pub conflicts: Vec<String>,
}

/// A project the game already has.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledProject {
    pub file_name: String,
    pub version_id: String,
    /// A newer version that fits this game, if there is one.
    pub update: Option<String>,
}

/// A required dependency the game does not have.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyNeed {
    pub project_id: String,
    pub title: String,
    /// The version that would be installed; `None` when none fits this game.
    pub version: Option<Version>,
}

/// How much of the log's end `logs` returns.
pub const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// How much of a log an export carries.
const EXPORT_LOG_BYTES: u64 = 8 * 1024 * 1024;
/// How much of a crash report `crash_report` returns.
pub const REPORT_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameLogs {
    pub latest: Option<String>,
    pub crashes: Vec<crate::diagnostics::CrashReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentEffect {
    /// Already in the requested state; nothing was touched or recorded.
    Unchanged,
    /// Renamed; this is the file name now.
    Renamed(String),
    Removed,
}

/// What a pack file is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PackKind {
    Modrinth,
    Prism,
    /// One of this launcher's own full backups.
    Backup,
}

/// Looks at a zip's entry names: a Modrinth pack names `modrinth.index.json`,
/// a MultiMC / Prism instance `mmc-pack.json`. Anything else is treated as a
/// Modrinth pack so its error is the usual one.
fn sniff_pack(path: &std::path::Path) -> PackKind {
    let Ok(file) = std::fs::File::open(path) else {
        return PackKind::Modrinth;
    };
    let Ok(archive) = zip::ZipArchive::new(file) else {
        return PackKind::Modrinth;
    };
    let names: Vec<&str> = archive.file_names().collect();
    if names.contains(&"modrinth.index.json") {
        PackKind::Modrinth
    } else if names.contains(&"lumilio-backup.json") {
        PackKind::Backup
    } else if names
        .iter()
        .any(|name| name.rsplit('/').next() == Some("mmc-pack.json"))
    {
        PackKind::Prism
    } else {
        PackKind::Modrinth
    }
}

enum UnpackError {
    Cancelled,
    Other(String),
}

/// Unpacks a MultiMC / Prism instance zip into `scratch` and returns the
/// folder holding `instance.cfg` (the top, or one folder down). Entries that
/// would leave `scratch` are skipped; the declared size is capped.
fn unpack_instance_zip(
    archive: &std::path::Path,
    scratch: &std::path::Path,
    cancel: &CancellationToken,
) -> Result<PathBuf, UnpackError> {
    const LIMIT: u64 = 16 * 1024 * 1024 * 1024;
    let other = |error: &dyn std::fmt::Display| UnpackError::Other(error.to_string());
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).map_err(|e| other(&e))?)
        .map_err(|e| other(&e))?;
    let mut declared = 0_u64;
    for index in 0..zip.len() {
        declared = declared.saturating_add(zip.by_index(index).map_err(|e| other(&e))?.size());
    }
    if declared > LIMIT {
        return Err(UnpackError::Other("the zip is too large".to_owned()));
    }
    std::fs::create_dir_all(scratch).map_err(|e| other(&e))?;
    for index in 0..zip.len() {
        if cancel.is_cancelled() {
            return Err(UnpackError::Cancelled);
        }
        let mut entry = zip.by_index(index).map_err(|e| other(&e))?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let target = scratch.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| other(&e))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| other(&e))?;
        }
        let mut out = std::fs::File::create(&target).map_err(|e| other(&e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| other(&e))?;
    }
    if scratch.join("instance.cfg").is_file() {
        return Ok(scratch.to_owned());
    }
    std::fs::read_dir(scratch)
        .map_err(|e| other(&e))?
        .flatten()
        .map(|entry| entry.path())
        .find(|dir| dir.join("instance.cfg").is_file())
        .ok_or_else(|| UnpackError::Other("the zip holds no instance".to_owned()))
}

/// How a backup failure is told: a cancel stays a cancel.
fn backup_failure(error: crate::backup::BackupError) -> ServiceError {
    match error {
        crate::backup::BackupError::Cancelled => ServiceError::Cancelled,
        other => ServiceError::Install(other.to_string()),
    }
}

/// Runs blocking work that is not file I/O of its own off the async threads.
async fn blocking_value<V: Send + 'static>(
    work: impl FnOnce() -> V + Send + 'static,
) -> Result<V, ServiceError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ServiceError::Io(std::io::Error::other(error)))
}

/// Runs blocking file work off the async threads.
async fn blocking<V: Send + 'static>(
    work: impl FnOnce() -> std::io::Result<V> + Send + 'static,
) -> std::io::Result<V> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(std::io::Error::other)?
}

/// Keeps a target reserved without holding a mutex across asynchronous work.
struct InstanceLease<'a> {
    targets: &'a StdMutex<BTreeSet<String>>,
    id: String,
}

impl Drop for InstanceLease<'_> {
    fn drop(&mut self) {
        self.targets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

/// Accounts for normal returns and futures interrupted before reporting a result.
struct TaskLease<'a> {
    board: &'a StdMutex<TaskBoard>,
    log: &'a ActivityLog,
    cancels: &'a StdMutex<BTreeMap<u64, CancellationToken>>,
    id: u64,
    ended: bool,
}

impl TaskLease<'_> {
    /// Remembers how to run this task again, should it fail.
    fn retrying(self, action: RetryAction) -> Self {
        self.board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_retry(self.id, action);
        self
    }

    /// The game this task made, known only once it is done.
    fn about(&self, instance: &str) {
        self.board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_instance(self.id, instance);
    }

    fn finish(mut self, outcome: TaskOutcome) {
        self.ended = true;
        self.record(outcome);
    }

    fn record(&self, outcome: TaskOutcome) {
        let mut board = self
            .board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // On append failure the terminal fact remains in the board's fallback.
        let _ = board.finish(self.id, outcome, now(), self.log);
    }
}

impl Drop for TaskLease<'_> {
    fn drop(&mut self) {
        // A finished or interrupted task can no longer be cancelled.
        self.cancels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
        if !self.ended {
            self.record(TaskOutcome::Failed(
                "operation interrupted before its result was reported; changes may already exist"
                    .into(),
            ));
        }
    }
}

/// Sums the bytes of all the engine's transfers into one task's progress.
/// A transfer of unknown length makes the whole total unknown (shown as
/// "working", not as a percentage).
async fn meter_bytes(
    board: &StdMutex<TaskBoard>,
    task: u64,
    mut events: tokio::sync::broadcast::Receiver<crate::transfer::TransferEvent>,
) {
    use crate::transfer::TransferEvent;
    use tokio::sync::broadcast::error::RecvError;
    let mut seen: BTreeMap<String, (u64, Option<u64>)> = BTreeMap::new();
    loop {
        match events.recv().await {
            Ok(TransferEvent::Progress {
                id,
                completed,
                total,
            }) => {
                seen.insert(id, (completed, total));
                let done: u64 = seen.values().map(|(done, _)| *done).sum();
                let total = seen
                    .values()
                    .map(|(_, total)| *total)
                    .try_fold(0_u64, |sum, total| total.map(|total| sum + total))
                    .unwrap_or(0);
                board
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .progress_bytes(task, done, total);
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

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

    pub async fn library(&self) -> Library {
        let store = self.store.lock().await;
        Library {
            instances: store.instances().to_vec(),
            collections: store.collections().to_vec(),
        }
    }

    /// Reads the registered instance even when its profile folder is missing.
    pub async fn instance(&self, id: &str) -> Result<InstanceRecord, ServiceError> {
        self.store
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))
    }

    pub async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.set_favorite(id, favorite)?)
    }

    /// Flips the favorite flag and returns the new value.
    pub async fn toggle_favorite(&self, id: &str) -> Result<bool, ServiceError> {
        let mut store = self.store.lock().await;
        let favorite = !store
            .get(id)
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?
            .favorite;
        store.set_favorite(id, favorite)?;
        Ok(favorite)
    }

    pub async fn create_collection(&self, name: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.create_collection(name)?)
    }

    pub async fn rename_collection(&self, from: &str, to: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.rename_collection(from, to)?)
    }

    /// Removes the collection only; its games stay in the library.
    pub async fn delete_collection(&self, name: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.delete_collection(name)?)
    }

    /// Puts a game in exactly these collections.
    pub async fn set_game_collections(
        &self,
        id: &str,
        collections: &[String],
    ) -> Result<(), ServiceError> {
        Ok(self
            .store
            .lock()
            .await
            .set_collections_of(id, collections)?)
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.rename(id, name)?)
    }

    /// The last successfully saved launcher defaults and public account settings.
    pub async fn settings(&self) -> LauncherSettings {
        self.settings.lock().await.get().clone()
    }

    /// Adds an offline account (optionally with its own profile id). The first
    /// account becomes the selected one.
    pub async fn add_account(&self, name: &str, uuid: Option<&str>) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.add_offline_account(name, uuid)?)
    }

    /// Chooses the account later launches use (by key); running games keep theirs.
    pub async fn select_account(&self, key: &str) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.select_account(key)?)
    }

    /// Forgets an account identity (by key); instances and worlds are
    /// untouched. A Microsoft account's stored sign-in goes with it.
    pub async fn remove_account(&self, key: &str) -> Result<(), ServiceError> {
        let removed = self
            .settings
            .lock()
            .await
            .get()
            .accounts
            .iter()
            .find(|entry| entry.key() == key)
            .map(|entry| entry.kind);
        self.settings.lock().await.remove_account(key)?;
        if removed == Some(AccountKind::Microsoft) {
            // The account is gone either way; a secret that cannot be deleted
            // is told, not hidden.
            self.credentials.delete(key)?;
        }
        Ok(())
    }

    /// The identity a launch presents: the selected account's real values,
    /// refreshing a Microsoft sign-in that is about to expire.
    async fn session_for(
        &self,
        settings: &crate::settings::LauncherSettings,
    ) -> Result<AuthSession, ServiceError> {
        let entry = settings
            .selected_account
            .as_deref()
            .and_then(|key| settings.accounts.iter().find(|entry| entry.key() == key))
            .ok_or(ServiceError::NoAccount)?;
        match entry.kind {
            AccountKind::Offline => entry
                .profile()
                .map(|profile| profile.session())
                .map_err(|_| ServiceError::NoAccount),
            AccountKind::Microsoft => self.microsoft_session(entry, false).await,
        }
    }

    /// A Microsoft account's session: the cached Minecraft token while it has
    /// five minutes left, else a refreshed one. `force` refreshes regardless.
    async fn microsoft_session(
        &self,
        entry: &crate::settings::AccountEntry,
        force: bool,
    ) -> Result<AuthSession, ServiceError> {
        let _one_at_a_time = self.auth_lock.lock().await;
        let key = entry.key();
        let profile_id = entry
            .profile_id()
            .map_err(|_| ServiceError::SignInRequired(entry.name.clone()))?;
        let stored = StoredLogin::load(self.credentials.as_ref(), &key)?;
        let Some(stored) = stored else {
            self.flag_sign_in(&key).await;
            return Err(ServiceError::SignInRequired(entry.name.clone()));
        };
        if !force && stored.expires_at > now() + TOKEN_MARGIN_SECONDS {
            return Ok(AuthSession::online(
                &entry.name,
                profile_id,
                &stored.access_token,
            ));
        }
        let client = MicrosoftClient::new(&self.transport);
        let oauth = match client.refresh(&Secret::new(stored.refresh_token)).await {
            Ok(oauth) => oauth,
            Err(AuthError::SignInRequired) => {
                self.flag_sign_in(&key).await;
                return Err(ServiceError::SignInRequired(entry.name.clone()));
            }
            Err(error) => return Err(error.into()),
        };
        // The refresh token has rotated: keep the new one before anything else.
        StoredLogin {
            refresh_token: oauth.refresh_token.expose().to_owned(),
            access_token: stored.access_token,
            expires_at: stored.expires_at,
        }
        .save(self.credentials.as_ref(), &key)?;
        let login = client.minecraft_login(&oauth).await?;
        if login.profile_id != profile_id {
            // Another Minecraft profile than the one this account was added
            // for: do not play as someone else.
            self.flag_sign_in(&key).await;
            return Err(ServiceError::SignInRequired(entry.name.clone()));
        }
        StoredLogin {
            refresh_token: oauth.refresh_token.expose().to_owned(),
            access_token: login.access_token.expose().to_owned(),
            expires_at: login.expires_at,
        }
        .save(self.credentials.as_ref(), &key)?;
        // A renamed profile shows its new name.
        if login.profile_name != entry.name {
            let _ = self
                .settings
                .lock()
                .await
                .sign_in_microsoft(login.profile_id, &login.profile_name);
        }
        Ok(AuthSession::online(
            &login.profile_name,
            login.profile_id,
            login.access_token.expose(),
        ))
    }

    async fn flag_sign_in(&self, key: &str) {
        // Best effort: the launch is already failing with a clearer error.
        let _ = self.settings.lock().await.set_needs_sign_in(key, true);
    }

    /// Signs a Microsoft account in with the device code flow. `on_code` is
    /// given the code to show the person; this then waits (polling at the
    /// service's pace) until they finish in the browser, declines, the code
    /// runs out, or `cancel` fires. On success the account is added (or
    /// updated, when its profile is already known) and selected if first, its
    /// secrets go to the credential store, and its key and name are returned.
    pub async fn microsoft_sign_in(
        &self,
        on_code: impl FnOnce(&DeviceCode) + Send,
        cancel: CancellationToken,
    ) -> Result<(String, String), ServiceError> {
        // Fail before showing a code that could never be kept.
        self.credentials.get(CREDENTIAL_PROBE)?;
        let client = MicrosoftClient::new(&self.transport);
        let code = client.request_device_code().await?;
        on_code(&code);
        let oauth = client.wait_for_sign_in(&code, &cancel).await?;
        let login = tokio::select! {
            () = cancel.cancelled() => return Err(AuthError::Cancelled.into()),
            login = client.minecraft_login(&oauth) => login?,
        };
        let key = format!("msa:{}", login.profile_id.compact());
        StoredLogin {
            refresh_token: oauth.refresh_token.expose().to_owned(),
            access_token: login.access_token.expose().to_owned(),
            expires_at: login.expires_at,
        }
        .save(self.credentials.as_ref(), &key)?;
        let recorded = self
            .settings
            .lock()
            .await
            .sign_in_microsoft(login.profile_id, &login.profile_name);
        if let Err(error) = recorded {
            // No account entry means no use for the secret.
            let _ = self.credentials.delete(&key);
            return Err(error.into());
        }
        Ok((key, login.profile_name))
    }

    /// Refreshes a Microsoft account's sign-in now (the account's ⋯ menu), so
    /// a problem shows before the next launch rather than during it.
    pub async fn refresh_account(&self, key: &str) -> Result<(), ServiceError> {
        let entry = self
            .settings
            .lock()
            .await
            .get()
            .accounts
            .iter()
            .find(|entry| entry.key() == key)
            .cloned()
            .ok_or_else(|| ServiceError::Settings(SettingsError::UnknownAccount(key.to_owned())))?;
        if entry.kind != AccountKind::Microsoft {
            return Ok(());
        }
        self.microsoft_session(&entry, true).await?;
        // A good refresh clears an earlier "must sign in again".
        self.settings.lock().await.set_needs_sign_in(key, false)?;
        Ok(())
    }

    /// What the launcher window does when this instance's game runs: the
    /// instance's own choice, else the launcher preference.
    pub async fn after_launch_for(&self, id: &str) -> crate::tuning::AfterLaunch {
        let own = self
            .store
            .lock()
            .await
            .get(id)
            .and_then(|record| record.settings.launch.after_launch);
        match own {
            Some(own) => own,
            None => self.settings.lock().await.get().preferences.after_launch,
        }
    }

    /// The instance the launcher plays by default. A saved id whose instance
    /// is gone reads as `None`; the caller picks another and saves it.
    pub async fn current_instance(&self) -> Option<String> {
        let saved = self.settings.lock().await.get().current_instance.clone()?;
        let exists = self.store.lock().await.get(&saved).is_some();
        exists.then_some(saved)
    }

    /// Remembers the current instance. It must exist.
    pub async fn set_current_instance(&self, id: &str) -> Result<(), ServiceError> {
        if self.store.lock().await.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.to_owned()));
        }
        Ok(self
            .settings
            .lock()
            .await
            .set_current_instance(Some(id.to_owned()))?)
    }

    pub async fn set_preferences(
        &self,
        preferences: crate::tuning::Preferences,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_preferences(preferences)?)
    }

    pub async fn set_launch_defaults(
        &self,
        launch: crate::tuning::LaunchTuning,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_launch_defaults(launch)?)
    }

    pub async fn set_download_concurrency(&self, count: Option<u32>) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_download_concurrency(count)?)
    }

    /// Turns a Java installation off (or back on) by its home folder.
    pub async fn set_java_disabled(&self, home: &Path, disabled: bool) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        let mut homes = settings.get().disabled_java.clone();
        homes.retain(|saved| saved != home);
        if disabled {
            homes.push(home.to_owned());
        }
        Ok(settings.set_disabled_java(homes)?)
    }

    /// Adds the Java the person chose: its `java` executable, its home
    /// folder, or a folder of installations. The folder joins the extra
    /// search folders so it is found again later.
    pub async fn add_java(&self, chosen: &Path) -> Result<crate::java::JavaRuntime, ServiceError> {
        let mut candidates = vec![chosen.to_owned()];
        if chosen.is_file()
            && let Some(home) = chosen.parent().and_then(Path::parent)
        {
            candidates.insert(0, home.to_owned());
        }
        for candidate in candidates {
            let probe = candidate.clone();
            let found = tokio::task::spawn_blocking(move || JavaLocator::new([probe]).discover())
                .await
                .unwrap_or_default();
            let Some(runtime) = found.into_iter().next() else {
                continue;
            };
            let mut settings = self.settings.lock().await;
            let mut roots = settings.get().extra_java_roots.clone();
            if !roots.contains(&candidate) {
                roots.push(candidate);
                settings.set_java_roots(roots)?;
            }
            return Ok(runtime);
        }
        Err(ServiceError::NoJavaAt(chosen.to_owned()))
    }

    pub async fn set_java_roots(&self, roots: Vec<PathBuf>) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_java_roots(roots)?)
    }

    pub async fn set_mirrors(
        &self,
        mirrors: Vec<crate::settings::MirrorRule>,
        prefer: bool,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_mirrors(mirrors, prefer)?)
    }

    pub async fn set_default_memory(
        &self,
        min: Option<u32>,
        max: Option<u32>,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_memory(min, max)?)
    }

    /// Saves overrides after checking their effective combination with current defaults.
    /// `None` clears an override and resumes inheritance.
    pub async fn update_instance_settings(
        &self,
        id: &str,
        settings: InstanceSettings,
    ) -> Result<(), ServiceError> {
        let defaults = self.settings.lock().await;
        if let Some(java) = settings.java_path.as_deref()
            && !java.exists()
        {
            return Err(ServiceError::NoJavaAt(java.to_owned()));
        }
        validate_memory(settings.min_memory_mb, settings.max_memory_mb)?;
        validate_memory(
            settings
                .min_memory_mb
                .or(defaults.get().default_min_memory_mb),
            settings
                .max_memory_mb
                .or(defaults.get().default_max_memory_mb),
        )?;
        Ok(self.store.lock().await.update_settings(id, settings)?)
    }

    /// Deletes an instance so that a failure or crash at any step can be
    /// undone or finished: journal, move the profile aside, commit the library,
    /// then remove the moved files. `meta/` is untouched because other
    /// instances may share it. If only the final cleanup fails the deletion
    /// still stands and the leftovers are removed at the next start.
    pub async fn delete_instance(&self, id: &str) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let record = self
            .store
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?;
        let layout = self.layout.clone();
        let deletion = {
            let record = record.clone();
            blocking(move || {
                let mut deletion = Deletion::begin(&layout, &record, now())?;
                match deletion.quarantine() {
                    Ok(()) => Ok(deletion),
                    Err(error) => {
                        deletion.discard();
                        Err(error)
                    }
                }
            })
            .await?
        };
        let committed = self.store.lock().await.remove(id);
        if let Err(error) = committed {
            let restored = blocking(move || deletion.rollback()).await;
            // A failed restore keeps the journal; the library error stays the cause.
            drop(restored);
            return Err(error.into());
        }
        // The deletion is committed; leftovers are retried by recovery.
        let _ = blocking(move || deletion.finish()).await;
        Ok(())
    }

    /// Continue / Recent / Needs Attention for Home.
    pub async fn home(&self) -> (Vec<InstanceRecord>, HomeSummary) {
        let instances = self.store.lock().await.instances().to_vec();
        let (settings, runtimes) = self.environment().await;
        let mut recent: Vec<&InstanceRecord> = instances.iter().collect();
        recent.sort_by_key(|record| std::cmp::Reverse(record.last_played));
        let mut problems: BTreeMap<String, Vec<Problem>> = BTreeMap::new();
        for record in recent.into_iter().take(DIAGNOSED_LIMIT) {
            let found = inspect_instance(&self.layout, &settings, &runtimes, record).await;
            problems.insert(record.id.clone(), found);
        }
        let summary = summarize(&instances, &problems, RECENT_LIMIT);
        (instances, summary)
    }

    /// Creates an instance. A missing game version means the newest release;
    /// a missing loader version means the recommended one for that game.
    pub async fn create_instance(
        &self,
        name: &str,
        game_version: Option<&str>,
        loader: Loader,
        loader_version: Option<&str>,
    ) -> Result<InstanceRecord, ServiceError> {
        if !LAUNCHABLE_LOADERS.contains(&loader) {
            return Err(ServiceError::Launch(LaunchServiceError::LoaderUnsupported(
                loader,
            )));
        }
        let chain = self.chain().await?;
        let game_version = match game_version {
            Some(version) => version.to_owned(),
            None => VersionCatalog::fetch(&self.transport, &chain)
                .await
                .map_err(|error| ServiceError::Remote(error.to_string()))?
                .latest_release()
                .map(|entry| entry.id().to_owned())
                .ok_or_else(|| ServiceError::Remote("the catalog lists no release".to_owned()))?,
        };
        let loader_version = match (loader, loader_version) {
            (Loader::Vanilla, _) => None,
            (_, Some(version)) => Some(version.to_owned()),
            (_, None) => {
                let url = loader::versions_url(loader, &game_version)
                    .ok_or_else(|| ServiceError::Remote("no loader source".to_owned()))?;
                let sources = chain.candidates(&url);
                let versions = loader::fetch_versions(&self.transport, &sources)
                    .await
                    .map_err(|error| ServiceError::Remote(error.to_string()))?;
                let chosen = loader::recommended(&versions).ok_or_else(|| {
                    ServiceError::Remote(format!("{game_version} has no {loader:?} version"))
                })?;
                Some(chosen.version.clone())
            }
        };
        let mut store = self.store.lock().await;
        let record = store.create(
            NewInstance {
                name: name.to_owned(),
                game_version,
                loader,
                loader_version,
            },
            now(),
        )?;
        Ok(record.clone())
    }

    /// Every game version the catalog lists, newest first, for choosing one
    /// (IA P-VERSION). Read fresh each time; a failure is reported, never
    /// replaced by a guess (L-LIB-02).
    pub async fn game_versions(&self) -> Result<Vec<crate::catalog::CatalogEntry>, ServiceError> {
        let chain = self.chain().await?;
        let catalog = VersionCatalog::fetch(&self.transport, &chain)
            .await
            .map_err(|error| ServiceError::Remote(error.to_string()))?;
        Ok(catalog.entries().to_vec())
    }

    /// The versions of `loader` available for `game_version`, newest first
    /// (IA P-LOADER-VERSION). Vanilla has none.
    pub async fn loader_versions(
        &self,
        loader: Loader,
        game_version: &str,
    ) -> Result<Vec<loader::LoaderVersion>, ServiceError> {
        let chain = self.chain().await?;
        let remote = |error: crate::fetch::FetchError| ServiceError::Remote(error.to_string());
        let decode = |error: loader::LoaderError| ServiceError::Remote(error.to_string());
        match loader {
            Loader::Vanilla => Ok(Vec::new()),
            Loader::Fabric | Loader::Quilt => {
                let url = loader::versions_url(loader, game_version)
                    .ok_or_else(|| ServiceError::Remote("no loader source".to_owned()))?;
                loader::fetch_versions(&self.transport, &chain.candidates(&url))
                    .await
                    .map_err(decode)
            }
            Loader::Forge => {
                let metadata = fetch_document(
                    &self.transport,
                    &chain.candidates(forge_meta::FORGE_METADATA_URL),
                )
                .await
                .map_err(remote)?;
                // Promotions only mark the recommended build; the list stands without them.
                let promotions = fetch_document(
                    &self.transport,
                    &chain.candidates(forge_meta::FORGE_PROMOTIONS_URL),
                )
                .await
                .ok();
                forge_meta::forge_versions(&metadata, promotions.as_deref(), game_version)
                    .map_err(decode)
            }
            Loader::NeoForge => {
                let modern = fetch_document(
                    &self.transport,
                    &chain.candidates(forge_meta::NEOFORGE_VERSIONS_URL),
                )
                .await
                .map_err(remote)?;
                let legacy = if game_version == forge_meta::NEOFORGE_LEGACY_GAME {
                    Some(
                        fetch_document(
                            &self.transport,
                            &chain.candidates(forge_meta::NEOFORGE_LEGACY_VERSIONS_URL),
                        )
                        .await
                        .map_err(remote)?,
                    )
                } else {
                    None
                };
                forge_meta::neoforge_versions(&modern, legacy.as_deref(), game_version)
                    .map_err(decode)
            }
        }
    }

    /// Everything a launch or an install needs about one instance, captured
    /// once: the record, effective settings and where to find the release.
    async fn launch_request(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        let (record, directories) = {
            let store = self.store.lock().await;
            let record = store
                .get(id)
                .cloned()
                .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?;
            let directories = store.directories(&record);
            (record, directories)
        };
        self.request_for(record, directories, quick_play, cancel)
            .await
    }

    /// The request to start (or install) `record`, which need not be what the
    /// library stores yet: a version change prepares its new combination
    /// before committing it.
    async fn request_for(
        &self,
        record: InstanceRecord,
        directories: crate::launch::LaunchDirectories,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        let own_quick_play = record.settings.launch.quick_play.clone();
        let chain = self.chain().await?;
        let (settings, runtimes) = self.environment().await;
        validate_memory(
            record
                .settings
                .min_memory_mb
                .or(settings.default_min_memory_mb),
            record
                .settings
                .max_memory_mb
                .or(settings.default_max_memory_mb),
        )?;

        // Releases already on disk launch without the network.
        let on_disk = |release: &str| {
            directories
                .versions()
                .join(release)
                .join(format!("{release}.json"))
                .is_file()
        };
        let vanilla_ready = on_disk(&record.game_version);
        let all_ready =
            vanilla_ready && record.release_id().is_some_and(|release| on_disk(&release));
        let manifest_url = if all_ready {
            None
        } else {
            let fetched = tokio::select! {
                () = cancel.cancelled() => return Err(ServiceError::Cancelled),
                fetched = VersionCatalog::fetch(&self.transport, &chain) => fetched,
            };
            match fetched {
                Ok(catalog) => catalog
                    .get(&record.game_version)
                    .map(|entry| entry.manifest_url().to_owned()),
                Err(_) if vanilla_ready => None,
                Err(error) => return Err(ServiceError::Remote(error.to_string())),
            }
        };
        let loader_profile_url = record.loader_version.as_deref().and_then(|version| {
            loader::profile_url(record.loader, &record.game_version, version).or_else(|| {
                crate::forge_install::installer_url(record.loader, &record.game_version, version)
            })
        });
        let session = self.session_for(&settings).await?;
        let request = LaunchRequest {
            instance: record,
            directories,
            session,
            manifest_url,
            loader_profile_url,
            runtimes,
            default_max_memory_mb: settings.default_max_memory_mb,
            default_min_memory_mb: settings.default_min_memory_mb,
            tuning: settings.launch.clone(),
            download_concurrency: settings.download_concurrency,
            // `None` asks for the instance's own choice; `Some(None)` for the menu.
            quick_play: quick_play.unwrap_or(own_quick_play),
        };
        Ok((request, chain))
    }

    /// Prepares and starts an instance, recording the session afterwards.
    pub async fn launch(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        self.launch_with(id, None, updates, cancel).await
    }

    /// Starts the game and goes straight into a saved world. The instance's
    /// own direct-start choice is not changed.
    pub async fn launch_world(
        &self,
        id: &str,
        world: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let target = QuickPlay::World(world.to_owned());
        if crate::tuning::quick_play_problem(&target).is_some() {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                "the world name is not a valid save folder".to_owned(),
            )));
        }
        self.launch_with(id, Some(Some(target)), updates, cancel)
            .await
    }

    async fn launch_with(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let started = now();
        if self.store.lock().await.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.to_owned()));
        }
        // Dropped on every way out; only the launcher dying leaves it behind.
        let _marker = SessionMarker::place(&self.layout, id, started)?;
        let prepared = self.launch_request(id, quick_play, &cancel).await;
        let (request, chain) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return Err(self.note_attempt(id, started, error)),
        };
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let exit = match launcher.launch(request, updates, cancel).await {
            Ok(exit) => exit,
            Err(error) => return Err(self.note_attempt(id, started, error.into())),
        };
        let mut store = self.store.lock().await;
        store.mark_installed(id, true)?;
        finish_session(&mut store, id, started, &exit)?;
        Ok(exit)
    }

    /// Writes a launch that never produced a process to the instance history,
    /// then hands the error back. An unknown instance has no history to write.
    fn note_attempt(&self, id: &str, started: u64, error: ServiceError) -> ServiceError {
        let outcome = match &error {
            ServiceError::NoSuchInstance(_) => return error,
            ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled) => {
                SessionOutcome::Cancelled
            }
            _ => SessionOutcome::FailedToPrepare,
        };
        // History is a record, not part of the result: a write failure must
        // not replace the error the user needs to see.
        let _ = record_attempt(self.layout.root(), id, started, outcome);
        error
    }

    /// Installs an instance's game files without starting it. The task shows
    /// in Activity and `cancel_task` can stop it. `installed` becomes true only
    /// when the whole install finished; a failure or cancel leaves it false.
    pub async fn install_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let (request, chain) = self.launch_request(id, Some(None), &cancel).await?;
        let task = self
            .begin(
                TaskCategory::Install,
                format!("安装 {}", request.instance.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::InstallInstance {
                instance: id.to_owned(),
            });
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let mut result =
            Self::install_tracked(&self.board, task.id, &launcher, request, updates, cancel).await;
        if result.is_ok() {
            result = self
                .store
                .lock()
                .await
                .mark_installed(id, true)
                .map_err(ServiceError::from);
        }
        self.end(task, &result);
        result
    }

    /// Checks every game file of an instance against its release and fetches
    /// whatever is missing or damaged again, rebuilding the extracted natives
    /// and (for Forge and NeoForge) the patched client. Files that are already
    /// right are not touched, so an intact game does nothing and needs no
    /// network. Worlds, mods and settings are never touched.
    pub async fn repair_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let (request, chain) = self.launch_request(id, Some(None), &cancel).await?;
        let task = self
            .begin(
                TaskCategory::Repair,
                format!("修复 {}", request.instance.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::RepairInstance {
                instance: id.to_owned(),
            });
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let mut result =
            Self::install_tracked(&self.board, task.id, &launcher, request, updates, cancel).await;
        if result.is_ok() {
            result = self
                .store
                .lock()
                .await
                .mark_installed(id, true)
                .map_err(ServiceError::from);
        }
        if result.is_ok() {
            self.record_change(id, ChangeKind::Repaired, "game files");
        }
        self.end(task, &result);
        result
    }

    /// Changes the game version and/or the loader of an instance.
    ///
    /// The new combination is prepared first: its shared files are fetched
    /// into `meta/` under their own release folder (nothing another instance
    /// uses is rewritten) and a Forge or NeoForge installer runs. Only when
    /// that has fully succeeded is the instance's record changed, in one
    /// library transaction. Any failure or cancel before the commit leaves
    /// the old combination exactly as it was and launchable; a failed commit
    /// does too. Worlds, mods and settings are not touched: mods built for
    /// another game version or loader may no longer work, which is why the
    /// caller offers a snapshot first. Refused while the instance launches or
    /// has another write in progress.
    pub async fn change_runtime(
        &self,
        id: &str,
        game_version: &str,
        loader: Loader,
        loader_version: Option<&str>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        if !LAUNCHABLE_LOADERS.contains(&loader) {
            return Err(ServiceError::Launch(LaunchServiceError::LoaderUnsupported(
                loader,
            )));
        }
        let game_version = game_version.trim();
        let loader_version = if loader == Loader::Vanilla {
            None
        } else {
            Some(
                loader_version
                    .map(str::trim)
                    .filter(|version| !version.is_empty())
                    .ok_or(ServiceError::Launch(
                        LaunchServiceError::LoaderVersionMissing,
                    ))?
                    .to_owned(),
            )
        };
        if game_version.is_empty() {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                "the game version is empty".to_owned(),
            )));
        }
        let current = self.instance(id).await?;
        if current.game_version == game_version
            && current.loader == loader
            && current.loader_version == loader_version
        {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                "that is already this game's version and loader".to_owned(),
            )));
        }
        let mut candidate = current.clone();
        candidate.game_version = game_version.to_owned();
        candidate.loader = loader;
        candidate.loader_version = loader_version.clone();
        let directories = self.layout.launch_directories(&candidate);
        let (request, chain) = self
            .request_for(candidate, directories, Some(None), &cancel)
            .await?;
        let task = self
            .begin(
                TaskCategory::Update,
                format!("更换 {} 的版本", current.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::ChangeRuntime {
                instance: id.to_owned(),
                game_version: game_version.to_owned(),
                loader,
                loader_version: loader_version.clone(),
            });
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let mut result =
            Self::install_tracked(&self.board, task.id, &launcher, request, updates, cancel).await;
        if result.is_ok() {
            result = self
                .store
                .lock()
                .await
                .set_runtime(id, game_version, loader, loader_version.as_deref())
                .map_err(ServiceError::from);
        }
        self.end(task, &result);
        result?;
        self.record_change(
            id,
            ChangeKind::GameVersionChanged,
            &format!(
                "{} {} → {} {}",
                current.game_version,
                loader_text(current.loader, current.loader_version.as_deref()),
                game_version,
                loader_text(loader, loader_version.as_deref()),
            ),
        );
        self.instance(id).await
    }

    async fn modrinth(&self) -> Result<ModrinthClient<T>, ServiceError> {
        Ok(ModrinthClient::new(self.transport.clone()).with_sources(self.chain().await?))
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchPage, ServiceError> {
        self.modrinth()
            .await?
            .search(query)
            .await
            .map_err(|error| ServiceError::Remote(error.to_string()))
    }

    /// Categories and game versions for the Discover filters. Fetched once and
    /// kept for the life of the service; a failure is not remembered.
    pub async fn discover_filters(&self) -> Result<DiscoverFilters, ServiceError> {
        let mut cached = self.filters.lock().await;
        if let Some(filters) = cached.as_ref() {
            return Ok(filters.clone());
        }
        let client = self.modrinth().await?;
        let (categories, game_versions) = tokio::join!(client.categories(), client.game_versions());
        let filters = DiscoverFilters {
            categories: categories.map_err(|error| ServiceError::Remote(error.to_string()))?,
            game_versions: game_versions
                .map_err(|error| ServiceError::Remote(error.to_string()))?,
        };
        *cached = Some(filters.clone());
        Ok(filters)
    }

    /// Everything the detail window shows about a project.
    pub async fn project_detail(&self, project: &str) -> Result<ProjectDetail, ServiceError> {
        let client = self.modrinth().await?;
        let (details, versions, owner) = tokio::join!(
            client.project(project),
            client.versions(project),
            client.owner(project)
        );
        let remote =
            |error: crate::discover::DiscoverError| ServiceError::Remote(error.to_string());
        let mut versions = versions.map_err(remote)?;
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(ProjectDetail {
            project: details.map_err(remote)?,
            versions,
            // The owner is decoration; not knowing it must not hide the page.
            owner: owner.ok().flatten(),
        })
    }

    /// Installs one specific version of a mod, resource pack or shader into
    /// an instance. Refused when it does not run on the instance.
    pub async fn install_version(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.install_from(instance_id, kind, project, Some(version_id), cancel)
            .await
    }

    /// Installs the newest compatible file of a mod, resource pack or shader
    /// into an instance and returns its file name.
    pub async fn install_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        self.install_from(instance_id, kind, project, None, cancel)
            .await
    }

    /// How much of the shared game files no game uses (ADR 0017). Reads only;
    /// walks the disk.
    pub async fn reclaimable(&self) -> Result<crate::reclaim::Reclaimable, ServiceError> {
        let instances = self.store.lock().await.instances().to_vec();
        let layout = self.layout.clone();
        blocking_value(move || crate::reclaim::scan(&layout, &instances))
            .await?
            .map_err(|error| ServiceError::Install(error.to_string()))
    }

    /// Removes the shared game files no game uses and returns the bytes freed.
    /// Refuses while anything is installing, repairing, launching or otherwise
    /// in use, then looks again before deleting, so what was shown a moment
    /// ago cannot remove something a game has started to need.
    pub async fn reclaim(&self) -> Result<u64, ServiceError> {
        let busy = {
            let operations = self
                .operations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            operations.iter().next().cloned()
        };
        if let Some(busy) = busy {
            return Err(ServiceError::InstanceBusy(busy));
        }
        // Held to the end: nothing else can start meanwhile.
        let _lease = self.reserve_instance("(shared files)")?;
        let found = self.reclaimable().await?;
        let layout = self.layout.clone();
        blocking_value(move || crate::reclaim::remove(&layout, &found))
            .await?
            .map_err(|error| ServiceError::Install(error.to_string()))
    }

    /// The games in a folder made by another launcher (ADR 0016). Reads only.
    pub async fn detect_games(
        &self,
        folder: &Path,
    ) -> Result<Vec<crate::import_game::FoundGame>, ServiceError> {
        let folder = folder.to_owned();
        blocking_value(move || crate::import_game::detect(&folder))
            .await?
            .map_err(|error| ServiceError::Install(error.to_string()))
    }

    /// Makes a new game of one found by [`Self::detect_games`]: its player
    /// files are copied (the source is never changed), and its game files are
    /// installed on first launch. Built whole before it appears.
    pub async fn import_game(
        &self,
        game: crate::import_game::FoundGame,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let task = self.begin(
            TaskCategory::Install,
            format!("导入 {}", game.name),
            None,
            Some(&cancel),
        );
        let result = self.import_game_inner(game, cancel).await;
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    async fn import_game_inner(
        &self,
        game: crate::import_game::FoundGame,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        if game.loader != Loader::Vanilla
            && game.loader_version.as_deref().is_none_or(str::is_empty)
        {
            return Err(ServiceError::Install(
                "the game's loader version could not be told".to_owned(),
            ));
        }
        let id = self.store.lock().await.suggest_id(&game.name)?;
        let staged = Staged::begin(&self.layout, "import", &id)?;
        let (source, into, token) = (game.clone(), staged.game_dir(), cancel.clone());
        let copied =
            blocking_value(move || crate::import_game::copy_game_files(&source, &into, &token))
                .await
                .and_then(|result| {
                    result.map_err(|error| match error {
                        crate::import_game::ImportError::Io(io)
                            if io.kind() == std::io::ErrorKind::Interrupted =>
                        {
                            ServiceError::Cancelled
                        }
                        other => ServiceError::Install(other.to_string()),
                    })
                });
        if let Err(error) = copied {
            staged.discard();
            return Err(error);
        }
        if let Err(error) = staged.mark_ready() {
            staged.discard();
            return Err(error.into());
        }
        let created = self
            .store
            .lock()
            .await
            .create_as(
                &id,
                NewInstance {
                    name: game.name.clone(),
                    game_version: game.game_version.clone(),
                    loader: game.loader,
                    loader_version: game.loader_version.clone(),
                },
                InstanceSettings::default(),
                false,
                now(),
            )
            .cloned();
        let record = match created {
            Ok(record) => record,
            Err(error) => {
                staged.discard();
                return Err(error.into());
            }
        };
        if let Err(error) = staged.publish(&self.layout) {
            let _ = self.store.lock().await.remove(&record.id);
            staged.discard();
            return Err(error.into());
        }
        Ok(record)
    }

    /// Saves the whole game (mods, config, worlds, options; not logs) as one
    /// file at `destination` (ADR 0015) and returns its size. Takes the
    /// instance lease so the files hold still. An Activity task; cancelling
    /// it leaves no file.
    pub async fn backup_instance(
        &self,
        id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<u64, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let record = self.instance(id).await?;
        let task = self
            .begin(
                TaskCategory::Install,
                format!("备份 {}", record.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::BackupInstance {
                instance: id.to_owned(),
                path: destination.display().to_string(),
            });
        let meta = crate::backup::BackupMeta {
            format: 1,
            name: record.name.clone(),
            game_version: record.game_version.clone(),
            loader: record.loader,
            loader_version: record.loader_version.clone(),
            created: now(),
            settings: record.settings.clone(),
        };
        let (game_dir, destination, token) =
            (self.layout.game(id), destination.to_owned(), cancel.clone());
        let result = tokio::task::spawn_blocking(move || {
            crate::backup::write(&game_dir, &meta, &destination, &token)
        })
        .await
        .map_err(std::io::Error::other)
        .map_err(ServiceError::from)
        .and_then(|written| written.map_err(backup_failure));
        self.end(task, &result);
        result
    }

    /// Makes a new game out of a backup (ADR 0015). The game is built whole
    /// before it appears; nothing existing is touched. Its game files are
    /// installed on first launch like any new game.
    pub async fn restore_backup(
        &self,
        archive: &Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let label = archive
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let task = self
            .begin(
                TaskCategory::Install,
                format!("恢复备份 {label}"),
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::RestoreBackup {
                path: archive.display().to_string(),
            });
        let result = self.restore_backup_inner(archive, cancel).await;
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    async fn restore_backup_inner(
        &self,
        archive: &Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let source = archive.to_owned();
        let meta = blocking_value({
            let source = source.clone();
            move || crate::backup::read_meta(&source)
        })
        .await?
        .map_err(backup_failure)?;
        let (id, name) = {
            let store = self.store.lock().await;
            let taken = store
                .instances()
                .iter()
                .any(|record| record.name.trim() == meta.name.trim());
            let name = if taken {
                format!("{}（恢复）", meta.name.trim())
            } else {
                meta.name.trim().to_owned()
            };
            (store.suggest_id(&name)?, name)
        };
        let staged = Staged::begin(&self.layout, "import", &id)?;
        let (into, token) = (staged.game_dir(), cancel.clone());
        let extracted = blocking_value(move || crate::backup::restore(&source, &into, &token))
            .await
            .and_then(|result| result.map_err(backup_failure));
        if let Err(error) = extracted {
            staged.discard();
            return Err(error);
        }
        if let Err(error) = staged.mark_ready() {
            staged.discard();
            return Err(error.into());
        }
        let mut settings = meta.settings.clone();
        // A Java path belongs to the machine the backup came from.
        settings.java_path = None;
        let created = self
            .store
            .lock()
            .await
            .create_as(
                &id,
                NewInstance {
                    name,
                    game_version: meta.game_version.clone(),
                    loader: meta.loader,
                    loader_version: meta.loader_version.clone(),
                },
                settings,
                false,
                now(),
            )
            .cloned();
        let record = match created {
            Ok(record) => record,
            Err(error) => {
                staged.discard();
                return Err(error.into());
            }
        };
        if let Err(error) = staged.publish(&self.layout) {
            let _ = self.store.lock().await.remove(&record.id);
            staged.discard();
            return Err(error.into());
        }
        Ok(record)
    }

    /// Downloads Mojang's Java runtime for this system into the launcher's own
    /// `runtimes` folder (ADR 0014) and returns it. `required` is the Java
    /// major a game needs; without it, the recommended one. Nothing is touched
    /// until every file has arrived and checked out; an installed runtime is
    /// returned as it is. It is an Activity task and can be cancelled.
    pub async fn install_java(
        &self,
        required: Option<u32>,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        let task = self
            .begin(
                TaskCategory::Install,
                match required {
                    Some(major) => format!("安装 Java {major}"),
                    None => "安装 Java".to_owned(),
                },
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::InstallJava { major: required });
        let result = self
            .install_java_inner(task.id, required, Self::java_manifest_allowed(), cancel)
            .await;
        self.end(task, &result);
        result
    }

    /// Which addresses a runtime file may come from; tests widen it.
    fn java_manifest_allowed() -> fn(&str) -> bool {
        #[cfg(test)]
        {
            |_| true
        }
        #[cfg(not(test))]
        {
            crate::java_runtime::is_trusted_source
        }
    }

    async fn install_java_inner(
        &self,
        task: u64,
        required: Option<u32>,
        allow: fn(&str) -> bool,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        use crate::java_runtime as runtime;
        let fail = |why: String| ServiceError::Install(why);
        let host = crate::environment::HostProfile::current();
        let platform = runtime::platform_key(&host)
            .ok_or_else(|| fail(runtime::JavaRuntimeError::UnsupportedPlatform.to_string()))?;
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let fetch = |address: String| {
            let candidates = chain.candidates(&address);
            let transport = self.transport.clone();
            async move {
                crate::fetch::fetch_document(&transport, &candidates)
                    .await
                    .map_err(|error| ServiceError::Remote(error.to_string()))
            }
        };
        let index = read_unless_cancelled(&cancel, fetch(runtime::INDEX_URL.to_owned())).await?;
        let index = String::from_utf8_lossy(&index).into_owned();
        let choice = runtime::choose_component(&index, platform, required)
            .map_err(|error| fail(error.to_string()))?;

        let runtimes = self.layout.runtimes();
        let folder = runtimes.join(&choice.component);
        if let Some(found) = Self::runtime_in(&folder) {
            return Ok(found);
        }
        let manifest = read_unless_cancelled(&cancel, fetch(choice.manifest.url.clone())).await?;
        let entries = runtime::parse_manifest(&String::from_utf8_lossy(&manifest), allow)
            .map_err(|error| fail(error.to_string()))?;

        let staging = runtimes.join(format!(".{}.installing", choice.component));
        let prepared = {
            let staging = staging.clone();
            let layout: Vec<(String, bool)> = entries
                .iter()
                .filter_map(|(path, entry)| match entry {
                    runtime::Entry::Directory => Some((path.clone(), true)),
                    _ => None,
                })
                .collect();
            blocking(move || {
                let _ = std::fs::remove_dir_all(&staging);
                std::fs::create_dir_all(&staging)?;
                for (path, _) in layout {
                    std::fs::create_dir_all(runtime::place(&staging, &path))?;
                }
                Ok(())
            })
            .await
        };
        prepared?;

        let mut requests = Vec::new();
        for (path, entry) in &entries {
            if let runtime::Entry::File { download, .. } = entry {
                let mut request = TransferRequest::new(
                    format!("java:{}:{path}", choice.component),
                    chain.candidates(&download.url),
                    runtime::place(&staging, path),
                )
                .map_err(|error| fail(error.to_string()))?;
                if download.size > 0 {
                    request = request.expect_size(download.size);
                }
                request = request
                    .expect_sha1(&download.sha1)
                    .map_err(|error| fail(error.to_string()))?;
                requests.push(request);
            }
        }
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| fail(error.to_string()))?;
        let report = self
            .metered(
                task,
                engine.subscribe(),
                engine.transfer_batch(requests, cancel.clone()),
            )
            .await;
        if report.failed() > 0 {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(if cancel.is_cancelled() {
                ServiceError::Cancelled
            } else {
                fail(format!(
                    "{} Java files could not be fetched",
                    report.failed()
                ))
            });
        }

        let publish = {
            let (staging, folder) = (staging.clone(), folder.clone());
            blocking(move || {
                for (path, entry) in &entries {
                    let target = runtime::place(&staging, path);
                    match entry {
                        runtime::Entry::File {
                            executable: true, ..
                        } => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                std::fs::set_permissions(
                                    &target,
                                    std::fs::Permissions::from_mode(0o755),
                                )?;
                            }
                        }
                        runtime::Entry::Link { target: to } => {
                            #[cfg(unix)]
                            {
                                if let Some(parent) = target.parent() {
                                    std::fs::create_dir_all(parent)?;
                                }
                                std::os::unix::fs::symlink(to, &target)?;
                            }
                            #[cfg(not(unix))]
                            let _ = to;
                        }
                        _ => {}
                    }
                }
                let _ = std::fs::remove_dir_all(&folder);
                std::fs::rename(&staging, &folder)
            })
            .await
        };
        if let Err(error) = publish {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error.into());
        }
        Self::runtime_in(&folder)
            .ok_or_else(|| fail("the downloaded Java does not start".to_owned()))
    }

    /// The runtime installed in `folder` (directly, or as a macOS bundle).
    fn runtime_in(folder: &Path) -> Option<crate::java::JavaRuntime> {
        [folder.to_path_buf(), folder.join("Contents/Home")]
            .iter()
            .find_map(|home| crate::java::JavaRuntime::inspect(home))
    }

    /// Downloads one version's file to a place the person chose, instead of
    /// into a game. Returns the file's name. A file already there is replaced
    /// (the person was asked by the save dialog).
    pub async fn save_version_as(
        &self,
        project: &str,
        version_id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let task = self
            .begin(
                TaskCategory::Download,
                format!("下载 {project}"),
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::SaveVersion {
                project: project.to_owned(),
                version_id: version_id.to_owned(),
                path: destination.display().to_string(),
            });
        let result = self
            .save_version_inner(task.id, project, version_id, destination, cancel)
            .await;
        self.end(task, &result);
        result
    }

    async fn save_version_inner(
        &self,
        task: u64,
        project: &str,
        version_id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.modrinth()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client
                .versions(project)
                .await
                .map_err(|error| ServiceError::Remote(error.to_string()))
        })
        .await?;
        let version = versions
            .iter()
            .find(|version| version.id == version_id)
            .ok_or(ServiceError::NoCompatibleVersion)?;
        let file = version
            .install_file()
            .ok_or_else(|| ServiceError::Install("this version has no file".to_owned()))?;
        let mut request = TransferRequest::new(
            format!("save:{}:{}", version.project_id, version.id),
            chain.candidates(&file.url),
            destination,
        )
        .map_err(|error| ServiceError::Install(error.to_string()))?;
        if file.size > 0 {
            request = request.expect_size(file.size);
        }
        if let Some(sha1) = &file.sha1 {
            request = request
                .expect_sha1(sha1)
                .map_err(|error| ServiceError::Install(error.to_string()))?;
        }
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        self.metered(task, engine.subscribe(), engine.transfer(request, cancel))
            .await
            .map_err(ServiceError::from)?;
        Ok(file.filename.clone())
    }

    /// What of this kind the game already has, by Modrinth project: the file
    /// and, if a newer compatible version exists, its version id. Files
    /// Modrinth does not know are not listed. Needs the network.
    pub async fn installed_projects(
        &self,
        instance_id: &str,
        kind: ProjectKind,
    ) -> Result<BTreeMap<String, InstalledProject>, ServiceError> {
        let list = self.content_details(instance_id, kind).await?;
        if list.sources_unavailable {
            return Err(ServiceError::Remote(
                "Modrinth could not be asked which files are known".to_owned(),
            ));
        }
        Ok(list
            .entries
            .into_iter()
            .filter_map(|entry| {
                let source = entry.source?;
                Some((
                    source.project_id,
                    InstalledProject {
                        file_name: entry.item.file_name,
                        version_id: source.version_id,
                        update: entry.update.map(|version| version.id),
                    },
                ))
            })
            .collect())
    }

    /// The required dependencies the game lacks (see [`Self::dependency_report`]).
    pub async fn missing_dependencies(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
    ) -> Result<Vec<DependencyNeed>, ServiceError> {
        Ok(self
            .dependency_report(instance_id, kind, project, version_id)
            .await?
            .needs)
    }

    /// What installing the version `install_content` would pick (or
    /// `version_id`) means for the game's other mods: the required
    /// dependencies it lacks (nearest first, up to four levels), the optional
    /// ones it lacks (the version's own only), and installed mods it declares
    /// itself incompatible with. Only mods have any of these. A dependency
    /// with no version for this game is listed with `version: None` so the
    /// person is told instead of left to crash.
    pub async fn dependency_report(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
    ) -> Result<DependencyReport, ServiceError> {
        const DEPTH: usize = 4;
        if kind != ProjectKind::Mod {
            return Ok(DependencyReport::default());
        }
        let record = self.instance(instance_id).await?;
        let client = self.modrinth().await?;
        let remote =
            |error: crate::discover::DiscoverError| ServiceError::Remote(error.to_string());
        let versions = client.versions(project).await.map_err(remote)?;
        let root = match version_id {
            Some(id) => versions
                .iter()
                .find(|version| version.id == id)
                .filter(|version| fits(version, kind, &record.game_version, record.loader)),
            None => pick_version(&versions, kind, &record.game_version, record.loader),
        }
        .ok_or(ServiceError::NoCompatibleVersion)?;

        // What the game already has, by project (files Modrinth does not know
        // cannot be matched, so a dependency they satisfy is offered anyway).
        let game_dir = self.layout.game(instance_id);
        let hashes = blocking(move || {
            let mut hashes = Vec::new();
            for item in scan_content(&game_dir, ProjectKind::Mod).unwrap_or_default() {
                if item.enabled
                    && !item.is_directory
                    && let Ok(hash) =
                        crate::content::sha1_hex(&game_dir.join("mods").join(&item.file_name))
                {
                    hashes.push(hash);
                }
            }
            Ok(hashes)
        })
        .await?;
        let installed: BTreeSet<String> = client
            .identify(&hashes)
            .await
            .map(|found| found.values().map(|v| v.project_id.clone()).collect())
            .unwrap_or_default();
        let mut have = installed.clone();
        have.insert(root.project_id.clone());

        let resolve = |dependency: &crate::discover::Dependency, candidates: &[Version]| {
            dependency
                .version_id
                .as_deref()
                .and_then(|wanted| candidates.iter().find(|v| v.id == wanted))
                .filter(|v| fits(v, kind, &record.game_version, record.loader))
                .or_else(|| pick_version(candidates, kind, &record.game_version, record.loader))
                .cloned()
        };

        let mut found: Vec<(String, Option<Version>)> = Vec::new();
        let mut level: Vec<Version> = vec![root.clone()];
        for _ in 0..DEPTH {
            let mut next = Vec::new();
            for version in &level {
                for dependency in version.required_dependencies() {
                    let Some(dependency_project) = dependency.project_id.clone() else {
                        continue;
                    };
                    if !have.insert(dependency_project.clone()) {
                        continue;
                    }
                    let candidates = client.versions(&dependency_project).await.map_err(remote)?;
                    let chosen = resolve(dependency, &candidates);
                    if let Some(chosen) = &chosen {
                        next.push(chosen.clone());
                    }
                    found.push((dependency_project, chosen));
                }
            }
            if next.is_empty() {
                break;
            }
            level = next;
        }

        // Optional ones of the version itself, and what it cannot live with.
        let mut optional: Vec<(String, Option<Version>)> = Vec::new();
        let mut conflicts: Vec<String> = Vec::new();
        for dependency in &root.dependencies {
            let Some(other) = dependency.project_id.clone() else {
                continue;
            };
            match dependency.kind {
                crate::discover::DependencyKind::Optional if have.insert(other.clone()) => {
                    if let Ok(candidates) = client.versions(&other).await {
                        optional.push((other, resolve(dependency, &candidates)));
                    }
                }
                crate::discover::DependencyKind::Incompatible if installed.contains(&other) => {
                    conflicts.push(other);
                }
                _ => {}
            }
        }

        let ids: Vec<String> = found
            .iter()
            .chain(optional.iter())
            .map(|(id, _)| id.clone())
            .chain(conflicts.iter().cloned())
            .collect();
        let titles: BTreeMap<String, String> = client
            .project_summaries(&ids)
            .await
            .map(|summaries| summaries.into_iter().map(|s| (s.id, s.title)).collect())
            .unwrap_or_default();
        let title_of = |id: &str| titles.get(id).cloned().unwrap_or_else(|| id.to_owned());
        let need = |(project_id, version): (String, Option<Version>)| DependencyNeed {
            title: title_of(&project_id),
            project_id,
            version,
        };
        Ok(DependencyReport {
            needs: found.into_iter().map(need).collect(),
            optional: optional.into_iter().map(need).collect(),
            conflicts: conflicts.iter().map(|id| title_of(id)).collect(),
        })
    }

    async fn install_from(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(instance_id)?;
        let record = self
            .store
            .lock()
            .await
            .get(instance_id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(instance_id.to_owned()))?;
        let task = self
            .begin(
                TaskCategory::Download,
                format!("安装 {project}"),
                Some(instance_id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::InstallContent {
                instance: instance_id.to_owned(),
                kind,
                project: project.to_owned(),
                version: version_id.map(str::to_owned),
            });
        let result = self
            .install_content_inner(task.id, &record, kind, project, version_id, cancel)
            .await;
        self.end(task, &result);
        result
    }

    async fn install_content_inner(
        &self,
        task: u64,
        record: &InstanceRecord,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.modrinth()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client
                .versions(project)
                .await
                .map_err(|error| ServiceError::Remote(error.to_string()))
        })
        .await?;
        let version = match version_id {
            Some(id) => versions
                .iter()
                .find(|version| version.id == id)
                .filter(|version| fits(version, kind, &record.game_version, record.loader)),
            None => pick_version(&versions, kind, &record.game_version, record.loader),
        }
        .ok_or(ServiceError::NoCompatibleVersion)?;
        let game_dir = self.layout.game(&record.id);
        let file = version.install_file().map(|file| file.url.clone());
        let sources = file.map(|url| chain.candidates(&url)).unwrap_or_default();
        let request: TransferRequest = install_request(kind, version, &game_dir, sources)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        let name = request
            .destination()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        // A same-named file with other content is the user's: never replace it
        // silently. Identical content is reused by the transfer.
        crate::content::folder(&game_dir, kind)?;
        let target = request.destination().to_path_buf();
        let expected = request.expected_sha1().map(str::to_owned);
        let in_the_way = blocking(move || {
            if target.symlink_metadata().is_err() {
                return Ok(false);
            }
            let same = target.is_file()
                && expected.is_some_and(|expected| {
                    crate::content::sha1_hex(&target).is_ok_and(|actual| actual == expected)
                });
            Ok(!same)
        })
        .await?;
        if in_the_way {
            return Err(ContentError::Conflict(name).into());
        }
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        self.metered(task, engine.subscribe(), engine.transfer(request, cancel))
            .await
            .map_err(ServiceError::from)?;
        HistoryLog::for_instance(self.layout.root(), &record.id).append(&HistoryEvent::Change {
            at: now(),
            kind: ChangeKind::ContentAdded,
            subject: name.clone(),
        })?;
        Ok(name)
    }

    /// Replaces an installed file with another version of the same project
    /// (IA P-VERSION-SWITCH: "update" and "switch version" are this one
    /// operation). The new file is downloaded and verified first; only then is
    /// the old one removed. A disabled file stays disabled. A different file
    /// already using the new name is never overwritten (`Conflict`).
    pub async fn switch_content_version(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        file_name: &str,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(instance_id)?;
        self.instance(instance_id).await?;
        let task = self
            .begin(
                TaskCategory::Update,
                format!("切换 {file_name} 的版本"),
                Some(instance_id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::SwitchContent {
                instance: instance_id.to_owned(),
                kind,
                file_name: file_name.to_owned(),
                project: project.to_owned(),
                version_id: version_id.to_owned(),
            });
        let result = self
            .switch_inner(
                task.id,
                instance_id,
                kind,
                file_name,
                project,
                version_id,
                cancel,
            )
            .await;
        self.end(task, &result);
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn switch_inner(
        &self,
        task: u64,
        instance_id: &str,
        kind: ProjectKind,
        file_name: &str,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.modrinth()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client
                .versions(project)
                .await
                .map_err(|error| ServiceError::Remote(error.to_string()))
        })
        .await?;
        let version = versions
            .into_iter()
            .find(|version| version.id == version_id)
            .ok_or(ServiceError::NoCompatibleVersion)?;
        let game_dir = self.layout.game(instance_id);
        let folder = crate::content::folder(&game_dir, kind)?;
        let disabled = file_name.ends_with(crate::content::DISABLED_SUFFIX);
        let new_name = version
            .install_file()
            .map(|file| file.filename.clone())
            .ok_or(ServiceError::NoCompatibleVersion)?;
        // Another file already using the new name is the user's.
        let current = file_name
            .strip_suffix(crate::content::DISABLED_SUFFIX)
            .unwrap_or(file_name);
        if new_name != current {
            let taken = [
                new_name.clone(),
                format!("{new_name}{}", crate::content::DISABLED_SUFFIX),
            ]
            .into_iter()
            .any(|name| folder.join(name).symlink_metadata().is_ok());
            if taken {
                return Err(ContentError::Conflict(new_name).into());
            }
        }
        let current_sha1 = {
            let path = folder.join(file_name);
            blocking(move || crate::content::sha1_hex(&path)).await?
        };
        let sources = version
            .install_file()
            .map(|file| chain.candidates(&file.url))
            .unwrap_or_default();
        let update = crate::updates::ContentUpdate {
            kind,
            file_name: file_name.to_owned(),
            current_sha1,
            latest: version,
        };
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        let written = self
            .metered(
                task,
                engine.subscribe(),
                crate::updates::apply(&engine, &update, &game_dir, sources, cancel),
            )
            .await
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        if disabled && written != file_name {
            // A rename: quick enough to do here.
            crate::content::set_enabled(&game_dir, kind, &written, false)?;
        }
        self.record_change(instance_id, ChangeKind::ContentUpdated, &written);
        Ok(written)
    }

    /// Copies local files into an instance (IA P-ADD-FILES): one result per
    /// file, so one refusal does not hide the others. Needs the instance lease.
    pub async fn import_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        files: Vec<std::path::PathBuf>,
    ) -> Result<Vec<(String, Result<String, ContentError>)>, ServiceError> {
        let _lease = self.reserve_instance(instance_id)?;
        self.instance(instance_id).await?;
        let game_dir = self.layout.game(instance_id);
        let results = tokio::task::spawn_blocking(move || {
            files
                .into_iter()
                .map(|file| {
                    let shown = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| file.display().to_string());
                    (shown, crate::content::import(&game_dir, kind, &file))
                })
                .collect::<Vec<_>>()
        })
        .await
        .map_err(std::io::Error::other)?;
        for (_, result) in &results {
            if let Ok(name) = result {
                self.record_change(instance_id, ChangeKind::ContentAdded, name);
            }
        }
        Ok(results)
    }

    /// Every version of a project, newest first, for choosing one to switch
    /// an installed file to (IA P-VERSION-SWITCH).
    pub async fn project_versions(&self, project: &str) -> Result<Vec<Version>, ServiceError> {
        let mut versions = self
            .modrinth()
            .await?
            .versions_with_changelog(project)
            .await
            .map_err(|error| ServiceError::Remote(error.to_string()))?;
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(versions)
    }

    /// Where an instance's game files live, for showing them in the file
    /// manager. Read-only: nothing is created.
    #[must_use]
    pub fn game_dir(&self, id: &str) -> std::path::PathBuf {
        self.layout.game(id)
    }

    /// The installed items of one kind. Reading needs no instance lease.
    pub async fn content(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<Vec<ContentItem>, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || scan_content(&game_dir, kind))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Lists one kind of content with where each file came from (IA
    /// P-CONTENT-ITEM). Files are hashed off the async threads; Modrinth is
    /// asked in three batches (versions by hash, projects, teams) plus one
    /// for the newest compatible versions. If it cannot be reached the list
    /// still comes back, marked `sources_unavailable`.
    pub async fn content_details(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<crate::content_sources::ContentList, ServiceError> {
        let record = self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let files = tokio::task::spawn_blocking(move || {
            let items = scan_content(&game_dir, kind)?;
            let folder = kind.install_folder().unwrap_or_default();
            Ok::<_, crate::content::ContentError>(
                items
                    .into_iter()
                    .map(|item| {
                        let hash = (!item.is_directory)
                            .then(|| {
                                crate::content::sha1_hex(
                                    &game_dir.join(folder).join(&item.file_name),
                                )
                                .ok()
                            })
                            .flatten();
                        (item, hash)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)?;

        let hashes: Vec<String> = files.iter().filter_map(|(_, hash)| hash.clone()).collect();
        let enabled: Vec<String> = files
            .iter()
            .filter(|(item, _)| item.enabled)
            .filter_map(|(_, hash)| hash.clone())
            .collect();
        let unknown = |files| crate::content_sources::ContentList {
            entries: crate::content_sources::assemble(
                files,
                &BTreeMap::new(),
                &[],
                &BTreeMap::new(),
                &BTreeMap::new(),
            ),
            sources_unavailable: true,
        };
        let Ok(client) = self.modrinth().await else {
            return Ok(unknown(files));
        };
        let (identified, latest) = tokio::join!(
            client.identify(&hashes),
            client.latest_for(&enabled, record.loader, &record.game_version)
        );
        let Ok(identified) = identified else {
            return Ok(unknown(files));
        };
        let mut project_ids: Vec<String> =
            identified.values().map(|v| v.project_id.clone()).collect();
        project_ids.sort();
        project_ids.dedup();
        // Labels are decoration: a failure here leaves titles to the files.
        let projects = client
            .project_summaries(&project_ids)
            .await
            .unwrap_or_default();
        let mut teams: Vec<String> = projects.iter().filter_map(|p| p.team.clone()).collect();
        teams.sort();
        teams.dedup();
        let authors = client.team_authors(&teams).await.unwrap_or_default();
        Ok(crate::content_sources::ContentList {
            entries: crate::content_sources::assemble(
                files,
                &identified,
                &projects,
                &authors,
                &latest.unwrap_or_default(),
            ),
            sources_unavailable: false,
        })
    }

    /// Enables or disables several files, one result each. Writes need the
    /// instance lease, so they are refused while it launches or runs.
    pub async fn set_content_state(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
        enabled: bool,
    ) -> Result<Vec<ContentResult>, ServiceError> {
        self.change_content(id, kind, file_names, move |game_dir, kind, name| {
            let renamed = crate::content::set_enabled(game_dir, kind, name, enabled)?;
            if renamed == name {
                return Ok((ContentEffect::Unchanged, None));
            }
            let change = if enabled {
                ChangeKind::ContentEnabled
            } else {
                ChangeKind::ContentDisabled
            };
            let subject = renamed
                .strip_suffix(".disabled")
                .unwrap_or(&renamed)
                .to_owned();
            Ok((ContentEffect::Renamed(renamed), Some((change, subject))))
        })
        .await
    }

    /// Deletes several files (folder packs recursively), one result each.
    pub async fn delete_content(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
    ) -> Result<Vec<ContentResult>, ServiceError> {
        self.change_content(id, kind, file_names, |game_dir, kind, name| {
            crate::content::remove(game_dir, kind, name)?;
            Ok((
                ContentEffect::Removed,
                Some((ChangeKind::ContentRemoved, name.to_owned())),
            ))
        })
        .await
    }

    async fn change_content<F>(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
        apply: F,
    ) -> Result<Vec<ContentResult>, ServiceError>
    where
        F: Fn(
                &std::path::Path,
                ProjectKind,
                &str,
            ) -> Result<(ContentEffect, Option<(ChangeKind, String)>), ContentError>
            + Send
            + Clone
            + 'static,
    {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let mut results = Vec::with_capacity(file_names.len());
        for name in file_names {
            let (path, file) = (game_dir.clone(), name.clone());
            let apply = apply.clone();
            let applied = tokio::task::spawn_blocking(move || apply(&path, kind, &file))
                .await
                .map_err(std::io::Error::other)?;
            results.push(match applied {
                Ok((effect, change)) => {
                    let recorded = match change {
                        Some((change, subject)) => HistoryLog::for_instance(self.layout.root(), id)
                            .append(&HistoryEvent::Change {
                                at: now(),
                                kind: change,
                                subject,
                            })
                            .is_ok(),
                        None => true,
                    };
                    ContentResult {
                        file_name: name.clone(),
                        outcome: Ok(effect),
                        recorded,
                    }
                }
                Err(error) => ContentResult {
                    file_name: name.clone(),
                    outcome: Err(error),
                    recorded: true,
                },
            });
        }
        Ok(results)
    }

    /// The instance's history, oldest first. Reading needs no lease; a log
    /// that cannot be read is an error, not an empty history.
    pub async fn history(&self, id: &str) -> Result<crate::history::HistoryRead, ServiceError> {
        self.instance(id).await?;
        let (root, id) = (self.layout.root().to_path_buf(), id.to_owned());
        tokio::task::spawn_blocking(move || HistoryLog::for_instance(&root, &id).read())
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// The end of `logs/latest.log` (at most [`LOG_TAIL_BYTES`]) and the crash
    /// reports, newest first. Reading needs no lease; a missing log is `None`.
    pub async fn logs(&self, id: &str) -> Result<GameLogs, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || {
            Ok::<_, std::io::Error>(GameLogs {
                latest: crate::diagnostics::read_latest_log(&game_dir, LOG_TAIL_BYTES)?,
                crashes: crate::diagnostics::list_crash_reports(&game_dir)?,
            })
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)
    }

    /// One crash report's text (cut at [`REPORT_BYTES`]) with the likely causes
    /// recognised in it. Names that are not plain file names are refused.
    pub async fn crash_report(
        &self,
        id: &str,
        file_name: &str,
    ) -> Result<(String, Vec<crate::diagnostics::CrashHint>), ServiceError> {
        self.instance(id).await?;
        let (game_dir, name) = (self.layout.game(id), file_name.to_owned());
        let text = tokio::task::spawn_blocking(move || {
            crate::diagnostics::read_crash_report(&game_dir, &name, REPORT_BYTES)
        })
        .await
        .map_err(std::io::Error::other)??;
        let hints = crate::diagnostics::analyze(&text);
        Ok((text, hints))
    }

    /// How much disk the game's own folder takes (mods, saves, options…),
    /// not counting game files shared with other games. Walks the disk.
    pub async fn instance_size(&self, id: &str) -> Result<u64, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        Ok(
            tokio::task::spawn_blocking(move || crate::storage::directory_size(&game_dir))
                .await
                .map_err(std::io::Error::other)?,
        )
    }

    /// A folder of the game directory (`relative` empty is the directory itself),
    /// folders first. Only plain names inside the game directory are accepted.
    pub async fn list_files(
        &self,
        id: &str,
        relative: &str,
    ) -> Result<Vec<crate::diagnostics::FileEntry>, ServiceError> {
        self.instance(id).await?;
        let (game_dir, relative) = (self.layout.game(id), relative.to_owned());
        Ok(blocking(move || crate::diagnostics::list_dir(&game_dir, &relative)).await?)
    }

    /// What is wrong with this instance right now, most severe first.
    pub async fn problems(&self, id: &str) -> Result<Vec<Problem>, ServiceError> {
        let record = self.instance(id).await?;
        let (settings, runtimes) = self.environment().await;
        Ok(inspect_instance(&self.layout, &settings, &runtimes, &record).await)
    }

    /// The instance's saved worlds. Reading needs no instance lease.
    pub async fn worlds(&self, id: &str) -> Result<Vec<WorldInfo>, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || worlds::scan(&game_dir))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Copies a world under a new folder name (`<folder> copy`, `copy 2`… when
    /// none is given) and returns that name. Never overwrites; the copy appears
    /// all at once. Takes the instance lease.
    pub async fn copy_world(
        &self,
        id: &str,
        folder: &str,
        new_folder: Option<&str>,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let (folder, requested) = (folder.to_owned(), new_folder.map(str::to_owned));
        let created = tokio::task::spawn_blocking(move || {
            // Under the lease nothing else is copying: leftovers are stale.
            worlds::sweep_temporary(&game_dir);
            let name = requested.unwrap_or_else(|| worlds::copy_name(&game_dir, &folder));
            worlds::duplicate(&game_dir, &folder, &name)
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::WorldCopied, &created);
        Ok(created)
    }

    /// Deletes one world folder. Takes the instance lease.
    pub async fn delete_world(&self, id: &str, folder: &str) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let name = folder.to_owned();
        tokio::task::spawn_blocking(move || worlds::delete(&game_dir, &name))
            .await
            .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::WorldDeleted, folder);
        Ok(())
    }

    /// Packs one world into a zip at `destination` and returns its size.
    /// Takes the instance lease, so the world is not changing underneath.
    pub async fn export_world(
        &self,
        id: &str,
        folder: &str,
        destination: &Path,
    ) -> Result<u64, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let (folder, destination) = (folder.to_owned(), destination.to_owned());
        tokio::task::spawn_blocking(move || worlds::export_zip(&game_dir, &folder, &destination))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Packs the chosen files of the game as a Modrinth modpack at
    /// `destination`. Files Modrinth knows are listed by address; the rest are
    /// carried inside. If Modrinth cannot be asked, everything is carried and
    /// the report says so. Takes the instance lease so the files hold still.
    pub async fn export_modpack(
        &self,
        id: &str,
        spec: crate::pack_export::ExportSpec,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<crate::pack_export::ExportReport, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let record = self.instance(id).await?;
        let task = self.begin(
            TaskCategory::Install,
            format!("导出 {}", spec.name.trim()),
            Some(id.to_owned()),
            Some(&cancel),
        );
        let result = self
            .export_modpack_inner(&record, spec, destination, cancel)
            .await
            .map_err(|error| match error {
                ServiceError::Export(crate::pack_export::ExportError::Cancelled) => {
                    ServiceError::Cancelled
                }
                other => other,
            });
        self.end(task, &result);
        result
    }

    async fn export_modpack_inner(
        &self,
        record: &InstanceRecord,
        spec: crate::pack_export::ExportSpec,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<crate::pack_export::ExportReport, ServiceError> {
        use crate::pack_export as pack;
        let game_dir = self.layout.game(&record.id);

        let (dir, include, stop) = (game_dir.clone(), spec.include.clone(), cancel.clone());
        // Only a Modrinth pack lists files by address, so only it needs hashes.
        let wants_hashes = spec.format == pack::PackFormat::Modrinth;
        let (files, hashed) = tokio::task::spawn_blocking(move || {
            let files = pack::collect_files(&dir, &include)?;
            let mut hashed = Vec::new();
            for path in pack::linkable(&files).into_iter().filter(|_| wants_hashes) {
                if stop.is_cancelled() {
                    return Err(pack::ExportError::Cancelled);
                }
                let file = dir.join(path);
                let (sha1, sha512) = (crate::content::sha1_hex(&file)?, pack::sha512_hex(&file)?);
                let size = std::fs::metadata(&file)?.len();
                hashed.push((path.clone(), sha1, sha512, size));
            }
            Ok::<_, pack::ExportError>((files, hashed))
        })
        .await
        .map_err(std::io::Error::other)??;

        if spec.format == pack::PackFormat::Prism {
            let pack_json = pack::prism_pack_json(
                &record.game_version,
                record.loader,
                record.loader_version.as_deref(),
            )?;
            let (destination, name, stop) =
                (destination.to_owned(), spec.name.clone(), cancel.clone());
            let bundled = files.len();
            let size = tokio::task::spawn_blocking(move || {
                pack::write_prism(&destination, &game_dir, &name, &pack_json, &files, &|| {
                    stop.is_cancelled()
                })
            })
            .await
            .map_err(std::io::Error::other)??;
            return Ok(pack::ExportReport {
                linked: 0,
                bundled,
                lookup_failed: false,
                size,
            });
        }

        let sha1s: Vec<String> = hashed.iter().map(|(_, sha1, _, _)| sha1.clone()).collect();
        let identified = match read_unless_cancelled(&cancel, self.modrinth()).await {
            Ok(client) => client.identify(&sha1s).await.ok(),
            Err(ServiceError::Cancelled) => return Err(ServiceError::Cancelled),
            Err(_) => None,
        };
        let lookup_failed = identified.is_none() && !sha1s.is_empty();
        let linked = identified
            .map(|identified| pack::link_identified(&hashed, &identified))
            .unwrap_or_default();
        let carried: Vec<String> = files
            .iter()
            .filter(|path| !linked.iter().any(|file| &file.path == *path))
            .cloned()
            .collect();
        let index = pack::index_json(
            &spec,
            &record.game_version,
            record.loader,
            record.loader_version.as_deref(),
            &linked,
        )?;
        let destination = destination.to_owned();
        let bundled = carried.len();
        let stop = cancel.clone();
        let size = tokio::task::spawn_blocking(move || {
            pack::write_pack(&destination, &game_dir, &index, &carried, &|| {
                stop.is_cancelled()
            })
        })
        .await
        .map_err(std::io::Error::other)??;
        Ok(pack::ExportReport {
            linked: linked.len(),
            bundled,
            lookup_failed,
            size,
        })
    }

    /// Adds the world in a zip as a new world and returns its folder name;
    /// existing worlds are never replaced. Takes the instance lease.
    pub async fn import_world(&self, id: &str, archive: &Path) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (game_dir, archive) = (self.layout.game(id), archive.to_owned());
        let created = tokio::task::spawn_blocking(move || {
            worlds::sweep_temporary(&game_dir);
            worlds::import_zip(&game_dir, &archive)
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::WorldImported, &created);
        Ok(created)
    }

    /// The instance's snapshots, newest first. Reading needs no lease.
    pub async fn snapshots(&self, id: &str) -> Result<Vec<SnapshotInfo>, ServiceError> {
        self.instance(id).await?;
        let (root, id) = (self.layout.root().to_path_buf(), id.to_owned());
        tokio::task::spawn_blocking(move || snapshots::list(&root, &id))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Backs up the instance's worlds and settings (or one world). Takes the
    /// instance lease so the files are not changing underneath it.
    pub async fn create_snapshot(
        &self,
        id: &str,
        scope: SnapshotScope,
        label: &str,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (root, game_dir) = (self.layout.root().to_path_buf(), self.layout.game(id));
        let (instance, label) = (id.to_owned(), label.to_owned());
        let created = tokio::task::spawn_blocking(move || {
            snapshots::create(&root, &instance, &game_dir, scope, &label, now())
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::SnapshotCreated, &created);
        Ok(created)
    }

    /// Replaces the snapshot's worlds and settings in the game. A failure or a
    /// crash puts everything back as it was; the snapshot stays for a retry.
    /// Returns the restored units. Takes the instance lease.
    pub async fn restore_snapshot(
        &self,
        id: &str,
        snapshot: &str,
    ) -> Result<Vec<std::path::PathBuf>, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (root, game_dir) = (self.layout.root().to_path_buf(), self.layout.game(id));
        let (instance, name) = (id.to_owned(), snapshot.to_owned());
        let units = tokio::task::spawn_blocking(move || {
            snapshots::restore(&root, &instance, &name, &game_dir)
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::SnapshotRestored, snapshot);
        Ok(units)
    }

    /// Deletes one snapshot. Never part of automatic clean-up. Takes the lease.
    pub async fn delete_snapshot(&self, id: &str, snapshot: &str) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (root, instance, name) = (
            self.layout.root().to_path_buf(),
            id.to_owned(),
            snapshot.to_owned(),
        );
        tokio::task::spawn_blocking(move || snapshots::delete(&root, &instance, &name))
            .await
            .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::SnapshotDeleted, snapshot);
        Ok(())
    }

    /// Notes a change that really happened. A history write failure must not
    /// turn a finished file operation into an error.
    fn record_change(&self, id: &str, kind: ChangeKind, subject: &str) {
        let _ = HistoryLog::for_instance(self.layout.root(), id).append(&HistoryEvent::Change {
            at: now(),
            kind,
            subject: subject.to_owned(),
        });
    }

    /// Copies an instance under a new name. The copy is built and checked in a
    /// staging folder, then published whole; the source is only read. It
    /// shares the source's release (installed game files live in `meta/`),
    /// inherits its settings and installed state, and starts with no favorite,
    /// play time, history or snapshots. `include_worlds` decides whether
    /// `saves/` comes along; `logs/` and `crash-reports/` never do. Refused
    /// while the source launches or runs; cancellable through `cancel_task`.
    pub async fn copy_instance(
        &self,
        source_id: &str,
        new_name: &str,
        include_worlds: bool,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let _lease = self.reserve_instance(source_id)?;
        let source = self.instance(source_id).await?;
        let id = self.store.lock().await.suggest_id(new_name)?;
        let task = self
            .begin(
                TaskCategory::Install,
                format!("复制 {}", source.name),
                Some(source_id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::CopyInstance {
                source: source_id.to_owned(),
                name: new_name.to_owned(),
                include_worlds,
            });
        let result = self
            .copy_staged(&source, new_name, &id, include_worlds, cancel)
            .await;
        self.end(task, &result);
        result
    }

    async fn copy_staged(
        &self,
        source: &InstanceRecord,
        new_name: &str,
        id: &str,
        include_worlds: bool,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let staged = Staged::begin(&self.layout, "copy", id)?;
        let (from, to) = (self.layout.game(&source.id), staged.game_dir());
        let token = cancel.clone();
        let copied = tokio::task::spawn_blocking(move || {
            crate::copy::copy_game(&from, &to, include_worlds, &token)
        })
        .await
        .map_err(std::io::Error::other);
        let copied = match copied {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(crate::copy::CopyError::Cancelled)) => Err(ServiceError::Cancelled),
            Ok(Err(crate::copy::CopyError::Io(error))) | Err(error) => Err(error.into()),
        };
        if let Err(error) = copied {
            staged.discard();
            return Err(error);
        }
        if let Err(error) = staged.mark_ready() {
            staged.discard();
            return Err(error.into());
        }
        let created = self
            .store
            .lock()
            .await
            .create_as(
                id,
                NewInstance {
                    name: new_name.to_owned(),
                    game_version: source.game_version.clone(),
                    loader: source.loader,
                    loader_version: source.loader_version.clone(),
                },
                source.settings.clone(),
                source.installed,
                now(),
            )
            .cloned();
        let record = match created {
            Ok(record) => record,
            Err(error) => {
                staged.discard();
                return Err(error.into());
            }
        };
        if let Err(error) = staged.publish(&self.layout) {
            let _ = self.store.lock().await.remove(&record.id);
            staged.discard();
            return Err(error.into());
        }
        Ok(record)
    }

    /// Installs a modpack project as a new instance.
    pub async fn install_modpack(
        &self,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let task = self
            .begin(
                TaskCategory::Install,
                format!("安装整合包 {project}"),
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::InstallModpack {
                project: project.to_owned(),
            });
        let result = self.install_modpack_inner(task.id, project, cancel).await;
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    /// Imports a local `.mrpack` as a new instance. Same rules as installing a
    /// downloaded pack; the file itself is never deleted or modified.
    pub async fn import_modpack_file(
        &self,
        pack: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let label = pack
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let task = self
            .begin(
                TaskCategory::Install,
                format!("导入整合包 {label}"),
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::ImportPack {
                path: pack.display().to_string(),
            });
        let result = match tokio::fs::metadata(pack).await {
            Ok(metadata) if metadata.is_file() => {
                let kind = {
                    let pack = pack.to_owned();
                    blocking_value(move || sniff_pack(&pack)).await?
                };
                match kind {
                    PackKind::Prism => self.import_prism_zip(pack, cancel).await,
                    PackKind::Backup => self.restore_backup_inner(pack, cancel).await,
                    PackKind::Modrinth => self.import_pack_from(task.id, pack, cancel).await,
                }
            }
            Ok(_) => Err(ServiceError::Install(format!("{label:?} is not a file"))),
            Err(error) => Err(error.into()),
        };
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    /// A MultiMC / Prism instance in a zip (ADR 0016): unpacked into a
    /// scratch folder, read like any other launcher's game, imported, and the
    /// scratch folder removed whatever happened.
    async fn import_prism_zip(
        &self,
        archive: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let scratch = self.layout.root().join("cache").join(format!(
            "prism-import-{}-{}",
            std::process::id(),
            now()
        ));
        let outcome = async {
            let (archive, scratch, token) = (archive.to_owned(), scratch.clone(), cancel.clone());
            let root = blocking_value(move || unpack_instance_zip(&archive, &scratch, &token))
                .await?
                .map_err(|error| match error {
                    UnpackError::Cancelled => ServiceError::Cancelled,
                    UnpackError::Other(why) => ServiceError::Install(why),
                })?;
            let found = self.detect_games(&root).await?;
            let game = found
                .into_iter()
                .next()
                .ok_or_else(|| ServiceError::Install("the zip holds no game".to_owned()))?;
            self.import_game_inner(game, cancel.clone()).await
        }
        .await;
        let _ = tokio::fs::remove_dir_all(&scratch).await;
        outcome
    }

    async fn import_pack_from(
        &self,
        task: u64,
        pack: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let chain = self.chain().await?;
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        self.metered(
            task,
            engine.subscribe(),
            modpack::import(
                &self.store,
                &engine,
                &chain,
                pack,
                now(),
                modpack::is_trusted_source,
                cancel,
            ),
        )
        .await
        .map_err(ServiceError::from)
    }

    async fn install_modpack_inner(
        &self,
        task: u64,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.modrinth()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client
                .versions(project)
                .await
                .map_err(|error| ServiceError::Remote(error.to_string()))
        })
        .await?;
        let version = versions
            .iter()
            .max_by(|a, b| a.published.cmp(&b.published))
            .ok_or(ServiceError::NoCompatibleVersion)?;
        let file = version
            .install_file()
            .ok_or(ServiceError::NoCompatibleVersion)?;
        if !crate::discover::is_safe_file_name(&file.filename) {
            return Err(ServiceError::Install(format!(
                "unsafe file name {:?}",
                file.filename
            )));
        }
        let pack = self.layout.root().join("downloads").join(&file.filename);
        let mut request = TransferRequest::new(
            format!("pack:{}", version.id),
            chain.candidates(&file.url),
            &pack,
        )
        .map_err(|error| ServiceError::Install(error.to_string()))?;
        if file.size > 0 {
            request = request.expect_size(file.size);
        }
        if let Some(sha1) = &file.sha1 {
            request = request
                .expect_sha1(sha1)
                .map_err(|error| ServiceError::Install(error.to_string()))?;
        }
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        self.metered(
            task,
            engine.subscribe(),
            engine.transfer(request, cancel.clone()),
        )
        .await
        .map_err(ServiceError::from)?;
        let record = self.import_pack_from(task, &pack, cancel).await;
        let _ = tokio::fs::remove_file(&pack).await;
        record
    }

    /// Asks a running task to stop. `false` when the id is unknown, already
    /// finished, or not cancellable; the task's own outcome is still decided by
    /// the work itself.
    pub fn cancel_task(&self, id: u64) -> bool {
        let token = self
            .cancels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .cloned();
        token.map(|token| token.cancel()).is_some()
    }

    /// Runs a task again from the input it was started with (its
    /// [`FinishedTask::retry`]). It is a new task of its own: the old entry
    /// stays in the history.
    pub async fn retry_task(
        &self,
        action: RetryAction,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        match action {
            RetryAction::InstallInstance { instance } => {
                self.install_instance(&instance, updates, cancel).await
            }
            RetryAction::RepairInstance { instance } => {
                self.repair_instance(&instance, updates, cancel).await
            }
            RetryAction::ChangeRuntime {
                instance,
                game_version,
                loader,
                loader_version,
            } => self
                .change_runtime(
                    &instance,
                    &game_version,
                    loader,
                    loader_version.as_deref(),
                    updates,
                    cancel,
                )
                .await
                .map(|_| ()),
            RetryAction::InstallContent {
                instance,
                kind,
                project,
                version,
            } => match version {
                Some(version) => self
                    .install_version(&instance, kind, &project, &version, cancel)
                    .await
                    .map(|_| ()),
                None => self
                    .install_content(&instance, kind, &project, cancel)
                    .await
                    .map(|_| ()),
            },
            RetryAction::SwitchContent {
                instance,
                kind,
                file_name,
                project,
                version_id,
            } => self
                .switch_content_version(&instance, kind, &file_name, &project, &version_id, cancel)
                .await
                .map(|_| ()),
            RetryAction::CopyInstance {
                source,
                name,
                include_worlds,
            } => self
                .copy_instance(&source, &name, include_worlds, cancel)
                .await
                .map(|_| ()),
            RetryAction::BackupInstance { instance, path } => self
                .backup_instance(&instance, Path::new(&path), cancel)
                .await
                .map(|_| ()),
            RetryAction::RestoreBackup { path } => self
                .restore_backup(Path::new(&path), cancel)
                .await
                .map(|_| ()),
            RetryAction::InstallJava { major } => {
                self.install_java(major, cancel).await.map(|_| ())
            }
            RetryAction::SaveVersion {
                project,
                version_id,
                path,
            } => self
                .save_version_as(&project, &version_id, Path::new(&path), cancel)
                .await
                .map(|_| ()),
            RetryAction::InstallModpack { project } => {
                self.install_modpack(&project, cancel).await.map(|_| ())
            }
            RetryAction::ImportPack { path } => self
                .import_modpack_file(Path::new(&path), cancel)
                .await
                .map(|_| ()),
        }
    }

    /// Forgets every finished task; running ones are untouched.
    pub fn clear_finished(&self) -> std::io::Result<()> {
        let mut board = self
            .board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.log.compact(0)?;
        board.clear_unrecorded();
        Ok(())
    }

    /// Running tasks and the most recent finished ones.
    pub fn activity(&self, finished_limit: usize) -> ActivityView {
        let board = self
            .board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let active = board.active().to_vec();
        let mut finished = self.log.recent(None, finished_limit).unwrap_or_default();
        for terminal in board.unrecorded() {
            finished.push(terminal.clone());
        }
        finished.sort_by_key(|task| std::cmp::Reverse(task.finished));
        finished.truncate(finished_limit);
        let cancellable = self
            .cancels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .copied()
            .collect();
        ActivityView {
            active,
            cancellable,
            finished,
        }
    }

    async fn chain(&self) -> Result<SourceChain, ServiceError> {
        Ok(self.settings.lock().await.source_chain()?)
    }

    async fn environment(
        &self,
    ) -> (
        crate::settings::LauncherSettings,
        Vec<crate::java::JavaRuntime>,
    ) {
        let settings = self.settings.lock().await.get().clone();
        let found = self.discover_java(&settings).await;
        // A Java the user turned off is never chosen, here or at launch.
        let runtimes = found
            .into_iter()
            .filter(|runtime| {
                !settings
                    .disabled_java
                    .iter()
                    .any(|home| home == runtime.home())
            })
            .collect();
        (settings, runtimes)
    }

    async fn discover_java(
        &self,
        settings: &crate::settings::LauncherSettings,
    ) -> Vec<crate::java::JavaRuntime> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from);
        let locator = match &self.runtime_roots {
            Some(roots) => JavaLocator::new(roots.clone()),
            None => {
                JavaLocator::standard(&HostProfile::current(), home.as_deref(), self.layout.root())
            }
        }
        .with_roots(settings.extra_java_roots.clone());
        tokio::task::spawn_blocking(move || locator.discover())
            .await
            .unwrap_or_default()
    }

    /// Every Java found on this machine, with whether the user turned it off.
    pub async fn java_installations(&self) -> Vec<(crate::java::JavaRuntime, bool)> {
        let settings = self.settings.lock().await.get().clone();
        self.discover_java(&settings)
            .await
            .into_iter()
            .map(|runtime| {
                let disabled = settings
                    .disabled_java
                    .iter()
                    .any(|home| home == runtime.home());
                (runtime, disabled)
            })
            .collect()
    }

    /// How much disk each part of the launcher's folder uses. Walks the disk
    /// on a worker thread; reading needs no lease.
    pub async fn storage_usage(&self) -> crate::storage::StorageUsage {
        let layout = self.layout.clone();
        tokio::task::spawn_blocking(move || crate::storage::measure(&layout))
            .await
            .unwrap_or_default()
    }

    /// Deletes extracted natives and the cache folder, returning the bytes
    /// freed. Natives are rebuilt by the next start that needs them. Where the
    /// system will not delete a file a running game still holds, this reports
    /// the error and the rest stays as it was.
    pub async fn clear_cache(&self) -> Result<u64, ServiceError> {
        let layout = self.layout.clone();
        let freed = tokio::task::spawn_blocking(move || crate::storage::clear_cache(&layout))
            .await
            .map_err(std::io::Error::other)??;
        Ok(freed)
    }

    /// Writes a zip for a bug report to `destination`: versions, settings
    /// summary, Java list, recent tasks and each game's latest log, with
    /// player names, profile ids and folders replaced.
    /// Hides what identifies the person: account names and ids, the launcher's
    /// folder and the home folder.
    fn redactor(&self, settings: &crate::settings::LauncherSettings) -> crate::bundle::Redactor {
        let mut redactor = crate::bundle::Redactor::new();
        for entry in &settings.accounts {
            redactor.hide(&entry.name, "<player>");
            if let Ok(profile) = entry.profile() {
                redactor.hide(&profile.id().to_string(), "<uuid>");
                redactor.hide(&profile.id().compact(), "<uuid>");
            }
        }
        redactor.hide_path(self.layout.root(), "<launcher>");
        for variable in ["HOME", "USERPROFILE"] {
            if let Some(home) = std::env::var_os(variable) {
                redactor.hide_path(Path::new(&home), "~");
            }
        }
        redactor
    }

    /// Saves the game's latest log (or one crash report, by file name) as text
    /// at `destination` with player names, ids and folders replaced, so it can
    /// be sent to someone. Written through a `.part` file.
    pub async fn export_log(
        &self,
        id: &str,
        crash_report: Option<&str>,
        destination: &Path,
    ) -> Result<(), ServiceError> {
        self.instance(id).await?;
        let settings = self.settings.lock().await.get().clone();
        let redactor = self.redactor(&settings);
        let (game_dir, destination) = (self.layout.game(id), destination.to_owned());
        let crash = crash_report.map(str::to_owned);
        blocking(move || {
            let text = match &crash {
                Some(name) => {
                    crate::diagnostics::read_crash_report(&game_dir, name, EXPORT_LOG_BYTES)?
                }
                None => crate::diagnostics::read_latest_log(&game_dir, EXPORT_LOG_BYTES)?
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::NotFound, "there is no log yet")
                    })?,
            };
            // Paths of this very game, however spelled, are no use to a reader.
            let mut redactor = redactor;
            redactor.hide_path(&game_dir, "<game>");
            let part = destination.with_extension("txt.part");
            let written = std::fs::write(&part, redactor.apply(&text))
                .and_then(|()| std::fs::rename(&part, &destination));
            if written.is_err() {
                let _ = std::fs::remove_file(&part);
            }
            written
        })
        .await?;
        Ok(())
    }

    pub async fn export_diagnostics(&self, destination: &Path) -> Result<(), ServiceError> {
        use std::fmt::Write as _;
        let (settings, _) = self.environment().await;
        let installations = self.java_installations().await;
        let instances = self.library().await.instances;
        let activity = self.activity(50);

        let redactor = self.redactor(&settings);

        let mut about = String::new();
        let _ = writeln!(about, "LumilioCL {}", env!("CARGO_PKG_VERSION"));
        let _ = writeln!(
            about,
            "system: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let _ = writeln!(about, "instances: {}", instances.len());
        let _ = writeln!(about, "accounts: {}", settings.accounts.len());

        let mut summary = String::new();
        let launch = &settings.launch;
        let _ = writeln!(
            summary,
            "memory: min {:?} MB, max {:?} MB",
            settings.default_min_memory_mb, settings.default_max_memory_mb
        );
        let _ = writeln!(summary, "preferences: {:?}", settings.preferences);
        let _ = writeln!(
            summary,
            "download concurrency: {:?}",
            settings.download_concurrency
        );
        let _ = writeln!(
            summary,
            "mirrors: {} (preferred: {})",
            settings.mirrors.len(),
            settings.prefer_mirrors
        );
        let _ = writeln!(
            summary,
            "launch defaults: window {:?}x{:?}, fullscreen {:?}, {} JVM arguments, {} game arguments, {} environment variables, pre-launch {}, wrapper {}, post-exit {}",
            launch.window_width,
            launch.window_height,
            launch.fullscreen,
            launch.jvm_arguments.len(),
            launch.game_arguments.len(),
            launch.environment.len(),
            launch.pre_launch.is_some(),
            launch.wrapper.is_some(),
            launch.post_exit.is_some(),
        );

        let mut java = String::new();
        for (runtime, disabled) in &installations {
            let _ = writeln!(
                java,
                "Java {} {} {}{} — {}",
                runtime.version(),
                runtime.vendor().unwrap_or("unknown vendor"),
                runtime.architecture().unwrap_or("unknown arch"),
                if *disabled { " (turned off)" } else { "" },
                runtime.home().display()
            );
        }

        let mut list = String::new();
        for record in &instances {
            let _ = writeln!(
                list,
                "{} · {} · {:?} {} · installed: {} · played {} s",
                record.id,
                record.game_version,
                record.loader,
                record.loader_version.as_deref().unwrap_or("-"),
                record.installed,
                record.play_seconds,
            );
        }

        let mut tasks = String::new();
        for task in &activity.finished {
            let _ = writeln!(
                tasks,
                "{} · {:?} · {:?}",
                task.label, task.category, task.outcome
            );
        }

        let mut files = vec![
            ("about.txt".to_owned(), about),
            ("settings.txt".to_owned(), summary),
            ("java.txt".to_owned(), java),
            ("instances.txt".to_owned(), list),
            ("activity.txt".to_owned(), tasks),
        ];
        for record in &instances {
            if let Ok(logs) = self.logs(&record.id).await
                && let Some(latest) = logs.latest
            {
                files.push((format!("logs/{}-latest.log", record.id), latest));
            }
        }
        for (_, text) in &mut files {
            *text = redactor.apply(text);
        }
        let destination = destination.to_owned();
        tokio::task::spawn_blocking(move || crate::bundle::write_bundle(&destination, &files))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }

    /// Files downloaded at once for content and installs.
    async fn transfer_concurrency(&self) -> usize {
        self.settings
            .lock()
            .await
            .get()
            .download_concurrency
            .map_or(TRANSFER_CONCURRENCY, |count| count as usize)
    }

    /// Installs through `launcher`, passing its updates on while showing their
    /// file counts as the task's progress.
    async fn install_tracked(
        board: &StdMutex<TaskBoard>,
        task: u64,
        launcher: &Launcher<T>,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let (inner, mut seen) = mpsc::unbounded_channel();
        let forward = async {
            while let Some(update) = seen.recv().await {
                if let LaunchUpdate::Signal(crate::launch_session::LaunchSignal::Progress {
                    done,
                    total,
                }) = &update
                {
                    board
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .progress(task, *done, *total);
                }
                let _ = updates.send(update);
            }
        };
        let (installed, ()) = tokio::join!(launcher.install(request, inner, cancel), forward);
        installed.map_err(ServiceError::from)
    }

    /// Runs `work` while the byte progress of the engine behind `events` is
    /// shown as the progress of task `task`.
    async fn metered<R>(
        &self,
        task: u64,
        events: tokio::sync::broadcast::Receiver<crate::transfer::TransferEvent>,
        work: impl std::future::Future<Output = R>,
    ) -> R {
        let meter = meter_bytes(&self.board, task, events);
        tokio::pin!(work);
        tokio::pin!(meter);
        tokio::select! {
            result = &mut work => result,
            () = &mut meter => work.await,
        }
    }

    fn begin(
        &self,
        category: TaskCategory,
        label: String,
        instance: Option<String>,
        cancel: Option<&CancellationToken>,
    ) -> TaskLease<'_> {
        let id = self
            .board
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .start(category, label, instance, now());
        if let Some(cancel) = cancel {
            self.cancels
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(id, cancel.clone());
        }
        TaskLease {
            board: &self.board,
            log: &self.log,
            cancels: &self.cancels,
            id,
            ended: false,
        }
    }

    fn end<V>(&self, task: TaskLease<'_>, result: &Result<V, ServiceError>) {
        let outcome = match result {
            Ok(_) => TaskOutcome::Succeeded,
            Err(ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled)) => {
                TaskOutcome::Cancelled
            }
            Err(error) => TaskOutcome::Failed(error.to_string()),
        };
        task.finish(outcome);
    }
}

#[cfg(test)]
mod tests;
