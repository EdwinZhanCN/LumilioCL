//! Real Instance Overview and Settings.Performance, bound to a stable ID.

mod actions;
mod content;
mod data;
mod diagnostics;
mod editors;
mod forms;
mod intent;
mod overview;
mod panels;
mod performance;
mod render;
mod settings;

#[cfg(test)]
mod tests;

pub use self::intent::{InstanceIntent, Section};
pub use self::panels::{Arrived, ProblemAction, export_notice, problem_action, problem_text};

use self::editors::Editor;
use self::forms::Fields;
use self::panels::{Confirm, Data};
use crate::toast::Toast;
use gpui::{App, Entity, Window};
use lumilio_core::{CrashHint, InstanceRecord, LauncherSettings};
use std::rc::Rc;

/// A crash report's text and the causes recognised in it, or why it could not be read.
type CrashRead = Result<(String, Vec<CrashHint>), String>;

pub const TABS: [&str; 7] = ["概览", "内容", "世界", "截图", "历史", "诊断", "设置"];
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
    id: String,
    record: Option<InstanceRecord>,
    defaults: LauncherSettings,
    fields: Option<Fields>,
    tab: usize,
    busy: bool,
    error: Option<String>,
    /// Why the instance could not be read, behind 技术详情.
    load_technical: Option<String>,
    /// Messages waiting for the next render to float up (§11).
    toasts: Vec<Toast>,
    handler: InstanceHandler,
    sync_fields: u8,
    data: Data,
    content_kind: usize,
    history_sub: usize,
    /// 诊断: 0 问题 · 1 日志 · 2 文件.
    diag_sub: usize,
    /// 创建快照: 0 is everything, then each world in the list.
    snapshot_scope: usize,
    /// 世界: index into the sort labels.
    world_sort: usize,
    /// 世界 tab: 0 worlds, 1 servers.
    worlds_sub: usize,
    /// What the last check found out about each server, by address.
    server_status: std::collections::HashMap<String, panels::ServerState>,
    /// 截图: the thumbnails asked for or made, by file name, and how many
    /// cards are shown (more are added on request).
    thumbs: std::collections::HashMap<String, panels::Thumb>,
    shots_shown: usize,
    /// Thumbnails for the cards shown should be asked for on the next render.
    ask_thumbs: bool,
    /// The game stopped: read the screenshots again on the next render.
    refresh_screenshots: bool,
    /// The screenshot open in the viewer, by file name.
    shot_open: Option<String>,
    /// The servers should be asked how they are on the next render.
    ping_servers: bool,
    /// The server the edit dialog is changing (its position and what it read
    /// when opened); `None` while it adds one.
    server_edit: Option<(usize, lumilio_core::ServerEntry)>,
    /// 日志: the least serious level shown, as an index of the level labels.
    log_level: usize,
    settings_sub: usize,
    /// The machine's memory in MB, for the memory hint; unknown until told.
    machine_memory_mb: Option<u64>,
    confirm: Option<Confirm>,
    /// The edit dialog that is open, what went wrong in it, and whether a
    /// finished save should close it on the next render.
    editor: Option<Editor>,
    editor_error: Option<String>,
    close_editor: bool,
    include_worlds: bool,
    /// Content tab: the filter, the selected file names, and the open
    /// switch-version dialog.
    content_filter: content::ContentFilter,
    selected: content::Selection,
    switch: Option<Entity<content::VersionSwitch>>,
    started: bool,
    /// A question the Library's menu asked of the page before it had the game.
    ask_later: Option<InstanceIntent>,
    /// A failed session whose crash report is wanted once the reports are known.
    inspect: Option<(u64, u64)>,
    /// When the game that is running started, in ms since the Unix epoch.
    game_started_ms: Option<i64>,
    /// The worlds should be read again (a game is running and one may have opened).
    refresh_worlds: bool,
    last_worlds_refresh: Option<std::time::Instant>,
    /// The game's output while it runs in this launcher, newest last.
    live_output: Option<Vec<String>>,
    /// The game just ended: read the log file again on the next render.
    refresh_logs: bool,
    log_scroll: gpui::ScrollHandle,
    /// The crash report being read or shown, and what was read.
    crash: Option<(String, Option<CrashRead>)>,
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
            toasts: Vec::new(),
            handler,
            sync_fields: 0,
            data: Data::default(),
            content_kind: 0,
            history_sub: 0,
            diag_sub: 0,
            world_sort: 0,
            worlds_sub: 0,
            server_status: std::collections::HashMap::new(),
            ping_servers: false,
            thumbs: std::collections::HashMap::new(),
            shots_shown: panels::SHOTS_PAGE,
            ask_thumbs: false,
            shot_open: None,
            refresh_screenshots: false,
            server_edit: None,
            snapshot_scope: 0,
            log_level: 0,
            settings_sub: 0,
            machine_memory_mb: None,
            confirm: None,
            editor: None,
            editor_error: None,
            close_editor: false,
            include_worlds: false,
            content_filter: content::ContentFilter::All,
            selected: content::Selection::new(),
            switch: None,
            started: false,
            inspect: None,
            ask_later: None,
            game_started_ms: None,
            refresh_worlds: false,
            last_worlds_refresh: None,
            live_output: None,
            refresh_logs: false,
            log_scroll: gpui::ScrollHandle::new(),
            crash: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn title(&self) -> &str {
        self.record
            .as_ref()
            .map_or("游戏详情", |record| record.name.as_str())
    }
}
