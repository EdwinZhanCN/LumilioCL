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
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    use sha1::{Digest, Sha1};

    use super::*;
    use crate::launch_session::{LaunchSession, LaunchSignal};
    use crate::transfer::{FileTransport, TransportFuture, TransportResponse};

    /// Answers addresses containing a key from a table, and `file:` addresses
    /// from disk; everything else fails like an unreachable host.
    type Answer = (String, Vec<u8>);
    type Replies = BTreeMap<String, std::collections::VecDeque<(u16, String)>>;

    #[derive(Clone, Default)]
    struct Scripted {
        answers: Arc<StdMutex<Vec<Answer>>>,
        cancel_on_file: Arc<StdMutex<Option<CancellationToken>>>,
        /// When set, answers that exist are held until this is notified.
        gate: Arc<StdMutex<Option<Arc<tokio::sync::Notify>>>>,
        /// Answers to `send`, by exact address; the last one repeats.
        replies: Arc<StdMutex<Replies>>,
        sent: Arc<StdMutex<Vec<crate::transfer::HttpRequest>>>,
    }

    impl Scripted {
        fn reply(&self, url: &str, status: u16, body: &str) {
            self.replies
                .lock()
                .unwrap()
                .entry(url.to_owned())
                .or_default()
                .push_back((status, body.to_owned()));
        }
        fn clear_replies(&self, url: &str) {
            self.replies.lock().unwrap().remove(url);
        }
        fn sent_to(&self, url: &str) -> usize {
            self.sent
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.url == url)
                .count()
        }
        fn answer(&self, key: &str, body: impl Into<Vec<u8>>) {
            self.answers
                .lock()
                .unwrap()
                .push((key.to_owned(), body.into()));
        }
        fn forget_all(&self) {
            self.answers.lock().unwrap().clear();
        }
    }

    impl Transport for Scripted {
        fn send<'a>(&'a self, request: crate::transfer::HttpRequest) -> TransportFuture<'a> {
            self.sent.lock().unwrap().push(request.clone());
            let mut replies = self.replies.lock().unwrap();
            let answer = replies.get_mut(&request.url).map(|queue| {
                if queue.len() > 1 {
                    queue.pop_front().unwrap()
                } else {
                    queue.front().cloned().unwrap()
                }
            });
            Box::pin(async move {
                match answer {
                    Some((status, body)) => {
                        Ok(TransportResponse::from_bytes(status, body.into_bytes()))
                    }
                    None => Err(crate::transfer::TransportError::transient("no route")),
                }
            })
        }

        /// A POST is answered like a GET of the same address.
        fn post_json<'a>(&'a self, source: &'a str, _body: Vec<u8>) -> TransportFuture<'a> {
            self.get(source)
        }

        fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
            if source.starts_with("file:")
                && let Some(cancel) = self.cancel_on_file.lock().unwrap().take()
            {
                cancel.cancel();
                return Box::pin(std::future::pending());
            }
            let found = self
                .answers
                .lock()
                .unwrap()
                .iter()
                .find(|(key, _)| source.contains(key.as_str()))
                .map(|(_, body)| body.clone());
            let gate = self.gate.lock().unwrap().clone();
            match found {
                Some(body) => Box::pin(async move {
                    if let Some(gate) = gate {
                        gate.notified().await;
                    }
                    Ok(TransportResponse::from_bytes(200, body))
                }),
                None if source.starts_with("file:") => FileTransport.get(source),
                None => Box::pin(async {
                    Err(crate::transfer::TransportError::permanent("unreachable"))
                }),
            }
        }
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        Sha1::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn file_url(path: &std::path::Path) -> String {
        url::Url::from_file_path(path).unwrap().to_string()
    }

    struct World {
        _dir: tempfile::TempDir,
        service: LauncherService<Scripted>,
        net: Scripted,
        server: PathBuf,
        secrets: Arc<crate::credentials::MemoryCredentials>,
    }

    fn world() -> World {
        let dir = tempfile::tempdir().unwrap();
        let server = dir.path().join("server");
        std::fs::create_dir_all(&server).unwrap();
        let net = Scripted::default();
        let root = dir.path().join("launcher");
        let secrets = Arc::new(crate::credentials::MemoryCredentials::default());
        let service = LauncherService::open(&root, net.clone())
            .unwrap()
            .with_runtime_roots(vec![root.join("runtimes")])
            .with_credentials(secrets.clone());
        service
            .settings
            .try_lock()
            .unwrap()
            .add_offline_account("Steve", None)
            .unwrap();
        World {
            _dir: dir,
            service,
            net,
            server,
            secrets,
        }
    }

    /// Publishes release `1.0` (a one-file client) and a catalog listing it.
    fn publish_release(world: &World) {
        publish_release_with(
            world,
            r#""minecraftArguments":"--username ${auth_player_name}""#,
        );
    }

    /// A release that passes the whole identity to the game.
    fn publish_identity_release(world: &World) {
        publish_release_with(
            world,
            r#""minecraftArguments":"--username ${auth_player_name} --uuid ${auth_uuid} --accessToken ${auth_access_token} --userType ${user_type}""#,
        );
    }

    /// A release whose game arguments declare direct quick play, like 1.20+.
    fn publish_modern_release(world: &World) {
        publish_release_with(
            world,
            r#""arguments":{"game":[
                "--username","${auth_player_name}",
                {"rules":[{"action":"allow","features":{"has_quick_plays_support":true}}],
                 "value":["--quickPlayPath","${quickPlayPath}"]},
                {"rules":[{"action":"allow","features":{"is_quick_play_singleplayer":true}}],
                 "value":["--quickPlaySingleplayer","${quickPlaySingleplayer}"]},
                {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],
                 "value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}]}"#,
        );
    }

    fn publish_release_with(world: &World, arguments: &str) {
        let client = b"pretend client jar";
        std::fs::write(world.server.join("client.jar"), client).unwrap();
        let manifest = format!(
            r#"{{"id":"1.0","mainClass":"net.example.Main",
                {arguments},
                "javaVersion":{{"component":"x","majorVersion":21}},
                "downloads":{{"client":{{"url":"{}","sha1":"{}","size":{}}}}},
                "libraries":[]}}"#,
            file_url(&world.server.join("client.jar")),
            sha1_hex(client),
            client.len()
        );
        std::fs::write(world.server.join("1.0.json"), manifest).unwrap();
        world.net.answer(
            "piston-meta.mojang.com/mc/game/version_manifest",
            format!(
                r#"{{"latest":{{"release":"1.0","snapshot":"1.0"}},"versions":[
                    {{"id":"1.0","type":"release","releaseTime":"2024-01-01T00:00:00+00:00",
                      "url":"{}"}}]}}"#,
                file_url(&world.server.join("1.0.json"))
            ),
        );
    }

    fn fake_java(world: &World, script: &str) {
        let jdk = world.service.layout().runtimes().join("jdk-21");
        std::fs::create_dir_all(jdk.join("bin")).unwrap();
        std::fs::write(jdk.join("release"), "JAVA_VERSION=\"21.0.1\"\n").unwrap();
        let java = jdk.join("bin/java");
        std::fs::write(&java, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn assert_send<T: Send>(_: &T) {}

    #[tokio::test]
    async fn explicit_cancel_unblocks_reads_and_records_cancelled_for_both_install_kinds() {
        let world = world();
        let record = world
            .service
            .create_instance("Target", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let defaults = world.service.settings.lock().await;
        let cancel = CancellationToken::new();
        let mut content = Box::pin(world.service.install_content(
            &record.id,
            ProjectKind::Mod,
            "project",
            cancel.clone(),
        ));
        assert!(futures_util::poll!(&mut content).is_pending());
        cancel.cancel();
        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(2), content)
                .await
                .unwrap(),
            Err(ServiceError::Cancelled)
        ));
        let cancel = CancellationToken::new();
        let mut pack = Box::pin(world.service.install_modpack("pack", cancel.clone()));
        assert!(futures_util::poll!(&mut pack).is_pending());
        cancel.cancel();
        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(2), pack)
                .await
                .unwrap(),
            Err(ServiceError::Cancelled)
        ));
        let activity = world.service.activity(10);
        assert!(activity.active.is_empty());
        assert_eq!(activity.finished.len(), 2);
        assert!(
            activity
                .finished
                .iter()
                .all(|task| task.outcome == TaskOutcome::Cancelled)
        );
        assert!(!world.service.layout.game(&record.id).exists());
        world.service.delete_instance(&record.id).await.unwrap();
        drop(defaults);
    }

    /// An instance with a profile holding one world file.
    async fn instance_with_profile(world: &World, name: &str) -> InstanceRecord {
        let record = world
            .service
            .create_instance(name, Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let saves = world.service.layout.game(&record.id).join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("level.dat"), b"world").unwrap();
        record
    }

    fn operation_dirs(world: &World) -> Vec<PathBuf> {
        std::fs::read_dir(world.service.layout.operations())
            .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn delete_removes_only_that_profile_and_leaves_no_journal() {
        let world = world();
        publish_release(&world);
        let target = instance_with_profile(&world, "Target").await;
        let other = instance_with_profile(&world, "Other").await;
        let meta = world.service.layout.meta();
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("shared.jar"), b"shared").unwrap();
        world.service.delete_instance(&target.id).await.unwrap();
        assert!(world.service.instance(&target.id).await.is_err());
        assert!(!world.service.layout.profile(&target.id).exists());
        assert!(
            world
                .service
                .layout
                .game(&other.id)
                .join("saves/level.dat")
                .exists()
        );
        assert!(meta.join("shared.jar").exists());
        assert!(operation_dirs(&world).is_empty());
    }

    #[tokio::test]
    async fn failed_isolation_keeps_record_and_profile_and_allows_retry() {
        let world = world();
        publish_release(&world);
        let record = instance_with_profile(&world, "Target").await;
        let profiles = world.service.layout.profiles();
        std::fs::set_permissions(&profiles, std::fs::Permissions::from_mode(0o555)).unwrap();
        let failed = world.service.delete_instance(&record.id).await;
        std::fs::set_permissions(&profiles, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(failed, Err(ServiceError::Io(_))));
        assert!(world.service.instance(&record.id).await.is_ok());
        assert!(
            world
                .service
                .layout
                .game(&record.id)
                .join("saves/level.dat")
                .exists()
        );
        assert!(operation_dirs(&world).is_empty());
        world.service.delete_instance(&record.id).await.unwrap();
        assert!(!world.service.layout.profile(&record.id).exists());
    }

    #[tokio::test]
    async fn failed_library_commit_restores_the_profile() {
        let world = world();
        publish_release(&world);
        let record = instance_with_profile(&world, "Target").await;
        world.service.store.lock().await.set_read_only(true);
        let failed = world.service.delete_instance(&record.id).await;
        world.service.store.lock().await.set_read_only(false);
        assert!(matches!(failed, Err(ServiceError::Store(_))));
        assert!(world.service.instance(&record.id).await.is_ok());
        assert_eq!(
            std::fs::read(
                world
                    .service
                    .layout
                    .game(&record.id)
                    .join("saves/level.dat")
            )
            .unwrap(),
            b"world"
        );
        assert!(operation_dirs(&world).is_empty());
        world.service.delete_instance(&record.id).await.unwrap();
    }

    #[tokio::test]
    async fn failed_cleanup_still_deletes_and_the_next_start_finishes_it() {
        let world = world();
        publish_release(&world);
        let record = instance_with_profile(&world, "Target").await;
        // A read-only folder inside the profile makes the final removal fail.
        let locked = world.service.layout.game(&record.id).join("saves");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        world.service.delete_instance(&record.id).await.unwrap();
        assert!(world.service.instance(&record.id).await.is_err());
        let leftovers = operation_dirs(&world);
        assert_eq!(leftovers.len(), 1, "journal and files stay for recovery");
        std::fs::set_permissions(
            leftovers[0].join("profile/game/saves"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let root = world.service.layout.root().to_path_buf();
        let (service, _dir) = reopen(world, &root);
        assert_eq!(
            service.startup_notes(),
            [RecoveryNote::DeleteCompleted {
                instance_id: record.id.clone()
            }]
        );
        assert!(
            service
                .layout
                .operations()
                .read_dir()
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[tokio::test]
    async fn crash_after_isolation_before_commit_rolls_back_on_next_start() {
        let world = world();
        publish_release(&world);
        let record = instance_with_profile(&world, "Target").await;
        // Rehearse the crash: journal and quarantine happened, the library did not commit.
        let mut crashed = Deletion::begin(&world.service.layout, &record, 7).unwrap();
        crashed.quarantine().unwrap();
        drop(crashed);
        assert!(!world.service.layout.profile(&record.id).exists());
        let root = world.service.layout.root().to_path_buf();
        let (service, _dir) = reopen(world, &root);
        assert_eq!(
            service.startup_notes(),
            [RecoveryNote::DeleteRolledBack {
                instance_id: record.id.clone()
            }]
        );
        assert!(service.instance(&record.id).await.is_ok());
        assert_eq!(
            std::fs::read(service.layout.game(&record.id).join("saves/level.dat")).unwrap(),
            b"world"
        );
        assert!(
            service
                .layout
                .operations()
                .read_dir()
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[tokio::test]
    async fn crash_after_commit_completes_and_conflicts_or_bad_journals_are_kept() {
        let world = world();
        publish_release(&world);
        let committed = instance_with_profile(&world, "Committed").await;
        let conflict = instance_with_profile(&world, "Conflict").await;
        let broken = instance_with_profile(&world, "Broken").await;
        let layout = world.service.layout.clone();
        let mut crashed = Deletion::begin(&layout, &committed, 1).unwrap();
        crashed.quarantine().unwrap();
        world
            .service
            .store
            .lock()
            .await
            .remove(&committed.id)
            .unwrap();
        drop(crashed);
        // Both copies exist for the conflicting one.
        let mut both = Deletion::begin(&layout, &conflict, 2).unwrap();
        both.quarantine().unwrap();
        std::fs::create_dir_all(layout.profile(&conflict.id)).unwrap();
        drop(both);
        // An unreadable journal beside real files must not be removed.
        let mut bad = Deletion::begin(&layout, &broken, 3).unwrap();
        bad.quarantine().unwrap();
        drop(bad);
        let bad_dir = layout.operations().join(format!("delete-{}-3", broken.id));
        std::fs::write(bad_dir.join("delete.json"), b"{ not json").unwrap();
        let root = layout.root().to_path_buf();
        let (service, _dir) = reopen(world, &root);
        let notes = service.startup_notes();
        assert_eq!(notes.len(), 3, "{notes:?}");
        assert!(notes.contains(&RecoveryNote::DeleteCompleted {
            instance_id: committed.id.clone()
        }));
        assert!(notes.iter().any(|note| matches!(
            note,
            RecoveryNote::DeleteConflict { instance_id, .. } if *instance_id == conflict.id
        )));
        assert!(notes.iter().any(|note| matches!(
            note,
            RecoveryNote::JournalUnusable { path, .. } if *path == bad_dir
        )));
        assert!(
            layout
                .operations()
                .join(format!("delete-{}-2", conflict.id))
                .join("profile")
                .exists()
        );
        assert!(bad_dir.join("profile/game/saves/level.dat").exists());
        assert!(
            !layout
                .operations()
                .join(format!("delete-{}-1", committed.id))
                .exists()
        );
    }

    #[tokio::test]
    async fn damaged_library_is_kept_reported_and_never_overwritten_by_a_second_failure() {
        let world = world();
        publish_release(&world);
        let record = instance_with_profile(&world, "Lost").await;
        let root = world.service.layout.root().to_path_buf();
        let (service, dir) = reopen(world, &root);
        drop(service);
        let database = root.join("launcher.db");
        std::fs::write(&database, b"first damage").unwrap();
        let service = LauncherService::open(&root, Scripted::default()).unwrap();
        let [
            RecoveryNote::LibraryRecovered {
                preserved,
                candidates,
            },
        ] = service.startup_notes()
        else {
            panic!("{:?}", service.startup_notes());
        };
        assert_eq!(std::fs::read(preserved).unwrap(), b"first damage");
        assert_eq!(candidates, std::slice::from_ref(&record.id));
        let preserved = preserved.clone();
        assert!(service.library().await.instances.is_empty());
        assert!(root.join("profiles").join(&record.id).exists());
        drop(service);
        std::fs::write(&database, b"second damage").unwrap();
        let service = LauncherService::open(&root, Scripted::default()).unwrap();
        let [
            RecoveryNote::LibraryRecovered {
                preserved: second, ..
            },
        ] = service.startup_notes()
        else {
            panic!("{:?}", service.startup_notes());
        };
        assert_ne!(*second, preserved);
        assert_eq!(std::fs::read(second).unwrap(), b"second damage");
        assert_eq!(std::fs::read(&preserved).unwrap(), b"first damage");
        drop(dir);
    }

    #[tokio::test]
    async fn newer_schema_refuses_to_open_and_leaves_the_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("launcher");
        drop(LauncherService::open(&root, Scripted::default()).unwrap());
        {
            let db = rusqlite::Connection::open(root.join("launcher.db")).unwrap();
            db.pragma_update(None, "user_version", 99).unwrap();
        }
        let before = std::fs::read(root.join("launcher.db")).unwrap();
        let refused = LauncherService::open(&root, Scripted::default());
        assert!(matches!(
            refused,
            Err(ServiceError::Store(StoreError::NewerSchema(99)))
        ));
        assert_eq!(std::fs::read(root.join("launcher.db")).unwrap(), before);
        assert!(!root.join("launcher.db.broken").exists());
        // The refused open must not keep the root locked.
        assert!(matches!(
            LauncherService::open(&root, Scripted::default()),
            Err(ServiceError::Store(StoreError::NewerSchema(99)))
        ));
    }

    #[tokio::test]
    async fn damaged_settings_and_torn_activity_lines_are_reported_with_counts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("launcher");
        drop(LauncherService::open(&root, Scripted::default()).unwrap());
        std::fs::write(root.join("settings.json"), b"{ nope").unwrap();
        let good = FinishedTask {
            category: TaskCategory::Install,
            label: "ok".into(),
            instance_id: None,
            started: 1,
            finished: 2,
            outcome: TaskOutcome::Succeeded,
            retry: None,
        };
        let mut lines = serde_json::to_string(&good).unwrap();
        lines.push_str("\n{torn\nnot even json\n");
        std::fs::write(root.join("activity.jsonl"), lines).unwrap();
        let service = LauncherService::open(&root, Scripted::default()).unwrap();
        let notes = service.startup_notes();
        assert!(notes.iter().any(|note| matches!(
            note,
            RecoveryNote::SettingsRecovered { preserved } if std::fs::read(preserved).unwrap() == b"{ nope"
        )));
        assert!(notes.contains(&RecoveryNote::ActivityLogSkipped { count: 2 }));
        assert_eq!(service.activity(10).finished.len(), 1);
    }

    /// Restarts an already-owned service (same transport is not needed here).
    fn reopen_service(
        service: LauncherService<Scripted>,
        root: &std::path::Path,
    ) -> (LauncherService<Scripted>, ()) {
        drop(service);
        (
            LauncherService::open(root, Scripted::default()).unwrap(),
            (),
        )
    }

    /// Closes `world`'s service and opens the same root again, as a restart.
    fn reopen(
        world: World,
        root: &std::path::Path,
    ) -> (LauncherService<Scripted>, tempfile::TempDir) {
        let World {
            service, net, _dir, ..
        } = world;
        drop(service);
        (LauncherService::open(root, net).unwrap(), _dir)
    }

    #[tokio::test]
    async fn clearing_finished_work_empties_the_history_and_keeps_running_tasks() {
        let world = world();
        let finished = |label: &str| FinishedTask {
            category: TaskCategory::Download,
            label: label.to_owned(),
            instance_id: None,
            started: 1,
            finished: 2,
            outcome: TaskOutcome::Succeeded,
            retry: None,
        };
        world.service.log.append(&finished("one")).unwrap();
        world.service.log.append(&finished("two")).unwrap();
        let running =
            world
                .service
                .board
                .lock()
                .unwrap()
                .start(TaskCategory::Install, "running", None, 5);
        assert_eq!(world.service.activity(10).finished.len(), 2);

        world.service.clear_finished().unwrap();

        let view = world.service.activity(10);
        assert!(view.finished.is_empty());
        assert_eq!(view.active.len(), 1);
        assert_eq!(view.active[0].id, running);
    }

    #[tokio::test]
    async fn a_failed_download_remembers_its_input_and_runs_again_from_it() {
        let world = world();
        let record = world
            .service
            .create_instance("Retry", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        assert!(
            world
                .service
                .install_content(
                    &record.id,
                    ProjectKind::Mod,
                    "cool",
                    CancellationToken::new()
                )
                .await
                .is_err()
        );
        let failed = world.service.activity(10).finished.remove(0);
        assert!(matches!(failed.outcome, TaskOutcome::Failed(_)));
        let action = failed.retry.expect("a failed install can be retried");
        assert_eq!(
            action,
            RetryAction::InstallContent {
                instance: record.id.clone(),
                kind: ProjectKind::Mod,
                project: "cool".into(),
                version: None,
            }
        );

        // The network is back: the same input now works.
        world
            .net
            .answer("/v2/project/cool/version", mod_version(&world));
        let (updates, _seen) = mpsc::unbounded_channel();
        world
            .service
            .retry_task(action, updates, CancellationToken::new())
            .await
            .unwrap();
        assert!(
            world
                .service
                .layout
                .game(&record.id)
                .join("mods/cool.jar")
                .is_file()
        );
        let view = world.service.activity(10);
        assert_eq!(
            view.finished.len(),
            2,
            "the first failure stays in the history"
        );
        assert_eq!(view.finished[0].outcome, TaskOutcome::Succeeded);
    }

    #[tokio::test]
    async fn the_bytes_of_all_transfers_add_up_to_one_task_and_an_unknown_length_hides_the_total() {
        use crate::transfer::TransferEvent;
        let board = StdMutex::new(TaskBoard::default());
        let task = board
            .lock()
            .unwrap()
            .start(TaskCategory::Download, "x", None, 1);
        let (events, receiver) = tokio::sync::broadcast::channel(16);
        let progress = |id: &str, completed, total| TransferEvent::Progress {
            id: id.to_owned(),
            completed,
            total,
        };
        events.send(progress("a", 10, Some(100))).unwrap();
        events.send(progress("b", 5, Some(50))).unwrap();
        events.send(progress("a", 40, Some(100))).unwrap();
        drop(events);
        meter_bytes(&board, task, receiver).await;
        let shown = board.lock().unwrap().active()[0].clone();
        assert_eq!(shown.unit, crate::ProgressUnit::Bytes);
        assert_eq!(shown.progress, Some((45, 150)));

        let task = board
            .lock()
            .unwrap()
            .start(TaskCategory::Download, "y", None, 1);
        let (events, receiver) = tokio::sync::broadcast::channel(16);
        events.send(progress("c", 7, None)).unwrap();
        events.send(progress("d", 3, Some(30))).unwrap();
        drop(events);
        meter_bytes(&board, task, receiver).await;
        let shown = board
            .lock()
            .unwrap()
            .active()
            .iter()
            .find(|t| t.id == task)
            .cloned()
            .unwrap();
        assert_eq!(shown.progress, Some((10, 0)), "total 0 means unknown");
    }

    #[tokio::test]
    async fn cancel_task_by_id_stops_only_that_running_task_and_expires_with_it() {
        let world = world();
        let record = world
            .service
            .create_instance("Target", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let defaults = world.service.settings.lock().await;
        let mut first = Box::pin(world.service.install_content(
            &record.id,
            ProjectKind::Mod,
            "one",
            CancellationToken::new(),
        ));
        assert!(futures_util::poll!(&mut first).is_pending());
        let mut second = Box::pin(
            world
                .service
                .install_modpack("pack", CancellationToken::new()),
        );
        assert!(futures_util::poll!(&mut second).is_pending());
        let view = world.service.activity(10);
        assert_eq!(view.active.len(), 2);
        assert_eq!(view.cancellable.len(), 2);
        let install_id = view
            .active
            .iter()
            .find(|task| task.category == TaskCategory::Download)
            .unwrap()
            .id;
        assert!(!world.service.cancel_task(install_id + 100));
        assert!(world.service.cancel_task(install_id));
        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(2), first)
                .await
                .unwrap(),
            Err(ServiceError::Cancelled)
        ));
        // The other task is untouched and still cancellable; the ended one is not.
        assert!(futures_util::poll!(&mut second).is_pending());
        let view = world.service.activity(10);
        assert_eq!(view.active.len(), 1);
        assert!(!view.cancellable.contains(&install_id));
        assert!(!world.service.cancel_task(install_id));
        drop(second);
        let view = world.service.activity(10);
        assert!(view.active.is_empty() && view.cancellable.is_empty());
        drop(defaults);
    }

    #[tokio::test]
    async fn cancelled_transfer_leaves_nothing_behind_and_allows_retry() {
        let world = world();
        let record = world
            .service
            .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        world
            .net
            .answer("/v2/project/cool/version", mod_version(&world));
        let target = world.service.layout.game(&record.id).join("mods/cool.jar");
        let cancel = CancellationToken::new();
        *world.net.cancel_on_file.lock().unwrap() = Some(cancel.clone());
        assert!(matches!(
            world
                .service
                .install_content(&record.id, ProjectKind::Mod, "cool", cancel)
                .await,
            Err(ServiceError::Cancelled)
        ));
        assert!(!target.exists());
        let leftovers: Vec<_> = std::fs::read_dir(target.parent().unwrap())
            .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
            .unwrap_or_default();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert!(change_subjects(&world, &record.id).is_empty());
        assert_eq!(
            world.service.activity(10).finished[0].outcome,
            TaskOutcome::Cancelled
        );
        world
            .service
            .install_content(
                &record.id,
                ProjectKind::Mod,
                "cool",
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
        let activity = world.service.activity(10);
        assert!(activity.active.is_empty());
        assert_eq!(activity.finished.len(), 2);
        assert!(
            activity
                .finished
                .iter()
                .any(|task| task.outcome == TaskOutcome::Succeeded)
        );
    }

    #[tokio::test]
    async fn switching_versions_replaces_the_file_keeps_it_disabled_and_never_clobbers() {
        let world = world();
        let record = world
            .service
            .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        world
            .net
            .answer("/v2/project/cool/version", mod_version(&world));
        let mods = world.service.layout.game(&record.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("cool-0.9.jar.disabled"), b"old build").unwrap();
        // Another file already called like the new version is the user's.
        std::fs::write(mods.join("cool.jar"), b"hand made").unwrap();
        let switch = || {
            world.service.switch_content_version(
                &record.id,
                ProjectKind::Mod,
                "cool-0.9.jar.disabled",
                "cool",
                "v1",
                CancellationToken::new(),
            )
        };
        assert!(matches!(
            switch().await,
            Err(ServiceError::Content(ContentError::Conflict(_)))
        ));
        assert_eq!(
            std::fs::read(mods.join("cool-0.9.jar.disabled")).unwrap(),
            b"old build"
        );
        assert_eq!(std::fs::read(mods.join("cool.jar")).unwrap(), b"hand made");

        std::fs::remove_file(mods.join("cool.jar")).unwrap();
        switch().await.unwrap();
        assert!(
            !mods.join("cool-0.9.jar.disabled").exists(),
            "the old file went"
        );
        assert_eq!(
            std::fs::read(mods.join("cool.jar.disabled")).unwrap(),
            b"a mod",
            "the new version, still switched off"
        );
        assert!(
            change_subjects(&world, &record.id)
                .contains(&(ChangeKind::ContentUpdated, "cool.jar".to_owned()))
        );
        // An unknown version is refused.
        assert!(matches!(
            world
                .service
                .switch_content_version(
                    &record.id,
                    ProjectKind::Mod,
                    "cool.jar.disabled",
                    "cool",
                    "nope",
                    CancellationToken::new(),
                )
                .await,
            Err(ServiceError::NoCompatibleVersion)
        ));
    }

    #[tokio::test]
    async fn install_never_replaces_a_same_named_file_with_other_content() {
        let world = world();
        let record = world
            .service
            .create_instance("Mods", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        world
            .net
            .answer("/v2/project/cool/version", mod_version(&world));
        let target = world.service.layout.game(&record.id).join("mods/cool.jar");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"my own build").unwrap();
        let install = || {
            world.service.install_content(
                &record.id,
                ProjectKind::Mod,
                "cool",
                CancellationToken::new(),
            )
        };
        assert!(matches!(
            install().await,
            Err(ServiceError::Content(ContentError::Conflict(_)))
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"my own build");
        assert!(change_subjects(&world, &record.id).is_empty());
        // Removing it on purpose clears the way.
        world
            .service
            .delete_content(&record.id, ProjectKind::Mod, &["cool.jar".to_owned()])
            .await
            .unwrap();
        install().await.unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
        // The same content again is simply there already.
        install().await.unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
    }

    #[tokio::test]
    async fn dropped_install_finishes_activity_and_releases_its_instance() {
        let world = world();
        let record = world
            .service
            .create_instance("Target", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let defaults = world.service.settings.lock().await;
        let mut install = Box::pin(world.service.install_content(
            &record.id,
            ProjectKind::Mod,
            "first",
            CancellationToken::new(),
        ));
        assert!(futures_util::poll!(&mut install).is_pending());
        assert_eq!(world.service.activity(10).active.len(), 1);
        drop(install);
        let activity = world.service.activity(10);
        assert!(activity.active.is_empty());
        assert_eq!(activity.finished.len(), 1);
        assert!(matches!(
            activity.finished[0].outcome,
            TaskOutcome::Failed(_)
        ));
        assert_eq!(
            activity.finished[0].instance_id.as_deref(),
            Some(record.id.as_str())
        );
        let mut retry = Box::pin(world.service.install_content(
            &record.id,
            ProjectKind::Mod,
            "second",
            CancellationToken::new(),
        ));
        assert!(futures_util::poll!(&mut retry).is_pending());
        drop(retry);
        drop(defaults);
        assert_eq!(world.service.activity(10).finished.len(), 2);
        assert!(world.service.activity(10).active.is_empty());
    }

    #[tokio::test]
    async fn log_failure_keeps_terminal_results_visible_without_resurrecting_tasks() {
        let world = world();
        let log_path = world.service.layout.root().join("activity.jsonl");
        std::fs::create_dir(&log_path).unwrap();
        let result: Result<(), ServiceError> = Ok(());
        for _ in 0..2 {
            let task =
                world
                    .service
                    .begin(TaskCategory::Install, "same operation".into(), None, None);
            world.service.end(task, &result);
        }
        let activity = world.service.activity(10);
        assert!(activity.active.is_empty());
        assert_eq!(activity.finished.len(), 2);
        assert!(
            activity
                .finished
                .iter()
                .all(|task| task.outcome == TaskOutcome::Succeeded)
        );
        assert!(world.service.activity(0).finished.is_empty());
        std::fs::remove_dir(log_path).unwrap();
        assert_eq!(world.service.activity(10).finished.len(), 2);
        assert!(world.service.activity(10).active.is_empty());
    }

    #[tokio::test]
    async fn pending_launch_rejects_conflicting_writes_and_drop_releases_target() {
        let world = world();
        let mut records = Vec::new();
        for name in ["Target", "Independent"] {
            records.push(
                world
                    .service
                    .create_instance(name, Some("1.0"), Loader::Vanilla, None)
                    .await
                    .unwrap(),
            );
        }
        let target = &records[0].id;
        // Pause launch at the settings boundary after it reserves its target.
        let defaults = world.service.settings.lock().await;
        let (updates, _receiver) = mpsc::unbounded_channel();
        let mut launch = Box::pin(
            world
                .service
                .launch(target, updates, CancellationToken::new()),
        );
        assert!(futures_util::poll!(&mut launch).is_pending());
        assert!(
            matches!(world.service.delete_instance(target).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
        );
        assert!(
            matches!(world.service.install_content(target, ProjectKind::Mod, "project", CancellationToken::new()).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
        );
        let (updates, _receiver) = mpsc::unbounded_channel();
        assert!(
            matches!(world.service.launch(target, updates, CancellationToken::new()).await, Err(ServiceError::InstanceBusy(id)) if id == *target)
        );
        assert_eq!(world.service.instance(target).await.unwrap().name, "Target");
        world.service.rename(target, "Renamed").await.unwrap();
        world.service.delete_instance(&records[1].id).await.unwrap();
        assert!(world.service.instance(target).await.is_ok());
        drop(launch);
        drop(defaults);
        world.service.delete_instance(target).await.unwrap();
        assert!(matches!(
            world.service.instance(target).await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    #[tokio::test]
    async fn failed_launch_does_not_leave_an_instance_reserved() {
        let world = world();
        let (updates, _receiver) = mpsc::unbounded_channel();
        assert!(matches!(
            world
                .service
                .launch("missing", updates, CancellationToken::new())
                .await,
            Err(ServiceError::NoSuchInstance(_))
        ));
        assert!(matches!(
            world.service.delete_instance("missing").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    #[tokio::test]
    async fn registered_instance_details_survive_rename_and_missing_profile() {
        let world = world();
        let record = world
            .service
            .store
            .lock()
            .await
            .create(
                NewInstance {
                    name: "Details".into(),
                    game_version: "1.21.1".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                },
                1,
            )
            .unwrap()
            .clone();
        world.service.set_favorite(&record.id, true).await.unwrap();
        {
            let mut store = world.service.store.lock().await;
            store.create_collection("Keep").unwrap();
            store.set_membership("Keep", &record.id, true).unwrap();
        }
        assert!(!world.service.layout().profile(&record.id).exists());
        world.service.rename(&record.id, "生存游戏").await.unwrap();
        let detail = world.service.instance(&record.id).await.unwrap();
        assert_eq!(detail.name, "生存游戏");
        assert_eq!(detail.id, record.id);
        assert!(detail.favorite);
        assert_eq!(
            world.service.library().await.collections[0].members,
            std::slice::from_ref(&record.id)
        );
        let root = world.service.layout().root().to_path_buf();
        drop(world.service);
        let reopened = LauncherService::open(root, world.net.clone()).unwrap();
        assert_eq!(reopened.instance(&record.id).await.unwrap(), detail);
        assert!(matches!(
            reopened.instance("missing").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    #[tokio::test]
    async fn instance_memory_overrides_are_validated_and_can_resume_inheritance() {
        let world = world();
        let record = world
            .service
            .store
            .lock()
            .await
            .create(
                NewInstance {
                    name: "Memory".into(),
                    game_version: "1.0".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                },
                1,
            )
            .unwrap()
            .clone();
        world
            .service
            .set_default_memory(Some(512), Some(2048))
            .await
            .unwrap();
        let settings = InstanceSettings {
            min_memory_mb: Some(4096),
            ..InstanceSettings::default()
        };
        assert!(matches!(
            world
                .service
                .update_instance_settings(&record.id, settings)
                .await,
            Err(ServiceError::Settings(SettingsError::InvalidMemory))
        ));
        assert_eq!(world.service.library().await.instances[0], record);
        let settings = InstanceSettings {
            max_memory_mb: Some(4096),
            ..InstanceSettings::default()
        };
        world
            .service
            .update_instance_settings(&record.id, settings.clone())
            .await
            .unwrap();
        assert_eq!(
            world.service.library().await.instances[0].settings,
            settings
        );
        world
            .service
            .update_instance_settings(&record.id, InstanceSettings::default())
            .await
            .unwrap();
        assert_eq!(
            world.service.library().await.instances[0].settings,
            InstanceSettings::default()
        );
        let settings = InstanceSettings {
            min_memory_mb: Some(1024),
            ..InstanceSettings::default()
        };
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await
            .unwrap();
        world
            .service
            .set_default_memory(Some(128), Some(512))
            .await
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        // There is no catalog answer: memory validation must win over network failure.
        assert!(matches!(
            world
                .service
                .launch(&record.id, tx, CancellationToken::new())
                .await,
            Err(ServiceError::Settings(SettingsError::InvalidMemory))
        ));
        assert!(!world.service.layout().game(&record.id).exists());
        let expected_library = world.service.library().await;
        let expected_settings = world.service.settings().await;
        let root = world.service.layout().root().to_path_buf();
        drop(world.service);
        let reopened = LauncherService::open(root, world.net.clone()).unwrap();
        assert_eq!(reopened.library().await, expected_library);
        assert_eq!(reopened.settings().await, expected_settings);
    }

    #[tokio::test]
    async fn service_settings_expose_only_saved_defaults_after_a_write_failure() {
        let world = world();
        world
            .service
            .set_default_memory(Some(512), Some(2048))
            .await
            .unwrap();
        let before = world.service.settings().await;
        std::fs::create_dir(world.service.layout().root().join("settings.json.tmp")).unwrap();
        assert!(
            world
                .service
                .set_default_memory(Some(1024), Some(4096))
                .await
                .is_err()
        );
        assert_eq!(world.service.settings().await, before);
    }

    #[tokio::test]
    async fn a_new_instance_defaults_to_the_newest_release_and_recommended_loader() {
        let world = world();
        publish_release(&world);
        world.net.answer(
            "meta.fabricmc.net/v2/versions/loader/1.0",
            r#"[{"loader":{"version":"0.17.0-beta","stable":false}},
                {"loader":{"version":"0.16.0","stable":true}}]"#,
        );
        let vanilla = world
            .service
            .create_instance("Plain", None, Loader::Vanilla, None)
            .await
            .unwrap();
        assert_eq!(vanilla.game_version, "1.0");
        assert_eq!(vanilla.loader_version, None);
        let fabric = world
            .service
            .create_instance("Modded", None, Loader::Fabric, None)
            .await
            .unwrap();
        assert_eq!(fabric.loader_version.as_deref(), Some("0.16.0"));
        assert_eq!(world.service.library().await.instances.len(), 2);

        // A chosen Forge build is recorded as chosen; installing it is the
        // launcher's job (its installer runs then).
        let forge = world
            .service
            .create_instance("Forge", Some("1.0"), Loader::Forge, Some("1.0.0"))
            .await
            .unwrap();
        assert_eq!(forge.loader, Loader::Forge);
        assert_eq!(forge.loader_version.as_deref(), Some("1.0.0"));
        assert_eq!(world.service.library().await.instances.len(), 3);
    }

    #[tokio::test]
    async fn an_unreachable_catalog_creates_nothing() {
        let world = world();
        let result = world
            .service
            .create_instance("X", None, Loader::Vanilla, None)
            .await;
        assert!(matches!(result, Err(ServiceError::Remote(_))));
        assert!(world.service.library().await.instances.is_empty());
    }

    #[tokio::test]
    async fn launching_installs_runs_records_and_then_works_offline() {
        let world = world();
        publish_release(&world);
        fake_java(&world, "echo \"Setting user: x\"");
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let launch = world
            .service
            .launch(&record.id, tx, CancellationToken::new());
        assert_send(&launch);
        let exit = launch.await.unwrap();
        assert_eq!(exit.code, Some(0));
        let mut session = LaunchSession::new();
        while let Ok(update) = rx.try_recv() {
            if let LaunchUpdate::Signal(signal) = update {
                session.apply(signal);
            }
        }
        assert!(session.is_finished());

        let library = world.service.library().await;
        let stored = &library.instances[0];
        assert!(stored.installed);
        assert!(stored.last_played.is_some());
        let history = HistoryLog::for_instance(world.service.layout().root(), &record.id)
            .sessions()
            .unwrap();
        assert_eq!(history.len(), 1);
        assert!(
            world
                .service
                .layout()
                .versions()
                .join("1.0/1.0.jar")
                .is_file()
        );

        // The network is gone; the installed release still launches.
        world.net.forget_all();
        let (tx, _rx) = mpsc::unbounded_channel();
        let again = world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(again.code, Some(0));
    }

    #[tokio::test]
    async fn creating_does_not_install_and_bad_requests_publish_nothing() {
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Only", None, Loader::Vanilla, None)
            .await
            .unwrap();
        assert!(!record.installed);
        assert!(!world.service.layout.versions().exists());
        assert!(world.service.activity(10).finished.is_empty());
        // With no catalog, "newest" cannot be resolved: no guessed version.
        world.net.forget_all();
        assert!(matches!(
            world
                .service
                .create_instance("Guess", None, Loader::Vanilla, None)
                .await,
            Err(ServiceError::Remote(_))
        ));
        assert_eq!(world.service.library().await.instances.len(), 1);
        // The same name again gets its own id and never touches the first profile.
        let again = world
            .service
            .create_instance("Only", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        assert_ne!(again.id, record.id);
    }

    async fn is_installed(world: &World, id: &str) -> bool {
        world.service.instance(id).await.unwrap().installed
    }

    #[tokio::test]
    async fn installing_is_separate_cancellable_and_only_success_marks_installed() {
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Pack", None, Loader::Vanilla, None)
            .await
            .unwrap();

        // Cancelled mid-download: not installed, recorded as cancelled.
        let cancel = CancellationToken::new();
        *world.net.cancel_on_file.lock().unwrap() = Some(cancel.clone());
        let (tx, mut rx) = mpsc::unbounded_channel();
        let cancelled = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            world.service.install_instance(&record.id, tx, cancel),
        )
        .await
        .expect("cancelling must not wait for a stalled download");
        assert!(matches!(
            cancelled,
            Err(ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled))
        ));
        assert!(!is_installed(&world, &record.id).await);
        assert_eq!(
            world.service.activity(10).finished[0].outcome,
            TaskOutcome::Cancelled
        );
        let mut session = LaunchSession::new();
        while let Ok(LaunchUpdate::Signal(signal)) = rx.try_recv() {
            session.apply(signal);
        }
        assert!(session.is_finished());

        // A failing download: not installed, recorded as failed with a reason.
        let client = world.server.join("client.jar");
        let kept = std::fs::read(&client).unwrap();
        std::fs::remove_file(&client).unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(
            world
                .service
                .install_instance(&record.id, tx, CancellationToken::new())
                .await
                .is_err()
        );
        assert!(!is_installed(&world, &record.id).await);
        assert!(matches!(
            world.service.activity(10).finished[0].outcome,
            TaskOutcome::Failed(_)
        ));

        // Retry succeeds; no game process or play session came with it.
        std::fs::write(&client, kept).unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .install_instance(&record.id, tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(is_installed(&world, &record.id).await);
        let stored = world.service.instance(&record.id).await.unwrap();
        assert!(stored.last_played.is_none());
        assert!(
            HistoryLog::for_instance(world.service.layout.root(), &record.id)
                .sessions()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            world.service.activity(10).finished[0].outcome,
            TaskOutcome::Succeeded
        );

        // Installed once, it launches with the network gone (AC-OFFLINE-01).
        fake_java(&world, "echo \"Setting user: x\"");
        world.net.forget_all();
        let (tx, _rx) = mpsc::unbounded_channel();
        let exit = world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(exit.code, Some(0));
    }

    async fn outcomes_of(world: &World, id: &str) -> Vec<SessionOutcome> {
        HistoryLog::for_instance(world.service.layout.root(), id)
            .sessions()
            .unwrap()
            .into_iter()
            .map(|event| match event {
                HistoryEvent::Session { outcome, .. } => outcome,
                HistoryEvent::Change { .. } => unreachable!("sessions() filters"),
            })
            .collect()
    }

    #[tokio::test]
    async fn every_way_a_launch_can_end_leaves_its_own_record() {
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Ends", None, Loader::Vanilla, None)
            .await
            .unwrap();

        // No Java: preparation fails before any process exists.
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(
            world
                .service
                .launch(&record.id, tx, CancellationToken::new())
                .await
                .is_err()
        );
        // Cancelled before it starts: no process, its own outcome.
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(world.service.launch(&record.id, tx, cancel).await.is_err());
        let stored = world.service.instance(&record.id).await.unwrap();
        assert!(stored.last_played.is_none());
        assert_eq!(stored.play_seconds, 0);

        // Dies at once, crashes after starting, exits cleanly.
        for script in [
            "exit 1",
            "echo \"Setting user: x\"; exit 3",
            "echo \"Setting user: x\"",
        ] {
            fake_java(&world, script);
            let (tx, _rx) = mpsc::unbounded_channel();
            world
                .service
                .launch(&record.id, tx, CancellationToken::new())
                .await
                .unwrap();
        }

        // Running, then the user stops it: the process really ends.
        fake_java(&world, "echo \"Setting user: x\"; sleep 30");
        let stop = CancellationToken::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let launch = world.service.launch(&record.id, tx, stop.clone());
        let stopper = async {
            while let Some(update) = rx.recv().await {
                if matches!(update, LaunchUpdate::Signal(LaunchSignal::Running)) {
                    stop.cancel();
                    break;
                }
            }
            // Keep the receiver open so later updates do not fail the send.
            while rx.recv().await.is_some() {}
        };
        let (exit, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            tokio::join!(launch, stopper)
        })
        .await
        .expect("stopping must end the process promptly");
        let exit = exit.unwrap();
        assert!(exit.killed && exit.was_running);

        assert_eq!(
            outcomes_of(&world, &record.id).await,
            [
                SessionOutcome::Stopped,
                SessionOutcome::Clean,
                SessionOutcome::Crashed,
                SessionOutcome::FailedToStart,
                SessionOutcome::Cancelled,
                SessionOutcome::FailedToPrepare,
            ]
        );
        // Nothing is left holding the instance.
        assert!(world.service.reserve_instance(&record.id).is_ok());
    }

    #[tokio::test]
    async fn a_launcher_that_died_mid_launch_reports_the_session_and_touches_no_process() {
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Died", None, Loader::Vanilla, None)
            .await
            .unwrap();
        // While a launch is in progress the marker exists; a normal return removes it.
        fake_java(&world, "echo \"Setting user: x\"");
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(!world.service.layout.sessions().join("died.json").exists());
        // A launcher killed mid-launch cannot clean up: rehearse by leaving the marker.
        let marker = SessionMarker::place(&world.service.layout, &record.id, 42).unwrap();
        std::mem::forget(marker);
        let root = world.service.layout.root().to_path_buf();
        let (service, _dir) = reopen(world, &root);
        assert!(
            service
                .startup_notes()
                .contains(&RecoveryNote::SessionInterrupted {
                    instance_id: record.id.clone(),
                    started: 42
                })
        );
        let history = HistoryLog::for_instance(&root, &record.id)
            .sessions()
            .unwrap();
        assert!(matches!(
            history[..],
            [
                HistoryEvent::Session {
                    started: 42,
                    outcome: SessionOutcome::Interrupted,
                    ..
                },
                ..
            ]
        ));
        assert!(!service.layout.sessions().join("died.json").exists());
        // Reported once; nothing was relaunched.
        let (service, _dir) = reopen_service(service, &root);
        assert!(service.startup_notes().is_empty());
        assert_eq!(
            service.instance(&record.id).await.unwrap().play_seconds,
            0,
            "an interrupted session counts no play time"
        );
        // An id that is not plain is refused before any file is named after it.
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(matches!(
            service
                .launch("../escape", tx, CancellationToken::new())
                .await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    fn change_subjects(world: &World, id: &str) -> Vec<(ChangeKind, String)> {
        HistoryLog::for_instance(world.service.layout.root(), id)
            .changes()
            .unwrap()
            .into_iter()
            .map(|event| match event {
                HistoryEvent::Change { kind, subject, .. } => (kind, subject),
                HistoryEvent::Session { .. } => unreachable!("changes() filters"),
            })
            .collect()
    }

    #[tokio::test]
    async fn content_changes_report_each_file_and_record_only_real_changes() {
        let world = world();
        let record = instance_with_profile(&world, "Mods").await;
        let mods = world.service.layout.game(&record.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("a.jar"), b"a").unwrap();
        std::fs::write(mods.join("b.jar"), b"b").unwrap();
        std::fs::write(mods.join("b.jar.disabled"), b"twin").unwrap();
        let names = |list: &[&str]| list.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();

        let results = world
            .service
            .set_content_state(
                &record.id,
                ProjectKind::Mod,
                &names(&["a.jar", "b.jar.disabled", "ghost.jar", "../x.jar"]),
                true,
            )
            .await
            .unwrap();
        // a is already enabled; b.jar.disabled collides with b.jar; two are refused.
        assert_eq!(
            results[0].outcome.as_ref().unwrap(),
            &ContentEffect::Unchanged
        );
        assert!(matches!(results[1].outcome, Err(ContentError::Conflict(_))));
        assert!(matches!(results[2].outcome, Err(ContentError::NotFound(_))));
        assert!(matches!(
            results[3].outcome,
            Err(ContentError::UnsafeFileName(_))
        ));
        assert!(change_subjects(&world, &record.id).is_empty());

        let results = world
            .service
            .set_content_state(&record.id, ProjectKind::Mod, &names(&["a.jar"]), false)
            .await
            .unwrap();
        assert_eq!(
            results[0].outcome.as_ref().unwrap(),
            &ContentEffect::Renamed("a.jar.disabled".into())
        );
        assert!(results[0].recorded);
        let listed = world
            .service
            .content(&record.id, ProjectKind::Mod)
            .await
            .unwrap();
        assert!(
            listed
                .iter()
                .any(|item| item.display_name == "a.jar" && !item.enabled)
        );

        let results = world
            .service
            .delete_content(
                &record.id,
                ProjectKind::Mod,
                &names(&["b.jar.disabled", "b.jar"]),
            )
            .await
            .unwrap();
        assert!(results.iter().all(|result| result.outcome.is_ok()));
        assert!(!mods.join("b.jar").exists() && !mods.join("b.jar.disabled").exists());
        assert_eq!(
            change_subjects(&world, &record.id),
            [
                (ChangeKind::ContentRemoved, "b.jar".to_owned()),
                (ChangeKind::ContentRemoved, "b.jar.disabled".to_owned()),
                (ChangeKind::ContentDisabled, "a.jar".to_owned()),
            ]
        );
        // Other instances and unknown ones are not reachable through this call.
        assert!(matches!(
            world.service.content("ghost", ProjectKind::Mod).await,
            Err(ServiceError::NoSuchInstance(_))
        ));
        assert!(matches!(
            world
                .service
                .set_content_state(&record.id, ProjectKind::Modpack, &names(&["a"]), true)
                .await
                .unwrap()[0]
                .outcome,
            Err(ContentError::NotAFileKind)
        ));
    }

    #[tokio::test]
    async fn content_writes_wait_for_a_launching_instance_but_reads_do_not() {
        let world = world();
        publish_release(&world);
        fake_java(&world, "echo \"Setting user: x\"; sleep 30");
        let record = world
            .service
            .create_instance("Busy", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let mods = world.service.layout.game(&record.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("a.jar"), b"a").unwrap();
        let stop = CancellationToken::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let launch = world.service.launch(&record.id, tx, stop.clone());
        let probe = async {
            while let Some(update) = rx.recv().await {
                if matches!(update, LaunchUpdate::Signal(LaunchSignal::Running)) {
                    break;
                }
            }
            let refused = world
                .service
                .set_content_state(&record.id, ProjectKind::Mod, &["a.jar".to_owned()], false)
                .await;
            assert!(matches!(refused, Err(ServiceError::InstanceBusy(_))));
            assert!(matches!(
                world
                    .service
                    .delete_content(&record.id, ProjectKind::Mod, &["a.jar".to_owned()])
                    .await,
                Err(ServiceError::InstanceBusy(_))
            ));
            assert_eq!(
                world
                    .service
                    .content(&record.id, ProjectKind::Mod)
                    .await
                    .unwrap()
                    .len(),
                1
            );
            stop.cancel();
            while rx.recv().await.is_some() {}
        };
        let (exit, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            tokio::join!(launch, probe)
        })
        .await
        .unwrap();
        exit.unwrap();
        assert!(mods.join("a.jar").exists());
        // Once the game is gone the same change goes through.
        world
            .service
            .set_content_state(&record.id, ProjectKind::Mod, &["a.jar".to_owned()], false)
            .await
            .unwrap();
        assert!(mods.join("a.jar.disabled").exists());
    }

    #[tokio::test]
    async fn history_and_problems_are_readable_without_the_lease() {
        let world = world();
        let record = world
            .service
            .create_instance("Read", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let saves = world.service.layout.game(&record.id).join("saves");
        std::fs::create_dir_all(saves.join("w")).unwrap();
        world
            .service
            .copy_world(&record.id, "w", None)
            .await
            .unwrap();
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        let read = world.service.history(&record.id).await.unwrap();
        assert!(matches!(
            read.events[..],
            [HistoryEvent::Change {
                kind: ChangeKind::WorldCopied,
                ..
            }]
        ));
        let problems = world.service.problems(&record.id).await.unwrap();
        assert!(
            problems
                .iter()
                .any(|problem| problem.kind == crate::diagnostics::ProblemKind::NotInstalled)
        );
        assert!(matches!(
            world.service.history("nobody").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
        assert!(matches!(
            world.service.problems("nobody").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    #[tokio::test]
    async fn logs_and_crash_reports_are_read_safely_without_the_lease() {
        let world = world();
        let record = world
            .service
            .create_instance("Logs", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        let none = world.service.logs(&record.id).await.unwrap();
        assert_eq!((none.latest, none.crashes.len()), (None, 0));
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::create_dir_all(game.join("crash-reports")).unwrap();
        let big = "x\n".repeat(LOG_TAIL_BYTES as usize);
        std::fs::write(game.join("logs/latest.log"), &big).unwrap();
        std::fs::write(
            game.join("crash-reports/crash-1.txt"),
            "java.lang.OutOfMemoryError: heap",
        )
        .unwrap();
        let _busy = world.service.reserve_instance(&record.id).unwrap();
        let logs = world.service.logs(&record.id).await.unwrap();
        assert!(logs.latest.unwrap().len() as u64 <= LOG_TAIL_BYTES);
        assert_eq!(logs.crashes[0].file_name, "crash-1.txt");
        let (text, hints) = world
            .service
            .crash_report(&record.id, "crash-1.txt")
            .await
            .unwrap();
        assert!(text.contains("OutOfMemory"));
        assert_eq!(hints, vec![crate::diagnostics::CrashHint::OutOfMemory]);
        assert!(
            world
                .service
                .crash_report(&record.id, "../../launcher.db")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn worlds_are_copied_and_deleted_under_the_lease_and_recorded() {
        let world = world();
        let record = world
            .service
            .create_instance("Worlds", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let saves = world.service.layout.game(&record.id).join("saves");
        std::fs::create_dir_all(saves.join("w/region")).unwrap();
        std::fs::write(saves.join("w/level.dat"), b"not nbt").unwrap();
        std::fs::write(saves.join("w/region/r.0.0.mca"), b"chunks").unwrap();

        let listed = world.service.worlds(&record.id).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].damaged, "a damaged world is still listed");

        // While the instance is busy, writes are refused and reads still work.
        {
            let _busy = world.service.reserve_instance(&record.id).unwrap();
            assert!(matches!(
                world.service.copy_world(&record.id, "w", None).await,
                Err(ServiceError::InstanceBusy(_))
            ));
            assert!(matches!(
                world.service.delete_world(&record.id, "w").await,
                Err(ServiceError::InstanceBusy(_))
            ));
            assert_eq!(world.service.worlds(&record.id).await.unwrap().len(), 1);
        }
        assert!(saves.join("w").is_dir());

        // A leftover half-copy is swept, the copy is whole, and nothing is overwritten.
        std::fs::create_dir_all(saves.join(".w copy.copying")).unwrap();
        let first = world
            .service
            .copy_world(&record.id, "w", None)
            .await
            .unwrap();
        let second = world
            .service
            .copy_world(&record.id, "w", None)
            .await
            .unwrap();
        assert_eq!((first.as_str(), second.as_str()), ("w copy", "w copy 2"));
        assert!(!saves.join(".w copy.copying").exists());
        assert_eq!(
            std::fs::read(saves.join("w copy/region/r.0.0.mca")).unwrap(),
            b"chunks"
        );
        assert!(matches!(
            world
                .service
                .copy_world(&record.id, "w", Some("w copy"))
                .await,
            Err(ServiceError::World(WorldError::AlreadyExists(_)))
        ));
        assert!(matches!(
            world.service.copy_world(&record.id, "ghost", None).await,
            Err(ServiceError::World(WorldError::NotFound(_)))
        ));

        world
            .service
            .delete_world(&record.id, "w copy")
            .await
            .unwrap();
        assert!(!saves.join("w copy").exists());
        assert!(saves.join("w").is_dir() && saves.join("w copy 2").is_dir());
        assert_eq!(
            change_subjects(&world, &record.id),
            [
                (ChangeKind::WorldDeleted, "w copy".to_owned()),
                (ChangeKind::WorldCopied, "w copy 2".to_owned()),
                (ChangeKind::WorldCopied, "w copy".to_owned()),
            ]
        );
        assert!(matches!(
            world.service.worlds("ghost").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    #[tokio::test]
    async fn a_world_is_exported_and_imported_under_the_lease_and_the_import_is_recorded() {
        let world = world();
        let record = world
            .service
            .create_instance("Zip", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let saves = world.service.layout.game(&record.id).join("saves");
        std::fs::create_dir_all(saves.join("w")).unwrap();
        std::fs::write(saves.join("w/level.dat"), b"not nbt").unwrap();
        let archive = world._dir.path().join("w.zip");

        {
            let _busy = world.service.reserve_instance(&record.id).unwrap();
            assert!(matches!(
                world.service.export_world(&record.id, "w", &archive).await,
                Err(ServiceError::InstanceBusy(_))
            ));
            assert!(matches!(
                world.service.import_world(&record.id, &archive).await,
                Err(ServiceError::InstanceBusy(_))
            ));
        }
        assert!(!archive.exists());
        assert!(
            world
                .service
                .export_world(&record.id, "w", &archive)
                .await
                .unwrap()
                > 0
        );
        assert!(matches!(
            world
                .service
                .export_world(&record.id, "ghost", &archive)
                .await,
            Err(ServiceError::World(WorldError::NotFound(_)))
        ));

        let imported = world
            .service
            .import_world(&record.id, &archive)
            .await
            .unwrap();
        assert_eq!(imported, "w 2");
        assert!(saves.join("w 2/level.dat").is_file());
        assert_eq!(
            change_subjects(&world, &record.id),
            [(ChangeKind::WorldImported, "w 2".to_owned())]
        );
    }

    #[tokio::test]
    async fn an_exported_pack_carries_everything_when_modrinth_is_out_of_reach_and_needs_the_lease()
    {
        let world = world();
        let record = world
            .service
            .create_instance("Pack", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
        std::fs::write(game.join("options.txt"), b"fov:70").unwrap();
        let spec = crate::pack_export::ExportSpec {
            format: crate::pack_export::PackFormat::Modrinth,
            name: "Pack".into(),
            version: "1".into(),
            summary: None,
            include: vec!["mods".into(), "options.txt".into()],
        };
        let pack = world._dir.path().join("pack.mrpack");

        {
            let _busy = world.service.reserve_instance(&record.id).unwrap();
            assert!(matches!(
                world
                    .service
                    .export_modpack(&record.id, spec.clone(), &pack, CancellationToken::new())
                    .await,
                Err(ServiceError::InstanceBusy(_))
            ));
        }
        assert!(!pack.exists());
        let report = world
            .service
            .export_modpack(&record.id, spec.clone(), &pack, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!((report.linked, report.bundled), (0, 2));
        assert!(report.lookup_failed && report.size > 0);
        let index = crate::modpack::read_index(&pack).unwrap();
        assert_eq!(
            (index.name.as_str(), index.minecraft.as_str()),
            ("Pack", "1.0")
        );
        assert!(index.files.is_empty());

        let mut nothing = spec;
        nothing.include = vec!["saves".into()];
        assert!(matches!(
            world
                .service
                .export_modpack(&record.id, nothing, &pack, CancellationToken::new())
                .await,
            Err(ServiceError::Export(
                crate::pack_export::ExportError::NothingChosen
            ))
        ));
    }

    #[tokio::test]
    async fn a_games_size_counts_its_own_folder_only() {
        let world = world();
        let record = world
            .service
            .create_instance("Size", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        let before = world.service.instance_size(&record.id).await.unwrap();
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), vec![0_u8; 1000]).unwrap();
        std::fs::write(game.join("options.txt"), vec![0_u8; 24]).unwrap();
        assert_eq!(
            world.service.instance_size(&record.id).await.unwrap(),
            before + 1024
        );
        assert!(matches!(
            world.service.instance_size("ghost").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    /// Publishes a fake Java runtime the way Mojang's index does.
    fn publish_java(world: &World, bad_hash: bool) {
        let release = b"JAVA_VERSION=\"21.0.1\"\nIMPLEMENTOR=\"Test\"\n";
        let launcher = b"#!/bin/sh\necho java\n";
        std::fs::write(world.server.join("jrelease"), release).unwrap();
        std::fs::write(world.server.join("jjava"), launcher).unwrap();
        let hash = |bytes: &[u8]| {
            if bad_hash {
                "0".repeat(40)
            } else {
                sha1_hex(bytes)
            }
        };
        let manifest = format!(
            r#"{{"files":{{
                "bin":{{"type":"directory"}},
                "bin/java":{{"type":"file","executable":true,"downloads":{{"raw":
                    {{"url":"{}","sha1":"{}","size":{}}}}}}},
                "release":{{"type":"file","downloads":{{"raw":
                    {{"url":"{}","sha1":"{}","size":{}}}}}}},
                "bin/jre":{{"type":"link","target":"java"}}}}}}"#,
            file_url(&world.server.join("jjava")),
            hash(launcher),
            launcher.len(),
            file_url(&world.server.join("jrelease")),
            hash(release),
            release.len(),
        );
        std::fs::write(world.server.join("jmanifest.json"), &manifest).unwrap();
        let platform =
            crate::java_runtime::platform_key(&crate::environment::HostProfile::current())
                .expect("tests run on a platform with downloadable Java");
        let index = format!(
            r#"{{"{platform}":{{"java-runtime-delta":[{{"manifest":
                {{"url":"{}","sha1":"x","size":1}},"version":{{"name":"21.0.1"}}}}]}}}}"#,
            file_url(&world.server.join("jmanifest.json")),
        );
        world.net.answer("java-runtime/", index);
    }

    #[tokio::test]
    async fn a_whole_game_is_backed_up_and_restored_as_a_new_one_with_nothing_touched() {
        let world = world();
        let record = world
            .service
            .create_instance("生存", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        world
            .service
            .store
            .lock()
            .await
            .update_settings(
                &record.id,
                InstanceSettings {
                    max_memory_mb: Some(4096),
                    java_path: Some("/somewhere/java".into()),
                    ..InstanceSettings::default()
                },
            )
            .unwrap();
        let game = world.service.layout.game(&record.id);
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::create_dir_all(game.join("saves/W")).unwrap();
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
        std::fs::write(game.join("saves/W/level.dat"), b"world").unwrap();
        std::fs::write(game.join("logs/latest.log"), b"log").unwrap();
        let archive = world._dir.path().join("b.zip");

        {
            let _busy = world.service.reserve_instance(&record.id).unwrap();
            assert!(matches!(
                world
                    .service
                    .backup_instance(&record.id, &archive, CancellationToken::new())
                    .await,
                Err(ServiceError::InstanceBusy(_))
            ));
        }
        assert!(
            world
                .service
                .backup_instance(&record.id, &archive, CancellationToken::new())
                .await
                .unwrap()
                > 0
        );

        let restored = world
            .service
            .restore_backup(&archive, CancellationToken::new())
            .await
            .unwrap();
        assert_ne!(restored.id, record.id);
        assert_eq!(restored.name, "生存（恢复）");
        assert_eq!(
            (restored.game_version.as_str(), restored.loader),
            ("1.0", Loader::Fabric)
        );
        assert_eq!(restored.settings.max_memory_mb, Some(4096));
        assert_eq!(
            restored.settings.java_path, None,
            "a Java path is per machine"
        );
        assert!(!restored.installed);
        let there = world.service.layout.game(&restored.id);
        assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
        assert_eq!(
            std::fs::read(there.join("saves/W/level.dat")).unwrap(),
            b"world"
        );
        assert!(!there.join("logs").exists());
        // The original is as it was.
        assert_eq!(
            std::fs::read(game.join("saves/W/level.dat")).unwrap(),
            b"world"
        );
        assert_eq!(world.service.library().await.instances.len(), 2);

        // A file that is not a backup changes nothing.
        let junk = world._dir.path().join("junk.zip");
        std::fs::write(&junk, b"nope").unwrap();
        assert!(
            world
                .service
                .restore_backup(&junk, CancellationToken::new())
                .await
                .is_err()
        );
        assert_eq!(world.service.library().await.instances.len(), 2);
        let failed = world.service.activity(5).finished.remove(0);
        assert_eq!(
            failed.retry,
            Some(RetryAction::RestoreBackup {
                path: junk.display().to_string()
            })
        );
    }

    #[tokio::test]
    async fn a_game_from_another_launcher_becomes_a_new_one_and_the_source_stays_as_it_was() {
        let world = world();
        let source = world._dir.path().join("prism-inst");
        let write = |path: &str, body: &str| {
            let full = source.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, body).unwrap();
        };
        write("instance.cfg", "name=我的生存\n");
        write(
            "mmc-pack.json",
            r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},
                {"uid":"net.fabricmc.fabric-loader","version":"0.15.7"}]}"#,
        );
        write(".minecraft/mods/a.jar", "mod");
        write(".minecraft/saves/W/level.dat", "world");
        write(".minecraft/logs/latest.log", "log");

        let found = world.service.detect_games(&source).await.unwrap();
        assert_eq!(found.len(), 1);
        let record = world
            .service
            .import_game(found[0].clone(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(record.name, "我的生存");
        assert_eq!(
            (
                record.game_version.as_str(),
                record.loader,
                record.loader_version.as_deref()
            ),
            ("1.20.1", Loader::Fabric, Some("0.15.7"))
        );
        assert!(!record.installed);
        let there = world.service.layout.game(&record.id);
        assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
        assert_eq!(
            std::fs::read(there.join("saves/W/level.dat")).unwrap(),
            b"world"
        );
        assert!(!there.join("logs").exists());
        assert!(
            source.join(".minecraft/mods/a.jar").is_file(),
            "the source is untouched"
        );

        // Not a launcher's folder; a cancelled import; a modded game with no loader version.
        let empty = world._dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(world.service.detect_games(&empty).await.is_err());
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            world.service.import_game(found[0].clone(), cancel).await,
            Err(ServiceError::Cancelled)
        ));
        let mut nameless = found[0].clone();
        nameless.loader_version = None;
        assert!(
            world
                .service
                .import_game(nameless, CancellationToken::new())
                .await
                .is_err()
        );
        assert_eq!(
            world.service.library().await.instances.len(),
            1,
            "failures add no game"
        );
        let leftovers = std::fs::read_dir(world.service.layout.operations())
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert_eq!(leftovers, 0, "no staging folder is left behind");
    }

    #[tokio::test]
    async fn unused_shared_files_are_measured_then_removed_only_when_nothing_is_running() {
        let world = world();
        let record = world
            .service
            .create_instance("Keep", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let meta = world.service.layout.meta();
        let put = |path: &str, body: &str| {
            let full = meta.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, body).unwrap();
        };
        put(
            "versions/1.0/1.0.json",
            r#"{"id":"1.0","assets":"9","assetIndex":{"id":"9"},"libraries":[]}"#,
        );
        put(
            "assets/indexes/9.json",
            r#"{"objects":{"a":{"hash":"aa11"}}}"#,
        );
        put("assets/objects/aa/aa11", "used");
        put("assets/objects/bb/bb22", "unused-bytes");
        put("versions/0.9/0.9.json", "{}");

        let found = world.service.reclaimable().await.unwrap();
        let bytes = found.bytes();
        assert!(bytes > 0);
        assert!(found.unused.iter().any(|(p, _)| p.ends_with("bb22")));
        assert!(
            found
                .unused
                .iter()
                .any(|(p, _)| p.ends_with("versions/0.9"))
        );
        assert!(!found.unused.iter().any(|(p, _)| p.ends_with("aa11")));

        {
            let _busy = world.service.reserve_instance(&record.id).unwrap();
            assert!(matches!(
                world.service.reclaim().await,
                Err(ServiceError::InstanceBusy(_))
            ));
        }
        assert!(
            meta.join("assets/objects/bb/bb22").is_file(),
            "nothing removed while busy"
        );
        assert_eq!(world.service.reclaim().await.unwrap(), bytes);
        assert!(!meta.join("assets/objects/bb/bb22").exists());
        assert!(!meta.join("versions/0.9").exists());
        assert!(meta.join("assets/objects/aa/aa11").is_file());
        assert!(meta.join("versions/1.0/1.0.json").is_file());
        assert!(world.service.reclaimable().await.unwrap().unused.is_empty());
    }

    #[tokio::test]
    async fn a_game_exported_as_a_prism_zip_comes_back_in_through_the_pack_importer() {
        let world = world();
        let record = world
            .service
            .create_instance("Round", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
        std::fs::write(game.join("options.txt"), b"fov:70").unwrap();
        let spec = crate::pack_export::ExportSpec {
            format: crate::pack_export::PackFormat::Prism,
            name: "Round".into(),
            version: "1".into(),
            summary: None,
            include: vec!["mods".into(), "options.txt".into()],
        };
        let zip = world._dir.path().join("round.zip");
        let report = world
            .service
            .export_modpack(&record.id, spec, &zip, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!((report.linked, report.bundled), (0, 2));

        let back = world
            .service
            .import_modpack_file(&zip, CancellationToken::new())
            .await
            .unwrap();
        assert_ne!(back.id, record.id);
        assert_eq!(
            (
                back.game_version.as_str(),
                back.loader,
                back.loader_version.as_deref()
            ),
            ("1.0", Loader::Fabric, Some("0.16.0"))
        );
        let there = world.service.layout.game(&back.id);
        assert_eq!(std::fs::read(there.join("mods/a.jar")).unwrap(), b"mod");
        assert_eq!(std::fs::read(there.join("options.txt")).unwrap(), b"fov:70");
        // The scratch folder is gone, and the finished task knows the new game.
        let scratch = world.service.layout.root().join("cache");
        let leftovers = std::fs::read_dir(&scratch).map(|e| e.count()).unwrap_or(0);
        assert_eq!(leftovers, 0);
        assert_eq!(
            world.service.activity(5).finished[0].instance_id.as_deref(),
            Some(back.id.as_str())
        );
    }

    #[tokio::test]
    async fn java_is_installed_whole_found_by_discovery_and_a_bad_file_leaves_nothing() {
        let world = world();
        publish_java(&world, true);
        let runtimes = world.service.layout.runtimes();
        assert!(
            world
                .service
                .install_java(Some(21), CancellationToken::new())
                .await
                .is_err()
        );
        assert!(!runtimes.join("java-runtime-delta").exists());
        let staging = runtimes.join(".java-runtime-delta.installing");
        assert!(!staging.exists(), "a failed install leaves no half runtime");
        let failed = world.service.activity(5).finished.remove(0);
        assert!(matches!(failed.outcome, TaskOutcome::Failed(_)));
        assert_eq!(
            failed.retry,
            Some(RetryAction::InstallJava { major: Some(21) })
        );

        publish_java(&world, false);
        let java = world
            .service
            .install_java(Some(21), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(java.major(), 21);
        assert!(java.executable().is_file());
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(java.executable())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "the launcher is executable");
        }
        assert!(runtimes.join("java-runtime-delta/bin/jre").is_symlink());
        // The ordinary discovery finds it, and asking again changes nothing.
        let found = world.service.java_installations().await;
        assert!(
            found
                .iter()
                .any(|(runtime, _)| runtime.home().ends_with("java-runtime-delta"))
        );
        let again = world
            .service
            .install_java(Some(21), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(again.home(), java.home());
        assert!(matches!(
            world
                .service
                .install_java(Some(99), CancellationToken::new())
                .await,
            Err(ServiceError::Install(_))
        ));
    }

    #[tokio::test]
    async fn a_version_can_be_saved_where_the_person_chooses_and_can_be_retried() {
        let world = world();
        world
            .net
            .answer("/v2/project/cool/version", mod_version(&world));
        let target = world._dir.path().join("elsewhere.jar");

        let name = world
            .service
            .save_version_as("cool", "v1", &target, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(name, "cool.jar");
        assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
        assert!(world.service.activity(10).finished[0].retry.is_some());

        // An unknown version is an error.
        assert!(matches!(
            world
                .service
                .save_version_as("cool", "nope", &target, CancellationToken::new())
                .await,
            Err(ServiceError::NoCompatibleVersion)
        ));
    }

    #[tokio::test]
    async fn installed_projects_list_the_file_and_the_newer_version_if_any() {
        let world = world();
        let record = world
            .service
            .create_instance("Have", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        let mods = world.service.layout.game(&record.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("libx.jar"), b"libx").unwrap();
        std::fs::write(mods.join("mine.jar"), b"mine").unwrap();
        let version = |id: &str, hash: &str| {
            format!(
                r#"{{"id":"{id}","project_id":"LIBX","name":"x","version_number":"1",
                    "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                    "date_published":"2024-01-01T00:00:00Z",
                    "files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                               "primary":true,"size":4,"hashes":{{"sha1":"{hash}"}}}}],
                    "dependencies":[]}}"#
            )
        };
        let known = sha1_hex(b"libx");
        // More specific address first: answers are matched by containment.
        world.net.answer(
            "/v2/version_files/update",
            format!(r#"{{"{known}":{}}}"#, version("v-new", "newhash")),
        );
        world.net.answer(
            "/v2/version_files",
            format!(r#"{{"{known}":{}}}"#, version("v-old", &known)),
        );

        let have = world
            .service
            .installed_projects(&record.id, ProjectKind::Mod)
            .await
            .unwrap();
        assert_eq!(have.len(), 1, "a file Modrinth does not know is not listed");
        let libx = &have["LIBX"];
        assert_eq!(libx.file_name, "libx.jar");
        assert_eq!(libx.version_id, "v-old");
        assert_eq!(libx.update.as_deref(), Some("v-new"));

        // Modrinth out of reach is an error, not "nothing installed".
        world.net.forget_all();
        assert!(
            world
                .service
                .installed_projects(&record.id, ProjectKind::Mod)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn an_exported_log_hides_the_player_and_the_folders_and_leaves_nothing_half_written() {
        let world = world();
        let record = world
            .service
            .create_instance("Log", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::create_dir_all(game.join("crash-reports")).unwrap();
        std::fs::write(
            game.join("logs/latest.log"),
            format!(
                "[1] [main/INFO]: Setting user: Steve\n[2] [main/INFO]: dir {}\n",
                game.display()
            ),
        )
        .unwrap();
        std::fs::write(
            game.join("crash-reports/crash-1.txt"),
            "Player Steve crashed",
        )
        .unwrap();
        let out = world._dir.path().join("out.txt");

        world
            .service
            .export_log(&record.id, None, &out)
            .await
            .unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("Setting user: <player>") && text.contains("dir <game>"));
        assert!(!text.contains("Steve"));
        assert!(!world._dir.path().join("out.txt.part").exists());

        world
            .service
            .export_log(&record.id, Some("crash-1.txt"), &out)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "Player <player> crashed"
        );

        // No log, an unsafe name, or an unwritable place: an error and no file.
        let empty = world
            .service
            .create_instance("None", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let none = world._dir.path().join("none.txt");
        assert!(
            world
                .service
                .export_log(&empty.id, None, &none)
                .await
                .is_err()
        );
        assert!(
            world
                .service
                .export_log(&record.id, Some("../x"), &none)
                .await
                .is_err()
        );
        let blocked = world._dir.path().join("no-such-folder/out.txt");
        assert!(
            world
                .service
                .export_log(&record.id, None, &blocked)
                .await
                .is_err()
        );
        assert!(!none.exists());
    }

    #[tokio::test]
    async fn a_cancelled_export_is_recorded_as_cancelled_and_leaves_no_pack() {
        let world = world();
        let record = world
            .service
            .create_instance("Stop", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
        let spec = crate::pack_export::ExportSpec {
            format: crate::pack_export::PackFormat::Modrinth,
            name: "Stop".into(),
            version: "1".into(),
            summary: None,
            include: vec!["mods".into()],
        };
        let pack = world._dir.path().join("stop.mrpack");
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            world
                .service
                .export_modpack(&record.id, spec, &pack, cancel)
                .await,
            Err(ServiceError::Cancelled)
        ));
        assert!(!pack.exists() && !world._dir.path().join("stop.mrpack.part").exists());
        assert_eq!(
            world.service.activity(5).finished[0].outcome,
            TaskOutcome::Cancelled
        );
    }

    #[tokio::test]
    async fn snapshots_back_up_restore_and_undo_a_failed_restore_under_the_lease() {
        use std::os::unix::fs::PermissionsExt;
        let world = world();
        let record = world
            .service
            .create_instance("Backup", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        std::fs::create_dir_all(game.join("saves/W")).unwrap();
        std::fs::write(game.join("saves/W/level.dat"), b"v1").unwrap();
        std::fs::write(game.join("options.txt"), b"fov:70").unwrap();

        // Nothing to back up is an error, not an empty snapshot.
        let empty = world
            .service
            .create_instance("Empty", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        assert!(matches!(
            world
                .service
                .create_snapshot(&empty.id, SnapshotScope::Full, "")
                .await,
            Err(ServiceError::Snapshot(SnapshotError::Empty))
        ));

        let snapshot = world
            .service
            .create_snapshot(&record.id, SnapshotScope::Full, "before")
            .await
            .unwrap();
        assert_eq!(world.service.snapshots(&record.id).await.unwrap().len(), 1);
        std::fs::write(game.join("saves/W/level.dat"), b"v2").unwrap();
        std::fs::write(game.join("options.txt"), b"fov:110").unwrap();

        // Busy instance: writes refused, listing still works.
        {
            let _busy = world.service.reserve_instance(&record.id).unwrap();
            assert!(matches!(
                world.service.restore_snapshot(&record.id, &snapshot).await,
                Err(ServiceError::InstanceBusy(_))
            ));
            assert!(matches!(
                world.service.delete_snapshot(&record.id, &snapshot).await,
                Err(ServiceError::InstanceBusy(_))
            ));
            assert_eq!(world.service.snapshots(&record.id).await.unwrap().len(), 1);
        }

        // A restore that cannot finish leaves the game exactly as it was.
        std::fs::set_permissions(&game, std::fs::Permissions::from_mode(0o555)).unwrap();
        let failed = world.service.restore_snapshot(&record.id, &snapshot).await;
        std::fs::set_permissions(&game, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(failed, Err(ServiceError::Snapshot(_))));
        assert_eq!(
            std::fs::read(game.join("saves/W/level.dat")).unwrap(),
            b"v2"
        );
        assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"fov:110");
        assert!(operation_dirs(&world).is_empty());
        assert!(
            !change_subjects(&world, &record.id)
                .iter()
                .any(|(kind, _)| *kind == ChangeKind::SnapshotRestored)
        );

        let units = world
            .service
            .restore_snapshot(&record.id, &snapshot)
            .await
            .unwrap();
        assert!(units.contains(&std::path::PathBuf::from("saves/W")));
        assert_eq!(
            std::fs::read(game.join("saves/W/level.dat")).unwrap(),
            b"v1"
        );
        assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"fov:70");
        world
            .service
            .delete_snapshot(&record.id, &snapshot)
            .await
            .unwrap();
        assert!(
            world
                .service
                .snapshots(&record.id)
                .await
                .unwrap()
                .is_empty()
        );
        let kinds: Vec<_> = change_subjects(&world, &record.id)
            .into_iter()
            .map(|(kind, _)| kind)
            .collect();
        assert_eq!(
            kinds,
            [
                ChangeKind::SnapshotDeleted,
                ChangeKind::SnapshotRestored,
                ChangeKind::SnapshotCreated
            ]
        );
    }

    #[tokio::test]
    async fn a_restore_the_launcher_died_in_is_rolled_back_when_the_service_opens() {
        let world = world();
        let record = world
            .service
            .create_instance("Crash", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout.game(&record.id);
        std::fs::create_dir_all(game.join("saves/W")).unwrap();
        std::fs::write(game.join("saves/W/level.dat"), b"original").unwrap();
        // Rehearse: the original was moved aside, the launcher died before the
        // staged world was placed.
        let operation = world
            .service
            .layout
            .operations()
            .join(format!("restore-{}-snap", record.id));
        std::fs::create_dir_all(operation.join("new/saves/W")).unwrap();
        std::fs::write(operation.join("new/saves/W/level.dat"), b"restored").unwrap();
        std::fs::create_dir_all(operation.join("old/saves")).unwrap();
        std::fs::rename(game.join("saves/W"), operation.join("old/saves/W")).unwrap();
        std::fs::write(
            operation.join("restore.json"),
            format!(
                r#"{{"schema":1,"instance_id":"{}","snapshot":"snap","units":["saves/W"]}}"#,
                record.id
            ),
        )
        .unwrap();
        let root = world.service.layout.root().to_path_buf();
        let (service, _dir) = reopen(world, &root);
        assert_eq!(
            service.startup_notes(),
            [RecoveryNote::RestoreRolledBack {
                instance_id: record.id.clone(),
                snapshot: "snap".into(),
                units: vec![std::path::PathBuf::from("saves/W")],
            }]
        );
        assert_eq!(
            std::fs::read(service.layout.game(&record.id).join("saves/W/level.dat")).unwrap(),
            b"original"
        );
        assert!(!operation.exists());
    }

    fn write_mrpack(path: &std::path::Path, index: &str, entries: &[(&str, &str)]) {
        use std::io::Write as _;
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("modrinth.index.json", options).unwrap();
        writer.write_all(index.as_bytes()).unwrap();
        for (name, body) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    fn pack_index(files: &str) -> String {
        format!(
            r#"{{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Cool Pack",
                "files":[{files}],"dependencies":{{"minecraft":"1.21.1"}}}}"#
        )
    }

    fn pack_file(path: &str, body: &str, extra: &str) -> String {
        format!(
            r#"{{"path":"{path}","hashes":{{"sha1":"{}"}},"fileSize":{},
                "downloads":["https://cdn.modrinth.com/data/{path}"]{extra}}}"#,
            sha1_hex(body.as_bytes()),
            body.len()
        )
    }

    fn no_leftovers(world: &World) {
        assert!(
            operation_dirs(world).is_empty(),
            "{:?}",
            operation_dirs(world)
        );
    }

    #[tokio::test]
    async fn importing_a_local_pack_builds_a_whole_instance_and_keeps_the_file() {
        let world = world();
        let other = instance_with_profile(&world, "Other").await;
        world
            .net
            .answer("cdn.modrinth.com/data/mods/a.jar", "mod-a");
        world
            .net
            .answer("cdn.modrinth.com/data/mods/server-only.jar", "never");
        let index = pack_index(&format!(
            "{},{}",
            pack_file("mods/a.jar", "mod-a", ""),
            pack_file(
                "mods/server-only.jar",
                "never",
                r#","env":{"client":"unsupported","server":"required"}"#
            ),
        ));
        let pack = world._dir.path().join("cool.mrpack");
        write_mrpack(
            &pack,
            &index,
            &[
                ("overrides/config/x.toml", "from-overrides"),
                ("overrides/options.txt", "base"),
                ("client-overrides/options.txt", "client-wins"),
                ("overrides/../escape.txt", "no"),
            ],
        );
        let record = world
            .service
            .import_modpack_file(&pack, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(record.name, "Cool Pack");
        let game = world.service.layout.game(&record.id);
        assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"mod-a");
        assert!(!game.join("mods/server-only.jar").exists());
        assert_eq!(
            std::fs::read(game.join("config/x.toml")).unwrap(),
            b"from-overrides"
        );
        assert_eq!(
            std::fs::read(game.join("options.txt")).unwrap(),
            b"client-wins"
        );
        assert!(!world.service.layout.profiles().join("escape.txt").exists());
        assert!(pack.is_file(), "a local pack is the user's file");
        no_leftovers(&world);
        // The other instance is exactly as it was.
        assert_eq!(
            std::fs::read(world.service.layout.game(&other.id).join("saves/level.dat")).unwrap(),
            b"world"
        );
        assert_eq!(world.service.library().await.instances.len(), 2);
        assert_eq!(
            world.service.activity(10).finished[0].outcome,
            TaskOutcome::Succeeded
        );
    }

    #[tokio::test]
    async fn the_library_stays_usable_while_a_pack_downloads() {
        let world = world();
        world
            .net
            .answer("cdn.modrinth.com/data/mods/a.jar", "mod-a");
        let pack = world._dir.path().join("slow.mrpack");
        write_mrpack(
            &pack,
            &pack_index(&pack_file("mods/a.jar", "mod-a", "")),
            &[],
        );
        let gate = Arc::new(tokio::sync::Notify::new());
        *world.net.gate.lock().unwrap() = Some(gate.clone());
        let import = world
            .service
            .import_modpack_file(&pack, CancellationToken::new());
        let probe = async {
            // Give the download time to start, then use the library.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let library =
                tokio::time::timeout(std::time::Duration::from_secs(2), world.service.library())
                    .await
                    .expect("the library must not wait for the download");
            assert!(library.instances.is_empty(), "nothing is published yet");
            assert_eq!(world.service.activity(10).active.len(), 1);
            gate.notify_waiters();
        };
        let (record, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(import, probe)
        })
        .await
        .unwrap();
        record.unwrap();
        assert_eq!(world.service.library().await.instances.len(), 1);
    }

    #[tokio::test]
    async fn bad_packs_and_failed_downloads_leave_nothing_and_touch_no_instance() {
        let world = world();
        let other = instance_with_profile(&world, "Keep").await;
        let dir = world._dir.path().to_path_buf();
        let assert_untouched = |world: &World| {
            let library = std::fs::read_dir(world.service.layout.profiles())
                .unwrap()
                .count();
            assert_eq!(library, 1, "only the existing profile is on disk");
            no_leftovers(world);
        };

        // Not an archive at all.
        let junk = dir.join("junk.mrpack");
        std::fs::write(&junk, b"not a zip").unwrap();
        assert!(
            world
                .service
                .import_modpack_file(&junk, CancellationToken::new())
                .await
                .is_err()
        );
        // A path that climbs out of the game folder.
        let climbing = dir.join("climb.mrpack");
        write_mrpack(
            &climbing,
            &pack_index(&pack_file("../evil.jar", "x", "")),
            &[],
        );
        assert!(
            world
                .service
                .import_modpack_file(&climbing, CancellationToken::new())
                .await
                .is_err()
        );
        // Only an untrusted host to download from.
        let untrusted = dir.join("untrusted.mrpack");
        let index = pack_index(&format!(
            r#"{{"path":"mods/a.jar","hashes":{{"sha1":"{}"}},"fileSize":1,"downloads":["https://evil.example/a.jar"]}}"#,
            sha1_hex(b"a")
        ));
        write_mrpack(&untrusted, &index, &[("overrides/config/x.toml", "x")]);
        assert!(matches!(
            world
                .service
                .import_modpack_file(&untrusted, CancellationToken::new())
                .await,
            Err(ServiceError::Install(_))
        ));
        // A download whose content does not match the pack's hash.
        world
            .net
            .answer("cdn.modrinth.com/data/mods/a.jar", "TAMPERED");
        let tampered = dir.join("tampered.mrpack");
        write_mrpack(
            &tampered,
            &pack_index(&pack_file("mods/a.jar", "mod-a", "")),
            &[("overrides/config/x.toml", "x")],
        );
        assert!(
            world
                .service
                .import_modpack_file(&tampered, CancellationToken::new())
                .await
                .is_err()
        );
        // Not a file (a folder).
        assert!(
            world
                .service
                .import_modpack_file(&dir, CancellationToken::new())
                .await
                .is_err()
        );
        assert_untouched(&world);

        // Cancelled while the download is in flight.
        world
            .net
            .answer("cdn.modrinth.com/data/mods/b.jar", "mod-b");
        let slow = dir.join("slow.mrpack");
        write_mrpack(
            &slow,
            &pack_index(&pack_file("mods/b.jar", "mod-b", "")),
            &[("overrides/config/x.toml", "x")],
        );
        *world.net.gate.lock().unwrap() = Some(Arc::new(tokio::sync::Notify::new()));
        let cancel = CancellationToken::new();
        let stop = cancel.clone();
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(world.service.import_modpack_file(&slow, cancel), async {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                stop.cancel();
            })
        })
        .await
        .expect("cancelling must not wait for the download");
        assert!(matches!(result, Err(ServiceError::Cancelled)));
        *world.net.gate.lock().unwrap() = None;
        assert_untouched(&world);
        assert_eq!(world.service.library().await.instances.len(), 1);
        assert_eq!(
            std::fs::read(world.service.layout.game(&other.id).join("saves/level.dat")).unwrap(),
            b"world"
        );
        assert!(
            world
                .service
                .activity(20)
                .finished
                .iter()
                .any(|task| task.outcome == TaskOutcome::Cancelled)
        );
    }

    #[tokio::test]
    async fn a_copy_is_independent_inherits_settings_and_leaves_the_source_alone() {
        let world = world();
        let source = instance_with_profile(&world, "Source").await;
        world.service.toggle_favorite(&source.id).await.unwrap();
        world
            .service
            .update_instance_settings(
                &source.id,
                InstanceSettings {
                    max_memory_mb: Some(4096),
                    ..InstanceSettings::default()
                },
            )
            .await
            .unwrap();
        world
            .service
            .store
            .lock()
            .await
            .mark_installed(&source.id, true)
            .unwrap();
        let game = world.service.layout.game(&source.id);
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::write(game.join("logs/latest.log"), b"log").unwrap();
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), b"mod").unwrap();
        let history = HistoryLog::for_instance(world.service.layout.root(), &source.id);
        history
            .append(&HistoryEvent::Change {
                at: 1,
                kind: ChangeKind::ContentAdded,
                subject: "a.jar".into(),
            })
            .unwrap();
        let before = std::fs::read(game.join("saves/level.dat")).unwrap();

        let with = world
            .service
            .copy_instance(&source.id, "Source", true, CancellationToken::new())
            .await
            .unwrap();
        let without = world
            .service
            .copy_instance(&source.id, "Source", false, CancellationToken::new())
            .await
            .unwrap();
        assert_ne!(with.id, source.id);
        assert_ne!(with.id, without.id, "the same name never reuses an id");
        // Inherits what makes it the same game; starts clean otherwise.
        assert!(with.installed && with.loader == source.loader);
        assert_eq!(with.settings.max_memory_mb, Some(4096));
        assert!(!with.favorite && with.last_played.is_none() && with.play_seconds == 0);
        // Files: worlds only when asked, volatile output never, history not copied.
        let copy = world.service.layout.game(&with.id);
        assert_eq!(std::fs::read(copy.join("mods/a.jar")).unwrap(), b"mod");
        assert_eq!(std::fs::read(copy.join("saves/level.dat")).unwrap(), before);
        assert!(!copy.join("logs").exists());
        let bare = world.service.layout.game(&without.id);
        assert!(bare.join("mods/a.jar").is_file() && !bare.join("saves").exists());
        assert!(
            HistoryLog::for_instance(world.service.layout.root(), &with.id)
                .read()
                .unwrap()
                .events
                .is_empty()
        );
        // Independent: changing the copy leaves the source as it was.
        std::fs::write(copy.join("mods/a.jar"), b"changed").unwrap();
        std::fs::write(copy.join("saves/level.dat"), b"changed").unwrap();
        assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"mod");
        assert_eq!(std::fs::read(game.join("saves/level.dat")).unwrap(), before);
        no_leftovers(&world);
        assert_eq!(world.service.library().await.instances.len(), 3);
        assert_eq!(
            world.service.activity(10).finished[0].outcome,
            TaskOutcome::Succeeded
        );
    }

    #[tokio::test]
    async fn a_copy_that_is_refused_cancelled_or_fails_changes_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let world = world();
        let source = instance_with_profile(&world, "Source").await;
        let game = world.service.layout.game(&source.id);
        std::fs::write(game.join("secret.txt"), b"s").unwrap();
        let tree_before = std::fs::read_dir(&game).unwrap().count();

        // Blank name: refused up front.
        assert!(matches!(
            world
                .service
                .copy_instance(&source.id, "  ", true, CancellationToken::new())
                .await,
            Err(ServiceError::Store(StoreError::InvalidName))
        ));
        // Unknown source.
        assert!(matches!(
            world
                .service
                .copy_instance("ghost", "X", true, CancellationToken::new())
                .await,
            Err(ServiceError::NoSuchInstance(_))
        ));
        // Busy source (launching or running): no copy of a moving target.
        {
            let _busy = world.service.reserve_instance(&source.id).unwrap();
            assert!(matches!(
                world
                    .service
                    .copy_instance(&source.id, "X", true, CancellationToken::new())
                    .await,
                Err(ServiceError::InstanceBusy(_))
            ));
        }
        // Cancelled before it starts.
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            world
                .service
                .copy_instance(&source.id, "X", true, cancel)
                .await,
            Err(ServiceError::Cancelled)
        ));
        // An unreadable file stops the copy part-way.
        std::fs::set_permissions(
            game.join("secret.txt"),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();
        let failed = world
            .service
            .copy_instance(&source.id, "X", true, CancellationToken::new())
            .await;
        std::fs::set_permissions(
            game.join("secret.txt"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(matches!(failed, Err(ServiceError::Io(_))));

        // Nothing was published, nothing is left behind, the source is as it was.
        assert_eq!(world.service.library().await.instances.len(), 1);
        assert_eq!(
            std::fs::read_dir(world.service.layout.profiles())
                .unwrap()
                .count(),
            1
        );
        no_leftovers(&world);
        assert_eq!(std::fs::read_dir(&game).unwrap().count(), tree_before);
        assert_eq!(std::fs::read(game.join("secret.txt")).unwrap(), b"s");
        // And a plain retry works.
        world
            .service
            .copy_instance(&source.id, "X", true, CancellationToken::new())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn favorites_toggle_and_persist() {
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Fav", None, Loader::Vanilla, None)
            .await
            .unwrap();
        assert!(world.service.toggle_favorite(&record.id).await.unwrap());
        assert!(world.service.library().await.instances[0].favorite);
        assert!(!world.service.toggle_favorite(&record.id).await.unwrap());
        assert!(matches!(
            world.service.toggle_favorite("ghost").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
    }

    async fn launch_to_end(world: &World, id: &str) -> Result<GameExit, ServiceError> {
        let (tx, _rx) = mpsc::unbounded_channel();
        world.service.launch(id, tx, CancellationToken::new()).await
    }

    #[tokio::test]
    async fn launch_defaults_reach_the_real_process_and_its_commands() {
        use crate::tuning::{EnvVar, LaunchTuning};
        let world = world();
        publish_release(&world);
        fake_java(
            &world,
            concat!(
                "echo \"Setting user: x\"\n",
                "echo \"$*\" > \"$INST_DIR/args.txt\"\n",
                "echo \"FOO=$FOO WRAPPED=$WRAPPED NAME=$INST_NAME\" > \"$INST_DIR/env.txt\"",
            ),
        );
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout().game(&record.id);
        world
            .service
            .set_launch_defaults(LaunchTuning {
                window_width: Some(1280),
                window_height: Some(720),
                fullscreen: Some(true),
                jvm_arguments: vec!["-Dtuned=1".into()],
                game_arguments: vec!["--demo".into()],
                environment: vec![EnvVar {
                    name: "FOO".into(),
                    value: "bar".into(),
                }],
                wrapper: Some("env WRAPPED=yes".into()),
                pre_launch: Some("echo before > \"$INST_DIR/pre.txt\"".into()),
                post_exit: Some("echo after > \"$INST_DIR/post.txt\"".into()),
            })
            .await
            .unwrap();

        launch_to_end(&world, &record.id).await.unwrap();
        let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
        assert!(args.contains("-Dtuned=1"), "{args}");
        assert!(args.contains("--width 1280 --height 720"), "{args}");
        assert!(args.contains("--fullscreen"), "{args}");
        assert!(args.trim_end().ends_with("--demo"), "{args}");
        let env = std::fs::read_to_string(game.join("env.txt")).unwrap();
        assert_eq!(env.trim(), "FOO=bar WRAPPED=yes NAME=Run");
        assert_eq!(
            std::fs::read_to_string(game.join("pre.txt"))
                .unwrap()
                .trim(),
            "before"
        );
        assert_eq!(
            std::fs::read_to_string(game.join("post.txt"))
                .unwrap()
                .trim(),
            "after"
        );
    }

    const ECHO_ARGS: &str = "echo \"Setting user: x\"\necho \"$*\" > \"$INST_DIR/args.txt\"";

    /// Publishes releases `ids` (each a one-file client) and a catalog listing them.
    fn publish_versions(world: &World, ids: &[&str]) {
        let mut entries = Vec::new();
        for id in ids {
            let client = format!("client of {id}").into_bytes();
            std::fs::write(world.server.join(format!("{id}.jar")), &client).unwrap();
            let manifest = format!(
                r#"{{"id":"{id}","mainClass":"net.example.Main",
                    "minecraftArguments":"--username ${{auth_player_name}}",
                    "javaVersion":{{"component":"x","majorVersion":21}},
                    "downloads":{{"client":{{"url":"{}","sha1":"{}","size":{}}}}},
                    "libraries":[]}}"#,
                file_url(&world.server.join(format!("{id}.jar"))),
                sha1_hex(&client),
                client.len()
            );
            std::fs::write(world.server.join(format!("{id}.json")), manifest).unwrap();
            entries.push(format!(
                r#"{{"id":"{id}","type":"release","releaseTime":"2024-01-01T00:00:00+00:00","url":"{}"}}"#,
                file_url(&world.server.join(format!("{id}.json")))
            ));
        }
        world.net.answer(
            "piston-meta.mojang.com/mc/game/version_manifest",
            format!(
                r#"{{"latest":{{"release":"{0}","snapshot":"{0}"}},"versions":[{1}]}}"#,
                ids[0],
                entries.join(",")
            ),
        );
    }

    #[tokio::test]
    async fn changing_the_version_prepares_the_new_files_then_commits_and_records_it() {
        let world = world();
        publish_versions(&world, &["1.0", "2.0"]);
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let versions = world.service.layout().versions();
        assert!(versions.join("1.0/1.0.jar").is_file());
        assert!(!versions.join("2.0/2.0.jar").exists());

        let (tx, _rx) = mpsc::unbounded_channel();
        let changed = world
            .service
            .change_runtime(
                &record.id,
                "2.0",
                Loader::Vanilla,
                None,
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(changed.game_version, "2.0");
        assert!(changed.installed);
        assert!(
            versions.join("2.0/2.0.jar").is_file(),
            "the new files are fetched"
        );
        assert!(
            versions.join("1.0/1.0.jar").is_file(),
            "the old shared files stay"
        );
        let stored = world.service.instance(&record.id).await.unwrap();
        assert_eq!(stored.game_version, "2.0");

        // It starts on the new version, offline, and the history says what changed.
        world.net.forget_all();
        launch_to_end(&world, &record.id).await.unwrap();
        let history = world.service.history(&record.id).await.unwrap();
        assert!(
            history.events.iter().any(|event| matches!(
                event,
                HistoryEvent::Change { kind: ChangeKind::GameVersionChanged, subject, .. }
                    if subject.contains("1.0") && subject.contains("2.0")
            )),
            "{:?}",
            history.events
        );

        // The same combination again is refused, not repeated.
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(
            world
                .service
                .change_runtime(
                    &record.id,
                    "2.0",
                    Loader::Vanilla,
                    None,
                    tx,
                    CancellationToken::new()
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_version_change_that_cannot_finish_leaves_the_old_game_launchable() {
        let world = world();
        publish_versions(&world, &["1.0", "2.0"]);
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let ask = |version: &'static str, cancel: CancellationToken| {
            let (tx, _rx) = mpsc::unbounded_channel();
            world
                .service
                .change_runtime(&record.id, version, Loader::Vanilla, None, tx, cancel)
        };

        // The new client cannot be downloaded: nothing changes.
        std::fs::remove_file(world.server.join("2.0.jar")).unwrap();
        assert!(ask("2.0", CancellationToken::new()).await.is_err());
        assert_eq!(
            world
                .service
                .instance(&record.id)
                .await
                .unwrap()
                .game_version,
            "1.0"
        );

        // Cancelled before it starts: nothing changes.
        std::fs::write(world.server.join("2.0.jar"), b"client of 2.0").unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(ask("2.0", cancel).await.is_err());
        assert_eq!(
            world
                .service
                .instance(&record.id)
                .await
                .unwrap()
                .game_version,
            "1.0"
        );

        // The library cannot be written: the files were fetched but the
        // record stays on the old version.
        world.service.store.lock().await.set_read_only(true);
        assert!(ask("2.0", CancellationToken::new()).await.is_err());
        world.service.store.lock().await.set_read_only(false);
        assert_eq!(
            world
                .service
                .instance(&record.id)
                .await
                .unwrap()
                .game_version,
            "1.0"
        );

        // Through all of it, the old version still starts without the network.
        world.net.forget_all();
        launch_to_end(&world, &record.id).await.unwrap();
        assert!(
            world
                .service
                .history(&record.id)
                .await
                .unwrap()
                .events
                .iter()
                .all(|event| {
                    !matches!(
                        event,
                        HistoryEvent::Change {
                            kind: ChangeKind::GameVersionChanged,
                            ..
                        }
                    )
                }),
            "no change was recorded"
        );
    }

    #[tokio::test]
    async fn a_loader_needs_its_version_and_an_unsupported_one_is_refused() {
        let world = world();
        publish_versions(&world, &["1.0"]);
        let record = world
            .service
            .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let error = world
            .service
            .change_runtime(
                &record.id,
                "1.0",
                Loader::Fabric,
                None,
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("loader version"), "{error}");
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(
            world
                .service
                .change_runtime(
                    "ghost",
                    "1.0",
                    Loader::Vanilla,
                    None,
                    tx,
                    CancellationToken::new()
                )
                .await
                .is_err()
        );
    }

    async fn repair_to_end(world: &World, id: &str) -> Result<(), ServiceError> {
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .repair_instance(id, tx, CancellationToken::new())
            .await
    }

    #[tokio::test]
    async fn repairing_refetches_only_what_is_damaged_and_needs_no_network_when_intact() {
        let world = world();
        publish_versions(&world, &["1.0"]);
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", Some("1.0"), Loader::Vanilla, None)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let jar = world.service.layout().versions().join("1.0/1.0.jar");
        let good = std::fs::read(&jar).unwrap();

        // Damaged: put back, and a worlds/mods folder stays as it was.
        let saves = world.service.layout().game(&record.id).join("saves/keep");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(&jar, b"broken").unwrap();
        repair_to_end(&world, &record.id).await.unwrap();
        assert_eq!(std::fs::read(&jar).unwrap(), good);
        assert!(saves.is_dir());

        // Intact and offline: nothing to fetch, nothing to fail.
        world.net.forget_all();
        repair_to_end(&world, &record.id).await.unwrap();
        assert_eq!(std::fs::read(&jar).unwrap(), good);

        // Missing: restored too, and both repairs are in the history.
        std::fs::remove_file(&jar).unwrap();
        world.net.answer(
            "piston-meta.mojang.com/mc/game/version_manifest",
            r#"{"latest":{"release":"1.0","snapshot":"1.0"},"versions":[]}"#.to_owned(),
        );
        repair_to_end(&world, &record.id).await.unwrap();
        assert_eq!(std::fs::read(&jar).unwrap(), good);
        let repaired = world
            .service
            .history(&record.id)
            .await
            .unwrap()
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    HistoryEvent::Change {
                        kind: ChangeKind::Repaired,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(repaired, 3);
        let activity = world.service.activity(10);
        assert!(
            activity
                .finished
                .iter()
                .any(|task| task.category == TaskCategory::Repair)
        );
    }

    #[tokio::test]
    async fn launching_into_a_world_adds_the_release_s_quick_play_arguments_only_where_declared() {
        let world = world();
        publish_modern_release(&world);
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout().game(&record.id);
        std::fs::create_dir_all(game.join("saves/My World")).unwrap();
        let go = |name: &'static str| {
            let (tx, _rx) = mpsc::unbounded_channel();
            world
                .service
                .launch_world(&record.id, name, tx, CancellationToken::new())
        };

        go("My World").await.unwrap();
        let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
        assert!(args.contains("--quickPlaySingleplayer My World"), "{args}");
        assert!(
            args.contains("--quickPlayPath quickPlay/lumilio.json"),
            "{args}"
        );
        assert!(!args.contains("--quickPlayMultiplayer"), "{args}");

        // A world that is not there never starts the game.
        std::fs::remove_file(game.join("args.txt")).unwrap();
        let error = go("Gone").await.unwrap_err();
        assert!(error.to_string().contains("no saved world"), "{error}");
        assert!(go("../escape").await.is_err());
        assert!(!game.join("args.txt").exists());

        // The ordinary launch goes to the menu: no quick-play arguments.
        launch_to_end(&world, &record.id).await.unwrap();
        let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
        assert!(!args.contains("quickPlay"), "{args}");
    }

    #[tokio::test]
    async fn a_version_without_quick_play_says_so_instead_of_starting_at_the_menu() {
        let world = world();
        publish_release(&world);
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout().game(&record.id);
        std::fs::create_dir_all(game.join("saves/My World")).unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let error = world
            .service
            .launch_world(&record.id, "My World", tx, CancellationToken::new())
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot start directly in singleplayer"),
            "{error}"
        );
        assert!(!game.join("args.txt").exists(), "the game must not start");
    }

    #[tokio::test]
    async fn a_chosen_server_uses_quick_play_where_declared_and_the_old_arguments_elsewhere() {
        use crate::tuning::{InstanceLaunch, QuickPlay};
        for (modern, expected) in [
            (true, "--quickPlayMultiplayer mc.example.com:25565"),
            (false, "--server mc.example.com --port 25565"),
        ] {
            let world = world();
            if modern {
                publish_modern_release(&world);
            } else {
                publish_release(&world);
            }
            fake_java(&world, ECHO_ARGS);
            let record = world
                .service
                .create_instance("Run", None, Loader::Vanilla, None)
                .await
                .unwrap();
            let mut settings = record.settings.clone();
            settings.launch = InstanceLaunch {
                quick_play: Some(QuickPlay::Server("mc.example.com:25565".into())),
                ..InstanceLaunch::default()
            };
            world
                .service
                .update_instance_settings(&record.id, settings)
                .await
                .unwrap();
            launch_to_end(&world, &record.id).await.unwrap();
            let args =
                std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt"))
                    .unwrap();
            assert!(args.contains(expected), "modern={modern}: {args}");
        }
    }

    #[tokio::test]
    async fn an_instance_overrides_some_defaults_and_follows_the_rest() {
        use crate::tuning::{InstanceLaunch, LaunchTuning};
        let world = world();
        publish_release(&world);
        fake_java(&world, ECHO_ARGS);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        world
            .service
            .set_launch_defaults(LaunchTuning {
                window_width: Some(1280),
                window_height: Some(720),
                game_arguments: vec!["--demo".into()],
                ..LaunchTuning::default()
            })
            .await
            .unwrap();
        let mut settings = record.settings.clone();
        settings.launch = InstanceLaunch {
            fullscreen: Some(true),
            ..InstanceLaunch::default()
        };
        world
            .service
            .update_instance_settings(&record.id, settings.clone())
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let game = world.service.layout().game(&record.id);
        let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
        assert!(
            args.contains("--width 1280 --height 720"),
            "follows the defaults: {args}"
        );
        assert!(args.contains("--fullscreen"), "its own override: {args}");
        assert!(args.contains("--demo"), "{args}");

        // Overriding the arguments with "none" drops them; clearing the
        // override follows the defaults again.
        settings.launch.game_arguments = Some(Vec::new());
        world
            .service
            .update_instance_settings(&record.id, settings.clone())
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
        assert!(!args.contains("--demo"), "{args}");
        settings.launch = InstanceLaunch::default();
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let args = std::fs::read_to_string(game.join("args.txt")).unwrap();
        assert!(
            args.contains("--demo") && !args.contains("--fullscreen"),
            "{args}"
        );
    }

    #[tokio::test]
    async fn a_failing_command_before_launch_stops_the_game_but_one_after_exit_does_not() {
        use crate::tuning::LaunchTuning;
        let world = world();
        publish_release(&world);
        fake_java(
            &world,
            "echo \"Setting user: x\"\necho ran > \"$INST_DIR/ran.txt\"",
        );
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let game = world.service.layout().game(&record.id);
        world
            .service
            .set_launch_defaults(LaunchTuning {
                pre_launch: Some("exit 3".into()),
                ..LaunchTuning::default()
            })
            .await
            .unwrap();
        let error = launch_to_end(&world, &record.id).await.unwrap_err();
        assert!(error.to_string().contains("before launch"), "{error}");
        assert!(error.to_string().contains('3'), "{error}");
        assert!(!game.join("ran.txt").exists(), "the game must not start");

        world
            .service
            .set_launch_defaults(LaunchTuning {
                post_exit: Some("exit 9".into()),
                ..LaunchTuning::default()
            })
            .await
            .unwrap();
        let exit = launch_to_end(&world, &record.id).await.unwrap();
        assert_eq!(
            exit.code,
            Some(0),
            "a failed command after exit changes nothing"
        );
        assert!(game.join("ran.txt").exists());
    }

    #[tokio::test]
    async fn adding_a_java_by_its_executable_or_folder_remembers_the_folder() {
        let world = world();
        let elsewhere = world._dir.path().join("jdks/zulu-17");
        std::fs::create_dir_all(elsewhere.join("bin")).unwrap();
        std::fs::write(elsewhere.join("release"), "JAVA_VERSION=\"17.0.9\"\n").unwrap();
        std::fs::write(elsewhere.join("bin/java"), "#!/bin/sh\n").unwrap();
        assert!(world.service.java_installations().await.is_empty());

        let added = world
            .service
            .add_java(&elsewhere.join("bin/java"))
            .await
            .unwrap();
        assert_eq!(added.home(), elsewhere);
        assert_eq!(world.service.java_installations().await.len(), 1);
        assert_eq!(
            world.service.settings().await.extra_java_roots,
            std::slice::from_ref(&elsewhere)
        );
        // The same one again changes nothing; a folder of installations works too.
        world.service.add_java(&elsewhere).await.unwrap();
        assert_eq!(world.service.settings().await.extra_java_roots.len(), 1);
        world
            .service
            .add_java(elsewhere.parent().unwrap())
            .await
            .unwrap();

        let nothing = world._dir.path().join("empty");
        std::fs::create_dir_all(&nothing).unwrap();
        assert!(matches!(
            world.service.add_java(&nothing).await,
            Err(ServiceError::NoJavaAt(_))
        ));
    }

    #[tokio::test]
    async fn a_java_the_user_turned_off_is_listed_but_never_chosen() {
        let world = world();
        publish_release(&world);
        fake_java(&world, "echo \"Setting user: x\"");
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let installed = world.service.java_installations().await;
        assert_eq!(installed.len(), 1);
        assert!(!installed[0].1);
        let home = installed[0].0.home().to_owned();

        world.service.set_java_disabled(&home, true).await.unwrap();
        assert!(
            world.service.java_installations().await[0].1,
            "still listed, marked off"
        );
        let error = launch_to_end(&world, &record.id).await.unwrap_err();
        assert!(error.to_string().contains("Java"), "{error}");

        world.service.set_java_disabled(&home, false).await.unwrap();
        assert!(!world.service.java_installations().await[0].1);
        launch_to_end(&world, &record.id).await.unwrap();
    }

    #[tokio::test]
    async fn clearing_the_cache_frees_space_and_games_still_start() {
        let world = world();
        publish_release(&world);
        fake_java(&world, "echo \"Setting user: x\"");
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let natives = world.service.layout().natives("1.0");
        std::fs::create_dir_all(&natives).unwrap();
        std::fs::write(natives.join("lib.so"), vec![1u8; 512]).unwrap();
        let before = world.service.storage_usage().await;
        assert!(before.cache >= 512 && before.shared > 0 && before.runtimes > 0);
        let freed = world.service.clear_cache().await.unwrap();
        assert!(freed >= 512);
        let after = world.service.storage_usage().await;
        assert_eq!(after.cache, 0);
        assert_eq!(after.shared, before.shared, "shared game files stay");
        launch_to_end(&world, &record.id).await.unwrap();
    }

    #[tokio::test]
    async fn the_diagnostics_bundle_hides_the_player_the_folders_and_commands() {
        use crate::tuning::LaunchTuning;
        use std::io::Read;
        let world = world();
        publish_release(&world);
        fake_java(&world, "echo \"Setting user: Steve\"");
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        world
            .service
            .set_launch_defaults(LaunchTuning {
                pre_launch: Some("echo secret-command".into()),
                ..LaunchTuning::default()
            })
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let game = world.service.layout().game(&record.id);
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::write(
            game.join("logs/latest.log"),
            format!(
                "Steve joined from {}\n",
                world.service.layout().root().display()
            ),
        )
        .unwrap();
        let path = world._dir.path().join("diagnostics.zip");
        world.service.export_diagnostics(&path).await.unwrap();

        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut all = String::new();
        for index in 0..archive.len() {
            archive
                .by_index(index)
                .unwrap()
                .read_to_string(&mut all)
                .unwrap();
        }
        assert!(all.contains("LumilioCL"), "{all}");
        assert!(all.contains("<player> joined from <launcher>"), "{all}");
        assert!(!all.contains("Steve"), "{all}");
        assert!(!all.contains(&world.service.layout().root().display().to_string()));
        assert!(!all.contains("secret-command"), "commands are only counted");
        assert!(all.contains("pre-launch true"));
    }

    const MS_ID: &str = "123e4567e89b12d3a456426614174000";

    /// Scripts the whole Microsoft chain, ending at `name`'s profile.
    fn microsoft_chain(world: &World, name: &str, mc_token: &str) {
        use crate::microsoft::{
            DEVICE_CODE_URL, MINECRAFT_LOGIN_URL, PROFILE_URL, TOKEN_URL, XBL_URL, XSTS_URL,
        };
        for url in [
            DEVICE_CODE_URL,
            TOKEN_URL,
            XBL_URL,
            XSTS_URL,
            MINECRAFT_LOGIN_URL,
            PROFILE_URL,
        ] {
            world.net.clear_replies(url);
        }
        world.net.reply(
            DEVICE_CODE_URL,
            200,
            r#"{"device_code":"dc","user_code":"AB12CD","verification_uri":"https://www.microsoft.com/link","expires_in":900,"interval":0}"#,
        );
        world.net.reply(
            TOKEN_URL,
            200,
            r#"{"access_token":"ms-access","refresh_token":"ms-refresh-next"}"#,
        );
        world.net.reply(
            XBL_URL,
            200,
            r#"{"Token":"xbl","DisplayClaims":{"xui":[{"uhs":"h"}]}}"#,
        );
        world.net.reply(
            XSTS_URL,
            200,
            r#"{"Token":"xsts","DisplayClaims":{"xui":[{"uhs":"h"}]}}"#,
        );
        world.net.reply(
            MINECRAFT_LOGIN_URL,
            200,
            &format!(r#"{{"access_token":"{mc_token}","expires_in":86400}}"#),
        );
        world.net.reply(
            PROFILE_URL,
            200,
            &format!(r#"{{"id":"{MS_ID}","name":"{name}"}}"#),
        );
    }

    async fn sign_in(world: &World) -> Result<(String, String), ServiceError> {
        world
            .service
            .microsoft_sign_in(|_| {}, CancellationToken::new())
            .await
    }

    #[tokio::test]
    async fn signing_in_adds_the_account_keeps_secrets_in_the_store_and_shows_the_code() {
        let world = world();
        microsoft_chain(&world, "Edwin_Zhan", "mc-1");
        let shown: Arc<StdMutex<Option<DeviceCode>>> = Arc::default();
        let sink = shown.clone();
        let (key, name) = world
            .service
            .microsoft_sign_in(
                move |code| *sink.lock().unwrap() = Some(code.clone()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            (key.as_str(), name.as_str()),
            ("msa:123e4567e89b12d3a456426614174000", "Edwin_Zhan")
        );
        assert_eq!(shown.lock().unwrap().as_ref().unwrap().user_code, "AB12CD");

        // A second account does not take over the selection; the offline one stays.
        let settings = world.service.settings().await;
        assert_eq!(settings.accounts.len(), 2);
        assert_eq!(settings.selected_account.as_deref(), Some("Steve"));
        // Public facts in settings; every secret only in the store.
        let file =
            std::fs::read_to_string(world.service.layout().root().join("settings.json")).unwrap();
        for secret in ["ms-refresh-next", "mc-1", "ms-access"] {
            assert!(!file.contains(secret), "{secret} leaked into settings.json");
        }
        let stored = StoredLogin::load(world.secrets.as_ref(), &key)
            .unwrap()
            .unwrap();
        assert_eq!(stored.refresh_token, "ms-refresh-next");
        assert_eq!(stored.access_token, "mc-1");

        // Signing in again updates the one account and its name.
        microsoft_chain(&world, "Renamed", "mc-2");
        sign_in(&world).await.unwrap();
        let accounts = world.service.settings().await.accounts;
        assert_eq!(accounts.len(), 2);
        assert!(accounts.iter().any(|entry| entry.name == "Renamed"));
    }

    #[tokio::test]
    async fn a_store_that_does_not_work_stops_the_sign_in_before_any_code_is_shown() {
        let world = world();
        microsoft_chain(&world, "Edwin_Zhan", "mc-1");
        world.secrets.break_it();
        let shown = Arc::new(StdMutex::new(false));
        let sink = shown.clone();
        let error = world
            .service
            .microsoft_sign_in(
                move |_| *sink.lock().unwrap() = true,
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, ServiceError::Auth(AuthError::CredentialStore(_))),
            "{error}"
        );
        assert!(
            !*shown.lock().unwrap(),
            "no code for a sign-in that cannot be kept"
        );
        assert_eq!(world.service.settings().await.accounts.len(), 1);
    }

    #[tokio::test]
    async fn a_refusal_or_a_cancel_leaves_no_account_and_no_secret_behind() {
        use crate::microsoft::PROFILE_URL;
        let world = world();
        microsoft_chain(&world, "Edwin_Zhan", "mc-1");
        world.net.clear_replies(PROFILE_URL);
        world
            .net
            .reply(PROFILE_URL, 404, r#"{"error":"NOT_FOUND"}"#);
        let error = sign_in(&world).await.unwrap_err();
        assert!(
            matches!(error, ServiceError::Auth(AuthError::NoGameOwnership)),
            "{error}"
        );
        assert_eq!(world.service.settings().await.accounts.len(), 1);
        assert!(world.secrets.values().is_empty());

        microsoft_chain(&world, "Edwin_Zhan", "mc-1");
        let cancel = CancellationToken::new();
        cancel.cancel();
        let error = world
            .service
            .microsoft_sign_in(|_| {}, cancel)
            .await
            .unwrap_err();
        assert!(
            matches!(error, ServiceError::Auth(AuthError::Cancelled)),
            "{error}"
        );
        assert_eq!(world.service.settings().await.accounts.len(), 1);
    }

    #[tokio::test]
    async fn a_launch_uses_the_real_profile_and_token_and_reuses_a_fresh_token_without_the_network()
    {
        use crate::microsoft::TOKEN_URL;
        let world = world();
        publish_identity_release(&world);
        fake_java(&world, ECHO_ARGS);
        microsoft_chain(&world, "Edwin_Zhan", "mc-1");
        let (key, _) = sign_in(&world).await.unwrap();
        world.service.select_account(&key).await.unwrap();
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let before = world.net.sent_to(TOKEN_URL);
        launch_to_end(&world, &record.id).await.unwrap();
        let args =
            std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt"))
                .unwrap();
        assert!(args.contains("--username Edwin_Zhan"), "{args}");
        assert!(
            args.contains(&format!("--uuid {MS_ID}")) || args.contains(MS_ID),
            "{args}"
        );
        assert!(args.contains("--accessToken mc-1"), "{args}");
        assert_eq!(
            world.net.sent_to(TOKEN_URL),
            before,
            "a fresh token needs no refresh"
        );
    }

    #[tokio::test]
    async fn an_expiring_token_is_refreshed_and_the_rotated_secret_is_kept() {
        use crate::microsoft::TOKEN_URL;
        let world = world();
        publish_identity_release(&world);
        fake_java(&world, ECHO_ARGS);
        microsoft_chain(&world, "Edwin_Zhan", "mc-1");
        let (key, _) = sign_in(&world).await.unwrap();
        world.service.select_account(&key).await.unwrap();
        // The cached token has run out.
        let mut stored = StoredLogin::load(world.secrets.as_ref(), &key)
            .unwrap()
            .unwrap();
        stored.expires_at = now() + 10;
        stored.save(world.secrets.as_ref(), &key).unwrap();
        microsoft_chain(&world, "Edwin_Zhan", "mc-fresh");
        world.net.clear_replies(TOKEN_URL);
        world.net.reply(
            TOKEN_URL,
            200,
            r#"{"access_token":"ms-access-2","refresh_token":"ms-refresh-3"}"#,
        );
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        launch_to_end(&world, &record.id).await.unwrap();
        let args =
            std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt"))
                .unwrap();
        assert!(args.contains("--accessToken mc-fresh"), "{args}");
        let kept = StoredLogin::load(world.secrets.as_ref(), &key)
            .unwrap()
            .unwrap();
        assert_eq!(
            kept.refresh_token, "ms-refresh-3",
            "the rotated token replaces the old"
        );
        assert_eq!(kept.access_token, "mc-fresh");
        assert!(kept.expires_at > now() + 3600);
    }

    #[tokio::test]
    async fn a_rejected_sign_in_stops_the_launch_and_marks_the_account() {
        use crate::microsoft::TOKEN_URL;
        let world = world();
        publish_identity_release(&world);
        fake_java(&world, ECHO_ARGS);
        microsoft_chain(&world, "Edwin_Zhan", "mc-1");
        let (key, _) = sign_in(&world).await.unwrap();
        world.service.select_account(&key).await.unwrap();
        let mut stored = StoredLogin::load(world.secrets.as_ref(), &key)
            .unwrap()
            .unwrap();
        stored.expires_at = 0;
        stored.save(world.secrets.as_ref(), &key).unwrap();
        world.net.clear_replies(TOKEN_URL);
        world
            .net
            .reply(TOKEN_URL, 400, r#"{"error":"invalid_grant"}"#);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let error = launch_to_end(&world, &record.id).await.unwrap_err();
        assert!(
            matches!(error, ServiceError::SignInRequired(ref name) if name == "Edwin_Zhan"),
            "{error}"
        );
        let accounts = world.service.settings().await.accounts;
        assert!(
            accounts
                .iter()
                .any(|entry| entry.kind == AccountKind::Microsoft && entry.needs_sign_in)
        );
        assert!(
            !world
                .service
                .layout()
                .game(&record.id)
                .join("args.txt")
                .exists(),
            "no game, and never as someone else"
        );

        // A network failure is a different error and does not mark the account.
        world.net.clear_replies(TOKEN_URL);
        world
            .service
            .settings
            .lock()
            .await
            .set_needs_sign_in(&key, false)
            .unwrap();
        let error = launch_to_end(&world, &record.id).await.unwrap_err();
        assert!(
            matches!(error, ServiceError::Auth(AuthError::Network(_))),
            "{error}"
        );
        assert!(
            world
                .service
                .settings()
                .await
                .accounts
                .iter()
                .all(|entry| !entry.needs_sign_in)
        );

        // Refreshing by hand after the account recovers clears the mark.
        microsoft_chain(&world, "Edwin_Zhan", "mc-9");
        world.service.refresh_account(&key).await.unwrap();
        assert!(
            world
                .service
                .settings()
                .await
                .accounts
                .iter()
                .all(|entry| !entry.needs_sign_in)
        );
    }

    #[tokio::test]
    async fn removing_a_microsoft_account_deletes_its_secret_and_tokens_stay_out_of_diagnostics() {
        use std::io::Read;
        let world = world();
        microsoft_chain(&world, "Edwin_Zhan", "mc-secret-token");
        let (key, _) = sign_in(&world).await.unwrap();
        let path = world._dir.path().join("diagnostics.zip");
        world.service.export_diagnostics(&path).await.unwrap();
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut all = String::new();
        for index in 0..archive.len() {
            archive
                .by_index(index)
                .unwrap()
                .read_to_string(&mut all)
                .unwrap();
        }
        for hidden in ["mc-secret-token", "ms-refresh-next", "Edwin_Zhan", MS_ID] {
            assert!(!all.contains(hidden), "{hidden} reached the bundle: {all}");
        }

        world.service.remove_account(&key).await.unwrap();
        assert!(
            world.secrets.values().is_empty(),
            "the secret goes with the account"
        );
        assert_eq!(world.service.settings().await.accounts.len(), 1);
        // An unknown key is an error and touches nothing.
        assert!(world.service.remove_account("msa:nope").await.is_err());
    }

    #[tokio::test]
    async fn launching_without_an_account_is_refused_instead_of_using_a_stand_in() {
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        world.service.remove_account("Steve").await.unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let result = world
            .service
            .launch(&record.id, tx, CancellationToken::new())
            .await;
        assert!(matches!(result, Err(ServiceError::NoAccount)));
        // The refusal leaves the instance free for the retry after adding one.
        world.service.add_account("Alex", None).await.unwrap();
        assert_eq!(
            world.service.settings().await.selected_account.as_deref(),
            Some("Alex")
        );
        assert!(world.service.delete_instance(&record.id).await.is_ok());
    }

    #[tokio::test]
    async fn accounts_selection_and_custom_ids_persist_across_reopen() {
        let world = world();
        world
            .service
            .add_account("Alex", Some("123e4567-e89b-12d3-a456-426614174000"))
            .await
            .unwrap();
        world.service.select_account("Alex").await.unwrap();
        let settings = world.service.settings().await;
        assert_eq!(settings.selected_account.as_deref(), Some("Alex"));
        let alex = settings.accounts.iter().find(|a| a.name == "Alex").unwrap();
        assert_eq!(
            alex.profile().unwrap().id().compact(),
            "123e4567e89b12d3a456426614174000"
        );
        // Same id again, or a malformed one, is refused and changes nothing.
        assert!(
            world
                .service
                .add_account("Bob", Some("123E4567E89B12D3A456426614174000"))
                .await
                .is_err()
        );
        assert!(
            world
                .service
                .add_account("Bob", Some("nope"))
                .await
                .is_err()
        );
        assert_eq!(world.service.settings().await.accounts.len(), 2);
    }

    #[tokio::test]
    async fn an_instance_java_must_exist_and_a_bad_override_changes_nothing() {
        use crate::tuning::InstanceLaunch;
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let mut settings = record.settings.clone();
        settings.java_path = Some(world._dir.path().join("no-such-java"));
        assert!(matches!(
            world
                .service
                .update_instance_settings(&record.id, settings.clone())
                .await,
            Err(ServiceError::NoJavaAt(_))
        ));
        settings.java_path = None;
        settings.launch = InstanceLaunch {
            window_width: Some(800),
            ..InstanceLaunch::default()
        };
        assert!(
            world
                .service
                .update_instance_settings(&record.id, settings)
                .await
                .is_err()
        );
        assert!(
            world
                .service
                .instance(&record.id)
                .await
                .unwrap()
                .settings
                .launch
                .is_default()
        );
    }

    #[tokio::test]
    async fn the_window_follows_the_instance_choice_then_the_preference() {
        use crate::tuning::{AfterLaunch, InstanceLaunch, Preferences};
        let world = world();
        publish_release(&world);
        let record = world
            .service
            .create_instance("Run", None, Loader::Vanilla, None)
            .await
            .unwrap();
        assert_eq!(
            world.service.after_launch_for(&record.id).await,
            AfterLaunch::Keep
        );
        world
            .service
            .set_preferences(Preferences {
                after_launch: AfterLaunch::Hide,
                ..Preferences::default()
            })
            .await
            .unwrap();
        assert_eq!(
            world.service.after_launch_for(&record.id).await,
            AfterLaunch::Hide
        );
        let mut settings = record.settings.clone();
        settings.launch = InstanceLaunch {
            after_launch: Some(AfterLaunch::Keep),
            ..InstanceLaunch::default()
        };
        world
            .service
            .update_instance_settings(&record.id, settings)
            .await
            .unwrap();
        assert_eq!(
            world.service.after_launch_for(&record.id).await,
            AfterLaunch::Keep
        );
        assert_eq!(
            world.service.after_launch_for("ghost").await,
            AfterLaunch::Hide
        );
    }

    #[tokio::test]
    async fn the_current_instance_persists_and_reads_as_none_once_deleted() {
        let world = world();
        publish_release(&world);
        let a = world
            .service
            .create_instance("A", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let b = world
            .service
            .create_instance("B", None, Loader::Vanilla, None)
            .await
            .unwrap();
        assert_eq!(world.service.current_instance().await, None);
        assert!(matches!(
            world.service.set_current_instance("ghost").await,
            Err(ServiceError::NoSuchInstance(_))
        ));
        world.service.set_current_instance(&b.id).await.unwrap();
        assert_eq!(world.service.current_instance().await, Some(b.id.clone()));
        world.service.delete_instance(&b.id).await.unwrap();
        assert_eq!(world.service.current_instance().await, None);
        world.service.set_current_instance(&a.id).await.unwrap();
        assert_eq!(world.service.current_instance().await, Some(a.id.clone()));
    }

    #[tokio::test]
    async fn launching_an_unknown_instance_is_an_error() {
        let world = world();
        let (tx, _rx) = mpsc::unbounded_channel();
        let result = world
            .service
            .launch("ghost", tx, CancellationToken::new())
            .await;
        assert!(matches!(result, Err(ServiceError::NoSuchInstance(_))));
    }

    fn mod_version(world: &World) -> Vec<u8> {
        let jar = b"a mod";
        std::fs::write(world.server.join("cool.jar"), jar).unwrap();
        format!(
            r#"[{{"id":"v1","project_id":"P","name":"Cool","version_number":"1",
                "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",
                "files":[{{"url":"{}","filename":"cool.jar","primary":true,
                           "size":{},"hashes":{{"sha1":"{}"}}}}],
                "dependencies":[]}}]"#,
            file_url(&world.server.join("cool.jar")),
            jar.len(),
            sha1_hex(jar)
        )
        .into_bytes()
    }

    /// A one-version project document whose version requires `requires`.
    fn project_versions(id: &str, game: &str, requires: &[&str]) -> Vec<u8> {
        let dependencies: Vec<String> = requires
            .iter()
            .map(|project| format!(r#"{{"project_id":"{project}","dependency_type":"required"}}"#))
            .collect();
        format!(
            r#"[{{"id":"v-{id}","project_id":"{id}","name":"{id}","version_number":"1",
                "version_type":"release","game_versions":["{game}"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",
                "files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                           "primary":true,"size":1,"hashes":{{"sha1":"{id}"}}}}],
                "dependencies":[{}]}}]"#,
            dependencies.join(",")
        )
        .into_bytes()
    }

    #[tokio::test]
    async fn required_dependencies_are_listed_nearest_first_once_and_without_what_is_there() {
        let world = world();
        let record = world
            .service
            .create_instance("Deps", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        // cool needs LIBX and LIBY; LIBX needs LIBY and cool back (a cycle).
        world.net.answer(
            "/v2/project/cool/version",
            project_versions("P", "1.0", &["LIBX", "LIBY"]),
        );
        world.net.answer(
            "/v2/project/LIBX/version",
            project_versions("LIBX", "1.0", &["LIBY", "P"]),
        );
        world.net.answer(
            "/v2/project/LIBY/version",
            project_versions("LIBY", "1.0", &[]),
        );

        let need = world
            .service
            .missing_dependencies(&record.id, ProjectKind::Mod, "cool", None)
            .await
            .unwrap();
        let ids: Vec<_> = need.iter().map(|n| n.project_id.as_str()).collect();
        assert_eq!(ids, ["LIBX", "LIBY"]);
        assert!(need.iter().all(|n| n.version.is_some()));

        // Only mods have dependencies to offer.
        assert!(
            world
                .service
                .missing_dependencies(&record.id, ProjectKind::ResourcePack, "cool", None)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn optional_mods_are_suggested_and_installed_ones_it_cannot_live_with_are_named() {
        let world = world();
        let record = world
            .service
            .create_instance("Rel", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        let file = |id: &str| {
            format!(
                r#""files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                    "primary":true,"size":4,"hashes":{{"sha1":"{id}"}}}}]"#
            )
        };
        let version = |id: &str, deps: &str| {
            format!(
                r#"[{{"id":"v-{id}","project_id":"{id}","name":"{id}","version_number":"1",
                    "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                    "date_published":"2024-01-01T00:00:00Z",{},"dependencies":[{deps}]}}]"#,
                file(id)
            )
            .into_bytes()
        };
        world.net.answer(
            "/v2/project/cool/version",
            version(
                "P",
                r#"{"project_id":"OPT","dependency_type":"optional"},
                   {"project_id":"BAD","dependency_type":"incompatible"},
                   {"project_id":"FINE","dependency_type":"incompatible"}"#,
            ),
        );
        world
            .net
            .answer("/v2/project/OPT/version", version("OPT", ""));
        // BAD is installed (Modrinth recognises it by hash); FINE is not.
        let mods = world.service.layout.game(&record.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("bad.jar"), b"bad").unwrap();
        world.net.answer(
            "/v2/version_files",
            format!(
                r#"{{"{}":{}}}"#,
                sha1_hex(b"bad"),
                String::from_utf8(version("BAD", ""))
                    .unwrap()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
            ),
        );

        let report = world
            .service
            .dependency_report(&record.id, ProjectKind::Mod, "cool", None)
            .await
            .unwrap();
        assert!(report.needs.is_empty());
        let optional: Vec<_> = report
            .optional
            .iter()
            .map(|n| n.project_id.as_str())
            .collect();
        assert_eq!(optional, ["OPT"]);
        assert!(report.optional[0].version.is_some());
        assert_eq!(
            report.conflicts,
            ["BAD"],
            "only what is installed conflicts"
        );
    }

    #[tokio::test]
    async fn a_dependency_already_in_the_game_is_not_offered_and_one_without_a_fit_is_flagged() {
        let world = world();
        let record = world
            .service
            .create_instance("Deps", Some("1.0"), Loader::Fabric, Some("0.16.0"))
            .await
            .unwrap();
        world.net.answer(
            "/v2/project/cool/version",
            project_versions("P", "1.0", &["LIBX", "OLD"]),
        );
        world.net.answer(
            "/v2/project/LIBX/version",
            project_versions("LIBX", "1.0", &[]),
        );
        world.net.answer(
            "/v2/project/OLD/version",
            project_versions("OLD", "0.9", &[]),
        );
        // The game already holds LIBX, which Modrinth recognises by hash.
        let mods = world.service.layout.game(&record.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("libx.jar"), b"libx").unwrap();
        world.net.answer(
            "/v2/version_files",
            format!(
                r#"{{"{}":{{"id":"v-LIBX","project_id":"LIBX","name":"x","version_number":"1",
                    "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                    "date_published":"2024-01-01T00:00:00Z",
                    "files":[{{"url":"https://cdn.modrinth.com/l.jar","filename":"l.jar",
                               "primary":true,"size":4,"hashes":{{"sha1":"{}"}}}}],
                    "dependencies":[]}}}}"#,
                sha1_hex(b"libx"),
                sha1_hex(b"libx")
            ),
        );

        let need = world
            .service
            .missing_dependencies(&record.id, ProjectKind::Mod, "cool", None)
            .await
            .unwrap();
        assert_eq!(need.len(), 1, "{need:?}");
        assert_eq!(need[0].project_id, "OLD");
        assert!(need[0].version.is_none(), "no version fits this game");
    }

    #[tokio::test]
    async fn installed_content_lands_in_the_profile_and_is_logged() {
        let world = world();
        let record = world
            .service
            .store
            .lock()
            .await
            .create(
                NewInstance {
                    name: "Mods".to_owned(),
                    game_version: "1.0".to_owned(),
                    loader: Loader::Fabric,
                    loader_version: Some("0.16.0".to_owned()),
                },
                1,
            )
            .unwrap()
            .clone();
        world
            .net
            .answer("/v2/project/cool/version", mod_version(&world));

        let name = world
            .service
            .install_content(
                &record.id,
                ProjectKind::Mod,
                "cool",
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(name, "cool.jar");
        let installed = world
            .service
            .layout()
            .game(&record.id)
            .join("mods/cool.jar");
        assert_eq!(std::fs::read(installed).unwrap(), b"a mod");
        let changes = HistoryLog::for_instance(world.service.layout().root(), &record.id)
            .changes()
            .unwrap();
        assert_eq!(changes.len(), 1);

        let activity = world.service.activity(10);
        assert!(activity.active.is_empty());
        assert_eq!(activity.finished.len(), 1);
        assert_eq!(activity.finished[0].outcome, TaskOutcome::Succeeded);
    }

    #[tokio::test]
    async fn content_for_another_game_version_is_refused_and_the_failure_is_logged() {
        let world = world();
        let record = world
            .service
            .store
            .lock()
            .await
            .create(
                NewInstance {
                    name: "Old".to_owned(),
                    game_version: "1.7".to_owned(),
                    loader: Loader::Fabric,
                    loader_version: Some("0.16.0".to_owned()),
                },
                1,
            )
            .unwrap()
            .clone();
        world
            .net
            .answer("/v2/project/cool/version", mod_version(&world));
        let result = world
            .service
            .install_content(
                &record.id,
                ProjectKind::Mod,
                "cool",
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(result, Err(ServiceError::NoCompatibleVersion)));
        assert!(
            !world
                .service
                .layout()
                .game(&record.id)
                .join("mods")
                .exists()
        );
        let activity = world.service.activity(10);
        assert!(matches!(
            activity.finished[0].outcome,
            TaskOutcome::Failed(_)
        ));
    }

    #[tokio::test]
    async fn deleting_an_instance_removes_its_profile_and_keeps_shared_files() {
        let world = world();
        publish_release(&world);
        fake_java(&world, "echo \"Setting user: x\"");
        let a = world
            .service
            .create_instance("A", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let b = world
            .service
            .create_instance("B", None, Loader::Vanilla, None)
            .await
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        world
            .service
            .launch(&a.id, tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(world.service.layout().game(&a.id).is_dir());

        world.service.delete_instance(&a.id).await.unwrap();
        assert!(!world.service.layout().profile(&a.id).exists());
        assert!(
            world
                .service
                .layout()
                .versions()
                .join("1.0/1.0.jar")
                .is_file()
        );
        assert_eq!(world.service.library().await.instances.len(), 1);

        // B never installed anything itself, and launches from the shared files.
        world.net.forget_all();
        let (tx, mut rx) = mpsc::unbounded_channel();
        world
            .service
            .launch(&b.id, tx, CancellationToken::new())
            .await
            .unwrap();
        let mut downloaded = false;
        while let Ok(update) = rx.try_recv() {
            downloaded |= matches!(
                update,
                LaunchUpdate::Signal(LaunchSignal::Phase(
                    crate::launch_session::LaunchPhase::Libraries
                ))
            );
        }
        assert!(!downloaded);
    }

    #[tokio::test]
    async fn filters_are_fetched_once_and_failures_are_not_remembered() {
        let world = world();
        assert!(matches!(
            world.service.discover_filters().await,
            Err(ServiceError::Remote(_))
        ));
        world.net.answer(
            "/v2/tag/category",
            r#"[{"name":"adventure","project_type":"mod","header":"categories"}]"#,
        );
        world.net.answer(
            "/v2/tag/game_version",
            r#"[{"version":"1.21.1","version_type":"release","date":"2024-08-08T00:00:00Z"}]"#,
        );
        let filters = world.service.discover_filters().await.unwrap();
        assert_eq!(filters.categories.len(), 1);
        assert_eq!(filters.game_versions[0].version, "1.21.1");
        // Served from memory: the network can vanish.
        world.net.forget_all();
        assert_eq!(world.service.discover_filters().await.unwrap(), filters);
    }

    #[tokio::test]
    async fn detail_bundles_project_versions_newest_first_and_survives_a_missing_owner() {
        let world = world();
        world.net.answer(
            "/v2/project/cool/version",
            r#"[{"id":"old","project_id":"P","name":"Old","version_number":"1","version_type":"release",
                 "game_versions":["1.0"],"loaders":["fabric"],"date_published":"2023-01-01T00:00:00Z",
                 "files":[{"url":"https://x/o.jar","filename":"o.jar","primary":true,"size":1,"hashes":{}}]},
                {"id":"new","project_id":"P","name":"New","version_number":"2","version_type":"release",
                 "game_versions":["1.0"],"loaders":["fabric"],"date_published":"2024-01-01T00:00:00Z",
                 "files":[{"url":"https://x/n.jar","filename":"n.jar","primary":true,"size":1,"hashes":{}}]}]"#,
        );
        world
            .net
            .answer("/v2/project/cool/members", "not json at all");
        world.net.answer(
            "/v2/project/cool",
            r##"{"id":"P","slug":"cool","title":"Cool","project_type":"mod","body":"# Hello"}"##,
        );
        let detail = world.service.project_detail("cool").await.unwrap();
        assert_eq!(detail.project.title, "Cool");
        assert_eq!(detail.project.body, "# Hello");
        let order: Vec<_> = detail.versions.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(order, ["new", "old"]);
        assert_eq!(detail.owner, None);
    }

    async fn fabric_instance(world: &World, game: &str) -> InstanceRecord {
        world
            .service
            .store
            .lock()
            .await
            .create(
                NewInstance {
                    name: format!("Pick {game}"),
                    game_version: game.to_owned(),
                    loader: Loader::Fabric,
                    loader_version: Some("0.16.0".to_owned()),
                },
                1,
            )
            .unwrap()
            .clone()
    }

    fn two_versions(world: &World) -> Vec<u8> {
        for (name, body) in [("v1.jar", b"one".as_slice()), ("v2.jar", b"two".as_slice())] {
            std::fs::write(world.server.join(name), body).unwrap();
        }
        let entry = |id: &str, file: &str, games: &str, body: &[u8]| {
            format!(
                r#"{{"id":"{id}","project_id":"P","name":"{id}","version_number":"{id}",
                    "version_type":"release","game_versions":{games},"loaders":["fabric"],
                    "date_published":"2024-01-0{}T00:00:00Z",
                    "files":[{{"url":"{}","filename":"{file}","primary":true,"size":{},
                               "hashes":{{"sha1":"{}"}}}}]}}"#,
                if id == "v1" { 1 } else { 2 },
                file_url(&world.server.join(file)),
                body.len(),
                sha1_hex(body)
            )
        };
        format!(
            "[{},{}]",
            entry("v1", "v1.jar", r#"["1.0"]"#, b"one"),
            entry("v2", "v2.jar", r#"["2.0"]"#, b"two")
        )
        .into_bytes()
    }

    #[tokio::test]
    async fn a_chosen_older_version_is_installed_instead_of_the_newest() {
        let world = world();
        let record = fabric_instance(&world, "1.0").await;
        world
            .net
            .answer("/v2/project/cool/version", two_versions(&world));
        let file = world
            .service
            .install_version(
                &record.id,
                ProjectKind::Mod,
                "cool",
                "v1",
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(file, "v1.jar");
        let path = world.service.layout().game(&record.id).join("mods/v1.jar");
        assert_eq!(std::fs::read(path).unwrap(), b"one");
    }

    #[tokio::test]
    async fn a_version_for_another_game_or_an_unknown_id_is_refused_before_any_download() {
        let world = world();
        let record = fabric_instance(&world, "1.0").await;
        world
            .net
            .answer("/v2/project/cool/version", two_versions(&world));
        for id in ["v2", "ghost"] {
            let result = world
                .service
                .install_version(
                    &record.id,
                    ProjectKind::Mod,
                    "cool",
                    id,
                    CancellationToken::new(),
                )
                .await;
            assert!(
                matches!(result, Err(ServiceError::NoCompatibleVersion)),
                "{id}"
            );
        }
        assert!(
            !world
                .service
                .layout()
                .game(&record.id)
                .join("mods")
                .exists()
        );
    }
}
