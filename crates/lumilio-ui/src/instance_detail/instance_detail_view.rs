use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, Entity, RenderImage, Window};
use lumilio_core::{InstanceRecord, LauncherSettings, PluginFinding};

use super::editors::Editor;
use super::forms::Fields;
use super::intent::{InstanceIntent, Section};
use super::panels::{Confirm, Data};
use super::{content, panels, plugin_tabs, settings};
use crate::{tr, tr_all};

/// A crash report's text and the causes recognised in it, or why it could not be read.
pub(super) type CrashRead = Result<(String, Vec<PluginFinding>), String>;

/// State of a single-value dropdown (design language §10: Select / dropdown).
pub type Dropdown =
    Entity<gpui_component::select::SelectState<gpui_component::select::SearchableVec<String>>>;

/// The tab labels in order. Indexing or slicing goes through `Deref`, which
/// takes the words again each time, so a language switch reaches the tab bar.
pub static TABS: Tabs = Tabs;

pub struct Tabs;

impl std::ops::Deref for Tabs {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "instance-tab-overview",
            "instance-tab-content",
            "instance-tab-worlds",
            "instance-tab-screenshots",
            "instance-tab-history",
            "instance-tab-diagnostics",
            "route-settings",
        ]
    }
}
pub const TAB_OVERVIEW: usize = 0;
pub const TAB_CONTENT: usize = 1;
pub const TAB_WORLDS: usize = 2;
pub const TAB_SCREENSHOTS: usize = 3;
pub const TAB_HISTORY: usize = 4;
pub const TAB_DIAGNOSTICS: usize = 5;
pub const TAB_SETTINGS: usize = 6;

/// What a finished write reports back to the view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operated {
    pub notice: String,
    /// Raw cause for the technical details; present only on failure.
    pub technical: Option<String>,
    /// Data the write made stale.
    pub refresh: Vec<Section>,
}

pub type InstanceHandler = Rc<dyn Fn(InstanceIntent, &mut Window, &mut App)>;

pub struct InstanceDetailView {
    pub(super) id: String,
    pub(super) record: Option<InstanceRecord>,
    pub(super) defaults: LauncherSettings,
    pub(super) fields: Option<Fields>,
    pub(super) tab: usize,
    pub(super) busy: bool,
    pub(super) error: Option<String>,
    /// Why the instance could not be read, behind 技术详情.
    pub(super) load_technical: Option<String>,
    pub(super) handler: InstanceHandler,
    pub(super) sync_fields: u8,
    pub(super) data: Data,
    pub(super) content_kind: usize,
    pub(super) history_sub: usize,
    /// 诊断: 0 问题 · 1 日志 · 2 文件.
    pub(super) diag_sub: usize,
    /// 创建快照: 0 is everything, then each world in the list.
    pub(super) snapshot_scope: usize,
    /// 世界: index into the sort labels.
    pub(super) world_sort: usize,
    /// The world sort's dropdown state, created once the view has a window.
    pub(super) world_sort_select: Option<Dropdown>,
    /// 世界 tab: 0 worlds, 1 servers.
    pub(super) worlds_sub: usize,
    /// What the last check found out about each server, by address.
    pub(super) server_status: std::collections::HashMap<String, panels::ServerState>,
    /// Each server's icon, by address: the game's cached one, or the live
    /// `favicon` once the server has answered.
    pub(super) server_icons: std::collections::HashMap<String, Arc<RenderImage>>,
    /// 截图: the thumbnails asked for or made, by file name, and how many
    /// cards are shown (more are added on request).
    pub(super) thumbs: std::collections::HashMap<String, panels::Thumb>,
    pub(super) shots_shown: usize,
    /// Thumbnails for the cards shown should be asked for on the next render.
    pub(super) ask_thumbs: bool,
    /// The game stopped: read the screenshots again on the next render.
    pub(super) refresh_screenshots: bool,
    /// The screenshot open in the viewer, by file name.
    pub(super) shot_open: Option<String>,
    /// The servers should be asked how they are on the next render.
    pub(super) ping_servers: bool,
    /// The server the edit dialog is changing (its position and what it read
    /// when opened); `None` while it adds one.
    pub(super) server_edit: Option<(usize, lumilio_core::ServerEntry)>,
    /// Independently selected log levels; report text bypasses this filter.
    pub(super) log_levels: Vec<lumilio_core::LogLevel>,
    pub(super) log_source: lumilio_core::GameLogSource,
    pub(super) log_source_select: Option<Dropdown>,
    pub(super) log_level_select: Option<Dropdown>,
    pub(super) log_choices: Vec<(lumilio_core::GameLogSource, String)>,
    pub(super) log_list_scroll: gpui::UniformListScrollHandle,
    pub(super) log_follow: bool,
    pub(super) analysis_serial: u64,
    pub(super) log_analysis: Option<(u64, String, Option<CrashRead>)>,
    pub(super) settings_sub: usize,
    /// 选项: the changes made in the game that are selected for a decision,
    /// and those whose preset value also changed (shown after a refusal).
    pub(super) options_selected: std::collections::BTreeSet<String>,
    pub(super) options_conflicts: Vec<String>,
    /// The game stopped: read its options again on the next render.
    pub(super) refresh_options: bool,
    /// The Java dropdown, created once the view has a window, and the
    /// entries it shows.
    pub(super) java_select: Option<Dropdown>,
    pub(super) java_picks: Vec<(settings::JavaPick, String)>,
    /// The record changed: read what the next launch's Java is again.
    pub(super) refresh_java: bool,
    /// The machine's memory in MB, for the memory hint; unknown until told.
    pub(super) machine_memory_mb: Option<u64>,
    pub(super) confirm: Option<Confirm>,
    /// The edit dialog that is open, what went wrong in it, and whether a
    /// finished save should close it on the next render.
    pub(super) editor: Option<Editor>,
    pub(super) editor_error: Option<String>,
    pub(super) close_editor: bool,
    pub(super) include_worlds: bool,
    /// Content tab: the filter, the selected file names, and the open
    /// switch-version dialog.
    pub(super) content_filter: content::ContentFilter,
    /// The filter's dropdown state, created once the view has a window.
    pub(super) content_filter_select: Option<content::FilterSelect>,
    pub(super) selected: content::Selection,
    pub(super) switch: Option<Entity<content::VersionSwitch>>,
    /// Files the game waits for that the launcher could not download.
    pub(super) pending: Vec<lumilio_core::PendingDownload>,
    pub(super) started: bool,
    /// A question the Library's menu asked of the page before it had the game.
    pub(super) ask_later: Option<InstanceIntent>,
    /// A failed session whose crash report is wanted once the reports are known.
    pub(super) inspect: Option<(u64, u64)>,
    /// When the game that is running started, in ms since the Unix epoch.
    pub(super) game_started_ms: Option<i64>,
    /// The worlds should be read again (a game is running and one may have opened).
    pub(super) refresh_worlds: bool,
    pub(super) last_worlds_refresh: Option<std::time::Instant>,
    /// The game's output while it runs in this launcher, newest last.
    pub(super) live_output: Option<Vec<String>>,
    /// The game just ended: read the log file again on the next render.
    pub(super) refresh_logs: bool,
    pub(super) enabled_plugins: Option<Vec<String>>,
    pub(super) refresh_analysis: bool,
    /// Tabs plugins contribute for this game, the one open (if a plugin tab
    /// is showing instead of a built-in one), and what each last showed.
    pub(super) plugin_tabs: Vec<lumilio_core::PluginTab>,
    pub(super) plugin_open: Option<String>,
    pub(super) plugin_pages: plugin_tabs::PluginPages,
    /// A plugin's view that arrived before the tab list named the plugin;
    /// shown once the list does.
    pub(super) early_plugin_views:
        std::collections::HashMap<String, Result<Option<lumilio_plugin_api::View>, String>>,
    /// Which plugin tabs show should be asked again on the next render.
    pub(super) refresh_plugin_tabs: bool,
    pub(super) log_scroll: gpui::ScrollHandle,
    /// The 插件 tab's plugin list scroll (nested in the page's own scroll).
    pub(super) plugin_list_scroll: gpui::ScrollHandle,
    /// Plugin content and its tables share this single scroll region.
    pub(super) plugin_pane_scroll: gpui::ScrollHandle,
    /// The crash report being read or shown, and what was read.
    pub(super) crash: Option<(String, Option<CrashRead>)>,
}

impl InstanceDetailView {
    pub fn new(id: String, handler: InstanceHandler) -> Self {
        Self {
            id,
            record: None,
            defaults: LauncherSettings::default(),
            fields: None,
            tab: 0,
            busy: false,
            error: None,
            load_technical: None,
            handler,
            sync_fields: 0,
            data: Data::default(),
            content_kind: 0,
            history_sub: 0,
            diag_sub: 0,
            world_sort: 0,
            world_sort_select: None,
            worlds_sub: 0,
            server_status: std::collections::HashMap::new(),
            server_icons: std::collections::HashMap::new(),
            ping_servers: false,
            thumbs: std::collections::HashMap::new(),
            shots_shown: panels::SHOTS_PAGE,
            ask_thumbs: false,
            shot_open: None,
            refresh_screenshots: false,
            server_edit: None,
            snapshot_scope: 0,
            log_levels: vec![
                lumilio_core::LogLevel::Error,
                lumilio_core::LogLevel::Warn,
                lumilio_core::LogLevel::Info,
                lumilio_core::LogLevel::Debug,
            ],
            log_source: lumilio_core::GameLogSource::Live,
            log_source_select: None,
            log_level_select: None,
            log_choices: Vec::new(),
            log_list_scroll: gpui::UniformListScrollHandle::new(),
            log_follow: true,
            analysis_serial: 0,
            log_analysis: None,
            settings_sub: 0,
            options_selected: std::collections::BTreeSet::new(),
            options_conflicts: Vec::new(),
            refresh_options: false,
            java_select: None,
            java_picks: Vec::new(),
            refresh_java: false,
            machine_memory_mb: None,
            confirm: None,
            editor: None,
            editor_error: None,
            close_editor: false,
            include_worlds: false,
            content_filter: content::ContentFilter::All,
            content_filter_select: None,
            selected: content::Selection::new(),
            switch: None,
            pending: Vec::new(),
            started: false,
            inspect: None,
            ask_later: None,
            game_started_ms: None,
            refresh_worlds: false,
            last_worlds_refresh: None,
            live_output: None,
            refresh_logs: false,
            enabled_plugins: None,
            refresh_analysis: false,
            plugin_tabs: Vec::new(),
            plugin_open: None,
            plugin_pages: std::collections::HashMap::new(),
            early_plugin_views: std::collections::HashMap::new(),
            refresh_plugin_tabs: false,
            log_scroll: gpui::ScrollHandle::new(),
            plugin_list_scroll: gpui::ScrollHandle::new(),
            plugin_pane_scroll: gpui::ScrollHandle::new(),
            crash: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn title(&self) -> &str {
        self.record
            .as_ref()
            .map_or(tr!("instance-title"), |record| record.name.as_str())
    }
}
