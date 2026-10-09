//! UI-independent launcher domain logic.

mod account;
mod activity;
mod activity_log;
mod artifact;
mod attention;
mod backup;
mod bundle;
mod catalog;
mod content;
mod content_sources;
mod copy;
mod credentials;
mod deletion;
mod diagnostics;
mod discover;
mod environment;
mod fetch;
mod forge_install;
mod forge_meta;
mod history;
mod import_game;
mod injector;
mod inspect;
mod install;
mod instance;
mod java;
mod java_runtime;
mod launch;
mod launch_readiness;
mod launch_session;
mod launcher;
mod layout;
mod loader;
mod microsoft;
mod mirror_presets;
mod modpack;
use lumilio_nbt as nbt;
mod model_assets;
mod pack_export;
mod persist;
mod plugins;
mod process;
mod reclaim;
mod recovery;
mod release;
mod repair;
mod resource_lock;
mod root_lock;
mod screenshots;
mod servers;
mod service;
mod settings;
mod skin;
mod snapshots;
mod staged;
mod storage;
mod transfer;
mod tuning;
mod updates;
mod worlds;
mod yggdrasil;

pub use account::{AuthSession, MAX_PROFILE_NAME, OfflineProfile, ProfileError, ProfileId};
pub use activity::{
    ActivityContext, ActivityEvent, ActivityGraph, ActivityGraphError, ActivityNode,
    ActivityRecord, ActivityReport, ActivityScheduler, ActivityState, CancellationToken,
    ProgressSnapshot,
};
pub use activity_log::{
    ActiveTask, ActivityLog, FinishedTask, ProgressUnit, RetryAction, TaskAction, TaskBoard,
    TaskCategory, TaskLabel, TaskOutcome,
};
pub use artifact::{CoordinateError, PackageCoordinate};
pub use attention::{AttentionItem, HomeSummary, summarize as summarize_home};
pub use bundle::{Redactor, write_bundle};
pub use catalog::{
    CatalogEntry, CatalogError, OFFICIAL_CATALOG_URL, VersionCatalog, VersionChannel, VersionKind,
};
pub use content::{
    ContentError, ContentItem, ModMetadata, read_mod_metadata, remove as remove_content,
    scan as scan_content, set_enabled as set_content_enabled, sha1_hex,
};
pub use content_sources::{ContentEntry, ContentList, ContentSource};
pub use credentials::{CredentialStore, MemoryCredentials, StoredLogin, SystemCredentials};
pub use diagnostics::{
    CrashReport, Facts, FileEntry, LOW_MEMORY_MB, LogLevel, LogLine, Problem, ProblemKind,
    Severity, diagnose, filter_log, list_crash_reports, list_dir, log_level, log_lines,
    read_crash_report, read_latest_log,
};
pub use discover::{
    CategoryTag, ContentClient, DependencyKind, DiscoverError, Environment, GalleryImage,
    GameVersionTag, GroupLabel, IntentError, KindAbilities, LoaderTag, Pick, Project, ProjectKind,
    ProjectLinks, ProjectSummary, ReleaseChannel, SITE_BASE, SearchHit, SearchPage, SearchQuery,
    SideSupport, SortIndex, SourceFilters, Stance, Version, VersionFile, VersionGroup,
    browse_page_url, environment, fits as version_fits, install_request as content_install_request,
    pick_version, project_page_url, version_groups,
};
pub use environment::{
    CompatibilityRule, HostProfile, MachineArchitecture, PlatformFamily, RuleDecision,
};
pub use fetch::{DOCUMENT_LIMIT, FetchError, fetch_document, post_document};
pub use forge_meta::{forge_versions, neoforge_game_version, neoforge_versions};
pub use history::{
    ChangeKind, HistoryEvent, HistoryLog, HistoryRead, SessionOutcome, finish_session,
    record_attempt,
};
pub use import_game::{FoundGame, GameOrigin, ImportError};
pub use inspect::inspect_instance;
pub use install::{
    ArchiveLimits, ArtifactKind, AssetIndex, AssetIndexError, AssetObject, InstallError,
    InstallEvent, InstallReport, InstallStage, InstallationPlan, Installer, NativeBundle,
    NativeError, NativePublication, NativePublisher, PlannedArtifact,
};
pub use instance::{
    Collection, InstanceRecord, InstanceSettings, InstanceStore, Loader, NewInstance, StoreError,
};
pub use java::{JavaLocator, JavaRuntime, choose as choose_java, parse_major as parse_java_major};
pub use launch::{
    LaunchContext, LaunchDirectories, LaunchError, LaunchPlan, NativeArchive, RequiredDownload,
};
pub use launch_readiness::{LaunchReadiness, LaunchRequirement};
pub use launch_session::{LaunchFailure, LaunchPhase, LaunchSession, LaunchSignal, LaunchStatus};
pub use launcher::{LaunchRequest, LaunchServiceError, LaunchUpdate, Launcher, runtimes_root};
pub use layout::Layout;
pub use loader::{
    LAUNCHABLE_LOADERS, LoaderError, LoaderVersion, decode_versions as decode_loader_versions,
    fetch_versions as fetch_loader_versions, normalize_profile as normalize_loader_profile,
    profile_url as loader_profile_url, recommended as recommended_loader,
    versions_url as loader_versions_url,
};
pub use lumilio_plugin_api::PluginState;
pub use microsoft::{
    AuthError, DeviceCode, MICROSOFT_CLIENT_ID, MicrosoftClient, MinecraftLogin, OAuthTokens,
    Secret, client_id as microsoft_client_id,
};
pub use mirror_presets::MirrorPreset;
pub use model_assets::{ModelAssetsError, ResourcePack};
pub use modpack::{
    ClientSupport, ModpackError, OVERRIDES_LIMIT, PackFile, PackIndex, extract_overrides,
    import as import_modpack, is_trusted_source, parse_index as parse_pack_index,
    plan as plan_modpack, read_index as read_pack_index,
};
pub use pack_export::{ExportError, ExportReport, ExportSpec, PackFormat};
pub mod world_map;
pub use plugins::{
    MapFailure, MapProviders, PluginContentSource, PluginEffect, PluginFinding, PluginHost,
    PluginInfo, PluginStatus, PluginTab,
};
pub use process::{
    DEFAULT_SETTLE, GameEvent, GameExit, GameOptions, LogStream, ProcessError,
    command_line as game_command_line, run as run_game,
};
pub use reclaim::{KeptReason, ReclaimError, Reclaimable};
pub use recovery::RecoveryNote;
pub use release::{
    ArgumentGroups, ArgumentTemplate, AssetCatalog, DownloadDescriptor, ExtractionPolicy,
    IdentifiedDownload, JavaRequirement, LibraryDependency, LoggingConfiguration, ReleaseError,
    ReleaseManifest, ReleaseSet,
};
pub use repair::{
    InstallationVerifier, IntegrityIssue, RepairError, RepairExecutor, RepairFinding, RepairPlan,
    RepairReport,
};
pub use screenshots::{ScreenshotError, ScreenshotInfo};
pub use servers::{PackPolicy, PingError, ServerEntry, ServerError, ServerStatus};
pub use service::{
    ActivityView, AppearanceApplyResult, ContentEffect, ContentResult, DependencyNeed,
    DependencyReport, DiscoverFilters, GameLogSource, GameLogs, InstalledProject, LauncherService,
    Library, ModelPreview, ProfileSnapshot, ProjectDetail, ServiceError, ThirdPartySignIn,
    now as unix_now,
};
pub use settings::{
    AccountEntry, AccountKind, AuthServerEntry, DownloadSourcePreference, LauncherSettings,
    MAX_MEMORY_MB, MirrorRule, SettingsError, SettingsStore,
};
pub use skin::{
    AccountLook, AppearanceChange, AppearanceError, AppearanceUpdate, LITTLE_SKIN_CSL, LibrarySkin,
    MojangCape, MojangClient, MojangProfile, MojangSkin, PairedCape, Pixels as SkinPixels,
    SkinChoice, SkinError, SkinLibrary, SkinModel, SkinSource, cape_pixels, looks_slim,
    skin_pixels,
};
pub use snapshots::{
    RESTORE_LIMIT, SnapshotError, SnapshotInfo, SnapshotScope, create as create_snapshot,
    delete as delete_snapshot, directory as snapshot_directory, list as list_snapshots,
    restore as restore_snapshot,
};
pub use storage::{
    StorageUsage, clear_cache, directory_size, measure as measure_storage, recommended_memory_mb,
    total_memory_mb,
};
pub use transfer::USER_AGENT;
pub use transfer::{
    ByteStream, DefaultTransport, FileTransport, HttpMethod, HttpRequest, HttpTransport,
    OfficialSource, PrefixMirror, RetryPolicy, SourceChain, SourceFailure, SourceProvider,
    TransferBatchReport, TransferEngine, TransferError, TransferEvent, TransferOutcome,
    TransferRequest, Transport, TransportError, TransportFuture, TransportResponse,
};
pub use tuning::{
    AfterLaunch, Appearance, DOWNLOAD_CONCURRENCY, DiscoverPreferences, EnvVar, InstanceLaunch,
    Language, LaunchTuning, MAX_WINDOW_SIDE, MotionPreference, Preferences, QuickPlay,
    QuickPlayProblem, TuningError, quick_play_problem, quick_play_world_unsupported, split_words,
};
pub use updates::{
    ContentUpdate, UpdateError, UpdateReport, apply as apply_update, check as check_updates,
};
pub use worlds::{
    WorldError, WorldInfo, copy_name as world_copy_name, delete as delete_world,
    duplicate as duplicate_world, scan as scan_worlds, size as world_size,
};
pub use yggdrasil::{AuthServer, LITTLE_SKIN_URL, Profile as CharacterProfile, YggdrasilError};
