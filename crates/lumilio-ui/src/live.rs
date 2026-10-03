//! The presentation model for live data (plan 0007, T2).
//!
//! Everything here is owned, window-free and built from `lumilio-core`
//! values by pure functions, so it is unit-tested without a GPUI context. The
//! shell renders it and reports [`LiveIntent`]s; the application decides what
//! they mean.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{App, Window};
use std::collections::BTreeMap;

use lumilio_core::{
    ActiveTask, ActivityView, AttentionItem, DiscoverFilters, Environment, FinishedTask,
    HomeSummary, InstanceRecord, LaunchTuning, LauncherSettings, Loader, MirrorRule, Preferences,
    ProjectKind, RecoveryNote, SearchHit, SearchPage, SearchQuery, SortIndex, StorageUsage,
    TaskCategory, TaskOutcome, environment,
};

use crate::home::{HomePresentation, RecentEntry, Subject, WorldHint};

/// What a live page asks the application to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveIntent {
    /// Create an instance with the newest release.
    NewInstance,
    /// Choose a local `.mrpack` and import it as a new instance.
    ImportPack,
    ToggleFavorite(String),
    /// Open the registered instance without launching it.
    OpenInstance(String),
    /// Launch this instance from Home's launch moment.
    Play(String),
    /// Run this Discover search.
    Search(DiscoverQuery),
    /// Fetch the category and game-version choices.
    LoadFilters,
    /// Open a project's public page in its own window.
    OpenProject {
        kind: ProjectKind,
        slug: String,
    },
    /// Install a project: into the chosen instance, or as a new one for a modpack.
    Install {
        kind: ProjectKind,
        slug: String,
        title: String,
        /// One specific version, for mods, packs and shaders.
        version: Option<String>,
    },
    /// Stop this running task (by the service's task id).
    CancelTask(u64),
    /// Forget every finished task on the Activity page.
    ClearFinished,
    /// Run a failed or cancelled task again from its original input.
    RetryTask(lumilio_core::RetryAction),
    /// Ask for a name and make a collection.
    NewCollection,
    /// Ask for another name for this collection.
    RenameCollection(String),
    /// Remove this collection; its games stay in the library.
    DeleteCollection(String),
    /// Ask which collections this game (by id) belongs to.
    EditCollections(String),
    /// Open this game and ask how to export it as a modpack.
    ExportPackOf(String),
    /// Open this game and ask for a name to copy it under.
    CopyGameOf(String),
    /// Open this game and ask whether to delete it.
    DeleteGameOf(String),
    /// Show this game's folder in the file manager.
    RevealGame(String),
    /// Download one version's file where the person chooses (the application
    /// asks where).
    SaveVersionAs {
        project: String,
        version_id: String,
        file_name: String,
        title: String,
    },
    /// Remember how the Library is ordered and filtered (see
    /// `Preferences::library_sort`).
    RememberLibraryView {
        sort: u8,
        loader: u8,
    },
    /// Measure the shared game files no game uses and offer to remove them.
    CheckReclaimable,
    /// Remove them (after the person confirmed).
    Reclaim,
    /// Choose a folder made by another launcher and bring its game over.
    ImportGame,
    /// Choose a backup file and make a new game of it.
    RestoreBackup,
    /// Show the folder the launcher keeps its games in.
    OpenGamesFolder,
    /// Download Java into the launcher (the major version, if wanted).
    InstallJava(Option<u32>),
    /// Replace an installed file with its newer version (from Discover).
    UpdateInstalled {
        kind: ProjectKind,
        project: String,
        title: String,
        file_name: String,
        version_id: String,
    },
    /// Go back to installing into the current game (from the target bar).
    UseCurrentTarget,
    /// Open this game and do what a problem's button says (from Home).
    Resolve(String, crate::instance_detail::ProblemAction),
    /// Files dropped on the Library page.
    DropFiles(Vec<PathBuf>),
    /// Choose which instance Discover installs into (the current instance).
    InstallTarget(String),
    /// Open the add-offline-account dialog.
    NewAccount,
    /// Open the Microsoft sign-in dialog.
    MicrosoftSignIn,
    /// Refresh this Microsoft account's sign-in now.
    RefreshAccount(String),
    /// Use this account (by key) for later launches.
    SelectAccount(String),
    /// Forget this account (by key); games and worlds stay.
    RemoveAccount(String),
    /// Put text on the clipboard and say so in a toast.
    CopyText {
        text: String,
        notice: String,
    },
    /// Read the settings, Java list and disk use again.
    LoadSettings,
    SetPreferences(Preferences),
    SetLaunchDefaults(LaunchTuning),
    SetMemory {
        min_mb: Option<u32>,
        max_mb: Option<u32>,
    },
    SetConcurrency(Option<u32>),
    SetMirrors {
        mirrors: Vec<MirrorRule>,
        prefer: bool,
    },
    SetJavaRoots(Vec<PathBuf>),
    /// Turn an installation (by its home folder) off or on.
    SetJavaDisabled {
        home: PathBuf,
        disabled: bool,
    },
    /// Choose a Java (executable or home folder) to add.
    AddJava,
    /// Show a file or folder in the system file manager.
    Reveal(PathBuf),
    ClearCache,
    /// Choose where to save the diagnostics bundle, then write it.
    ExportDiagnostics,
    Refresh,
}

pub type LiveHandler = Rc<dyn Fn(LiveIntent, &mut Window, &mut App)>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryCard {
    pub id: String,
    pub name: String,
    /// `1.21.1 · Fabric`.
    pub meta: String,
    pub game_version: String,
    pub favorite: bool,
    /// Never played instances say so instead of showing a time.
    pub played: String,
    /// Stable number the pixel cover is drawn from.
    pub seed: u32,
    pub loader: Loader,
    pub world: WorldHint,
    /// When it was made, for ordering.
    pub created: u64,
}

/// One of the person's collections: a name and the games filed under it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionRow {
    pub name: String,
    /// Instance ids, in the order they were added.
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRow {
    /// Modrinth's id, to recognise what the game already has.
    pub project_id: String,
    pub kind: ProjectKind,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub environment: Option<Environment>,
    /// Category names as Modrinth spells them.
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: String,
    pub follows: String,
    pub updated: String,
    pub icon_url: Option<String>,
    /// Seeds the placeholder cover while (or instead of) the real icon.
    pub seed: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SearchStatus {
    /// Nothing searched yet.
    #[default]
    Idle,
    Searching,
    Failed(String),
    Done {
        total: u64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityRow {
    /// The running task's id (running rows only).
    pub task: Option<u64>,
    pub category: TaskCategory,
    pub title: String,
    pub detail: String,
    /// `None` while running with unknown progress.
    pub fraction: Option<f32>,
    pub state: ActivityState,
    /// The task id to cancel with, while the row is running and stoppable.
    pub cancel: Option<u64>,
    /// Done and total in `unit`, while running and known.
    pub amount: Option<(u64, u64)>,
    pub unit: lumilio_core::ProgressUnit,
    /// How fast it is going, in `unit` per second, once two readings exist.
    pub rate: Option<f64>,
    /// The game it concerns, for opening it.
    pub instance: Option<String>,
    /// How to run it again, if it can be.
    pub retry: Option<lumilio_core::RetryAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityState {
    Running,
    Done,
    Failed(String),
    Cancelled,
}

/// One account of the Accounts page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRow {
    /// What selection and removal name this account by (not its display name).
    pub key: String,
    pub name: String,
    /// The dashed profile id the game sees.
    pub uuid: String,
    pub selected: bool,
    /// The id was chosen by the user, not derived from the name.
    pub custom_id: bool,
    /// Signed in with Microsoft rather than offline.
    pub microsoft: bool,
    /// The stored sign-in no longer works.
    pub needs_sign_in: bool,
}

impl AccountRow {
    /// `离线账户` or `Microsoft`.
    #[must_use]
    pub const fn kind_label(&self) -> &'static str {
        if self.microsoft {
            "Microsoft"
        } else {
            "离线账户"
        }
    }
}

/// The rows of the Accounts page, in the order the accounts were added.
pub fn account_rows(settings: &LauncherSettings) -> Vec<AccountRow> {
    settings
        .accounts
        .iter()
        .filter_map(|entry| {
            let id = entry.profile_id().ok()?;
            let microsoft = entry.kind == lumilio_core::AccountKind::Microsoft;
            Some(AccountRow {
                selected: settings.selected_account.as_deref() == Some(entry.key().as_str()),
                key: entry.key(),
                uuid: id.to_string(),
                custom_id: !microsoft && entry.uuid.is_some(),
                name: entry.name.clone(),
                microsoft,
                needs_sign_in: entry.needs_sign_in,
            })
        })
        .collect()
}

/// One Java installation of the Java tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaRow {
    pub home: PathBuf,
    /// `Java 21.0.1`.
    pub title: String,
    /// `Zulu · arm64 · /path/to/home`.
    pub detail: String,
    pub disabled: bool,
}

/// Everything the Settings page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsView {
    pub preferences: Preferences,
    pub launch: LaunchTuning,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub download_concurrency: Option<u32>,
    pub mirrors: Vec<MirrorRule>,
    pub prefer_mirrors: bool,
    pub java_roots: Vec<PathBuf>,
    pub java: Vec<JavaRow>,
    /// `None` while the disk is still being measured.
    pub storage: Option<StorageUsage>,
    pub data_dir: PathBuf,
    pub total_memory_mb: Option<u64>,
}

/// The Settings page's data, from the saved settings and what was found on
/// this machine.
pub fn settings_view(
    settings: &LauncherSettings,
    java: &[(lumilio_core::JavaRuntime, bool)],
    storage: Option<StorageUsage>,
    data_dir: &Path,
    total_memory_mb: Option<u64>,
) -> SettingsView {
    SettingsView {
        preferences: settings.preferences.clone(),
        launch: settings.launch.clone(),
        min_memory_mb: settings.default_min_memory_mb,
        max_memory_mb: settings.default_max_memory_mb,
        download_concurrency: settings.download_concurrency,
        mirrors: settings.mirrors.clone(),
        prefer_mirrors: settings.prefer_mirrors,
        java_roots: settings.extra_java_roots.clone(),
        java: java
            .iter()
            .map(|(runtime, disabled)| JavaRow {
                home: runtime.home().to_owned(),
                title: format!("Java {}", runtime.version()),
                detail: [
                    runtime.vendor().map(str::to_owned),
                    runtime.architecture().map(str::to_owned),
                    Some(runtime.home().display().to_string()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
                disabled: *disabled,
            })
            .collect(),
        storage,
        data_dir: data_dir.to_owned(),
        total_memory_mb,
    }
}

/// One sentence about why a Microsoft sign-in step failed, and what to do.
#[must_use]
pub fn auth_message(error: &lumilio_core::AuthError) -> String {
    use lumilio_core::AuthError;
    match error {
        AuthError::Declined => "你在浏览器里拒绝了这次登录".to_owned(),
        AuthError::Expired => "代码已经过期，请重新开始登录".to_owned(),
        AuthError::Cancelled => "登录已取消".to_owned(),
        AuthError::SignInRequired => "登录已失效，需要重新登录".to_owned(),
        AuthError::NoXboxAccount => {
            "这个 Microsoft 账户还没有 Xbox 档案，请先在 xbox.com 创建一个".to_owned()
        }
        AuthError::ChildAccount => {
            "这是儿童账户，需要家长在 Microsoft 家庭组里允许在线游戏".to_owned()
        }
        AuthError::XboxUnavailable => "你所在的地区不提供 Xbox 服务".to_owned(),
        AuthError::AdultVerificationRequired => {
            "这个账户需要先在 Xbox 网站完成成年人验证".to_owned()
        }
        AuthError::NoGameOwnership => "这个账户没有 Minecraft Java 版".to_owned(),
        AuthError::ServicesRefused(_) => {
            "Minecraft 服务拒绝了这个启动器的登录，可能这个应用注册还没有通过 Mojang 的审批"
                .to_owned()
        }
        AuthError::CredentialStore(_) => {
            "系统凭据库不可用，登录信息无法安全保存，所以没有登录".to_owned()
        }
        AuthError::Network(_) => "连不上登录服务，请检查网络后重试".to_owned(),
        AuthError::Protocol(_) => "登录服务的回答不符合预期".to_owned(),
    }
}

/// What a failed account change looks like in the dialog: one sentence about
/// what to change, and the raw cause behind 技术详情.
pub fn account_failure(error: &lumilio_core::ServiceError) -> (String, String) {
    use lumilio_core::{ServiceError, SettingsError};
    let message = match error {
        ServiceError::Auth(auth) => auth_message(auth),
        ServiceError::SignInRequired(name) => {
            format!("账户 {name} 需要重新登录 Microsoft")
        }
        ServiceError::Settings(SettingsError::DuplicateAccount(name)) => {
            format!("已经有叫“{name}”的账户了（名称不分大小写）")
        }
        ServiceError::Settings(SettingsError::DuplicateUuid(_)) => {
            "这个 UUID 已经被另一个账户使用".to_owned()
        }
        ServiceError::Settings(SettingsError::Profile(_)) => "名称或 UUID 不符合要求".to_owned(),
        _ => "没能保存账户".to_owned(),
    };
    (message, error.to_string())
}

/// Everything the live pages show.
#[derive(Clone, Debug)]
pub struct LiveModel {
    pub library: Vec<LibraryCard>,
    pub collections: Vec<CollectionRow>,
    /// Games with something wrong, for Home.
    pub attention: Vec<crate::home::AttentionRow>,
    pub library_loaded: bool,
    pub results: Vec<SearchRow>,
    pub search: SearchStatus,
    pub query: DiscoverQuery,
    pub filters: FilterModel,
    /// Which instance Discover installs into.
    pub install_target: Option<String>,
    /// The target was set by browsing from that game's page, not by the
    /// current game; Discover says so and offers the way back.
    pub target_locked: bool,
    /// What the target game already has of the searched kind, by project.
    pub installed: BTreeMap<String, lumilio_core::InstalledProject>,
    pub activity: Vec<ActivityRow>,
    /// The last reading of each running task, for its speed.
    rates: BTreeMap<u64, RateSample>,
    pub accounts: Vec<AccountRow>,
    pub accounts_loaded: bool,
    pub settings: Option<SettingsView>,
}

impl Default for LiveModel {
    fn default() -> Self {
        Self {
            library: Vec::new(),
            collections: Vec::new(),
            attention: Vec::new(),
            library_loaded: false,
            results: Vec::new(),
            search: SearchStatus::Idle,
            query: DiscoverQuery::new(ProjectKind::Modpack),
            filters: FilterModel::default(),
            install_target: None,
            target_locked: false,
            installed: BTreeMap::new(),
            activity: Vec::new(),
            rates: BTreeMap::new(),
            accounts: Vec::new(),
            accounts_loaded: false,
            settings: None,
        }
    }
}

/// What was last seen of a running task.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RateSample {
    done: u64,
    at_ms: u64,
    per_sec: Option<f64>,
}

/// The shortest gap between two readings that is worth a speed.
const RATE_GAP_MS: u64 = 200;

/// The next reading's speed: the change since the last reading, smoothed
/// with the earlier speed so a burst does not jump the number around.
fn next_sample(previous: Option<RateSample>, done: u64, now_ms: u64) -> RateSample {
    let Some(previous) = previous else {
        return RateSample {
            done,
            at_ms: now_ms,
            per_sec: None,
        };
    };
    let gap = now_ms.saturating_sub(previous.at_ms);
    if gap < RATE_GAP_MS || done < previous.done {
        return previous;
    }
    let instant = (done - previous.done) as f64 / (gap as f64 / 1000.);
    RateSample {
        done,
        at_ms: now_ms,
        per_sec: Some(
            previous
                .per_sec
                .map_or(instant, |old| old * 0.6 + instant * 0.4),
        ),
    }
}

/// `3.2 MB`, `850 KB`.
fn bytes_text(bytes: f64) -> String {
    const KB: f64 = 1024.;
    if bytes >= KB * KB * KB {
        format!("{:.1} GB", bytes / (KB * KB * KB))
    } else if bytes >= KB * KB {
        format!("{:.1} MB", bytes / (KB * KB))
    } else {
        format!("{:.0} KB", (bytes / KB).max(1.))
    }
}

/// How fast a task goes, in words.
#[must_use]
pub fn rate_text(unit: lumilio_core::ProgressUnit, per_sec: f64) -> String {
    match unit {
        lumilio_core::ProgressUnit::Bytes => format!("{}/秒", bytes_text(per_sec)),
        lumilio_core::ProgressUnit::Items if per_sec >= 10. => format!("{per_sec:.0} 个文件/秒"),
        lumilio_core::ProgressUnit::Items => format!("{per_sec:.1} 个文件/秒"),
    }
}

/// How long is left at this speed, in words; `None` when it is not moving.
#[must_use]
pub fn eta_text(remaining: u64, per_sec: f64) -> Option<String> {
    if per_sec <= 0. || !per_sec.is_finite() {
        return None;
    }
    let seconds = (remaining as f64 / per_sec).ceil() as u64;
    Some(match seconds {
        0..=59 => "不到 1 分钟".to_owned(),
        60..=3599 => format!("约 {} 分钟", seconds.div_ceil(60)),
        _ => format!("约 {} 小时 {} 分钟", seconds / 3600, seconds % 3600 / 60),
    })
}

impl LiveModel {
    /// Replaces the Activity rows, working out how fast each running task goes
    /// from how far it got since the last time, and naming each game.
    pub fn set_activity(&mut self, mut rows: Vec<ActivityRow>, now_ms: u64) {
        let mut next = BTreeMap::new();
        for row in &mut rows {
            if let Some(name) = self
                .library
                .iter()
                .find(|card| card.id == row.detail)
                .map(|card| card.name.clone())
            {
                row.detail = name;
            }
            let (Some(task), Some((done, _))) = (row.task, row.amount) else {
                continue;
            };
            let sample = next_sample(self.rates.get(&task).copied(), done, now_ms);
            row.rate = sample.per_sec.filter(|rate| *rate > 0.);
            next.insert(task, sample);
        }
        self.rates = next;
        self.activity = rows;
    }

    /// The games of one collection that are still in the library.
    pub fn collection_cards(&self, collection: &CollectionRow) -> Vec<&LibraryCard> {
        collection
            .members
            .iter()
            .filter_map(|id| self.library.iter().find(|card| &card.id == id))
            .collect()
    }

    /// Each collection with whether this game is in it, for the picker.
    pub fn memberships_of(&self, id: &str) -> Vec<(String, bool)> {
        self.collections
            .iter()
            .map(|collection| {
                (
                    collection.name.clone(),
                    collection.members.iter().any(|member| member == id),
                )
            })
            .collect()
    }

    /// The account later launches use, if one is chosen.
    pub fn selected_account(&self) -> Option<&AccountRow> {
        self.accounts.iter().find(|row| row.selected)
    }

    /// How many pages the current results span.
    pub fn pages(&self) -> u32 {
        match self.search {
            SearchStatus::Done { total } => {
                u32::try_from(total.div_ceil(u64::from(self.query.page_size.max(1))))
                    .unwrap_or(u32::MAX)
            }
            _ => 0,
        }
    }

    pub fn active_tasks(&self) -> u32 {
        u32::try_from(
            self.activity
                .iter()
                .filter(|row| row.state == ActivityState::Running)
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    /// Sets the library and keeps the install target valid: the previous
    /// choice if it still exists, else the most recently played instance.
    pub fn set_library(&mut self, library: Vec<LibraryCard>, target_hint: Option<String>) {
        let exists = |id: &String| library.iter().any(|card| &card.id == id);
        let keep = self.install_target.clone().filter(exists);
        self.install_target = keep
            .or(target_hint.filter(exists))
            .or_else(|| library.first().map(|card| card.id.clone()));
        self.library = library;
        self.library_loaded = true;
    }
}

const fn ui_loader(loader: Loader) -> crate::cover::Loader {
    match loader {
        Loader::Vanilla => crate::cover::Loader::Vanilla,
        Loader::Fabric => crate::cover::Loader::Fabric,
        Loader::Forge => crate::cover::Loader::Forge,
        Loader::NeoForge => crate::cover::Loader::NeoForge,
        Loader::Quilt => crate::cover::Loader::Quilt,
    }
}

pub const fn loader_label(loader: Loader) -> &'static str {
    ui_loader(loader).label()
}

/// The sample-side loader, for the cover painter.
pub const fn cover_loader(loader: Loader) -> crate::cover::Loader {
    ui_loader(loader)
}

/// FNV-1a: a stable number from text, so a cover never changes between runs.
pub fn seed_of(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

pub fn world_of(id: &str) -> WorldHint {
    match seed_of(id) / 7 % 4 {
        0 => WorldHint::Overworld,
        1 => WorldHint::Underground,
        2 => WorldHint::Redstone,
        _ => WorldHint::Nether,
    }
}

/// `刚刚`, `5 分钟前`, `3 小时前`, `昨天`, `4 天前`, `2 个月前`.
pub fn relative_time(then: u64, now: u64) -> String {
    let seconds = now.saturating_sub(then);
    match seconds {
        0..60 => "刚刚".to_owned(),
        60..3600 => format!("{} 分钟前", seconds / 60),
        3600..86_400 => format!("{} 小时前", seconds / 3600),
        86_400..172_800 => "昨天".to_owned(),
        172_800..2_592_000 => format!("{} 天前", seconds / 86_400),
        2_592_000..31_536_000 => format!("{} 个月前", seconds / 2_592_000),
        _ => format!("{} 年前", seconds / 31_536_000),
    }
}

pub fn instance_meta(record: &InstanceRecord) -> String {
    format!("{} · {}", record.game_version, loader_label(record.loader))
}

pub fn library_card(record: &InstanceRecord, now: u64) -> LibraryCard {
    LibraryCard {
        id: record.id.clone(),
        name: record.name.clone(),
        meta: instance_meta(record),
        game_version: record.game_version.clone(),
        favorite: record.favorite,
        played: record.last_played.map_or_else(
            || "还没玩过".to_owned(),
            |at| format!("上次游玩 {}", relative_time(at, now)),
        ),
        seed: seed_of(&record.id),
        loader: record.loader,
        world: world_of(&record.id),
        created: record.created_at,
    }
}

/// Home's "needs attention" rows: each game's worst problem and what to do
/// first. Games no longer in the library are skipped.
#[must_use]
pub fn attention_rows(
    items: &[AttentionItem],
    library: &[LibraryCard],
) -> Vec<crate::home::AttentionRow> {
    items
        .iter()
        .filter_map(|item| {
            let card = library.iter().find(|card| card.id == item.instance_id)?;
            let (title, detail) = crate::instance_detail::problem_text(&item.headline);
            Some(crate::home::AttentionRow {
                instance: item.instance_id.clone(),
                name: card.name.clone(),
                title,
                detail,
                action: crate::instance_detail::problem_action(&item.headline.kind),
                more: item.total.saturating_sub(1),
            })
        })
        .collect()
}

/// Library order: favorites first, then most recently played, then newest.
pub fn library_cards(records: &[InstanceRecord], now: u64) -> Vec<LibraryCard> {
    let mut sorted: Vec<&InstanceRecord> = records.iter().collect();
    sorted.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then(b.last_played.cmp(&a.last_played))
            .then(b.created_at.cmp(&a.created_at))
    });
    sorted
        .into_iter()
        .map(|record| library_card(record, now))
        .collect()
}

/// Home from the library and its summary.
///
/// Continue is the summary's pick, or — before anything was ever played — the
/// newest instance, so the first launch is one press away. No instances at all
/// is first use. Returns the instance Continue would launch alongside.
pub fn home_presentation(
    records: &[InstanceRecord],
    summary: &HomeSummary,
    prefer: Option<&str>,
    now: u64,
) -> (HomePresentation, Option<String>) {
    let find = |id: &str| records.iter().find(|record| record.id == id);
    let subject_of = |record: &InstanceRecord| Subject {
        title: record.name.clone(),
        metadata: match record.last_played {
            Some(at) => format!(
                "{} · 上次游玩于 {}",
                instance_meta(record),
                relative_time(at, now)
            ),
            None => format!("{} · 还没玩过", instance_meta(record)),
        },
        world: world_of(&record.id),
    };
    // The current instance wins; without one, the latest played, then the newest.
    let chosen = prefer
        .and_then(find)
        .or_else(|| summary.continue_with.as_deref().and_then(find))
        .or_else(|| records.iter().max_by_key(|record| record.created_at));
    let Some(chosen) = chosen else {
        return (HomePresentation::FirstUse, None);
    };
    let recent = summary
        .recent
        .iter()
        .filter_map(|id| find(id))
        .filter(|record| record.id != chosen.id)
        .map(|record| RecentEntry {
            id: Some(record.id.clone()),
            title: record.name.clone(),
            metadata: format!(
                "{} · {}",
                instance_meta(record),
                record
                    .last_played
                    .map_or_else(String::new, |at| relative_time(at, now))
            ),
        })
        .collect();
    (
        HomePresentation::Continue {
            subject: subject_of(chosen),
            recent,
        },
        Some(chosen.id.clone()),
    )
}

pub fn count_label(count: u64) -> String {
    match count {
        0..10_000 => count.to_string(),
        10_000..100_000_000 => format!("{:.1} 万", count as f64 / 10_000.0),
        _ => format!("{:.1} 亿", count as f64 / 100_000_000.0),
    }
}

pub fn search_row(hit: &SearchHit, now: u64) -> SearchRow {
    SearchRow {
        project_id: hit.project_id.clone(),
        kind: hit.kind,
        slug: hit.slug.clone(),
        title: hit.title.clone(),
        author: hit.author.clone(),
        summary: hit.description.clone(),
        environment: environment(hit.client_side, hit.server_side),
        categories: hit.categories.clone(),
        loaders: hit.loaders.clone(),
        downloads: count_label(hit.downloads),
        follows: count_label(hit.follows),
        updated: parse_rfc3339(&hit.updated).map_or_else(String::new, |at| relative_time(at, now)),
        icon_url: hit.icon_url.clone(),
        seed: seed_of(&hit.project_id),
    }
}

pub fn search_rows(page: &SearchPage, now: u64) -> Vec<SearchRow> {
    page.hits.iter().map(|hit| search_row(hit, now)).collect()
}

/// Seconds since the epoch for `2026-09-27T10:00:00Z` and the offset/fraction
/// variants Modrinth sends; `None` for anything else.
pub fn parse_rfc3339(text: &str) -> Option<u64> {
    let (date, time) = text.split_once('T')?;
    let mut ymd = date.split('-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (ymd.next()??, ymd.next()??, ymd.next()??);
    if ymd.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let clock: String = time
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == ':')
        .collect();
    let mut hms = clock.split(':').map(|part| part.parse::<i64>().ok());
    let (hour, minute, second) = (
        hms.next()??,
        hms.next()??,
        hms.next().flatten().unwrap_or(0),
    );
    // Days since 1970-01-01 (proleptic Gregorian, Hinnant's algorithm).
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    u64::try_from(days * 86_400 + hour * 3600 + minute * 60 + second).ok()
}

// ── Discover query ──────────────────────────────────────────────────────

pub const PAGE_SIZES: [u32; 4] = [10, 20, 50, 100];

pub const SORTS: [SortIndex; 5] = [
    SortIndex::Relevance,
    SortIndex::Downloads,
    SortIndex::Follows,
    SortIndex::Newest,
    SortIndex::Updated,
];

pub const fn sort_label(sort: SortIndex) -> &'static str {
    match sort {
        SortIndex::Relevance => "相关度",
        SortIndex::Downloads => "下载量",
        SortIndex::Follows => "关注数",
        SortIndex::Newest => "最新发布",
        SortIndex::Updated => "最近更新",
    }
}

/// What the Discover page is asking for. The page state *is* this value, so a
/// refresh of the list can never disagree with the controls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverQuery {
    pub kind: ProjectKind,
    pub text: String,
    pub sort: SortIndex,
    /// Zero-based.
    pub page: u32,
    pub page_size: u32,
    pub game_version: Option<String>,
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
}

/// One change to the query made on the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoverChange {
    Kind(ProjectKind),
    Sort(SortIndex),
    PageSize(u32),
    Version(Option<String>),
    ToggleCategory(String),
    ToggleLoader(String),
    Page(u32),
    /// Drop the version, category and loader filters.
    ClearFilters,
}

fn toggle(list: &mut Vec<String>, name: String) {
    match list.iter().position(|item| *item == name) {
        Some(at) => {
            list.remove(at);
        }
        None => list.push(name),
    }
}

impl DiscoverQuery {
    pub fn new(kind: ProjectKind) -> Self {
        Self {
            kind,
            text: String::new(),
            sort: SortIndex::Relevance,
            page: 0,
            page_size: 20,
            game_version: None,
            categories: Vec::new(),
            loaders: Vec::new(),
        }
    }

    /// Applies one change. Anything but paging returns to the first page, and
    /// a different project type forgets its categories and loaders (they do
    /// not carry over).
    #[must_use]
    pub fn apply(mut self, change: DiscoverChange) -> Self {
        if let DiscoverChange::Page(page) = change {
            self.page = page;
            return self;
        }
        self.page = 0;
        match change {
            DiscoverChange::Kind(kind) => {
                if kind != self.kind {
                    self.kind = kind;
                    self.categories.clear();
                    self.loaders.clear();
                }
            }
            DiscoverChange::Sort(sort) => self.sort = sort,
            DiscoverChange::PageSize(size) => self.page_size = size.clamp(1, 100),
            DiscoverChange::Version(version) => self.game_version = version,
            DiscoverChange::ToggleCategory(name) => toggle(&mut self.categories, name),
            DiscoverChange::ToggleLoader(name) => toggle(&mut self.loaders, name),
            DiscoverChange::ClearFilters => {
                self.game_version = None;
                self.categories.clear();
                self.loaders.clear();
            }
            DiscoverChange::Page(_) => {}
        }
        self
    }

    pub fn to_search(&self) -> SearchQuery {
        SearchQuery {
            text: self.text.clone(),
            kind: self.kind,
            game_version: self.game_version.clone(),
            loaders: self.loaders.clone(),
            categories: self.categories.clone(),
            sort: self.sort,
            page: self.page,
            page_size: self.page_size,
        }
    }

    /// Whether any filter narrows the results.
    pub fn filtered(&self) -> bool {
        self.game_version.is_some() || !self.categories.is_empty() || !self.loaders.is_empty()
    }

    /// Only mods and modpacks have loaders worth choosing.
    pub const fn has_loaders(&self) -> bool {
        matches!(self.kind, ProjectKind::Mod | ProjectKind::Modpack)
    }
}

// ── Pager ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageItem {
    /// Zero-based page.
    Page(u32),
    Gap,
}

/// The numbers a pager shows: first, last, and the current page with one
/// neighbour each side; a gap of a single page is filled instead of elided.
pub fn page_items(current: u32, pages: u32) -> Vec<PageItem> {
    if pages == 0 {
        return Vec::new();
    }
    let last = pages - 1;
    let current = current.min(last);
    let mut shown: Vec<u32> = vec![0, last, current];
    if current > 0 {
        shown.push(current - 1);
    }
    if current < last {
        shown.push(current + 1);
    }
    shown.sort_unstable();
    shown.dedup();
    let mut items = Vec::new();
    let mut previous: Option<u32> = None;
    for page in shown {
        match previous {
            Some(before) if page == before + 2 => items.push(PageItem::Page(before + 1)),
            Some(before) if page > before + 2 => items.push(PageItem::Gap),
            _ => {}
        }
        items.push(PageItem::Page(page));
        previous = Some(page);
    }
    items
}

// ── Filters ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FilterModel {
    pub loaded: bool,
    pub error: Option<String>,
    /// Release versions, newest first.
    pub versions: Vec<String>,
    categories: Vec<(ProjectKind, String, String)>,
}

impl FilterModel {
    pub fn from_core(filters: &DiscoverFilters) -> Self {
        Self {
            loaded: true,
            error: None,
            versions: filters
                .game_versions
                .iter()
                .filter(|tag| tag.release)
                .map(|tag| tag.version.clone())
                .collect(),
            categories: filters
                .categories
                .iter()
                .map(|tag| (tag.kind, tag.header.clone(), tag.name.clone()))
                .collect(),
        }
    }

    /// Category names for a project type, in Modrinth's order, without the
    /// resolution and performance groups that only some types have (they are
    /// kept: each is a category facet like any other).
    pub fn categories(&self, kind: ProjectKind) -> Vec<&str> {
        self.categories
            .iter()
            .filter(|(tag_kind, _, _)| *tag_kind == kind)
            .map(|(_, _, name)| name.as_str())
            .collect()
    }
}

/// The Chinese name of a Modrinth category, loader or feature tag; unknown
/// ones are shown as Modrinth spells them, capitalized.
pub fn tag_label(name: &str) -> String {
    let known = match name {
        "adventure" => "冒险",
        "cursed" => "诅咒",
        "decoration" => "装饰",
        "economy" => "经济",
        "equipment" => "装备",
        "food" => "食物",
        "game-mechanics" => "游戏机制",
        "library" => "库",
        "magic" => "魔法",
        "management" => "管理",
        "minigame" => "小游戏",
        "mobs" => "生物",
        "optimization" => "优化",
        "social" => "社交",
        "storage" => "存储",
        "technology" => "科技",
        "transportation" => "交通",
        "utility" => "实用",
        "worldgen" => "世界生成",
        "challenging" => "挑战",
        "combat" => "战斗",
        "kitchen-sink" => "大杂烩",
        "lightweight" => "轻量",
        "multiplayer" => "多人",
        "quests" => "任务",
        "audio" => "音频",
        "blocks" => "方块",
        "core-shaders" => "核心着色器",
        "entities" => "实体",
        "environment" => "环境",
        "fonts" => "字体",
        "gui" => "界面",
        "items" => "物品",
        "locale" => "本地化",
        "modded" => "模组",
        "models" => "模型",
        "realistic" => "写实",
        "simplistic" => "极简",
        "themed" => "主题",
        "tweaks" => "调整",
        "vanilla-like" => "原版风格",
        "atmosphere" => "大气",
        "bloom" => "泛光",
        "cartoon" => "卡通",
        "colored-lighting" => "彩色光照",
        "fantasy" => "奇幻",
        "foliage" => "植被",
        "path-tracing" => "路径追踪",
        "pbr" => "PBR",
        "reflections" => "反射",
        "semi-realistic" => "半写实",
        "low" => "低性能影响",
        "medium" => "中性能影响",
        "high" => "高性能影响",
        "potato" => "土豆机",
        "screenshot" => "截图用",
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "quilt" => "Quilt",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_owned();
    }
    let spaced = name.replace('-', " ");
    let mut chars = spaced.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

pub const fn environment_label(environment: Environment) -> &'static str {
    match environment {
        Environment::ClientAndServer => "客户端和服务端",
        Environment::ClientOnly => "客户端",
        Environment::ServerOnly => "服务端",
    }
}

/// The loaders worth offering as filters.
/// One calm sentence about what start-up recovery did, worst news first.
/// `None` when it had nothing to report. Paths and reasons stay out of it; the
/// full notes are in the log.
pub fn recovery_message(notes: &[RecoveryNote]) -> Option<String> {
    use RecoveryNote::*;
    let rank = |note: &RecoveryNote| match note {
        LibraryRecovered { .. } => 0,
        SettingsRecovered { .. } => 1,
        DeleteStuck { .. }
        | PublishStuck { .. }
        | RestoreStuck { .. }
        | DeleteConflict { .. }
        | JournalUnusable { .. } => 2,
        RestoreRolledBack { .. } => 3,
        SessionInterrupted { .. } => 4,
        ProfileMissing { .. } => 5,
        DeleteRolledBack { .. }
        | DeleteCompleted { .. }
        | PublishCompleted { .. }
        | PublishDiscarded { .. } => 6,
        ActivityLogSkipped { .. } => 7,
    };
    let first = notes.iter().min_by_key(|note| rank(note))?;
    let headline = match first {
        LibraryRecovered { candidates, .. } if candidates.is_empty() => {
            "游戏库文件无法读取，已保留原文件并重新开始".to_owned()
        }
        LibraryRecovered { candidates, .. } => format!(
            "游戏库文件无法读取，已保留原文件并重新开始；磁盘上找到 {} 个可能的游戏目录",
            candidates.len()
        ),
        SettingsRecovered { .. } => "设置文件无法读取，已保留原文件并使用默认设置".to_owned(),
        DeleteStuck { .. } | PublishStuck { .. } | RestoreStuck { .. } => {
            "上次中断的操作还没能收尾，文件已保留，下次启动会再试".to_owned()
        }
        DeleteConflict { .. } | JournalUnusable { .. } => {
            "发现无法自动处理的中断记录，相关文件没有被改动".to_owned()
        }
        RestoreRolledBack { .. } => "上次快照恢复被中断，已回到恢复前的样子".to_owned(),
        SessionInterrupted { .. } => "启动器上次在游戏运行时退出，那次游玩的结果未知".to_owned(),
        ProfileMissing { .. } => "有游戏的目录不见了，可以在诊断里查看".to_owned(),
        DeleteRolledBack { .. } => "上次删除被中断，游戏已原样保留".to_owned(),
        DeleteCompleted { .. } => "上次中断的删除已经完成".to_owned(),
        PublishCompleted { .. } => "上次中断的导入或复制已经完成".to_owned(),
        PublishDiscarded { .. } => "上次中断的导入或复制没有完成，已清理，可以重新开始".to_owned(),
        ActivityLogSkipped { count } => format!("动态记录里有 {count} 行无法读取，已跳过"),
    };
    Some(match notes.len() {
        1 => headline,
        more => format!("{headline}（另有 {} 项恢复记录）", more - 1),
    })
}

pub const LOADER_CHOICES: [&str; 4] = ["fabric", "forge", "neoforge", "quilt"];

fn active_row(task: &ActiveTask, cancellable: bool) -> ActivityRow {
    ActivityRow {
        task: Some(task.id),
        amount: task.progress.filter(|(_, total)| *total > 0),
        unit: task.unit,
        rate: None,
        instance: task.instance_id.clone(),
        retry: None,
        category: task.category,
        title: task.label.clone(),
        detail: task.instance_id.clone().unwrap_or_default(),
        fraction: task
            .progress
            .filter(|(_, total)| *total > 0)
            .map(|(done, total)| (done as f64 / total as f64).clamp(0., 1.) as f32),
        state: ActivityState::Running,
        cancel: cancellable.then_some(task.id),
    }
}

fn finished_row(task: &FinishedTask, now: u64) -> ActivityRow {
    let state = match &task.outcome {
        TaskOutcome::Succeeded => ActivityState::Done,
        TaskOutcome::Failed(message) => ActivityState::Failed(message.clone()),
        TaskOutcome::Cancelled => ActivityState::Cancelled,
    };
    ActivityRow {
        task: None,
        amount: None,
        unit: lumilio_core::ProgressUnit::Items,
        rate: None,
        instance: task.instance_id.clone(),
        retry: task.retry.clone(),
        category: task.category,
        title: task.label.clone(),
        detail: relative_time(task.finished, now),
        fraction: None,
        state,
        cancel: None,
    }
}

/// Running tasks first, then finished ones newest first.
pub fn activity_rows(view: &ActivityView, now: u64) -> Vec<ActivityRow> {
    view.active
        .iter()
        .map(|task| active_row(task, view.cancellable.contains(&task.id)))
        .chain(view.finished.iter().map(|task| finished_row(task, now)))
        .collect()
}

/// The Activity tabs: everything, then one tab per task category.
pub const ACTIVITY_CATEGORIES: [Option<TaskCategory>; 5] = [
    None,
    Some(TaskCategory::Download),
    Some(TaskCategory::Install),
    Some(TaskCategory::Update),
    Some(TaskCategory::Repair),
];

/// The rows of one Activity tab, in their given order.
pub fn activity_in_tab(rows: &[ActivityRow], tab: usize) -> Vec<&ActivityRow> {
    let wanted = ACTIVITY_CATEGORIES.get(tab).copied().flatten();
    rows.iter()
        .filter(|row| wanted.is_none_or(|category| row.category == category))
        .collect()
}

#[cfg(test)]
mod tests {
    use lumilio_core::{AttentionItem, InstanceSettings};

    use super::*;

    fn record(
        id: &str,
        favorite: bool,
        last_played: Option<u64>,
        created_at: u64,
    ) -> InstanceRecord {
        InstanceRecord {
            id: id.to_owned(),
            name: id.to_uppercase(),
            game_version: "1.21.1".to_owned(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.0".to_owned()),
            favorite,
            created_at,
            last_played,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
        }
    }

    const NOW: u64 = 10_000_000;

    #[test]
    fn recovery_leads_with_the_worst_news_and_counts_the_rest() {
        use lumilio_core::RecoveryNote::*;
        assert_eq!(recovery_message(&[]), None);
        let note = recovery_message(&[
            DeleteCompleted {
                instance_id: "a".into(),
            },
            LibraryRecovered {
                preserved: "/x".into(),
                candidates: vec!["old".into(), "older".into()],
            },
            SessionInterrupted {
                instance_id: "b".into(),
                started: 1,
            },
        ])
        .unwrap();
        assert!(note.starts_with("游戏库文件无法读取"));
        assert!(note.contains("2 个可能的游戏目录") && note.contains("另有 2 项"));
        let single = recovery_message(&[ActivityLogSkipped { count: 3 }]).unwrap();
        assert!(single.contains('3') && !single.contains("另有"));
        assert!(!note.contains("/x"), "paths stay out of the sentence");
    }

    #[test]
    fn relative_time_steps_through_the_units() {
        assert_eq!(relative_time(NOW, NOW), "刚刚");
        assert_eq!(relative_time(NOW - 59, NOW), "刚刚");
        assert_eq!(relative_time(NOW - 60, NOW), "1 分钟前");
        assert_eq!(relative_time(NOW - 7200, NOW), "2 小时前");
        assert_eq!(relative_time(NOW - 90_000, NOW), "昨天");
        assert_eq!(relative_time(NOW - 4 * 86_400, NOW), "4 天前");
        assert_eq!(relative_time(NOW - 70 * 86_400, NOW), "2 个月前");
        // A clock that ran backwards never panics or goes negative.
        assert_eq!(relative_time(NOW + 500, NOW), "刚刚");
    }

    #[test]
    fn covers_are_stable_per_instance_and_differ_between_them() {
        assert_eq!(seed_of("survival"), seed_of("survival"));
        assert_ne!(seed_of("survival"), seed_of("creative"));
        assert_eq!(world_of("survival"), world_of("survival"));
    }

    #[test]
    fn the_library_lists_favorites_then_recent_then_newest() {
        let records = [
            record("old", false, None, 1),
            record("new", false, None, 9),
            record("played", false, Some(500), 2),
            record("star", true, None, 3),
        ];
        let order: Vec<_> = library_cards(&records, NOW)
            .into_iter()
            .map(|card| card.id)
            .collect();
        assert_eq!(order, ["star", "played", "new", "old"]);
    }

    #[test]
    fn cards_say_when_an_instance_was_never_played() {
        let never = library_card(&record("a", false, None, 1), NOW);
        assert_eq!(never.played, "还没玩过");
        assert_eq!(never.meta, "1.21.1 · Fabric");
        let played = library_card(&record("b", false, Some(NOW - 3600), 1), NOW);
        assert_eq!(played.played, "上次游玩 1 小时前");
    }

    #[test]
    fn home_is_first_use_with_no_instances() {
        let (home, id) = home_presentation(&[], &HomeSummary::default(), None, NOW);
        assert_eq!(home, HomePresentation::FirstUse);
        assert_eq!(id, None);
    }

    #[test]
    fn home_offers_the_newest_instance_until_something_was_played() {
        let records = [record("a", false, None, 1), record("b", false, None, 5)];
        let (home, id) = home_presentation(&records, &HomeSummary::default(), None, NOW);
        assert_eq!(id.as_deref(), Some("b"));
        let HomePresentation::Continue { subject, recent } = home else {
            panic!("expected Continue");
        };
        assert_eq!(subject.title, "B");
        assert!(subject.metadata.contains("还没玩过"));
        assert!(recent.is_empty());
    }

    #[test]
    fn home_follows_the_summary() {
        let records = [
            record("a", false, Some(100), 1),
            record("b", false, Some(900_000), 2),
            record("c", false, Some(500_000), 3),
        ];
        let summary = HomeSummary {
            continue_with: Some("b".to_owned()),
            recent: vec!["c".to_owned(), "a".to_owned(), "ghost".to_owned()],
            needs_attention: Vec::<AttentionItem>::new(),
        };
        let (home, id) = home_presentation(&records, &summary, None, NOW);
        assert_eq!(id.as_deref(), Some("b"));
        let HomePresentation::Continue { subject, recent } = home else {
            panic!("expected Continue");
        };
        assert!(subject.metadata.contains("上次游玩于"));
        // A stale id in the summary is skipped, not shown.
        let titles: Vec<_> = recent.iter().map(|entry| entry.title.as_str()).collect();
        assert_eq!(titles, ["C", "A"]);
    }

    #[test]
    fn home_continues_with_the_current_instance_and_does_not_list_it_twice() {
        let records = [
            record("a", false, Some(100), 1),
            record("b", false, Some(900_000), 2),
            record("c", false, Some(500_000), 3),
        ];
        let summary = HomeSummary {
            continue_with: Some("b".to_owned()),
            recent: vec!["c".to_owned(), "a".to_owned()],
            needs_attention: Vec::<AttentionItem>::new(),
        };
        let (home, id) = home_presentation(&records, &summary, Some("c"), NOW);
        assert_eq!(id.as_deref(), Some("c"));
        let HomePresentation::Continue { subject, recent } = home else {
            panic!("expected Continue");
        };
        assert_eq!(subject.title, "C");
        let titles: Vec<_> = recent.iter().map(|entry| entry.title.as_str()).collect();
        assert_eq!(titles, ["A"]);
        // A current instance that no longer exists is ignored.
        let (_, id) = home_presentation(&records, &summary, Some("ghost"), NOW);
        assert_eq!(id.as_deref(), Some("b"));
    }

    #[test]
    fn account_rows_show_the_selection_and_custom_ids() {
        use lumilio_core::{AccountEntry, LauncherSettings, ProfileId};
        let mut settings = LauncherSettings::default();
        settings.accounts = vec![
            AccountEntry {
                name: "Steve".into(),
                ..AccountEntry::default()
            },
            AccountEntry {
                name: "Alex".into(),
                uuid: Some("123e4567e89b12d3a456426614174000".into()),
                ..AccountEntry::default()
            },
            AccountEntry {
                name: "broken name".into(),
                uuid: Some("not an id".into()),
                ..AccountEntry::default()
            },
            AccountEntry {
                name: "Edwin_Zhan".into(),
                uuid: Some("00000000000000000000000000000abc".into()),
                kind: lumilio_core::AccountKind::Microsoft,
                needs_sign_in: true,
            },
        ];
        settings.selected_account = Some("Alex".into());
        let rows = account_rows(&settings);
        // An unreadable entry is left out rather than shown wrong.
        assert_eq!(rows.len(), 3);
        let microsoft = &rows[2];
        assert!(microsoft.microsoft && microsoft.needs_sign_in && !microsoft.custom_id);
        assert_eq!(microsoft.key, "msa:00000000000000000000000000000abc");
        assert_eq!(microsoft.kind_label(), "Microsoft");
        assert_eq!(rows[0].kind_label(), "离线账户");
        assert_eq!(rows[0].key, "Steve");
        assert!(!rows[0].selected && !rows[0].custom_id);
        assert_eq!(rows[0].uuid, ProfileId::offline("Steve").to_string());
        assert!(rows[1].selected && rows[1].custom_id);
        assert_eq!(rows[1].uuid, "123e4567-e89b-12d3-a456-426614174000");
    }

    #[test]
    fn the_settings_view_reads_the_saved_values_and_describes_each_java() {
        let mut settings = LauncherSettings::default();
        settings.default_max_memory_mb = Some(4096);
        settings.download_concurrency = Some(6);
        settings.prefer_mirrors = true;
        settings.extra_java_roots = vec!["/opt/jdks".into()];
        let view = settings_view(&settings, &[], None, Path::new("/data"), Some(16_384));
        assert_eq!(view.max_memory_mb, Some(4096));
        assert_eq!(view.min_memory_mb, None);
        assert_eq!(view.download_concurrency, Some(6));
        assert!(view.prefer_mirrors);
        assert_eq!(view.java_roots, [PathBuf::from("/opt/jdks")]);
        assert_eq!(view.data_dir, PathBuf::from("/data"));
        assert!(view.java.is_empty() && view.storage.is_none());
        assert_eq!(view.total_memory_mb, Some(16_384));
    }

    #[test]
    fn account_failures_say_what_to_change() {
        use lumilio_core::{ServiceError, SettingsError};
        let (message, technical) = account_failure(&ServiceError::Settings(
            SettingsError::DuplicateAccount("Steve".into()),
        ));
        assert!(message.contains("Steve") && message.contains("已经有"));
        assert!(technical.contains("already exists"));
        let (message, _) = account_failure(&ServiceError::Settings(SettingsError::DuplicateUuid(
            "x".into(),
        )));
        assert!(message.contains("UUID"));
        let (message, _) = account_failure(&ServiceError::Cancelled);
        assert!(message.contains("没能保存"));
    }

    #[test]
    fn download_counts_are_short() {
        assert_eq!(count_label(999), "999");
        assert_eq!(count_label(12_345), "1.2 万");
        assert_eq!(count_label(250_000_000), "2.5 亿");
    }

    #[test]
    fn activity_shows_running_first_and_maps_outcomes() {
        let view = ActivityView {
            active: vec![ActiveTask {
                id: 1,
                category: TaskCategory::Download,
                label: "安装 sodium".to_owned(),
                instance_id: Some("a".to_owned()),
                started: 0,
                progress: Some((50, 200)),
                unit: lumilio_core::ProgressUnit::Items,
                retry: None,
            }],
            cancellable: [1].into(),
            finished: vec![
                FinishedTask {
                    category: TaskCategory::Install,
                    label: "ok".to_owned(),
                    instance_id: None,
                    started: 0,
                    finished: NOW - 120,
                    outcome: TaskOutcome::Succeeded,
                    retry: None,
                },
                FinishedTask {
                    category: TaskCategory::Download,
                    label: "bad".to_owned(),
                    instance_id: None,
                    started: 0,
                    finished: NOW,
                    outcome: TaskOutcome::Failed("no route".to_owned()),
                    retry: Some(lumilio_core::RetryAction::InstallModpack {
                        project: "pack".into(),
                    }),
                },
            ],
        };
        let rows = activity_rows(&view, NOW);
        assert_eq!(rows[0].state, ActivityState::Running);
        assert_eq!(rows[0].fraction, Some(0.25));
        assert_eq!(rows[0].cancel, Some(1));
        assert!(rows[1..].iter().all(|row| row.cancel.is_none()));
        assert_eq!(rows[1].state, ActivityState::Done);
        assert_eq!(rows[1].detail, "2 分钟前");
        assert_eq!(rows[2].state, ActivityState::Failed("no route".to_owned()));
        let model = LiveModel {
            activity: rows,
            ..LiveModel::default()
        };
        assert_eq!(model.active_tasks(), 1);
    }

    #[test]
    fn home_attention_names_the_game_its_worst_problem_and_the_first_remedy() {
        use lumilio_core::{Problem, ProblemKind, Severity};
        let item = |id: &str, total| AttentionItem {
            instance_id: id.to_owned(),
            headline: Problem {
                severity: Severity::Error,
                kind: ProblemKind::DamagedFiles { count: 2 },
            },
            total,
        };
        let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
        let rows = attention_rows(&[item("a", 3), item("gone", 1)], &[card("a")]);
        assert_eq!(rows.len(), 1, "a deleted game is skipped");
        assert_eq!(rows[0].name, "A");
        assert_eq!(rows[0].more, 2);
        assert!(rows[0].title.contains("损坏"));
        assert_eq!(
            rows[0].action.map(|(action, _)| action),
            Some(crate::instance_detail::ProblemAction::Repair)
        );
    }

    #[test]
    fn a_finished_row_keeps_what_it_needs_to_be_retried_or_opened() {
        let view = ActivityView {
            active: Vec::new(),
            cancellable: Default::default(),
            finished: vec![FinishedTask {
                category: TaskCategory::Install,
                label: "bad".to_owned(),
                instance_id: Some("a".to_owned()),
                started: 0,
                finished: NOW,
                outcome: TaskOutcome::Failed("no route".to_owned()),
                retry: Some(lumilio_core::RetryAction::RepairInstance {
                    instance: "a".into(),
                }),
            }],
        };
        let rows = activity_rows(&view, NOW);
        assert_eq!(rows[0].instance.as_deref(), Some("a"));
        assert_eq!(
            rows[0].retry,
            Some(lumilio_core::RetryAction::RepairInstance {
                instance: "a".into()
            })
        );
    }

    #[test]
    fn speed_is_the_smoothed_change_between_readings_and_ignores_noise() {
        let first = next_sample(None, 100, 1_000);
        assert_eq!(first.per_sec, None, "one reading is no speed");
        // 100 more bytes in half a second.
        let second = next_sample(Some(first), 200, 1_500);
        assert_eq!(second.per_sec, Some(200.));
        // A reading too soon after, or one that went backwards, changes nothing.
        assert_eq!(next_sample(Some(second), 900, 1_550), second);
        assert_eq!(next_sample(Some(second), 150, 3_000), second);
        // Smoothing: a burst moves the number only part of the way.
        let third = next_sample(Some(second), 1_200, 2_500);
        let rate = third.per_sec.unwrap();
        assert!(rate > 200. && rate < 1_000., "{rate}");
    }

    #[test]
    fn speed_and_time_left_read_naturally() {
        use lumilio_core::ProgressUnit::{Bytes, Items};
        assert_eq!(rate_text(Bytes, 3.2 * 1024. * 1024.), "3.2 MB/秒");
        assert_eq!(rate_text(Bytes, 900. * 1024.), "900 KB/秒");
        assert_eq!(rate_text(Items, 42.4), "42 个文件/秒");
        assert_eq!(rate_text(Items, 2.46), "2.5 个文件/秒");
        assert_eq!(eta_text(100, 10.).as_deref(), Some("不到 1 分钟"));
        assert_eq!(eta_text(900, 10.).as_deref(), Some("约 2 分钟"));
        assert_eq!(eta_text(36_000, 5.).as_deref(), Some("约 2 小时 0 分钟"));
        assert_eq!(eta_text(10, 0.), None);
    }

    #[test]
    fn set_activity_gives_running_rows_a_speed_names_the_game_and_forgets_finished_tasks() {
        let running = |done| ActivityRow {
            task: Some(7),
            amount: Some((done, 1_000)),
            unit: lumilio_core::ProgressUnit::Bytes,
            rate: None,
            instance: Some("a".into()),
            retry: None,
            category: TaskCategory::Download,
            title: "t".into(),
            detail: "a".into(),
            fraction: None,
            state: ActivityState::Running,
            cancel: None,
        };
        let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
        let mut model = LiveModel::default();
        model.set_library(vec![card("a")], None);
        model.set_activity(vec![running(100)], 1_000);
        assert_eq!(model.activity[0].rate, None);
        assert_eq!(model.activity[0].detail, "A", "the game's name, not its id");
        model.set_activity(vec![running(300)], 2_000);
        assert_eq!(model.activity[0].rate, Some(200.));
        model.set_activity(Vec::new(), 3_000);
        model.set_activity(vec![running(900)], 4_000);
        assert_eq!(
            model.activity[0].rate, None,
            "a task that ended starts over"
        );
    }

    #[test]
    fn an_activity_tab_shows_only_its_category_and_all_shows_everything() {
        let row = |category, title: &str| ActivityRow {
            task: None,
            amount: None,
            unit: lumilio_core::ProgressUnit::Items,
            rate: None,
            instance: None,
            retry: None,
            category,
            title: title.to_owned(),
            detail: String::new(),
            fraction: None,
            state: ActivityState::Done,
            cancel: None,
        };
        let rows = vec![
            row(TaskCategory::Download, "d"),
            row(TaskCategory::Install, "i"),
            row(TaskCategory::Repair, "r"),
            row(TaskCategory::Install, "i2"),
        ];
        assert_eq!(activity_in_tab(&rows, 0).len(), 4);
        let installs: Vec<_> = activity_in_tab(&rows, 2)
            .iter()
            .map(|row| row.title.as_str())
            .collect();
        assert_eq!(installs, ["i", "i2"]);
        assert!(activity_in_tab(&rows, 3).is_empty());
        assert_eq!(activity_in_tab(&rows, 99).len(), 4, "unknown tab = all");
    }

    #[test]
    fn a_collection_lists_its_games_that_still_exist_and_marks_membership() {
        let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
        let mut model = LiveModel::default();
        model.set_library(vec![card("a"), card("b"), card("c")], None);
        model.collections = vec![
            CollectionRow {
                name: "生存".into(),
                members: vec!["c".into(), "gone".into(), "a".into()],
            },
            CollectionRow {
                name: "空".into(),
                members: Vec::new(),
            },
        ];
        let ids: Vec<_> = model
            .collection_cards(&model.collections[0])
            .iter()
            .map(|card| card.id.as_str())
            .collect();
        assert_eq!(
            ids,
            ["c", "a"],
            "kept in the order added; deleted games are skipped"
        );
        assert_eq!(
            model.memberships_of("a"),
            [("生存".to_owned(), true), ("空".to_owned(), false)]
        );
        assert_eq!(
            model.memberships_of("b"),
            [("生存".to_owned(), false), ("空".to_owned(), false)]
        );
    }

    #[test]
    fn unknown_progress_is_not_a_fraction() {
        let task = ActiveTask {
            id: 1,
            category: TaskCategory::Install,
            label: String::new(),
            instance_id: None,
            started: 0,
            progress: Some((0, 0)),
            unit: lumilio_core::ProgressUnit::Items,
            retry: None,
        };
        assert_eq!(active_row(&task, false).fraction, None);
    }

    #[test]
    fn the_install_target_survives_refreshes_and_falls_back_when_deleted() {
        let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
        let mut model = LiveModel::default();
        model.set_library(vec![card("a"), card("b")], Some("b".to_owned()));
        assert_eq!(model.install_target.as_deref(), Some("b"));
        // The person's own choice wins over later hints.
        model.install_target = Some("a".to_owned());
        model.set_library(vec![card("a"), card("b")], Some("b".to_owned()));
        assert_eq!(model.install_target.as_deref(), Some("a"));
        // The chosen instance was deleted: fall back rather than dangle.
        model.set_library(vec![card("b")], None);
        assert_eq!(model.install_target.as_deref(), Some("b"));
        model.set_library(Vec::new(), None);
        assert_eq!(model.install_target, None);
    }

    fn query() -> DiscoverQuery {
        DiscoverQuery::new(ProjectKind::Mod)
    }

    #[test]
    fn any_change_but_paging_returns_to_the_first_page() {
        let mut q = query();
        q.page = 7;
        assert_eq!(q.clone().apply(DiscoverChange::Page(8)).page, 8);
        for change in [
            DiscoverChange::Sort(SortIndex::Downloads),
            DiscoverChange::PageSize(50),
            DiscoverChange::Version(Some("1.21".to_owned())),
            DiscoverChange::ToggleCategory("magic".to_owned()),
            DiscoverChange::ToggleLoader("fabric".to_owned()),
            DiscoverChange::ClearFilters,
            DiscoverChange::Kind(ProjectKind::Shader),
        ] {
            assert_eq!(q.clone().apply(change.clone()).page, 0, "{change:?}");
        }
    }

    #[test]
    fn toggles_add_then_remove_and_a_new_kind_forgets_categories_and_loaders() {
        let q = query()
            .apply(DiscoverChange::ToggleCategory("magic".to_owned()))
            .apply(DiscoverChange::ToggleCategory("technology".to_owned()))
            .apply(DiscoverChange::ToggleLoader("fabric".to_owned()));
        assert_eq!(q.categories, ["magic", "technology"]);
        let q = q.apply(DiscoverChange::ToggleCategory("magic".to_owned()));
        assert_eq!(q.categories, ["technology"]);
        assert!(q.filtered());

        let same = q.clone().apply(DiscoverChange::Kind(ProjectKind::Mod));
        assert_eq!(same.categories, ["technology"], "the same kind keeps them");
        let q = q
            .apply(DiscoverChange::Version(Some("1.21.1".to_owned())))
            .apply(DiscoverChange::Kind(ProjectKind::ResourcePack));
        assert!(q.categories.is_empty() && q.loaders.is_empty());
        assert_eq!(
            q.game_version.as_deref(),
            Some("1.21.1"),
            "versions carry over"
        );
    }

    #[test]
    fn clearing_filters_keeps_text_and_sort() {
        let mut q = query().apply(DiscoverChange::Sort(SortIndex::Follows));
        q.text = "sodium".to_owned();
        let q = q
            .apply(DiscoverChange::ToggleCategory("x".to_owned()))
            .apply(DiscoverChange::Version(Some("1.0".to_owned())))
            .apply(DiscoverChange::ClearFilters);
        assert!(!q.filtered());
        assert_eq!((q.text.as_str(), q.sort), ("sodium", SortIndex::Follows));
    }

    #[test]
    fn the_core_query_carries_everything_and_bounds_the_page_size() {
        let mut q = query()
            .apply(DiscoverChange::Version(Some("1.21.1".to_owned())))
            .apply(DiscoverChange::ToggleLoader("quilt".to_owned()))
            .apply(DiscoverChange::ToggleCategory("magic".to_owned()))
            .apply(DiscoverChange::Sort(SortIndex::Updated))
            .apply(DiscoverChange::PageSize(5000));
        q.text = "tech".to_owned();
        let q = q.apply(DiscoverChange::Page(3));
        let search = q.to_search();
        assert_eq!(search.page_size, 100);
        assert_eq!(search.page, 3);
        assert_eq!(search.loaders, ["quilt"]);
        assert_eq!(search.categories, ["magic"]);
        assert_eq!(search.game_version.as_deref(), Some("1.21.1"));
        assert_eq!(
            (search.text.as_str(), search.sort),
            ("tech", SortIndex::Updated)
        );
    }

    fn shown(current: u32, pages: u32) -> String {
        page_items(current, pages)
            .iter()
            .map(|item| match item {
                PageItem::Page(page) => (page + 1).to_string(),
                PageItem::Gap => "…".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn the_pager_shows_ends_and_neighbours() {
        assert_eq!(shown(0, 928), "1 2 … 928");
        assert_eq!(shown(5, 928), "1 … 5 6 7 … 928");
        assert_eq!(shown(927, 928), "1 … 927 928");
        assert_eq!(shown(1, 928), "1 2 3 … 928");
        assert_eq!(
            shown(2, 928),
            "1 2 3 4 … 928",
            "a single hidden page is shown, not elided"
        );
        assert_eq!(shown(0, 1), "1");
        assert_eq!(shown(0, 3), "1 2 3");
        assert_eq!(shown(9, 4), "1 2 3 4", "an out-of-range page is clamped");
        assert_eq!(shown(0, 0), "");
    }

    #[test]
    fn pages_follow_the_total_and_the_page_size() {
        let mut model = LiveModel::default();
        assert_eq!(model.pages(), 0);
        model.search = SearchStatus::Done { total: 41 };
        model.query.page_size = 20;
        assert_eq!(model.pages(), 3);
        model.query.page_size = 50;
        assert_eq!(model.pages(), 1);
        model.search = SearchStatus::Done { total: 0 };
        assert_eq!(model.pages(), 0);
    }

    #[test]
    fn timestamps_parse_with_fractions_and_offsets() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(
            parse_rfc3339("2024-02-29T12:30:15.123456Z"),
            Some(1_709_209_815)
        );
        assert_eq!(
            parse_rfc3339("2024-02-29T12:30:15+00:00"),
            Some(1_709_209_815)
        );
        assert_eq!(parse_rfc3339("yesterday"), None);
        assert_eq!(parse_rfc3339("2024-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339(""), None);
    }

    #[test]
    fn rows_show_counts_environment_and_a_relative_update() {
        let hit = SearchHit {
            project_id: "P".to_owned(),
            slug: "fo".to_owned(),
            title: "FO".to_owned(),
            description: "fast".to_owned(),
            author: "me".to_owned(),
            kind: ProjectKind::Modpack,
            categories: vec!["lightweight".to_owned()],
            loaders: vec!["fabric".to_owned()],
            downloads: 17_731_400,
            follows: 4886,
            updated: "2026-09-27T10:00:00Z".to_owned(),
            client_side: lumilio_core::SideSupport::Required,
            server_side: lumilio_core::SideSupport::Optional,
            icon_url: Some("https://cdn/x.png".to_owned()),
        };
        let now = parse_rfc3339("2026-09-30T10:00:00Z").unwrap();
        let row = search_row(&hit, now);
        assert_eq!(row.downloads, "1773.1 万");
        assert_eq!(row.follows, "4886");
        assert_eq!(row.updated, "3 天前");
        assert_eq!(row.environment, Some(Environment::ClientAndServer));
        assert_eq!(row.icon_url.as_deref(), Some("https://cdn/x.png"));
        let unknown = SearchHit {
            updated: String::new(),
            ..hit
        };
        assert_eq!(search_row(&unknown, now).updated, "");
    }

    #[test]
    fn tag_labels_are_chinese_when_known_and_readable_when_not() {
        assert_eq!(tag_label("optimization"), "优化");
        assert_eq!(tag_label("fabric"), "Fabric");
        assert_eq!(tag_label("some-new-thing"), "Some new thing");
        assert_eq!(tag_label(""), "");
    }

    #[test]
    fn filters_list_only_releases_and_the_kinds_own_categories() {
        use lumilio_core::{CategoryTag, GameVersionTag};
        let core = DiscoverFilters {
            categories: vec![
                CategoryTag {
                    name: "magic".to_owned(),
                    header: "categories".to_owned(),
                    kind: ProjectKind::Mod,
                },
                CategoryTag {
                    name: "bloom".to_owned(),
                    header: "features".to_owned(),
                    kind: ProjectKind::Shader,
                },
            ],
            game_versions: vec![
                GameVersionTag {
                    version: "26.4-pre1".to_owned(),
                    release: false,
                    published: String::new(),
                },
                GameVersionTag {
                    version: "26.3".to_owned(),
                    release: true,
                    published: String::new(),
                },
            ],
        };
        let model = FilterModel::from_core(&core);
        assert_eq!(model.versions, ["26.3"]);
        assert_eq!(model.categories(ProjectKind::Mod), ["magic"]);
        assert_eq!(model.categories(ProjectKind::Shader), ["bloom"]);
        assert!(model.categories(ProjectKind::Modpack).is_empty());
    }
}
