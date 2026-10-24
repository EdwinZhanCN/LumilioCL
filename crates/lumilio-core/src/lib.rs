//! UI-independent launcher domain logic.

mod account;
mod activity;
mod attention;
mod backup;
mod blocking;
mod bundle;
mod clock;
mod content;
mod context;
mod copy;
mod credentials;
mod deletion;
mod diagnostics;
mod discover;
mod error;
mod fetch;
mod fingerprint;
mod forge_install;
mod forge_meta;
mod game_settings;
mod history;
mod import_game;
mod injector;
mod inspect;
mod install;
mod instances;
mod java;
mod java_runtime;
mod launching;
mod layout;
mod loader;
mod microsoft;
mod minecraft;
mod mirror_presets;
mod modpack;
mod network;

pub use account::{AuthSession, MAX_PROFILE_NAME, OfflineProfile, ProfileError, ProfileId};
pub use activity::{
    ActiveTask, ActivityContext, ActivityEvent, ActivityGraph, ActivityGraphError, ActivityLog,
    ActivityNode, ActivityRecord, ActivityReport, ActivityScheduler, ActivityState, ActivityView,
    CancellationToken, FileDownload, FinishedTask, ProgressSnapshot, ProgressUnit, RetryAction,
    SourceSwitch, TaskAction, TaskBoard, TaskCategory, TaskDownloads, TaskLabel, TaskOutcome,
    Traffic,
};
pub use attention::{
    AttentionItem, HomeChecks, HomeSummary, summarize as summarize_home, with_account,
};
pub use bundle::{Redactor, write_bundle};
pub use clock::now as unix_now;
pub use content::{
    AddedFile, ContentEffect, ContentEntry, ContentError, ContentItem, ContentList, ContentResult,
    ContentSource, ContentSourceInfo, DependencyNeed, DependencyReport, DiscoverFilters,
    InstalledProject, ModMetadata, ProjectDetail, read_mod_metadata, remove as remove_content,
    scan as scan_content, set_enabled as set_content_enabled, sha1_hex,
};
pub use context::{Context, Revisions, Session, SessionPhase};
pub use credentials::{CredentialStore, MemoryCredentials, StoredLogin, SystemCredentials};
pub use diagnostics::{
    CrashReport, Facts, FileEntry, GameLogSource, GameLogs, LOW_MEMORY_MB, LogLevel, LogLine,
    Problem, ProblemKind, Severity, diagnose, filter_log, list_crash_reports, list_dir, log_level,
    log_lines, read_crash_report, read_latest_log,
};
pub use discover::{
    Catalog, CategoryTag, ContentClient, DEFAULT_SOURCE, DependencyKind, DiscoverError,
    Environment, FileHashes, GalleryImage, GameVersionTag, GroupLabel, IntentError, KindAbilities,
    LoaderTag, PackFileKind, Pick, Project, ProjectKind, ProjectLinks, ProjectOrigin,
    ProjectSummary, ReleaseChannel, ResolvedFile, SearchHit, SearchPage, SearchQuery, SideSupport,
    SortIndex, SourceFilters, Stance, TextFormat, Version, VersionFile, VersionGroup, environment,
    fits as version_fits, install_request as content_install_request, pick_version, version_groups,
};
pub use error::ServiceError;
pub use fetch::{DOCUMENT_LIMIT, FetchError, fetch_document, post_document};
pub use forge_meta::{forge_versions, neoforge_game_version, neoforge_versions};
pub use game_settings::{
    GameSettingGroup, GameSettingKind, GameSettingValue, GameSettingsError, GameSettingsView,
    InstanceOptions, OptionChange, OptionsFile, SettingChange, SettingResolution, SettingsPreset,
    game_setting_group, game_setting_kind,
};
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
pub use instances::{
    Collection, InstanceRecord, InstanceSettings, InstanceStore, Loader, NewInstance, StoreError,
};
pub use java::{
    InstanceJava, JavaChoice, JavaLocator, JavaNeeds, JavaRule, JavaRuntime, RuleStrength,
    choose as choose_java, install_major as java_install_major, parse_major as parse_java_major,
};
pub use java_runtime::graalvm::LICENSE_URL as GRAALVM_LICENSE_URL;
pub use java_runtime::{JavaBuild, JavaSource};
pub use launching::{
    DEFAULT_SETTLE, GAME_OUTPUT_LOG, GameEvent, GameExit, GameOptions, LaunchFailure, LaunchPhase,
    LaunchProblem, LaunchReadiness, LaunchRequest, LaunchRequirement, LaunchServiceError,
    LaunchSession, LaunchSignal, LaunchStatus, LaunchUpdate, Launcher, LogStream, ProcessError,
    command_line as game_command_line, run as run_game, runtimes_root,
};
pub use layout::Layout;
pub use loader::{
    LAUNCHABLE_LOADERS, LoaderError, LoaderVersion, decode_versions as decode_loader_versions,
    fetch_versions as fetch_loader_versions, normalize_profile as normalize_loader_profile,
    profile_url as loader_profile_url, recommended as recommended_loader,
    versions_url as loader_versions_url,
};
pub use lumilio_block_assets::{BakedPack, BlockCatalog, BlockNames};
pub use lumilio_minecraft::{
    ArgumentGroups, ArgumentTemplate, AssetCatalog, CatalogEntry, CatalogError, CompatibilityRule,
    CoordinateError, DownloadDescriptor, ExtractionPolicy, HostProfile, IdentifiedDownload,
    JavaRequirement, LaunchContext, LaunchDirectories, LaunchError, LaunchPlan, LibraryDependency,
    LoggingConfiguration, MachineArchitecture, NativeArchive, OFFICIAL_CATALOG_URL,
    PackageCoordinate, PlatformFamily, ReleaseError, ReleaseManifest, ReleaseSet, RequiredDownload,
    RuleDecision, VersionCatalog, VersionChannel, VersionKind,
};
use lumilio_nbt as nbt;
pub use lumilio_plugin_api::PluginState;
use lumilio_transfer as transfer;
use lumilio_transfer::resource_lock;
pub use lumilio_transfer::with_causes;
pub use microsoft::{
    AuthError, DeviceCode, MICROSOFT_CLIENT_ID, MicrosoftClient, MinecraftLogin, OAuthTokens,
    Secret, client_id as microsoft_client_id,
};
pub use minecraft::fetch_catalog;
pub use mirror_presets::MirrorPreset;
pub use modpack::{
    ClientSupport, ModpackError, OVERRIDES_LIMIT, PackFile, PackIndex, PendingDownload,
    extract_overrides, import as import_modpack, is_trusted_source,
    parse_index as parse_pack_index, plan as plan_modpack, read_index as read_pack_index,
};
pub use pack_export::{ExportError, ExportReport, ExportSpec, PackFormat};
pub use picture::{PictureError, fit_within, png_within};
pub use plugins::write::CONFLICT as WRITE_CONFLICT;
pub use plugins::{
    MapFailure, MapProviders, PluginContentSource, PluginEffect, PluginFinding, PluginHost,
    PluginInfo, PluginStatus, PluginTab,
};
pub use reclaim::{KeptReason, ReclaimError, Reclaimable};
pub use recovery::RecoveryNote;
pub use repair::{
    InstallationVerifier, IntegrityIssue, RepairError, RepairExecutor, RepairFinding, RepairPlan,
    RepairReport,
};
pub use schematic_assets::{
    AssetNotice, AssetPlan, BUNDLE_URL, BundleIndex, EntityGeometry, PlannedSource, ProvidedSource,
    SchematicAssetsError, SourceRole, run_dumper,
};
pub use schematic_ops::schemes::{ReplaceSchemes, SchemeRow};
pub use schematic_ops::{KeptBackup, SchematicLogEntry, SchematicOperation};
pub use screenshots::{ScreenshotError, ScreenshotInfo};
pub use servers::{
    Motd, MotdSpan, PackPolicy, PingError, ProtocolVersion, ServerEntry, ServerError, ServerStatus,
};
pub use service::{
    AppearanceApplyResult, LauncherService, Library, ProfileSnapshot, ThirdPartySignIn,
};
pub use settings::{
    AccountEntry, AccountKind, AuthServerEntry, DownloadSourcePreference, LauncherSettings,
    MAX_MEMORY_MB, MirrorRule, SettingsError, SettingsStore, TERMS_VERSION,
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
pub use transfer::{
    ByteStream, CONNECTIONS_PER_FILE, ConnectionBudget, ContentDigest, DefaultTransport,
    FileTransport, HttpMethod, HttpRequest, HttpTransport, OfficialSource, PrefixMirror,
    RetryPolicy, SourceChain, SourceFailure, SourceProvider, TransferBatchReport, TransferEngine,
    TransferError, TransferEvent, TransferOutcome, TransferRequest, Transport, TransportError,
    TransportFuture, TransportResponse, USER_AGENT,
};
pub use translation::{
    TranslatedDocument, TranslationError, TranslationPreferences, TranslationProvider,
};
pub use tuning::{
    AfterLaunch, Appearance, DOWNLOAD_CONCURRENCY, DiscoverPreferences, EnvVar, FONT_SCALE_PERCENT,
    InstanceLaunch, Language, LaunchTuning, LookPreferences, MAX_WINDOW_SIDE, MotionPreference,
    Preferences, QuickPlay, QuickPlayProblem, TuningError, WallpaperChoice, quick_play_problem,
    quick_play_world_unsupported, split_words,
};
pub use updates::{
    ContentUpdate, UpdateError, UpdateReport, apply as apply_update, check as check_updates,
};
pub use wallpaper::{WallpaperError, import_wallpaper, remove_wallpapers};
pub use worlds::{
    GameMode, ModelPreview, WorldError, WorldInfo, copy_name as world_copy_name,
    delete as delete_world, duplicate as duplicate_world, scan as scan_worlds, size as world_size,
};
pub use yggdrasil::{AuthServer, LITTLE_SKIN_URL, Profile as CharacterProfile, YggdrasilError};

mod pack_export;
mod persist;
mod picture;
mod plugins;
mod reclaim;
mod recovery;
mod repair;

mod root_lock;
mod schematic_assets;
mod schematic_ops;
mod screenshots;
mod servers;
mod service;
mod settings;
mod skin;
mod snapshots;
mod staged;
mod storage;

mod translation;
mod tuning;
mod updates;
mod wallpaper;
mod worlds;
mod yggdrasil;

pub mod world_map;
