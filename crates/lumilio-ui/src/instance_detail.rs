//! Real Instance Overview and Settings.Performance, bound to a stable ID.

use std::rc::Rc;

use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::input::InputState;
use gpui_component::{ActiveTheme as _, TITLE_BAR_HEIGHT, h_flex, v_flex};
use lumilio_core::{CrashHint, InstanceRecord, InstanceSettings, LauncherSettings, ProjectKind};

use crate::toast::Toast;
use crate::{
    kit, live,
    theme::{self, ShellColors},
};

#[path = "instance_content.rs"]
mod content;
#[path = "instance_diagnostics.rs"]
mod diagnostics;
#[path = "instance_editors.rs"]
mod editors;
#[path = "instance_panels.rs"]
mod panels;
#[path = "instance_settings.rs"]
mod settings;

use editors::Editor;

pub use panels::{Arrived, ProblemAction, export_notice, problem_action, problem_text};
use panels::{Confirm, Data};

/// A piece of data the sections load on demand.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Section {
    Content(ProjectKind),
    Worlds,
    Snapshots,
    History,
    Problems,
    Logs,
    Files,
    Size,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstanceIntent {
    Play,
    Rename(String),
    SaveMemory(InstanceSettings),
    /// Save every instance setting after one dialog changed its part.
    SaveSettings(InstanceSettings),
    Reload,
    /// Fetch this data and answer with [`InstanceDetailView::arrived`].
    Load(Section),
    SetContent {
        kind: ProjectKind,
        files: Vec<String>,
        enabled: bool,
    },
    DeleteContent {
        kind: ProjectKind,
        files: Vec<String>,
    },
    CopyWorld(String),
    /// Back up this world alone (a snapshot of just that world).
    BackupWorld(String),
    /// Ask where to save this world as a .zip, then export it.
    ExportWorld(String),
    /// The place chosen after [`InstanceIntent::ExportWorld`]; the
    /// application sends this to itself.
    ExportWorldTo {
        folder: String,
        path: std::path::PathBuf,
    },
    /// Ask for a world .zip and add it to the saves.
    ImportWorld,
    /// Open the export dialog; the application lists the game's files for it.
    ExportPack,
    /// The pack chosen in that dialog; the application asks where to save it.
    ExportPackAs(lumilio_core::ExportSpec),
    /// Write the pack here; the application sends this to itself.
    ExportPackTo {
        spec: lumilio_core::ExportSpec,
        path: std::path::PathBuf,
    },
    /// The archive chosen after [`InstanceIntent::ImportWorld`]; the
    /// application sends this to itself.
    AddWorld(std::path::PathBuf),
    /// Start the game and go straight into this world (by folder name).
    PlayWorld(String),
    DeleteWorld(String),
    /// Read this crash report; answer with [`InstanceDetailView::crash_arrived`].
    OpenCrash(String),
    /// List this folder of the game directory (`""` is the directory itself);
    /// answer with [`Arrived::Files`].
    OpenFolder(String),
    /// Open the Accounts page.
    OpenAccounts,
    /// Once the page has the game: open the copy dialog. (Sent by the
    /// application after it opens the page for the Library's menu.)
    AskCopy,
    /// Once the page has the game: ask whether to delete it.
    AskDelete,
    /// Once the page has the game: do what a problem's button says. (Sent by
    /// the application after Home's button opens the page.)
    Resolve(ProblemAction),
    /// Ask where to save a full backup of this game; the application then
    /// sends [`InstanceIntent::BackupGameTo`] to itself.
    BackupGame,
    BackupGameTo(std::path::PathBuf),
    /// Make this game the current one (what Continue starts).
    SetCurrent,
    /// Stop the game that is starting or running.
    Stop,
    /// Save the latest log (or this crash report) with names and paths hidden;
    /// the application asks where.
    ExportLog(Option<String>),
    /// Show this path of the game directory in the file manager.
    RevealPath(String),
    CreateSnapshot,
    /// A snapshot with a note, of everything or of one world (by folder).
    CreateSnapshotAs {
        note: String,
        world: Option<String>,
    },
    RestoreSnapshot(String),
    DeleteSnapshot(String),
    /// Download the game files without launching.
    Install,
    /// Download Java (the needed major, if known) into the launcher.
    InstallJava(Option<u32>),
    /// Check every game file and fetch what is missing or damaged again.
    Repair,
    /// Open the dialog that changes the game version and loader.
    OpenRuntimeChange,
    /// Change to this game version and loader (from that dialog).
    ChangeRuntime {
        loader: lumilio_core::Loader,
        game_version: String,
        loader_version: Option<String>,
    },
    /// Copy this instance under a new name.
    Copy {
        name: String,
        include_worlds: bool,
    },
    /// Delete this instance; the answer closes the view on success.
    Delete,
    /// Replace an installed file with another version of its project.
    SwitchContent {
        kind: ProjectKind,
        file_name: String,
        project: String,
        version_id: String,
    },
    /// Update several files: (file, project, version) each.
    UpdateContent {
        kind: ProjectKind,
        updates: Vec<(String, String, String)>,
    },
    /// Ask for local files and add them (P-ADD-FILES).
    ImportContent(ProjectKind),
    /// A project's versions; answer with [`InstanceDetailView::versions_arrived`].
    LoadVersions(String),
    /// Show an installed file in the file manager.
    RevealContent {
        kind: ProjectKind,
        file_name: String,
    },
    /// Open a project's page in Discover.
    OpenProject {
        kind: ProjectKind,
        slug: String,
    },
    /// Browse Discover for this kind, to add to this game.
    BrowseContent(ProjectKind),
    /// The files chosen after [`InstanceIntent::ImportContent`]; the
    /// application sends this to itself.
    AddFiles {
        kind: ProjectKind,
        files: Vec<std::path::PathBuf>,
    },
}

/// A crash report's text and the causes recognised in it, or why it could not be read.
type CrashRead = Result<(String, Vec<CrashHint>), String>;

pub const TABS: [&str; 6] = ["概览", "内容", "世界", "历史", "诊断", "设置"];
pub const TAB_OVERVIEW: usize = 0;
pub const TAB_CONTENT: usize = 1;
pub const TAB_WORLDS: usize = 2;
pub const TAB_HISTORY: usize = 3;
pub const TAB_DIAGNOSTICS: usize = 4;
pub const TAB_SETTINGS: usize = 5;

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

struct Fields {
    name: Entity<InputState>,
    min: Entity<InputState>,
    max: Entity<InputState>,
    copy_name: Entity<InputState>,
    content_search: Entity<InputState>,
    snapshot_note: Entity<InputState>,
    world_search: Entity<InputState>,
    log_search: Entity<InputState>,
    file_search: Entity<InputState>,
}

/// Parsing belongs to the form; range and effective-pair validation stays in core.
fn memory_draft(
    saved: &InstanceSettings,
    defaults: &LauncherSettings,
    min: &str,
    max: &str,
) -> Result<InstanceSettings, &'static str> {
    let parse = |value: &str| {
        let value = value.trim();
        if value.is_empty() {
            Ok(None)
        } else {
            value
                .parse::<u32>()
                .map(Some)
                .map_err(|_| "请输入整数 MB，留空可继承默认值")
        }
    };
    let mut next = saved.clone();
    next.min_memory_mb = parse(min)?;
    next.max_memory_mb = parse(max)?;
    next.validate_memory(
        defaults.default_min_memory_mb,
        defaults.default_max_memory_mb,
    )
    .map_err(|_| "内存需在 1–1048576 MB 内，最小值不能超过生效的最大值")?;
    Ok(next)
}

fn memory_text(value: Option<u32>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

/// The read-only value of a memory row: inherited values say so (§10).
fn effective_label(value: Option<u32>, default: Option<u32>) -> String {
    match (value, default) {
        (Some(value), _) => format!("{value} MB"),
        (None, Some(value)) => format!("跟随默认 · {value} MB"),
        (None, None) => "由 Java 决定".to_owned(),
    }
}

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

    /// A response for a different target never becomes this view's data.
    pub fn loaded(
        &mut self,
        result: Result<(InstanceRecord, LauncherSettings), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok((record, defaults)) if record.id == self.id => {
                self.record = Some(record);
                self.defaults = defaults;
                self.sync_fields = 3;
                self.error = None;
                self.load_technical = None;
            }
            Ok(_) => return,
            Err(detail) => {
                self.error = Some("没有读到这个游戏，可以重试".into());
                self.load_technical = Some(detail);
            }
        }
        self.busy = false;
        cx.notify();
    }

    pub fn saved(
        &mut self,
        memory: bool,
        result: Result<(InstanceRecord, LauncherSettings), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(data) => {
                self.loaded(Ok(data), cx);
                self.sync_fields = if memory { 2 } else { 1 };
                self.editor_result(None, cx);
                self.toast(
                    Toast::success(if memory {
                        "内存已保存，下次启动时生效"
                    } else {
                        "名称已保存"
                    }),
                    cx,
                );
            }
            Err(detail) => {
                self.busy = false;
                const FAILED: &str = "没有保存成功，输入已保留，可以重试";
                if !self.editor_result(Some(FAILED.into()), cx) {
                    self.toast(Toast::error(FAILED).technical(detail), cx);
                }
                cx.notify();
            }
        }
    }

    /// What was saved from a Settings dialog (which closes itself).
    pub fn settings_saved(
        &mut self,
        result: Result<(InstanceRecord, LauncherSettings), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(data) => {
                self.loaded(Ok(data), cx);
                self.toast(Toast::success("已保存，下次启动时生效"), cx);
            }
            Err(detail) => {
                self.busy = false;
                self.toast(Toast::error("没有保存成功，可以重试").technical(detail), cx);
                cx.notify();
            }
        }
    }

    /// What the game is on now: game version, loader, loader version.
    #[must_use]
    pub fn runtime(&self) -> Option<(String, lumilio_core::Loader, Option<String>)> {
        let record = self.record.as_ref()?;
        Some((
            record.game_version.clone(),
            record.loader,
            record.loader_version.clone(),
        ))
    }

    /// Shows one group of the Settings tab (dev review and tests).
    pub fn select_settings_group(&mut self, group: usize, cx: &mut Context<Self>) {
        self.settings_sub = group.min(settings::SUBTABS.len() - 1);
        cx.notify();
    }

    /// The machine's memory, for the hint on the memory rows.
    pub fn set_machine_memory(&mut self, total_mb: Option<u64>, cx: &mut Context<Self>) {
        self.machine_memory_mb = total_mb;
        cx.notify();
    }

    pub fn select_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.tab = tab.min(TABS.len() - 1);
        self.confirm = None;
        cx.notify();
    }

    /// Selecting from the page also loads what the new tab shows.
    fn open_tab(&mut self, tab: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.select_tab(tab, cx);
        match self.tab {
            TAB_OVERVIEW => {
                self.ensure(Section::Problems, window, cx);
                self.ensure(Section::Size, window, cx);
                self.ensure(Section::History, window, cx);
            }
            TAB_CONTENT => {
                let kind = panels::CONTENT_KINDS[self.content_kind.min(2)];
                self.ensure(Section::Content(kind), window, cx);
            }
            TAB_WORLDS => self.ensure(Section::Worlds, window, cx),
            TAB_DIAGNOSTICS => self.open_diagnostics(window, cx),
            TAB_HISTORY => {
                let section = if self.history_sub == 2 {
                    Section::Snapshots
                } else {
                    Section::History
                };
                self.ensure(section, window, cx);
            }
            _ => {}
        }
    }

    /// Loads a section once; a section already loaded or on its way is left alone.
    pub fn ensure(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if !self.data.has(section) {
            self.request(section, window, cx);
        }
    }

    /// Asks for fresh data, unless that is already under way.
    pub fn request(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if self.data.pending.contains(&section) {
            return;
        }
        self.data.pending.push(section);
        (self.handler)(InstanceIntent::Load(section), window, cx);
    }

    /// A section's data arrived. A failure is page state: the section shows
    /// it in place, with its own 技术详情 (§11).
    pub fn arrived(&mut self, arrived: Arrived, cx: &mut Context<Self>) {
        self.data.store(arrived);
        cx.notify();
    }

    /// Tells what just happened, as a toast over the page.
    pub fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toasts.push(toast);
        cx.notify();
    }

    /// Toasts not shown yet (a window without the framework `Root` keeps them).
    pub fn pending_toasts(&self) -> &[Toast] {
        &self.toasts
    }

    /// Asks the question `intent` stands for (copy or delete) as soon as the
    /// page has its game.
    pub fn ask_later(&mut self, intent: InstanceIntent, cx: &mut Context<Self>) {
        self.ask_later = Some(intent);
        cx.notify();
    }

    /// What the game has printed since it started, while it runs. When it
    /// stops, the log file has the whole story, so it is read again.
    pub fn game_output(&mut self, lines: Vec<String>, running: bool, cx: &mut Context<Self>) {
        if running {
            if self.live_output.is_none() {
                self.game_started_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok());
            }
            self.live_output = Some(lines);
            // A world opens some time after the start: look again now and then.
            let due = self
                .last_worlds_refresh
                .is_none_or(|at| at.elapsed() >= std::time::Duration::from_secs(5));
            if self.tab == TAB_WORLDS && due {
                self.last_worlds_refresh = Some(std::time::Instant::now());
                self.refresh_worlds = true;
            }
        } else if self.live_output.take().is_some() {
            self.refresh_logs = true;
            self.game_started_ms = None;
            if self.data.has(Section::Worlds) {
                self.refresh_worlds = true;
            }
        }
        cx.notify();
    }

    /// A crash report arrived (or failed to); shown under the list.
    pub fn crash_arrived(&mut self, file: String, result: CrashRead, cx: &mut Context<Self>) {
        // Only the report last asked for is shown.
        if self.crash.as_ref().map(|crash| &crash.0) == Some(&file) {
            self.crash = Some((file, Some(result)));
            cx.notify();
        }
    }

    /// A write finished: show what happened. Returns what it made stale and
    /// has to be read again; the caller loads those and answers with `arrived`.
    pub fn operated(&mut self, done: Operated, cx: &mut Context<Self>) -> Vec<Section> {
        self.busy = false;
        self.confirm = None;
        let failure = done.technical.is_some().then(|| done.notice.clone());
        if let Some(switch) = self.switch.clone() {
            let message = failure.clone();
            switch.update(cx, |switch, cx| switch.finished(message, cx));
            if failure.is_none() {
                self.switch = None;
            }
            self.toasts.push(match &done.technical {
                Some(detail) => Toast::error(done.notice).technical(detail.clone()),
                None => Toast::success(done.notice),
            });
        } else if !self.editor_result(failure, cx) {
            let toast = match done.technical {
                Some(detail) => Toast::error(done.notice).technical(detail),
                None => Toast::success(done.notice),
            };
            self.toasts.push(toast);
        }
        // Writes feed the change history and can change what is wrong.
        let mut stale = done.refresh;
        stale.push(Section::History);
        stale.push(Section::Problems);
        let mut reload = Vec::new();
        for section in stale {
            // Only what has been looked at is worth re-reading.
            if self.data.has(section)
                && !self.data.pending.contains(&section)
                && !reload.contains(&section)
            {
                self.data.pending.push(section);
                reload.push(section);
            }
        }
        cx.notify();
        reload
    }

    /// Sends a write, once: the buttons are disabled while it runs.
    fn send(&mut self, intent: InstanceIntent, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.confirm = None;
        cx.notify();
        (self.handler)(intent, window, cx);
    }

    /// The second click of a destructive action.
    fn confirmed(&mut self, what: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let intent = match what.clone() {
            Confirm::DeleteWorld(folder) => InstanceIntent::DeleteWorld(folder),
            Confirm::DeleteSnapshot(id) => InstanceIntent::DeleteSnapshot(id),
            Confirm::RestoreSnapshot(id) => InstanceIntent::RestoreSnapshot(id),
        };
        self.send(intent, window, cx);
    }

    fn submit_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fields) = &self.fields else { return };
        let name = fields.copy_name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.editor_error = Some("请输入新游戏的名称".into());
            cx.notify();
            return;
        }
        let include_worlds = self.include_worlds;
        self.send(
            InstanceIntent::Copy {
                name,
                include_worlds,
            },
            window,
            cx,
        );
    }

    fn ensure_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(record) = &self.record else { return };
        let name = record.name.clone();
        let copy_name = format!("{name} 副本");
        let min = memory_text(record.settings.min_memory_mb);
        let max = memory_text(record.settings.max_memory_mb);
        if let Some(fields) = &self.fields {
            for (field, text, mask) in [
                (&fields.name, name, 1),
                (&fields.min, min, 2),
                (&fields.max, max, 2),
                (&fields.copy_name, copy_name, 4),
            ] {
                if self.sync_fields & mask != 0 {
                    field.update(cx, |field, cx| field.set_value(text, window, cx));
                }
            }
        } else {
            let mut input = |text: String, placeholder: &'static str, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(text)
                        .placeholder(placeholder)
                })
            };
            self.fields = Some(Fields {
                name: input(name, "游戏名称", cx),
                min: input(min, "继承默认", cx),
                max: input(max, "继承默认", cx),
                copy_name: input(copy_name, "新游戏的名称", cx),
                content_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索")),
                snapshot_note: cx.new(|cx| InputState::new(window, cx).placeholder("备注（可空）")),
                world_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索世界")),
                log_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索日志")),
                file_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索文件")),
            });
            if let Some(fields) = &self.fields {
                Self::watch_content_search(&fields.content_search.clone(), cx);
                Self::watch_content_search(&fields.log_search.clone(), cx);
                Self::watch_content_search(&fields.world_search.clone(), cx);
                Self::watch_content_search(&fields.file_search.clone(), cx);
            }
        }
        self.sync_fields = 0;
    }

    fn submit(&mut self, memory: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let (Some(record), Some(fields)) = (&self.record, &self.fields) else {
            return;
        };
        let intent = if memory {
            match memory_draft(
                &record.settings,
                &self.defaults,
                fields.min.read(cx).value().as_ref(),
                fields.max.read(cx).value().as_ref(),
            ) {
                Ok(settings) => InstanceIntent::SaveMemory(settings),
                Err(message) => {
                    self.editor_error = Some(message.into());
                    cx.notify();
                    return;
                }
            }
        } else {
            let name = fields.name.read(cx).value().trim().to_owned();
            if name.is_empty() {
                self.editor_error = Some("请输入游戏名称".into());
                cx.notify();
                return;
            }
            InstanceIntent::Rename(name)
        };
        self.busy = true;
        self.editor_error = None;
        cx.notify();
        (self.handler)(intent, window, cx);
    }

    fn reset_memory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(fields) = &self.fields else { return };
        for field in [&fields.min, &fields.max] {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        self.submit(true, window, cx);
    }

    fn edit_button(
        &self,
        id: &'static str,
        editor: Editor,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        theme::clickable(
            kit::ghost(id, "编辑", |_, _| {})
                .disabled(self.busy)
                .debug_selector(move || id.into())
                .on_click(
                    cx.listener(move |view, _, window, cx| view.open_editor(editor, window, cx)),
                ),
            !self.busy,
        )
        .into_any_element()
    }

    fn overview(&self, colors: ShellColors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let record = self.record.as_ref().expect("loaded");
        v_flex()
            .w_full()
            .gap_5()
            .child(kit::list(
                vec![
                    // ia[instance.overview]: 改名 | 概览「名称」行 [编辑] → 弹窗 | 改名保留游戏目录、收藏和历史记录 | L-LIB-04
                    kit::value_row(
                        "overview-name",
                        "名称",
                        Some(editors::RENAME_HELP.into()),
                        record.name.clone(),
                        Some(self.edit_button("instance-rename", Editor::Rename, cx)),
                        colors,
                    ),
                    kit::value_row(
                        "overview-version",
                        "游戏版本",
                        None,
                        format!(
                            "{} · {}",
                            record.game_version,
                            live::loader_label(record.loader)
                        ),
                        None,
                        colors,
                    ),
                    kit::value_row(
                        "overview-installed",
                        "安装记录",
                        Some(
                            if record.installed {
                                "曾完成安装，启动时仍会检查文件。"
                            } else {
                                "尚未完成安装，首次启动时准备游戏文件。"
                            }
                            .into(),
                        ),
                        if record.installed {
                            "已安装"
                        } else {
                            "待安装"
                        },
                        None,
                        colors,
                    ),
                    // ia[instance.overview]: 占用空间 | 概览「占用空间」行 | 后台计算，只算这个游戏自己的文件 | —
                    kit::value_row(
                        "overview-size",
                        "占用空间",
                        Some("这个游戏自己的文件；多个游戏共用的游戏文件不算在内。".into()),
                        match &self.data.size {
                            None => "计算中…".to_owned(),
                            Some(Ok(bytes)) => panels::size_label(*bytes),
                            Some(Err(_)) => "读不到".to_owned(),
                        },
                        None,
                        colors,
                    ),
                ],
                colors,
            ))
            .children(self.problems_block(colors, cx))
            .children(self.recent_block(colors, cx))
            .into_any_element()
    }

    fn performance(&self, colors: ShellColors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let machine = self.machine_memory_row(colors);
        v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(kit::section_label("内存", colors))
                    // ia[instance.settings]: 内存 | 设置 · 性能组，「内存」区 [编辑] 弹窗（最小 / 最大） | 留空跟随默认；性能组另有只读的“本机内存 · 推荐最大”一行 | L-SET-01
                    .child(self.edit_button("instance-memory-edit", Editor::Memory, cx)),
            )
            .child(kit::list(
                vec![
                    kit::value_row(
                        "memory-min",
                        "最小内存",
                        Some(editors::MIN_MEMORY_HELP.into()),
                        effective_label(
                            record.settings.min_memory_mb,
                            self.defaults.default_min_memory_mb,
                        ),
                        None,
                        colors,
                    ),
                    kit::value_row(
                        "memory-max",
                        "最大内存",
                        Some(editors::MAX_MEMORY_HELP.into()),
                        effective_label(
                            record.settings.max_memory_mb,
                            self.defaults.default_max_memory_mb,
                        ),
                        None,
                        colors,
                    ),
                ]
                .into_iter()
                .chain(machine)
                .collect(),
                colors,
            ))
            .into_any_element()
    }

    /// Primary 启动游戏; everything else in More (design language §7).
    fn page_actions(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let record = self.record.as_ref()?;
        let busy = self.busy;
        let play = self.handler.clone();
        let entity = cx.entity().downgrade();
        let with_view = move |run: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let entity = entity.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = entity.update(cx, |view, cx| run(view, window, cx));
            }
        };
        let running = self.live_output.is_some();
        let primary = if running {
            // ia[instance]: 结束游戏 | 本启动器启动的游戏运行时，页头主按钮变「结束游戏」 | 终止进程，会话写入历史 | H-PLAY-09
            kit::action(
                "instance-stop",
                "结束游戏",
                Some(crate::assets::UiIcon::Close),
                false,
                move |window, cx| play(InstanceIntent::Stop, window, cx),
            )
            .debug_selector(|| "instance-stop".into())
        } else {
            // ia[instance]: 开始游戏 | 页头主按钮「启动游戏」 | 首页启动时刻接管 | L-PLAY-01
            kit::action(
                "instance-play",
                "启动游戏",
                Some(crate::assets::UiIcon::Play),
                true,
                move |window, cx| play(InstanceIntent::Play, window, cx),
            )
            .disabled(busy)
            .debug_selector(|| "instance-play".into())
        };
        let mut actions = kit::PageActions::new("instance-actions")
            .primary(theme::clickable(primary, running || !busy));
        if !record.installed {
            actions = actions.more(
                // ia[instance]: 安装游戏文件 | 页头 ⋯ 菜单（游戏还没装好时才有） | 后台任务，动态可见；完成 toast | L-OPS-02
                kit::MenuEntry::new(
                    "安装游戏文件",
                    with_view(|view, window, cx| view.send(InstanceIntent::Install, window, cx)),
                )
                .disabled(busy),
            );
        }
        actions
            // ia[instance]: 设为当前游戏 | 页头 ⋯ 菜单 | 导航右段芯片跟着换；之后的启动指向它 | H-NAV-03
            .more(kit::MenuEntry::new(
                "设为当前游戏",
                with_view(|view, window, cx| {
                    (view.handler)(InstanceIntent::SetCurrent, window, cx)
                }),
            ))
            .more(
                // ia[instance]: 修复游戏文件 | 页头 ⋯ 菜单 | 核对并补齐/重下损坏文件，后台任务，动态可见 | H-INSTANCE-11 | 运行中或没装好时禁用
                kit::MenuEntry::new(
                    "修复游戏文件",
                    with_view(|view, window, cx| view.send(InstanceIntent::Repair, window, cx)),
                )
                .disabled(busy || running || !record.installed),
            )
            .more(
                // ia[instance]: 创建快照 | 页头 ⋯ 菜单 → 弹窗（备注可空；范围＝全部或某个世界） | 后台创建，历史·快照可见 | L-HIST-01
                kit::MenuEntry::new(
                    "创建快照…",
                    with_view(|view, window, cx| view.open_editor(Editor::Snapshot, window, cx)),
                )
                .disabled(busy),
            )
            // ia[instance]: 在访达中显示 | 页头 ⋯ 菜单 | 打开游戏目录 | H-INSTANCE-10
            .more(kit::MenuEntry::new(
                "在访达中显示",
                with_view(|view, window, cx| {
                    (view.handler)(InstanceIntent::RevealPath(String::new()), window, cx)
                }),
            ))
            .more(
                // ia[instance]: 复制游戏 | 页头 ⋯ 菜单 → 弹窗（新名称、是否复制存档） | 后台复制成独立副本；不复制历史、快照和游玩时间 | L-LIB-05
                kit::MenuEntry::new(
                    "复制这个游戏…",
                    with_view(|view, window, cx| view.open_editor(Editor::Copy, window, cx)),
                )
                .disabled(busy),
            )
            .more(
                // ia[instance]: 完整备份 | 页头 ⋯ 菜单 → 选位置 | 后台打成一个 zip（含存档，不含日志），toast；之后可在游戏库「从备份恢复」 | — | ADR 0015；运行中禁用
                kit::MenuEntry::new(
                    "完整备份…",
                    with_view(|view, window, cx| {
                        (view.handler)(InstanceIntent::BackupGame, window, cx)
                    }),
                )
                .disabled(busy || running),
            )
            // ia[instance]: 导出整合包 | 页头 ⋯ 菜单 → 导出弹窗（格式、勾选文件） | 后台导出，可取消 | L-LIB-08
            .more(kit::MenuEntry::new(
                "导出整合包…",
                with_view(|view, window, cx| {
                    (view.handler)(InstanceIntent::ExportPack, window, cx)
                }),
            ))
            .more(
                // ia[instance]: 删除游戏 | 页头 ⋯ 菜单 → 警告弹窗 | 删除后从历史中移除并后退 | L-LIB-06
                kit::MenuEntry::new(
                    "删除游戏…",
                    with_view(|view, window, cx| view.confirm_delete(window, cx)),
                )
                .danger()
                .disabled(busy),
            )
            .render(colors)
    }
}

impl Render for InstanceDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_fields(window, cx);
        self.settle_editor(window, cx);
        crate::toast::flush(&mut self.toasts, window, cx);
        if self.record.is_some()
            && let Some(question) = self.ask_later.take()
        {
            match question {
                InstanceIntent::AskCopy => self.open_editor(Editor::Copy, window, cx),
                InstanceIntent::AskDelete => self.confirm_delete(window, cx),
                InstanceIntent::Resolve(action) => self.run_problem(action, window, cx),
                _ => {}
            }
        }
        if self.inspect.is_some() {
            self.open_inspected_crash(window, cx);
        }
        if std::mem::take(&mut self.refresh_worlds) && self.data.has(Section::Worlds) {
            self.request(Section::Worlds, window, cx);
        }
        if std::mem::take(&mut self.refresh_logs) && self.data.has(Section::Logs) {
            self.request(Section::Logs, window, cx);
        }
        if self.record.is_some() && !self.started {
            self.started = true;
            self.ensure(Section::Problems, window, cx);
            self.ensure(Section::Size, window, cx);
            self.ensure(Section::History, window, cx);
        }
        let colors = ShellColors::from_theme(cx.theme());
        let content = if self.record.is_some() {
            match self.tab {
                TAB_OVERVIEW => self.overview(colors, cx),
                TAB_CONTENT => self.content_panel(colors, cx),
                TAB_WORLDS => self.worlds_panel(colors, cx),
                TAB_HISTORY => self.history_panel(colors, cx),
                TAB_DIAGNOSTICS => self.diagnostics_panel(colors, cx),
                _ => self.settings_tab(colors, cx),
            }
        } else if let Some(message) = &self.error {
            let retry = self.handler.clone();
            v_flex()
                .gap_3()
                .child(kit::empty("没有读到游戏", message.clone(), colors))
                .child(
                    h_flex()
                        .justify_center()
                        .gap_2()
                        .child(kit::action(
                            "instance-retry",
                            "重试",
                            None,
                            true,
                            move |window, cx| retry(InstanceIntent::Reload, window, cx),
                        ))
                        .children(
                            self.load_technical
                                .clone()
                                .map(|detail| kit::technical("instance-technical", detail)),
                        ),
                )
                .into_any_element()
        } else {
            kit::empty("正在读取游戏…", "", colors).into_any_element()
        };
        let tabs = cx.listener(|view, index: &usize, window, cx| view.open_tab(*index, window, cx));
        let content = v_flex()
            .w_full()
            .gap_5()
            // Room for the cover above the title; going back is the
            // navigation's job (design language §6).
            .child(div().h(px(84.)))
            .child(kit::header(
                self.title().to_owned(),
                self.record
                    .as_ref()
                    .map_or_else(String::new, live::instance_meta),
                self.page_actions(colors, cx),
                colors,
            ))
            .child(kit::tabs(
                "live-instance-tabs",
                &TABS,
                self.tab,
                move |index, window, cx| tabs(&index, window, cx),
            ))
            .child(kit::entrance(content, ("live-instance-body", self.tab)));
        div()
            .id("live-instance")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .relative()
                    .w_full()
                    .children(self.record.as_ref().map(|record| {
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(px(168.))
                            .child(crate::cover::element(
                                live::seed_of(&record.id),
                                live::cover_loader(record.loader),
                                live::world_of(&record.id),
                                colors.background,
                                theme::HERO_FADE,
                                px(0.),
                            ))
                    }))
                    .child(
                        theme::content_column()
                            .mx_auto()
                            .pt(TITLE_BAR_HEIGHT + px(12.))
                            .pb(theme::BOTTOM_SAFE_AREA)
                            .child(content),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Modifiers, TestAppContext};
    use lumilio_core::Loader;
    use std::cell::RefCell;

    fn record() -> InstanceRecord {
        InstanceRecord {
            id: "survival".into(),
            name: "生存".into(),
            game_version: "1.21.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
            favorite: true,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings {
                java_path: Some("/jdk/bin/java".into()),
                jvm_arguments: vec!["-Dx=y".into()],
                ..InstanceSettings::default()
            },
        }
    }

    #[test]
    fn memory_form_preserves_other_overrides_and_uses_core_validation() {
        let saved = record().settings;
        let mut defaults = LauncherSettings::default();
        defaults.default_min_memory_mb = Some(512);
        defaults.default_max_memory_mb = Some(2048);
        let next = memory_draft(&saved, &defaults, "", "4096").unwrap();
        assert_eq!(next.java_path, saved.java_path);
        assert_eq!(next.jvm_arguments, saved.jvm_arguments);
        assert_eq!(next.min_memory_mb, None);
        assert_eq!(next.max_memory_mb, Some(4096));
        assert_eq!(memory_draft(&saved, &defaults, "", "").unwrap(), saved);
        for (min, max) in [
            ("-1", ""),
            ("x", ""),
            ("0", ""),
            ("4096", ""),
            ("", "1048577"),
            ("4096", "1024"),
        ] {
            assert!(memory_draft(&saved, &defaults, min, max).is_err());
        }
    }

    /// The view under the framework `Root`, which renders the dialog layer.
    fn rooted(
        cx: &mut TestAppContext,
        seen: Rc<RefCell<Vec<InstanceIntent>>>,
    ) -> (Entity<InstanceDetailView>, &mut gpui::VisualTestContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let slot: Rc<RefCell<Option<Entity<InstanceDetailView>>>> = Rc::default();
        let keep = slot.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|_| {
                InstanceDetailView::new(
                    "survival".into(),
                    Rc::new(move |intent, _, _| seen.borrow_mut().push(intent)),
                )
            });
            *keep.borrow_mut() = Some(view.clone());
            gpui_component::Root::new(view, window, cx)
        });
        let view = slot.borrow().clone().expect("view");
        (view, cx)
    }

    fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
        cx.run_until_parked();
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is not on screen"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn memory_is_read_only_on_the_page_and_edited_in_a_dialog(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_SETTINGS, cx);
            view.settings_sub = 3;
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("instance-memory-save").is_none(),
            "the page has no inline form"
        );
        click(cx, "instance-memory-edit");
        assert!(
            cx.debug_bounds("dialog-layer").is_some(),
            "the dialog opened"
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                let fields = view.fields.as_ref().unwrap();
                fields
                    .max
                    .update(cx, |input, cx| input.set_value("4096", window, cx));
            })
        });
        click(cx, "instance-memory-save");
        let mut next = record();
        next.settings.max_memory_mb = Some(4096);
        assert_eq!(
            seen.borrow().as_slice(),
            &[
                InstanceIntent::Load(Section::Problems),
                InstanceIntent::Load(Section::Size),
                InstanceIntent::Load(Section::History),
                InstanceIntent::SaveMemory(next.settings.clone())
            ]
        );
        click(cx, "instance-memory-save");
        assert_eq!(seen.borrow().len(), 4, "no duplicate submit while busy");

        // A failure keeps the dialog and the draft, and says so inside it.
        view.update(cx, |view, cx| view.saved(true, Err("disk full".into()), cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("instance-editor-error").is_some());
        view.read_with(cx, |view, cx| {
            assert_eq!(view.fields.as_ref().unwrap().max.read(cx).value(), "4096");
            assert_eq!(view.record.as_ref().unwrap().settings.max_memory_mb, None);
            assert_eq!(view.editor, Some(Editor::Memory));
        });

        // "Follow the defaults" clears both and saves at once.
        click(cx, "instance-memory-reset");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::SaveMemory(record().settings))
        );

        // Success closes the dialog and the page shows the saved value.
        view.update(cx, |view, cx| {
            view.saved(true, Ok((next, LauncherSettings::default())), cx)
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("dialog-layer").is_none(),
            "the dialog itself closed, not only its content"
        );
        view.read_with(cx, |view, _| assert_eq!(view.editor, None));
    }

    #[gpui::test]
    fn the_settings_groups_show_effective_values_and_restoring_removes_the_override(
        cx: &mut TestAppContext,
    ) {
        use lumilio_core::{InstanceLaunch, LaunchTuning};
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        let mut own = record();
        own.settings.launch = InstanceLaunch {
            fullscreen: Some(true),
            ..InstanceLaunch::default()
        };
        let mut defaults = LauncherSettings::default();
        defaults.launch = LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            ..LaunchTuning::default()
        };
        view.update(cx, |view, cx| {
            view.loaded(Ok((own.clone(), defaults)), cx);
            view.set_machine_memory(Some(16_384), cx);
            view.select_tab(TAB_SETTINGS, cx);
        });
        cx.run_until_parked();

        // Every group's rows are there.
        for (sub, selectors) in [
            (
                0,
                &["isettings-window", "isettings-after", "isettings-quick"][..],
            ),
            (
                1,
                &["isettings-version", "isettings-loader", "isettings-files"][..],
            ),
            (2, &["isettings-java", "isettings-jvm"][..]),
            (3, &["instance-memory-edit", "isettings-machine-memory"][..]),
            (
                4,
                &["isettings-game-args", "isettings-env", "isettings-commands"][..],
            ),
        ] {
            view.update(cx, |view, cx| {
                view.settings_sub = sub;
                cx.notify();
            });
            cx.run_until_parked();
            for selector in selectors {
                assert!(
                    cx.debug_bounds(selector).is_some(),
                    "{selector} in group {sub}"
                );
            }
        }

        // The window row follows the defaults for size and shows its own fullscreen.
        view.update(cx, |view, cx| {
            view.settings_sub = 0;
            cx.notify();
        });
        cx.run_until_parked();
        click(cx, "isettings-window-edit");
        assert!(
            cx.debug_bounds("settings-save").is_some(),
            "the dialog opened"
        );
        assert_eq!(seen.borrow().len(), 3, "only the overview's loads so far");

        // Restoring defaults sends the settings with the override removed.
        click(cx, "settings-reset");
        let mut cleared = own.settings.clone();
        cleared.launch = InstanceLaunch::default();
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::SaveSettings(cleared))
        );

        // The answer shows a toast and the new values; a failure keeps the page.
        view.update(cx, |view, cx| {
            view.settings_saved(Err("disk full".into()), cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| assert!(!view.busy));
    }

    #[gpui::test]
    fn an_invalid_draft_is_explained_in_the_dialog_and_never_sent(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
        });
        click(cx, "instance-rename");
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                let fields = view.fields.as_ref().unwrap();
                fields
                    .name
                    .update(cx, |input, cx| input.set_value("  ", window, cx));
            })
        });
        click(cx, "instance-rename-save");
        assert!(cx.debug_bounds("instance-editor-error").is_some());
        assert!(
            !seen
                .borrow()
                .iter()
                .any(|intent| matches!(intent, InstanceIntent::Rename(_))),
            "an empty name is never sent"
        );
    }

    fn world(folder: &str) -> lumilio_core::WorldInfo {
        lumilio_core::WorldInfo {
            folder: folder.into(),
            name: folder.into(),
            last_played_ms: None,
            game_version: None,
            hardcore: false,
            has_icon: false,
            damaged: false,
            lock_touched_ms: None,
        }
    }

    fn item(name: &str, enabled: bool) -> lumilio_core::ContentItem {
        lumilio_core::ContentItem {
            file_name: if enabled {
                name.into()
            } else {
                format!("{name}.disabled")
            },
            display_name: name.into(),
            enabled,
            size: 2048,
            modified: 0,
            is_directory: false,
        }
    }

    #[gpui::test]
    fn tabs_load_their_data_once_and_ignore_a_second_request_while_pending(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let sink = seen.clone();
        let (view, cx) = cx.add_window_view(|_, _| {
            InstanceDetailView::new(
                "survival".into(),
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
            )
        });
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx)
        });
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().as_slice(),
            &[
                InstanceIntent::Load(Section::Problems),
                InstanceIntent::Load(Section::Size),
                InstanceIntent::Load(Section::History),
            ],
            "the overview asks for what it shows, once"
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.open_tab(TAB_WORLDS, window, cx);
                view.open_tab(TAB_OVERVIEW, window, cx);
                view.open_tab(TAB_WORLDS, window, cx);
            })
        });
        let loads = seen
            .borrow()
            .iter()
            .filter(|intent| **intent == InstanceIntent::Load(Section::Worlds))
            .count();
        assert_eq!(loads, 1, "a pending section is not requested again");
        view.update(cx, |view, cx| {
            view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx)
        });
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.open_tab(TAB_OVERVIEW, window, cx);
                view.open_tab(TAB_WORLDS, window, cx);
            })
        });
        let loads = seen
            .borrow()
            .iter()
            .filter(|intent| **intent == InstanceIntent::Load(Section::Worlds))
            .count();
        assert_eq!(loads, 1, "a loaded section is shown, not re-read");
    }

    #[gpui::test]
    fn the_runtime_group_offers_a_change_and_a_repair(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_SETTINGS, cx);
            view.settings_sub = 1;
        });
        cx.run_until_parked();
        click(cx, "isettings-version-change");
        click(cx, "isettings-loader-change");
        click(cx, "isettings-repair");
        let writes: Vec<_> = seen
            .borrow()
            .iter()
            .filter(|intent| !matches!(intent, InstanceIntent::Load(_)))
            .cloned()
            .collect();
        assert_eq!(
            writes,
            [
                InstanceIntent::OpenRuntimeChange,
                InstanceIntent::OpenRuntimeChange,
                InstanceIntent::Repair
            ]
        );
        assert_eq!(
            view.read_with(cx, |view, _| view.runtime()),
            Some(("1.21.1".into(), Loader::Vanilla, None))
        );
    }

    #[gpui::test]
    fn a_world_can_be_entered_directly_unless_the_version_is_too_old(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let sink = seen.clone();
        let (view, cx) = cx.add_window_view(|_, _| {
            InstanceDetailView::new(
                "survival".into(),
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
            )
        });
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_WORLDS, cx);
            view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
        });
        cx.run_until_parked();
        let enter = cx.debug_bounds("world-play-0").expect("enter button");
        cx.simulate_click(enter.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::PlayWorld("w".into()))
        );

        // An old game version cannot go straight in: the button does nothing.
        let mut old = record();
        old.game_version = "1.16.5".into();
        view.update(cx, |view, cx| {
            view.busy = false;
            view.loaded(Ok((old, LauncherSettings::default())), cx);
        });
        cx.run_until_parked();
        let before = seen.borrow().len();
        let enter = cx.debug_bounds("world-play-0").expect("enter button");
        cx.simulate_click(enter.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(seen.borrow().len(), before);
    }

    #[gpui::test]
    fn the_world_list_is_searched_and_a_world_can_be_imported(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_WORLDS, cx);
            view.arrived(Arrived::Worlds(Ok(vec![world("alpha"), world("beta")])), cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("world-play-1").is_some());

        let search = view.read_with(cx, |view, _| {
            view.fields.as_ref().unwrap().world_search.clone()
        });
        cx.update(|window, cx| search.update(cx, |search, cx| search.set_value("bet", window, cx)));
        cx.run_until_parked();
        assert!(cx.debug_bounds("world-play-1").is_none(), "one world left");
        click(cx, "world-play-0");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::PlayWorld("beta".into()))
        );

        view.update(cx, |view, cx| {
            view.busy = false;
            cx.notify();
        });
        click(cx, "world-import");
        assert_eq!(seen.borrow().last(), Some(&InstanceIntent::ImportWorld));
        assert!(
            !view.read_with(cx, |view, _| view.busy),
            "choosing a file is not a write; cancelling it must not leave the page busy"
        );
    }

    #[gpui::test]
    fn a_crashed_session_leads_to_the_log_and_a_clean_one_does_not(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        let session = |started, outcome| lumilio_core::HistoryEvent::Session {
            started,
            seconds: 60,
            exit_code: None,
            outcome,
        };
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_HISTORY, cx);
            view.history_sub = 1;
            view.arrived(
                Arrived::History(Ok(lumilio_core::HistoryRead {
                    events: vec![
                        session(100, lumilio_core::SessionOutcome::Clean),
                        session(200, lumilio_core::SessionOutcome::Crashed),
                    ],
                    skipped: 0,
                })),
                cx,
            );
        });
        cx.run_until_parked();
        click(cx, "session-inspect");
        view.read_with(cx, |view, _| {
            assert_eq!(view.tab, TAB_DIAGNOSTICS);
            assert_eq!(view.diag_sub, 1);
        });
        assert!(seen.borrow().contains(&InstanceIntent::Load(Section::Logs)));
        assert!(
            !seen
                .borrow()
                .iter()
                .any(|intent| matches!(intent, InstanceIntent::OpenCrash(_))),
            "the reports are not known yet"
        );

        // The list arrives: the report written during that session opens.
        view.update(cx, |view, cx| {
            view.arrived(
                Arrived::Logs(Ok(lumilio_core::GameLogs {
                    latest: None,
                    crashes: vec![
                        lumilio_core::CrashReport {
                            file_name: "elsewhen.txt".into(),
                            modified: 9000,
                        },
                        lumilio_core::CrashReport {
                            file_name: "crash-2.txt".into(),
                            modified: 262,
                        },
                    ],
                })),
                cx,
            )
        });
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::OpenCrash("crash-2.txt".into()))
        );
    }

    #[gpui::test]
    fn deleting_a_world_asks_first_and_never_double_submits(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_WORLDS, cx);
            view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
        });
        cx.run_until_parked();
        let writes = |seen: &Rc<RefCell<Vec<InstanceIntent>>>| {
            seen.borrow()
                .iter()
                .filter(|intent| !matches!(intent, InstanceIntent::Load(_)))
                .cloned()
                .collect::<Vec<_>>()
        };
        let ask = cx.debug_bounds("world-delete-0").expect("delete button");
        cx.simulate_click(ask.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(writes(&seen).is_empty(), "the click only asks, in a dialog");
        view.read_with(cx, |view, _| {
            assert_eq!(view.confirm, Some(Confirm::DeleteWorld("w".into())));
        });
        // Answering yes (the dialog's button calls this) sends it once.
        let yes = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, cx| {
                view.update(cx, |view, cx| {
                    view.confirmed(&Confirm::DeleteWorld("w".into()), window, cx)
                })
            });
            cx.run_until_parked();
        };
        yes(cx);
        assert_eq!(writes(&seen), vec![InstanceIntent::DeleteWorld("w".into())]);
        // While it runs, the world cannot be deleted again.
        view.read_with(cx, |view, _| assert!(view.busy));
        let sent = writes(&seen).len();
        yes(cx);
        assert_eq!(writes(&seen).len(), sent);
        // The answer re-reads only what was looked at (worlds, no snapshots).
        let stale = view.update(cx, |view, cx| {
            view.operated(
                Operated {
                    notice: "世界已删除".into(),
                    technical: None,
                    refresh: vec![Section::Worlds, Section::Snapshots],
                },
                cx,
            )
        });
        assert_eq!(stale, vec![Section::Worlds]);
        view.read_with(cx, |view, _| assert!(!view.busy));
    }

    /// Files Modrinth does not know.
    fn listed(items: Vec<lumilio_core::ContentItem>) -> lumilio_core::ContentList {
        lumilio_core::ContentList {
            entries: items
                .into_iter()
                .map(|item| lumilio_core::ContentEntry {
                    item,
                    sha1: None,
                    source: None,
                    update: None,
                })
                .collect(),
            sources_unavailable: false,
        }
    }

    fn known_version(id: &str, number: &str, game: &str) -> lumilio_core::Version {
        lumilio_core::Version {
            id: id.into(),
            project_id: "P".into(),
            name: number.into(),
            number: number.into(),
            channel: lumilio_core::ReleaseChannel::Release,
            game_versions: vec![game.into()],
            loaders: vec!["fabric".into()],
            published: "2026-09-01T00:00:00Z".into(),
            files: vec![lumilio_core::VersionFile {
                url: "https://cdn/x.jar".into(),
                filename: format!("apple-{number}.jar"),
                primary: true,
                size: 1,
                sha1: Some("new".into()),
            }],
            dependencies: Vec::new(),
            downloads: 0,
            changelog: "Updated for 26.3".into(),
        }
    }

    /// A long version shares a row with the Update key and may not run
    /// underneath it. The newer version is not in the row: the Update key
    /// opens the dialog that names it.
    #[gpui::test]
    fn a_long_version_stays_clear_of_the_update_key(cx: &mut TestAppContext) {
        let (view, cx) = rooted(cx, Rc::default());
        let mut fabric = record();
        fabric.loader = lumilio_core::Loader::Fabric;
        fabric.game_version = "26.3".into();
        let newest = known_version("v2", "1.11.7+26.3", "26.3");
        let mut list = listed(vec![item("iris-fabric-1.11.4+mc26.2.jar", true)]);
        list.entries[0].source = Some(lumilio_core::ContentSource {
            project_id: "P".into(),
            slug: "iris".into(),
            title: "Iris Shaders".into(),
            author: Some("coderbot".into()),
            icon_url: None,
            version_id: "v1".into(),
            version_number: "1.11.4+26.2-fabric-with-a-long-suffix".into(),
        });
        list.entries[0].update = Some(newest);
        view.update(cx, |view, cx| {
            view.loaded(Ok((fabric, LauncherSettings::default())), cx);
            view.select_tab(TAB_CONTENT, cx);
            view.arrived(Arrived::Content(ProjectKind::Mod, Ok(list)), cx);
        });
        cx.run_until_parked();
        let version = cx
            .debug_bounds("content-version-0")
            .expect("version column");
        let update = cx
            .debug_bounds("content-switch-version-0")
            .expect("update key");
        let actions = cx.debug_bounds("content-actions-0").expect("action column");
        assert!(
            update.left() >= actions.left() && update.right() <= actions.right(),
            "the Update key ({update:?}) sits inside its column ({actions:?})"
        );
        assert!(
            version.right() <= update.left(),
            "the version column ({version:?}) ends before the Update key ({update:?})"
        );
    }

    #[gpui::test]
    fn an_identified_file_switches_version_in_a_dialog_and_bulk_acts_on_the_selection(
        cx: &mut TestAppContext,
    ) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        let mut fabric = record();
        fabric.loader = lumilio_core::Loader::Fabric;
        fabric.game_version = "26.3".into();
        let newest = known_version("v2", "3.0.11", "26.3");
        let mut list = listed(vec![item("apple.jar", true), item("mine.jar", true)]);
        list.entries[0].source = Some(lumilio_core::ContentSource {
            project_id: "P".into(),
            slug: "appleskin".into(),
            title: "AppleSkin".into(),
            author: Some("squeek502".into()),
            icon_url: None,
            version_id: "v1".into(),
            version_number: "3.0.10".into(),
        });
        list.entries[0].update = Some(newest.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((fabric, LauncherSettings::default())), cx);
            view.select_tab(TAB_CONTENT, cx);
            view.arrived(Arrived::Content(ProjectKind::Mod, Ok(list)), cx);
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("content-switch-version-1").is_none(),
            "an unknown file cannot switch versions"
        );
        click(cx, "content-switch-version-0");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::LoadVersions("P".into()))
        );
        view.update(cx, |view, cx| {
            view.versions_arrived(
                "P",
                Ok(vec![
                    newest.clone(),
                    known_version("v1", "3.0.10", "26.3"),
                    known_version("v0", "2.0", "1.20"),
                ]),
                cx,
            )
        });
        cx.run_until_parked();
        // Opened as "update": the newest compatible version is chosen.
        click(cx, "switch-commit");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::SwitchContent {
                kind: ProjectKind::Mod,
                file_name: "apple.jar".into(),
                project: "P".into(),
                version_id: "v2".into(),
            })
        );
        view.update(cx, |view, cx| {
            view.operated(
                Operated {
                    notice: "已换成 apple-3.0.11.jar".into(),
                    technical: None,
                    refresh: vec![Section::Content(ProjectKind::Mod)],
                },
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("dialog-layer").is_none(),
            "the dialog closed"
        );

        // Select both rows: the bulk bar replaces the toolbar and acts on both.
        view.update(cx, |view, cx| {
            view.selected.insert("apple.jar".into());
            view.selected.insert("mine.jar".into());
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("content-bulk").is_some());
        click(cx, "content-bulk-disable");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::SetContent {
                kind: ProjectKind::Mod,
                files: vec!["apple.jar".into(), "mine.jar".into()],
                enabled: false,
            })
        );
        view.read_with(cx, |view, _| assert!(view.selected.is_empty()));
    }

    #[gpui::test]
    fn content_switches_send_one_change_and_a_failure_keeps_the_list(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let sink = seen.clone();
        let (view, cx) = cx.add_window_view(|_, _| {
            InstanceDetailView::new(
                "survival".into(),
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
            )
        });
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_CONTENT, cx);
            view.arrived(
                Arrived::Content(
                    ProjectKind::Mod,
                    Ok(listed(vec![item("a.jar", true), item("b.jar", false)])),
                ),
                cx,
            );
        });
        cx.run_until_parked();
        let switch = cx.debug_bounds("content-switch-1").expect("second switch");
        cx.simulate_click(switch.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::SetContent {
                kind: ProjectKind::Mod,
                files: vec!["b.jar.disabled".into()],
                enabled: true,
            })
        );
        let sent = seen.borrow().len();
        let other = cx.debug_bounds("content-switch-0").expect("first switch");
        cx.simulate_click(other.center(), Modifiers::none());
        assert_eq!(seen.borrow().len(), sent, "busy: no second change");
        view.update(cx, |view, cx| {
            view.operated(
                Operated {
                    notice: "没有成功".into(),
                    technical: Some("disk full".into()),
                    refresh: Vec::new(),
                },
                cx,
            )
        });
        view.read_with(cx, |view, _| {
            assert!(!view.busy);
            assert!(matches!(&view.data.content[0], Some(Ok(list)) if list.entries.len() == 2));
            assert_eq!(
                view.pending_toasts(),
                [Toast::error("没有成功").technical("disk full")],
                "the failure floats up; the page keeps its list"
            );
        });
    }

    #[gpui::test]
    fn the_log_tab_loads_once_and_shows_only_the_report_last_asked_for(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let sink = seen.clone();
        let (view, cx) = cx.add_window_view(|_, _| {
            InstanceDetailView::new(
                "survival".into(),
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
            )
        });
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.diag_sub = 1;
        });
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.open_tab(TAB_DIAGNOSTICS, window, cx))
        });
        assert!(seen.borrow().contains(&InstanceIntent::Load(Section::Logs)));
        view.update(cx, |view, cx| {
            view.arrived(
                Arrived::Logs(Ok(lumilio_core::GameLogs {
                    latest: Some("[main] Done".into()),
                    crashes: vec![lumilio_core::CrashReport {
                        file_name: "crash-1.txt".into(),
                        modified: 5,
                    }],
                })),
                cx,
            );
        });
        cx.run_until_parked();
        let open = cx.debug_bounds("crash-open").expect("view button");
        cx.simulate_click(open.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::OpenCrash("crash-1.txt".into()))
        );
        view.update(cx, |view, cx| {
            view.crash_arrived("crash-0.txt".into(), Ok(("old".into(), Vec::new())), cx);
        });
        view.read_with(cx, |view, _| {
            assert!(matches!(&view.crash, Some((file, None)) if file == "crash-1.txt"));
        });
        view.update(cx, |view, cx| {
            view.crash_arrived(
                "crash-1.txt".into(),
                Ok(("OOM".into(), vec![CrashHint::OutOfMemory])),
                cx,
            );
        });
        view.read_with(cx, |view, _| {
            assert!(matches!(&view.crash, Some((_, Some(Ok((text, _))))) if text == "OOM"));
        });
        cx.run_until_parked();
        let export = cx.debug_bounds("instance-log-export").expect("export log");
        cx.simulate_click(export.center(), Modifiers::none());
        assert_eq!(seen.borrow().last(), Some(&InstanceIntent::ExportLog(None)));
        let export = cx.debug_bounds("crash-export").expect("export report");
        cx.simulate_click(export.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::ExportLog(Some("crash-1.txt".into())))
        );
    }

    #[gpui::test]
    fn the_diagnostics_tab_loads_the_part_it_shows_and_lists_one_folder_at_a_time(
        cx: &mut TestAppContext,
    ) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx)
        });
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.open_tab(TAB_DIAGNOSTICS, window, cx))
        });
        assert!(
            seen.borrow()
                .contains(&InstanceIntent::Load(Section::Problems)),
            "问题 is the first part"
        );
        assert!(
            !seen
                .borrow()
                .contains(&InstanceIntent::Load(Section::Files))
        );

        view.update(cx, |view, _| view.diag_sub = 2);
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.open_diagnostics(window, cx));
            view.update(cx, |view, cx| view.open_diagnostics(window, cx));
        });
        let loads = |seen: &Rc<RefCell<Vec<InstanceIntent>>>| {
            seen.borrow()
                .iter()
                .filter(|intent| **intent == InstanceIntent::Load(Section::Files))
                .count()
        };
        assert_eq!(loads(&seen), 1, "asked once while the answer is on its way");

        let entry = |name: &str, is_dir: bool| lumilio_core::FileEntry {
            name: name.into(),
            is_dir,
            size: 12,
            modified: 5,
        };
        view.update(cx, |view, cx| {
            view.arrived(
                Arrived::Files(
                    String::new(),
                    Ok(vec![entry("config", true), entry("options.txt", false)]),
                ),
                cx,
            )
        });
        click(cx, "file-open-0");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::OpenFolder("config".into()))
        );
        let count = seen.borrow().len();
        // The answer has not come: a second click does not ask again.
        click(cx, "file-open-0");
        assert_eq!(seen.borrow().len(), count);

        view.update(cx, |view, cx| {
            view.arrived(
                Arrived::Files("config".into(), Ok(vec![entry("sodium.json", false)])),
                cx,
            )
        });
        click(cx, "file-reveal-0");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::RevealPath("config/sodium.json".into()))
        );
    }

    /// Gpui hands a wheel event to every scrollable under the pointer, so a
    /// log that scrolls inside a page that scrolls moved both at once.
    #[gpui::test]
    fn scrolling_the_log_leaves_the_page_where_it_is(cx: &mut TestAppContext) {
        use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};

        let (view, cx) = rooted(cx, Rc::default());
        let text: String = (0..300)
            .map(|n| format!("[{n}] [main/INFO]: line {n}\n"))
            .collect();
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.diag_sub = 1;
            view.select_tab(TAB_DIAGNOSTICS, cx);
            view.arrived(
                Arrived::Logs(Ok(lumilio_core::GameLogs {
                    latest: Some(text),
                    crashes: Vec::new(),
                })),
                cx,
            );
        });
        cx.simulate_resize(gpui::size(px(1080.), px(500.)));
        cx.run_until_parked();

        let wheel = |cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>| {
            cx.simulate_event(ScrollWheelEvent {
                position: at,
                delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
                modifiers: gpui::Modifiers::none(),
                touch_phase: TouchPhase::Moved,
            });
            cx.run_until_parked();
        };
        let lines = cx.debug_bounds("instance-log-lines").expect("log lines");
        let at = point(lines.center().x, lines.top() + px(40.));
        wheel(cx, at);
        let after = cx.debug_bounds("instance-log-lines").expect("log lines");
        assert_eq!(
            after.origin.y, lines.origin.y,
            "the page stayed put while the log scrolled"
        );
        let scrolled = view.read_with(cx, |view, _| view.log_scroll.offset().y);
        assert!(scrolled < px(0.), "the log itself scrolled ({scrolled:?})");
    }

    #[gpui::test]
    fn the_log_view_filters_by_level_and_search_and_keeps_stack_traces_with_their_line(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = rooted(cx, Rc::default());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.diag_sub = 1;
            view.select_tab(TAB_DIAGNOSTICS, cx);
            view.arrived(
                Arrived::Logs(Ok(lumilio_core::GameLogs {
                    latest: Some(
                        "[1] [main/INFO]: loaded sodium\n[2] [main/WARN]: slow tick\n[3] [main/ERROR]: boom\n\tat a.B(B.java)"
                            .into(),
                    ),
                    crashes: Vec::new(),
                })),
                cx,
            );
        });
        cx.run_until_parked();
        let shown = |view: &Entity<InstanceDetailView>, cx: &mut gpui::VisualTestContext| {
            view.read_with(cx, |view, cx| view.visible_log(cx).unwrap())
        };
        assert_eq!(shown(&view, cx).lines().count(), 4);
        view.update(cx, |view, cx| {
            view.log_level = 1;
            cx.notify();
        });
        assert_eq!(
            shown(&view, cx),
            "[3] [main/ERROR]: boom\n\tat a.B(B.java)",
            "the trace line belongs to the error"
        );
        view.update(cx, |view, _| view.log_level = 2);
        assert_eq!(shown(&view, cx).lines().count(), 3);
        view.update(cx, |view, _| view.log_level = 0);
        let search = view.read_with(cx, |view, _| {
            view.fields.as_ref().unwrap().log_search.clone()
        });
        cx.update(|window, cx| {
            search.update(cx, |search, cx| search.set_value("SODIUM", window, cx))
        });
        assert_eq!(shown(&view, cx), "[1] [main/INFO]: loaded sodium");
    }

    #[gpui::test]
    fn the_running_games_output_replaces_the_log_file_until_it_stops(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.diag_sub = 1;
            view.select_tab(TAB_DIAGNOSTICS, cx);
            view.arrived(
                Arrived::Logs(Ok(lumilio_core::GameLogs {
                    latest: Some("[1] [main/INFO]: from the file".into()),
                    crashes: Vec::new(),
                })),
                cx,
            );
        });
        cx.run_until_parked();
        let shown = |view: &Entity<InstanceDetailView>, cx: &mut gpui::VisualTestContext| {
            view.read_with(cx, |view, cx| view.visible_log(cx).unwrap())
        };
        assert_eq!(shown(&view, cx), "[1] [main/INFO]: from the file");

        view.update(cx, |view, cx| {
            view.game_output(
                vec![
                    "[2] [main/INFO]: live one".into(),
                    "[3] [main/WARN]: live two".into(),
                ],
                true,
                cx,
            )
        });
        assert_eq!(
            shown(&view, cx),
            "[2] [main/INFO]: live one\n[3] [main/WARN]: live two"
        );
        // The level filter works on the live lines too.
        view.update(cx, |view, _| view.log_level = 2);
        assert_eq!(shown(&view, cx), "[3] [main/WARN]: live two");

        let before = seen
            .borrow()
            .iter()
            .filter(|i| **i == InstanceIntent::Load(Section::Logs))
            .count();
        view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
        cx.run_until_parked();
        let after = seen
            .borrow()
            .iter()
            .filter(|i| **i == InstanceIntent::Load(Section::Logs))
            .count();
        assert_eq!(
            after,
            before + 1,
            "the file is read again once the game ends"
        );
        view.update(cx, |view, _| view.log_level = 0);
        assert_eq!(shown(&view, cx), "[1] [main/INFO]: from the file");
    }

    #[gpui::test]
    fn the_header_starts_the_game_and_becomes_a_stop_button_while_it_runs(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx)
        });
        click(cx, "instance-play");
        assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Play));
        assert!(cx.debug_bounds("instance-stop").is_none());

        view.update(cx, |view, cx| view.game_output(Vec::new(), true, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("instance-play").is_none());
        click(cx, "instance-stop");
        assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Stop));

        view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("instance-play").is_some());
    }

    #[gpui::test]
    fn the_overview_sizes_the_game_lists_recent_things_and_acts_on_a_problem(
        cx: &mut TestAppContext,
    ) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx)
        });
        cx.run_until_parked();
        for section in [Section::Problems, Section::Size, Section::History] {
            assert!(
                seen.borrow().contains(&InstanceIntent::Load(section)),
                "the overview asks for {section:?}"
            );
        }
        view.update(cx, |view, cx| {
            view.arrived(Arrived::Size(Ok(3 * 1024 * 1024)), cx);
            view.arrived(
                Arrived::Problems(Ok(vec![lumilio_core::Problem {
                    severity: lumilio_core::Severity::Error,
                    kind: lumilio_core::ProblemKind::DamagedFiles { count: 2 },
                }])),
                cx,
            );
            view.arrived(
                Arrived::History(Ok(lumilio_core::HistoryRead {
                    events: vec![lumilio_core::HistoryEvent::Session {
                        started: 5,
                        seconds: 90,
                        exit_code: None,
                        outcome: lumilio_core::SessionOutcome::Clean,
                    }],
                    skipped: 0,
                })),
                cx,
            );
        });
        cx.run_until_parked();
        click(cx, "problem-action-0");
        assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Repair));

        view.update(cx, |view, cx| {
            view.busy = false;
            cx.notify();
        });
        click(cx, "overview-all-sessions");
        view.read_with(cx, |view, _| {
            assert_eq!(view.tab, TAB_HISTORY);
            assert_eq!(view.history_sub, 1, "straight to the play records");
        });
    }

    #[gpui::test]
    fn a_question_asked_before_the_game_is_loaded_waits_for_it(cx: &mut TestAppContext) {
        let (view, cx) = rooted(cx, Rc::default());
        view.update(cx, |view, cx| view.ask_later(InstanceIntent::AskCopy, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(view.editor.is_none(), "nothing to copy yet");
        });
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx)
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert_eq!(view.editor, Some(Editor::Copy));
            assert!(view.ask_later.is_none(), "asked once");
        });
    }

    #[gpui::test]
    fn a_snapshot_is_taken_from_a_dialog_with_a_note_and_a_range(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_HISTORY, cx);
            view.history_sub = 2;
            view.arrived(Arrived::Snapshots(Ok(Vec::new())), cx);
            view.arrived(Arrived::Worlds(Ok(vec![world("alpha"), world("beta")])), cx);
        });
        cx.run_until_parked();
        click(cx, "snapshot-create");
        view.read_with(cx, |view, _| {
            assert_eq!(view.editor, Some(Editor::Snapshot))
        });

        // A note and the second world.
        let note = view.read_with(cx, |view, _| {
            view.fields.as_ref().unwrap().snapshot_note.clone()
        });
        cx.update(|window, cx| {
            note.update(cx, |note, cx| {
                note.set_value("  before the farm  ", window, cx)
            })
        });
        view.update(cx, |view, cx| {
            view.snapshot_scope = 2;
            cx.notify();
        });
        click(cx, "instance-snapshot-go");
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::CreateSnapshotAs {
                note: "before the farm".into(),
                world: Some("beta".into()),
            })
        );
    }

    #[gpui::test]
    fn the_java_dialog_can_browse_the_disk_for_a_path(cx: &mut TestAppContext) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_SETTINGS, cx);
            view.select_settings_group(2, cx);
        });
        cx.run_until_parked();
        click(cx, "isettings-java-edit");
        assert!(cx.debug_bounds("settings-browse").is_some());
    }

    #[gpui::test]
    fn while_a_game_runs_the_worlds_cannot_be_touched_and_a_dropped_archive_is_one_write(
        cx: &mut TestAppContext,
    ) {
        let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
        let (view, cx) = rooted(cx, seen.clone());
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx);
            view.select_tab(TAB_WORLDS, cx);
            view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| view.game_output(Vec::new(), true, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, _| assert!(view.game_started_ms.is_some()));
        let before = seen.borrow().len();
        click(cx, "world-import");
        click(cx, "world-play-0");
        assert_eq!(
            seen.borrow().len(),
            before,
            "nothing is sent for a world while the game runs"
        );

        view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
        cx.run_until_parked();
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.import_dropped(
                    &[
                        "/tmp/a.txt".into(),
                        "/tmp/w.zip".into(),
                        "/tmp/x.zip".into(),
                    ],
                    window,
                    cx,
                )
            })
        });
        assert_eq!(
            seen.borrow().last(),
            Some(&InstanceIntent::AddWorld("/tmp/w.zip".into()))
        );
    }

    #[gpui::test]
    fn a_response_for_another_instance_cannot_replace_the_view(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let view = cx.new(|_| InstanceDetailView::new("survival".into(), Rc::new(|_, _, _| {})));
        view.update(cx, |view, cx| {
            view.loaded(Ok((record(), LauncherSettings::default())), cx)
        });
        let mut other = record();
        other.id = "other".into();
        other.name = "另一个游戏".into();
        view.update(cx, |view, cx| {
            view.loaded(Ok((other, LauncherSettings::default())), cx)
        });
        assert_eq!(
            view.read_with(cx, |view, _| view.title().to_owned()),
            "生存"
        );
    }
}
