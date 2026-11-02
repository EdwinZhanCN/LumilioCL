//! UI-independent launcher domain logic.
//!
//! Each capability module owns its state and operations (accounts,
//! appearance, instances, launching, Java, content, worlds, game settings,
//! diagnostics, activity, plugins, settings); `storage` and `network` are
//! what they share, and [`LauncherService`] composes them for front ends.

mod accounts;
mod activity;
mod appearance;
mod blocking;
mod clock;
mod content;
mod context;
mod diagnostics;
mod error;
mod game_settings;
mod instances;
mod java;
mod launcher_service;
mod launching;
mod minecraft;
mod network;
mod plugins;
mod settings;
mod storage;
mod worlds;

pub use accounts::ThirdPartySignIn;
pub use accounts::account::{
    AuthSession, MAX_PROFILE_NAME, OfflineProfile, ProfileError, ProfileId,
};
pub use accounts::credentials::{
    CredentialStore, MemoryCredentials, StoredLogin, SystemCredentials,
};
pub use accounts::microsoft::{
    AuthError, DeviceCode, MICROSOFT_CLIENT_ID, MicrosoftClient, MinecraftLogin, OAuthTokens,
    Secret, client_id as microsoft_client_id,
};
pub use accounts::yggdrasil::{
    AuthServer, LITTLE_SKIN_URL, Profile as CharacterProfile, YggdrasilError,
};
pub use activity::{
    ActiveTask, ActivityContext, ActivityEvent, ActivityGraph, ActivityGraphError, ActivityLog,
    ActivityNode, ActivityRecord, ActivityReport, ActivityScheduler, ActivityState, ActivityView,
    CancellationToken, FileDownload, FinishedTask, ProgressSnapshot, ProgressUnit, RetryAction,
    SourceSwitch, TaskAction, TaskBoard, TaskCategory, TaskDownloads, TaskLabel, TaskOutcome,
    Traffic,
};
pub use appearance::skin::{
    AccountLook, AppearanceChange, AppearanceError, AppearanceUpdate, LITTLE_SKIN_CSL, LibrarySkin,
    MojangCape, MojangClient, MojangProfile, MojangSkin, PairedCape, Pixels as SkinPixels,
    SkinChoice, SkinError, SkinLibrary, SkinModel, SkinSource, cape_pixels, looks_slim,
    skin_pixels,
};
pub use appearance::{AppearanceApplyResult, ProfileSnapshot};
pub use clock::now as unix_now;
pub use content::discover::{
    Catalog, CategoryTag, ContentClient, DEFAULT_SOURCE, DependencyKind, DiscoverError,
    Environment, FileHashes, GalleryImage, GameVersionTag, GroupLabel, IntentError, KindAbilities,
    LoaderTag, PackFileKind, Pick, Project, ProjectKind, ProjectLinks, ProjectOrigin,
    ProjectSummary, ReleaseChannel, ResolvedFile, SearchHit, SearchPage, SearchQuery, SideSupport,
    SortIndex, SourceFilters, Stance, TextFormat, Version, VersionFile, VersionGroup, environment,
    fits as version_fits, install_request as content_install_request, pick_version, version_groups,
};
pub use content::translation::{
    TranslatedDocument, TranslationError, TranslationPreferences, TranslationProvider,
};
pub use content::updates::{
    ContentUpdate, UpdateError, UpdateReport, apply as apply_update, check as check_updates,
};
pub use content::{
    AddedFile, ContentEffect, ContentEntry, ContentError, ContentItem, ContentList, ContentLookup,
    ContentResult, ContentSource, ContentSourceInfo, DependencyNeed, DependencyReport,
    DiscoverFilters, InstalledProject, ModMetadata, ProjectDetail, read_mod_metadata,
    remove as remove_content, scan as scan_content, set_enabled as set_content_enabled, sha1_hex,
};
pub use context::{Context, Revisions, Session, SessionPhase};
pub use diagnostics::attention::{
    AttentionItem, HomeChecks, HomeSummary, summarize as summarize_home, with_account,
};
pub use diagnostics::bundle::{Redactor, write_bundle};
pub use diagnostics::inspect::inspect_instance;
pub use diagnostics::reclaim::{KeptReason, ReclaimError, Reclaimable};
pub use diagnostics::{
    CrashReport, Facts, FileEntry, GameLogSource, GameLogs, LOW_MEMORY_MB, LogLevel, LogLine,
    Problem, ProblemKind, Severity, StorageUsage, clear_cache, diagnose, directory_size,
    filter_log, list_crash_reports, list_dir, log_level, log_lines, measure_storage,
    read_crash_report, read_latest_log,
};
pub use error::ServiceError;
pub use game_settings::{
    GameSettingGroup, GameSettingKind, GameSettingValue, GameSettingsError, GameSettingsView,
    InstanceOptions, OptionChange, OptionsFile, SettingChange, SettingResolution, SettingsPreset,
    game_setting_group, game_setting_kind,
};
pub use instances::history::{
    ChangeKind, HistoryEvent, HistoryLog, HistoryRead, SessionOutcome, finish_session,
    record_attempt,
};
pub use instances::import_game::{FoundGame, GameOrigin, ImportError};
pub use instances::modpack::{
    ClientSupport, ModpackError, OVERRIDES_LIMIT, PackFile, PackIndex, PendingDownload,
    extract_overrides, import as import_modpack, is_trusted_source,
    parse_index as parse_pack_index, plan as plan_modpack, read_index as read_pack_index,
};
pub use instances::pack_export::{ExportError, ExportReport, ExportSpec, PackFormat};
pub use instances::snapshot_files::{
    RESTORE_LIMIT, SnapshotError, SnapshotInfo, SnapshotScope, create as create_snapshot,
    delete as delete_snapshot, directory as snapshot_directory, list as list_snapshots,
    restore as restore_snapshot,
};
pub use instances::{
    Collection, InstanceRecord, InstanceSettings, InstanceStore, Library, Loader, NewInstance,
    StoreError,
};
pub use java::downloads::graalvm::LICENSE_URL as GRAALVM_LICENSE_URL;
pub use java::downloads::{JavaBuild, JavaSource};
pub use java::{
    InstanceJava, JavaChoice, JavaLocator, JavaNeeds, JavaRule, JavaRuntime, RuleStrength,
    choose as choose_java, install_major as java_install_major, parse_major as parse_java_major,
};
pub use launcher_service::LauncherService;
pub use launching::{
    DEFAULT_SETTLE, GAME_OUTPUT_LOG, GameEvent, GameExit, GameOptions, LaunchFailure, LaunchPhase,
    LaunchProblem, LaunchReadiness, LaunchRequest, LaunchRequirement, LaunchServiceError,
    LaunchSession, LaunchSignal, LaunchStatus, LaunchUpdate, Launcher, LogStream, ProcessError,
    command_line as game_command_line, run as run_game, runtimes_root,
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
pub use minecraft::fetch_catalog;
pub use minecraft::forge_meta::{forge_versions, neoforge_game_version, neoforge_versions};
pub use minecraft::install::{
    ArchiveLimits, ArtifactKind, AssetIndex, AssetIndexError, AssetObject, InstallError,
    InstallEvent, InstallReport, InstallStage, InstallationPlan, Installer, NativeBundle,
    NativeError, NativePublication, NativePublisher, PlannedArtifact,
};
pub use minecraft::loader::{
    LAUNCHABLE_LOADERS, LoaderError, LoaderVersion, decode_versions as decode_loader_versions,
    fetch_versions as fetch_loader_versions, normalize_profile as normalize_loader_profile,
    profile_url as loader_profile_url, recommended as recommended_loader,
    versions_url as loader_versions_url,
};
pub use minecraft::repair::{
    InstallationVerifier, IntegrityIssue, RepairError, RepairExecutor, RepairFinding, RepairPlan,
    RepairReport,
};
pub use network::fetch::{DOCUMENT_LIMIT, FetchError, fetch_document, post_document};
pub use plugins::write::CONFLICT as WRITE_CONFLICT;
pub use plugins::{
    MapFailure, MapProviders, PluginContentSource, PluginEffect, PluginFinding, PluginHost,
    PluginInfo, PluginStatus, PluginTab,
};
pub use settings::mirror_presets::MirrorPreset;
pub use settings::picture::{PictureError, fit_within, png_within};
pub use settings::tuning::{
    AfterLaunch, Appearance, DOWNLOAD_CONCURRENCY, DiscoverPreferences, EnvVar, FONT_SCALE_PERCENT,
    InstanceLaunch, Language, LaunchTuning, LookPreferences, MAX_WINDOW_SIDE, MotionPreference,
    Preferences, QuickPlay, QuickPlayProblem, TuningError, WallpaperChoice, quick_play_problem,
    quick_play_world_unsupported, split_words,
};
pub use settings::wallpaper::{WallpaperError, import_wallpaper, remove_wallpapers};
pub use settings::{
    AccountEntry, AccountKind, AuthServerEntry, DownloadSourcePreference, LauncherSettings,
    MAX_MEMORY_MB, MirrorRule, SettingsError, SettingsStore, TERMS_VERSION,
};
pub use settings::{recommended_memory_mb, total_memory_mb};
pub use storage::Layout;
pub use storage::recovery::RecoveryNote;
pub use transfer::{
    ByteStream, CONNECTIONS_PER_FILE, ConnectionBudget, ContentDigest, DefaultTransport,
    FileTransport, HttpMethod, HttpRequest, HttpTransport, OfficialSource, PrefixMirror,
    RetryPolicy, SourceChain, SourceFailure, SourceProvider, TransferBatchReport, TransferEngine,
    TransferError, TransferEvent, TransferOutcome, TransferRequest, Transport, TransportError,
    TransportFuture, TransportResponse, USER_AGENT,
};
pub use worlds::multiplayer::{
    Motd, MotdSpan, PackPolicy, PingError, ProtocolVersion, ServerEntry, ServerError, ServerStatus,
};
pub use worlds::schematic_assets::{
    AssetNotice, AssetPlan, BUNDLE_URL, BundleIndex, EntityGeometry, PlannedSource, ProvidedSource,
    SchematicAssetsError, SourceRole, run_dumper,
};
pub use worlds::schematic_ops::schemes::{ReplaceSchemes, SchemeRow};
pub use worlds::schematic_ops::{KeptBackup, SchematicLogEntry, SchematicOperation};
pub use worlds::screenshot_files::{ScreenshotError, ScreenshotInfo};
pub use worlds::world_map;
pub use worlds::{
    GameMode, ModelPreview, WorldError, WorldInfo, copy_name as world_copy_name,
    delete as delete_world, duplicate as duplicate_world, scan as scan_worlds, size as world_size,
};

#[cfg(test)]
mod testing;
