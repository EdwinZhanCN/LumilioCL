//! The real Instance sections: Overview actions, Content, Worlds and History.
//!
//! A child module of `instance_detail`, so it shares the view's private state.
//! Copy for domain facts is mapped here; core types never carry display text.

use gpui::{AnyElement, App, Context, Div, IntoElement, Window, div, prelude::*, px};
use gpui_component::input::Input;
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};
use lumilio_core::{
    ChangeKind, ContentList, CrashHint, FileEntry, GameLogs, HistoryEvent, HistoryRead, Problem,
    ProblemKind, ProjectKind, SessionOutcome, Severity, SnapshotInfo, SnapshotScope, WorldInfo,
};

use super::{InstanceDetailView, InstanceIntent, Section};
use crate::{assets::UiIcon, kit, live, theme::ShellColors, toast::Toast};

/// The three content kinds, in the order of their sub-tabs.
pub const CONTENT_KINDS: [ProjectKind; 3] = [
    ProjectKind::Mod,
    ProjectKind::ResourcePack,
    ProjectKind::Shader,
];
pub const CONTENT_LABELS: [&str; 3] = ["Mod", "资源包", "光影"];
pub const HISTORY_LABELS: [&str; 3] = ["变更", "游玩记录", "快照"];
pub const WORLD_SORTS: [&str; 2] = ["最近游玩", "名称"];

/// The world a running game most likely has open: the one whose lock file was
/// touched after the game started, the newest of them.
#[must_use]
pub fn playing_world(worlds: &[WorldInfo], started_ms: i64) -> Option<&str> {
    worlds
        .iter()
        .filter_map(|world| Some((world.lock_touched_ms.filter(|at| *at >= started_ms)?, world)))
        .max_by_key(|(at, _)| *at)
        .map(|(_, world)| world.folder.as_str())
}

/// The worlds that match `query` (in the name or the folder, ignoring case),
/// most recently played first, or by name.
pub fn ordered_worlds<'a>(worlds: &'a [WorldInfo], query: &str, sort: usize) -> Vec<&'a WorldInfo> {
    let query = query.trim().to_lowercase();
    let mut shown: Vec<_> = worlds
        .iter()
        .filter(|world| {
            query.is_empty()
                || world.name.to_lowercase().contains(&query)
                || world.folder.to_lowercase().contains(&query)
        })
        .collect();
    if sort == 1 {
        shown.sort_by_key(|world| world.name.to_lowercase());
    } else {
        shown.sort_by_key(|world| std::cmp::Reverse(world.last_played_ms));
    }
    shown
}

pub type Loaded<T> = Option<Result<T, String>>;

/// What the sections show. `None` means not loaded yet; an old value stays on
/// screen while a refresh is under way.
#[derive(Default)]
pub struct Data {
    pub content: [Loaded<ContentList>; 3],
    pub worlds: Loaded<Vec<WorldInfo>>,
    pub snapshots: Loaded<Vec<SnapshotInfo>>,
    pub history: Loaded<HistoryRead>,
    pub problems: Loaded<Vec<Problem>>,
    pub logs: Loaded<GameLogs>,
    /// The folder shown in 文件 (relative to the game directory) and its entries.
    pub files: Loaded<(String, Vec<FileEntry>)>,
    pub size: Loaded<u64>,
    pub pending: Vec<Section>,
}

/// One data set arriving from the service.
pub enum Arrived {
    Content(ProjectKind, Result<ContentList, String>),
    Worlds(Result<Vec<WorldInfo>, String>),
    Snapshots(Result<Vec<SnapshotInfo>, String>),
    History(Result<HistoryRead, String>),
    Problems(Result<Vec<Problem>, String>),
    Logs(Result<GameLogs, String>),
    Files(String, Result<Vec<FileEntry>, String>),
    Size(Result<u64, String>),
}

impl Arrived {
    pub fn section(&self) -> Section {
        match self {
            Self::Content(kind, _) => Section::Content(*kind),
            Self::Worlds(_) => Section::Worlds,
            Self::Snapshots(_) => Section::Snapshots,
            Self::History(_) => Section::History,
            Self::Problems(_) => Section::Problems,
            Self::Logs(_) => Section::Logs,
            Self::Files(..) => Section::Files,
            Self::Size(_) => Section::Size,
        }
    }
}

impl Data {
    pub fn store(&mut self, arrived: Arrived) {
        self.pending.retain(|section| *section != arrived.section());
        match arrived {
            Arrived::Content(kind, result) => {
                if let Some(index) = CONTENT_KINDS.iter().position(|k| *k == kind) {
                    self.content[index] = Some(result);
                }
            }
            Arrived::Worlds(result) => self.worlds = Some(result),
            Arrived::Snapshots(result) => self.snapshots = Some(result),
            Arrived::History(result) => self.history = Some(result),
            Arrived::Problems(result) => self.problems = Some(result),
            Arrived::Logs(result) => self.logs = Some(result),
            Arrived::Size(result) => self.size = Some(result),
            Arrived::Files(folder, result) => {
                self.files = Some(result.map(|entries| (folder, entries)));
            }
        }
    }

    pub fn has(&self, section: Section) -> bool {
        match section {
            Section::Content(kind) => CONTENT_KINDS
                .iter()
                .position(|k| *k == kind)
                .is_some_and(|index| self.content[index].is_some()),
            Section::Worlds => self.worlds.is_some(),
            Section::Snapshots => self.snapshots.is_some(),
            Section::History => self.history.is_some(),
            Section::Problems => self.problems.is_some(),
            Section::Logs => self.logs.is_some(),
            Section::Files => self.files.is_some(),
            Section::Size => self.size.is_some(),
        }
    }
}

/// A destructive step waiting for a second click.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Confirm {
    DeleteWorld(String),
    DeleteSnapshot(String),
    RestoreSnapshot(String),
}

impl Confirm {
    /// The question and what it costs, for the dialog; the last is the
    /// confirming button's label.
    pub fn words(
        &self,
        worlds: &[WorldInfo],
        snapshots: &[SnapshotInfo],
    ) -> (String, String, &'static str) {
        match self {
            Self::DeleteWorld(folder) => {
                let name = worlds
                    .iter()
                    .find(|world| &world.folder == folder)
                    .map_or(folder.as_str(), |world| world.name.as_str());
                (
                    format!("删除世界“{name}”？"),
                    "世界和里面的东西都会删除，之后不能找回。想留个后手的话，先创建备份。"
                        .to_owned(),
                    "删除",
                )
            }
            Self::RestoreSnapshot(id) => {
                let label = snapshots
                    .iter()
                    .find(|snapshot| &snapshot.id == id)
                    .map_or("这份快照", |snapshot| snapshot.label.as_str());
                (
                    format!("恢复“{label}”？"),
                    "恢复会用快照替换现有的世界和设置，当前状态会先自动存一份；中途失败或退出会回到恢复前的样子。".to_owned(),
                    "恢复",
                )
            }
            Self::DeleteSnapshot(id) => {
                let label = snapshots
                    .iter()
                    .find(|snapshot| &snapshot.id == id)
                    .map_or("这份快照", |snapshot| snapshot.label.as_str());
                (
                    format!("删除快照“{label}”？"),
                    "删除后不能再恢复到这个时间点。".to_owned(),
                    "删除",
                )
            }
        }
    }
}

// ── copy ────────────────────────────────────────────────────────────────

pub fn change_label(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::ContentAdded => "添加了",
        ChangeKind::ContentRemoved => "移除了",
        ChangeKind::ContentEnabled => "启用了",
        ChangeKind::ContentDisabled => "停用了",
        ChangeKind::ContentUpdated => "更新了",
        ChangeKind::SettingsChanged => "修改了设置",
        ChangeKind::GameVersionChanged => "更换了游戏版本",
        ChangeKind::Repaired => "修复了",
        ChangeKind::WorldCopied => "复制了世界",
        ChangeKind::WorldDeleted => "删除了世界",
        ChangeKind::WorldImported => "导入了世界",
        ChangeKind::SnapshotCreated => "创建了快照",
        ChangeKind::SnapshotRestored => "恢复了快照",
        ChangeKind::SnapshotDeleted => "删除了快照",
    }
}

pub fn outcome_label(outcome: SessionOutcome) -> &'static str {
    match outcome {
        SessionOutcome::Clean => "正常结束",
        SessionOutcome::Crashed => "异常退出",
        SessionOutcome::FailedToStart => "没能启动",
        SessionOutcome::Stopped => "被手动停止",
        SessionOutcome::FailedToPrepare => "准备阶段失败",
        SessionOutcome::Cancelled => "启动前取消",
        SessionOutcome::Interrupted => "启动器中途退出，结果未知",
    }
}

/// What a finished export tells, in words.
pub fn export_notice(report: &lumilio_core::ExportReport, path: &std::path::Path) -> String {
    if report.lookup_failed {
        format!(
            "没能连上 Modrinth，{} 个文件都直接放进了整合包。已导出到 {}",
            report.bundled,
            path.display()
        )
    } else {
        format!(
            "已导出到 {}：{} 个文件按地址列出，{} 个放在包里",
            path.display(),
            report.linked,
            report.bundled
        )
    }
}

pub fn duration_label(seconds: u64) -> String {
    match seconds {
        0..=59 => "不到 1 分钟".to_owned(),
        60..=3599 => format!("{} 分钟", seconds / 60),
        _ => format!("{} 小时 {} 分钟", seconds / 3600, seconds % 3600 / 60),
    }
}

pub fn size_label(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    match bytes {
        0 => "—".to_owned(),
        1..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{} KB", bytes / 1024),
        _ if bytes < 1024 * MB => format!("{:.1} MB", bytes as f64 / MB as f64),
        _ => format!("{:.2} GB", bytes as f64 / (1024 * MB) as f64),
    }
}

/// The newest `count` sessions: (started, seconds, outcome).
#[must_use]
pub fn recent_sessions(read: &HistoryRead, count: usize) -> Vec<(u64, u64, SessionOutcome)> {
    read.events
        .iter()
        .rev()
        .filter_map(|event| match event {
            HistoryEvent::Session {
                started,
                seconds,
                outcome,
                ..
            } => Some((*started, *seconds, *outcome)),
            HistoryEvent::Change { .. } => None,
        })
        .take(count)
        .collect()
}

/// The newest `count` changes: (at, kind, subject).
#[must_use]
pub fn recent_changes(read: &HistoryRead, count: usize) -> Vec<(u64, ChangeKind, String)> {
    read.events
        .iter()
        .rev()
        .filter_map(|event| match event {
            HistoryEvent::Change { at, kind, subject } => Some((*at, *kind, subject.clone())),
            HistoryEvent::Session { .. } => None,
        })
        .take(count)
        .collect()
}

/// What a problem's button does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProblemAction {
    Install,
    Repair,
    /// Download the Java a game needs (the major version, if known).
    InstallJava(Option<u32>),
    ChangeRuntime,
    /// A group of the game's settings (see `instance_settings::SUBTABS`).
    Settings(usize),
    Content,
    Accounts,
    Logs,
}

/// The one thing to offer next to a problem, with its label.
#[must_use]
pub fn problem_action(kind: &ProblemKind) -> Option<(ProblemAction, &'static str)> {
    Some(match kind {
        ProblemKind::NotInstalled => (ProblemAction::Install, "安装"),
        ProblemKind::LoaderUnsupported(_) => (ProblemAction::ChangeRuntime, "更换…"),
        ProblemKind::NoJava { required } => (ProblemAction::InstallJava(*required), "安装 Java"),
        ProblemKind::NoAccount => (ProblemAction::Accounts, "去添加"),
        ProblemKind::DamagedFiles { .. } => (ProblemAction::Repair, "修复"),
        ProblemKind::WrongLoaderMods { .. } | ProblemKind::DuplicateMods { .. } => {
            (ProblemAction::Content, "去内容")
        }
        ProblemKind::LowMemory { .. } => (ProblemAction::Settings(3), "去调整"),
        ProblemKind::LastSessionFailed(_) => (ProblemAction::Logs, "查看日志"),
    })
}

/// One line per problem; the headline is calm, the cause stays short.
pub fn problem_text(problem: &Problem) -> (String, String) {
    match &problem.kind {
        ProblemKind::NotInstalled => (
            "游戏文件还没有安装".into(),
            "可以现在安装，也可以首次启动时自动准备".into(),
        ),
        ProblemKind::LoaderUnsupported(loader) => (
            format!("暂时不能启动 {}", live::loader_label(*loader)),
            "这个加载器还没有支持，可以改用 Fabric、Quilt 或原版".into(),
        ),
        ProblemKind::NoJava { required } => (
            "没有找到可用的 Java".into(),
            match required {
                Some(major) => format!("这个版本需要 Java {major}"),
                None => "请安装 Java 后重试".into(),
            },
        ),
        ProblemKind::NoAccount => ("还没有选择账户".into(), "离线游玩需要一个玩家名".into()),
        ProblemKind::DamagedFiles { count } => (
            "有游戏文件缺失或损坏".into(),
            format!("共 {count} 个，重新安装会补回它们"),
        ),
        ProblemKind::WrongLoaderMods { names } => {
            ("有 Mod 不是为当前加载器制作的".into(), names.join("、"))
        }
        ProblemKind::DuplicateMods { ids } => ("同一个 Mod 装了不止一份".into(), ids.join("、")),
        ProblemKind::LowMemory { max_mb } => (
            "最大内存偏低".into(),
            format!("当前上限 {max_mb} MB，游戏可能因内存不足而崩溃"),
        ),
        ProblemKind::LastSessionFailed(outcome) => (
            "上一次游玩没有顺利结束".into(),
            outcome_label(*outcome).into(),
        ),
    }
}

pub fn hint_text(hint: CrashHint) -> &'static str {
    match hint {
        CrashHint::OutOfMemory => "内存不足：试着调高最大内存，或停用一些 Mod",
        CrashHint::JavaTooOld => "Java 版本太旧：这个游戏需要更新的 Java",
        CrashHint::BadJvmOption => "有无法识别的 Java 参数：检查「设置」里的附加参数",
        CrashHint::ModConflict => "Mod 之间有冲突或缺少依赖：可以逐个停用后再试",
        CrashHint::GraphicsFailure => "显卡或 OpenGL 出错：更新显卡驱动或关闭光影",
    }
}

pub fn problem_tone(severity: Severity, colors: ShellColors) -> gpui::Hsla {
    match severity {
        Severity::Error => colors.danger,
        Severity::Warning => kit::tone_warn(),
        Severity::Info => colors.muted,
    }
}

pub(super) fn clock(seconds: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    live::relative_time(seconds, now)
}

type Act = dyn Fn(&mut InstanceDetailView, &mut Window, &mut Context<InstanceDetailView>);

/// A click handler that updates this view (weakly, so it never keeps it alive).
pub(super) fn act(
    cx: &Context<InstanceDetailView>,
    run: impl Fn(&mut InstanceDetailView, &mut Window, &mut Context<InstanceDetailView>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    let run: Box<Act> = Box::new(run);
    move |window, app| {
        if let Some(view) = view.upgrade() {
            view.update(app, |view, cx| run(view, window, cx));
        }
    }
}

pub(super) fn act_index(
    cx: &Context<InstanceDetailView>,
    run: impl Fn(&mut InstanceDetailView, usize, &mut Window, &mut Context<InstanceDetailView>)
    + 'static,
) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    move |index, window, app| {
        if let Some(view) = view.upgrade() {
            view.update(app, |view, cx| run(view, index, window, cx));
        }
    }
}

// ── rendering ───────────────────────────────────────────────────────────

impl InstanceDetailView {
    /// A load error or a loading note instead of a body.
    pub(super) fn status<T>(
        &self,
        data: &Loaded<T>,
        colors: ShellColors,
        retry: Section,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        match data {
            None => Some(kit::empty("正在读取…", "", colors).into_any_element()),
            Some(Err(detail)) => Some(
                v_flex()
                    .gap_3()
                    .child(kit::empty("没有读到这部分内容", "可以重试", colors))
                    .child(
                        h_flex()
                            .justify_center()
                            .gap_2()
                            .child(kit::action(
                                ("instance-section-retry", 0usize),
                                "重试",
                                Some(UiIcon::Refresh),
                                false,
                                act(cx, move |view, window, cx| view.request(retry, window, cx)),
                            ))
                            .child(kit::technical("instance-section-technical", detail.clone())),
                    )
                    .into_any_element(),
            ),
            Some(Ok(_)) => None,
        }
    }

    /// A button that asks first, in a dialog that says what goes away.
    fn asking(
        &self,
        id: (&'static str, usize),
        label: &'static str,
        confirm: Confirm,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        kit::ghost(
            id,
            label,
            act(cx, move |view, window, cx| {
                view.ask_confirm(confirm.clone(), window, cx)
            }),
        )
        .disabled(self.busy)
        .debug_selector(move || format!("{}-{}", id.0, id.1))
        .into_any_element()
    }

    /// The button next to a problem, if there is something to do about it.
    pub(super) fn problem_button(
        &self,
        row: usize,
        kind: &ProblemKind,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (action, label) = problem_action(kind)?;
        Some(
            kit::ghost(
                ("problem-action", row),
                label,
                act(cx, move |view, window, cx| {
                    view.run_problem(action, window, cx)
                }),
            )
            .disabled(self.busy)
            .debug_selector(move || format!("problem-action-{row}"))
            .into_any_element(),
        )
    }

    /// Does what a problem's button says.
    pub(super) fn run_problem(
        &mut self,
        action: ProblemAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            ProblemAction::Install => self.send(InstanceIntent::Install, window, cx),
            ProblemAction::Repair => self.send(InstanceIntent::Repair, window, cx),
            ProblemAction::InstallJava(major) => {
                self.send(InstanceIntent::InstallJava(major), window, cx)
            }
            ProblemAction::ChangeRuntime => {
                (self.handler)(InstanceIntent::OpenRuntimeChange, window, cx)
            }
            ProblemAction::Accounts => (self.handler)(InstanceIntent::OpenAccounts, window, cx),
            ProblemAction::Settings(group) => {
                self.select_settings_group(group, cx);
                self.open_tab(super::TAB_SETTINGS, window, cx);
            }
            ProblemAction::Content => self.open_tab(super::TAB_CONTENT, window, cx),
            ProblemAction::Logs => {
                self.diag_sub = 1;
                self.open_tab(super::TAB_DIAGNOSTICS, window, cx);
            }
        }
    }

    /// The latest sessions and changes (three each), each with a way to the
    /// whole history. Nothing at all while the history is unread or empty.
    pub(super) fn recent_block(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Some(Ok(read)) = &self.data.history else {
            return None;
        };
        let sessions = recent_sessions(read, 3);
        let changes = recent_changes(read, 3);
        if sessions.is_empty() && changes.is_empty() {
            return None;
        }
        let part = |title: &'static str,
                    id: &'static str,
                    sub: usize,
                    rows: Vec<Div>,
                    cx: &mut Context<Self>| {
            (!rows.is_empty()).then(|| {
                kit::section(
                    title,
                    colors,
                    v_flex().gap_2().child(kit::list(rows, colors)).child(
                        h_flex().justify_end().child(
                            kit::ghost(
                                id,
                                "查看全部",
                                act(cx, move |view, window, cx| {
                                    view.history_sub = sub;
                                    view.open_tab(super::TAB_HISTORY, window, cx);
                                }),
                            )
                            .debug_selector(move || id.into()),
                        ),
                    ),
                )
            })
        };
        let session_rows = sessions
            .iter()
            .map(|(started, seconds, outcome)| {
                kit::row(
                    outcome_label(*outcome),
                    format!("{} · {}", clock(*started), duration_label(*seconds)),
                    None,
                    None,
                    colors,
                )
            })
            .collect();
        let change_rows = changes
            .iter()
            .map(|(at, kind, subject)| {
                kit::row(
                    format!("{} {}", change_label(*kind), subject),
                    clock(*at),
                    None,
                    None,
                    colors,
                )
            })
            .collect();
        Some(
            v_flex()
                .w_full()
                .gap_5()
                .children(part(
                    "最近游玩",
                    "overview-all-sessions",
                    1,
                    session_rows,
                    cx,
                ))
                .children(part("最近变更", "overview-all-changes", 0, change_rows, cx))
                .into_any_element(),
        )
    }

    pub(super) fn problems_block(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(status) = self.status(&self.data.problems, colors, Section::Problems, cx) {
            // A failed problem scan must not crowd the overview.
            return matches!(self.data.problems, Some(Err(_))).then_some(status);
        }
        let Some(Ok(problems)) = &self.data.problems else {
            return None;
        };
        let rows: Vec<_> = problems
            .iter()
            .enumerate()
            .map(|(row, problem)| {
                let (title, detail) = problem_text(problem);
                kit::row(
                    title,
                    detail,
                    Some(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(problem_tone(problem.severity, colors))
                            .into_any_element(),
                    ),
                    self.problem_button(row, &problem.kind, cx),
                    colors,
                )
            })
            .collect();
        Some(if rows.is_empty() {
            kit::section(
                "状态",
                colors,
                kit::surface(colors).p_4().child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child("没有发现问题"),
                ),
            )
            .into_any_element()
        } else {
            kit::section("需要留意", colors, kit::list(rows, colors)).into_any_element()
        })
    }

    /// World archives dropped on the Worlds tab: one at a time, each its own
    /// write.
    pub(super) fn import_dropped(
        &mut self,
        paths: &[std::path::PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let zips: Vec<_> = paths
            .iter()
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
            })
            .collect();
        match zips.as_slice() {
            [] => self.toast(Toast::info("这里只能放世界的 .zip"), cx),
            [only] => self.send(InstanceIntent::AddWorld((*only).clone()), window, cx),
            [first, ..] => {
                self.toast(Toast::info("一次只导入一个世界，先导入了第一个"), cx);
                self.send(InstanceIntent::AddWorld((*first).clone()), window, cx);
            }
        }
    }

    pub(super) fn worlds_panel(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        if let Some(status) = self.status(&self.data.worlds, colors, Section::Worlds, cx) {
            return status;
        }
        let Some(Ok(worlds)) = &self.data.worlds else {
            return div().into_any_element();
        };
        let running = self.live_output.is_some();
        // While a game runs the instance is in use: nothing here can change.
        let busy = self.busy || running;
        let playing = self
            .game_started_ms
            .filter(|_| running)
            .and_then(|started| playing_world(worlds, started));
        let import = kit::action(
            "world-import",
            "导入世界",
            Some(UiIcon::Download),
            false,
            act(cx, |view, window, cx| {
                (view.handler)(InstanceIntent::ImportWorld, window, cx)
            }),
        )
        .disabled(busy)
        .debug_selector(|| "world-import".into());
        if worlds.is_empty() {
            return v_flex()
                .w_full()
                .gap_3()
                .child(h_flex().justify_end().child(import))
                .child(kit::empty(
                    "还没有世界",
                    "进入游戏创建的世界会出现在这里，也可以导入一个 .zip",
                    colors,
                ))
                .into_any_element();
        }
        let query = self
            .fields
            .as_ref()
            .map(|fields| fields.world_search.read(cx).value().to_string())
            .unwrap_or_default();
        let shown = ordered_worlds(worlds, &query, self.world_sort);
        let entity = cx.entity().downgrade();
        let rows: Vec<_> = shown
            .iter()
            .enumerate()
            .map(|(row, world)| {
                let mut detail = Vec::new();
                if playing == Some(world.folder.as_str()) {
                    detail.push("正在游玩".to_owned());
                }
                if world.damaged {
                    detail.push("存档信息无法读取".to_owned());
                }
                if world.hardcore {
                    detail.push("极限模式".to_owned());
                }
                if let Some(version) = &world.game_version {
                    detail.push(version.clone());
                }
                if let Some(played) = world.last_played_ms.filter(|ms| *ms > 0) {
                    detail.push(format!("{}游玩", clock(played as u64 / 1000)));
                }
                if detail.is_empty() {
                    detail.push(world.folder.clone());
                }
                let folder = world.folder.clone();
                let too_old = self.record.as_ref().is_some_and(|record| {
                    lumilio_core::quick_play_world_unsupported(&record.game_version)
                });
                let enter = {
                    let folder = folder.clone();
                    let button = kit::ghost(
                        ("world-play", row),
                        "进入",
                        act(cx, move |view, window, cx| {
                            view.send(InstanceIntent::PlayWorld(folder.clone()), window, cx)
                        }),
                    )
                    .disabled(busy || world.damaged || too_old)
                    .debug_selector(move || format!("world-play-{row}"));
                    if too_old {
                        button.tooltip("这个游戏版本不能直接进入世界，请从主菜单进入")
                    } else {
                        button
                    }
                };
                // Writes go through `send` (one at a time); the rest hand the
                // application a request it answers itself.
                let entry = |label: &'static str, intent: InstanceIntent, write: bool| {
                    let entity = entity.clone();
                    kit::MenuEntry::new(label, move |window, app| {
                        let intent = intent.clone();
                        let _ = entity.update(app, |view, cx| {
                            if write {
                                view.send(intent, window, cx);
                            } else {
                                (view.handler)(intent, window, cx);
                            }
                        });
                    })
                    .disabled(busy)
                };
                let menu = kit::more_menu(
                    ("world-more", row),
                    vec![
                        entry("复制", InstanceIntent::CopyWorld(folder.clone()), true),
                        entry(
                            "创建备份",
                            InstanceIntent::BackupWorld(folder.clone()),
                            true,
                        ),
                        entry(
                            "导出为 .zip…",
                            InstanceIntent::ExportWorld(folder.clone()),
                            false,
                        ),
                        entry(
                            "在访达中显示",
                            InstanceIntent::RevealPath(format!("saves/{folder}")),
                            false,
                        ),
                    ],
                    colors,
                );
                let trail = h_flex()
                    .gap_1()
                    .items_center()
                    .child(enter)
                    .child(self.asking(
                        ("world-delete", row),
                        "删除",
                        Confirm::DeleteWorld(folder),
                        cx,
                    ))
                    .child(menu);
                kit::row(
                    world.name.clone(),
                    detail.join(" · "),
                    None,
                    Some(trail.into_any_element()),
                    colors,
                )
            })
            .collect();
        let controls = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .child(kit::segments(
                "instance-world-sort",
                &WORLD_SORTS,
                self.world_sort.min(WORLD_SORTS.len() - 1),
                act_index(cx, |view, index: usize, _, cx| {
                    view.world_sort = index;
                    cx.notify();
                }),
            ))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .children(self.fields.as_ref().map(|fields| {
                        div().w(px(220.)).child(
                            Input::new(&fields.world_search).small().prefix(
                                Icon::new(UiIcon::Search)
                                    .size(px(14.))
                                    .text_color(colors.muted),
                            ),
                        )
                    }))
                    .child(import),
            );
        v_flex()
            .id("instance-worlds-drop")
            .w_full()
            .gap_3()
            .on_drop(
                cx.listener(|view, paths: &gpui::ExternalPaths, window, cx| {
                    view.import_dropped(paths.paths(), window, cx)
                }),
            )
            .child(controls)
            .child(div().text_sm().text_color(colors.muted).child(if running {
                "游戏正在运行，世界先不能复制、备份、导出或删除；结束游戏后再来。"
            } else {
                "复制和导入总是另存为新世界，不会覆盖已有的世界。游戏运行时不能修改。"
            }))
            .child(if rows.is_empty() {
                kit::empty("没有匹配的世界", "换个关键词试试", colors).into_any_element()
            } else {
                kit::list(rows, colors).into_any_element()
            })
            .into_any_element()
    }

    pub(super) fn history_panel(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let sub = self.history_sub.min(2);
        let segments = kit::segments(
            "instance-history-kinds",
            &HISTORY_LABELS,
            sub,
            act_index(cx, |view, index: usize, window, cx| {
                view.history_sub = index;
                view.confirm = None;
                let section = if index == 2 {
                    Section::Snapshots
                } else {
                    Section::History
                };
                view.ensure(section, window, cx);
                cx.notify();
            }),
        );
        let body = if sub == 2 {
            self.snapshots_body(colors, cx)
        } else {
            self.events_body(sub == 1, colors, cx)
        };
        v_flex()
            .w_full()
            .gap_4()
            .child(h_flex().child(segments))
            .child(body)
            .into_any_element()
    }

    fn events_body(
        &self,
        sessions: bool,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(status) = self.status(&self.data.history, colors, Section::History, cx) {
            return status;
        }
        let Some(Ok(read)) = &self.data.history else {
            return div().into_any_element();
        };
        let rows: Vec<_> = read
            .events
            .iter()
            .rev()
            .filter_map(|event| match (event, sessions) {
                (
                    HistoryEvent::Session {
                        started,
                        seconds,
                        outcome,
                        ..
                    },
                    true,
                ) => {
                    // A session that went wrong points at where to find out why.
                    let (started, seconds) = (*started, *seconds);
                    let inspect = matches!(
                        outcome,
                        SessionOutcome::Crashed | SessionOutcome::FailedToStart
                    )
                    .then(|| {
                        kit::ghost(
                            ("session-inspect", started as usize),
                            "查看日志",
                            act(cx, move |view, window, cx| {
                                view.inspect_session(started, seconds, window, cx)
                            }),
                        )
                        .debug_selector(|| "session-inspect".into())
                        .into_any_element()
                    });
                    Some(kit::row(
                        outcome_label(*outcome),
                        format!("{} · {}", clock(started), duration_label(seconds)),
                        matches!(
                            outcome,
                            SessionOutcome::Crashed | SessionOutcome::FailedToStart
                        )
                        .then(|| {
                            div()
                                .size(px(8.))
                                .rounded_full()
                                .bg(colors.danger)
                                .into_any_element()
                        }),
                        inspect,
                        colors,
                    ))
                }
                (HistoryEvent::Change { at, kind, subject }, false) => Some(kit::row(
                    format!("{} {}", change_label(*kind), subject),
                    clock(*at),
                    None,
                    None,
                    colors,
                )),
                _ => None,
            })
            .collect();
        let skipped = read.skipped;
        v_flex()
            .w_full()
            .gap_3()
            .children((skipped > 0).then(|| {
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(format!("有 {skipped} 行记录无法读取，已跳过"))
            }))
            .child(if rows.is_empty() {
                kit::empty(
                    if sessions {
                        "还没有游玩记录"
                    } else {
                        "还没有变更记录"
                    },
                    "",
                    colors,
                )
                .into_any_element()
            } else {
                kit::list(rows, colors).into_any_element()
            })
            .into_any_element()
    }

    fn snapshots_body(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy;
        let create = h_flex().child(
            kit::action(
                "snapshot-create",
                "现在创建快照",
                Some(UiIcon::Plus),
                true,
                act(cx, |view, window, cx| {
                    view.open_editor(super::editors::Editor::Snapshot, window, cx)
                }),
            )
            .disabled(busy)
            .debug_selector(|| "snapshot-create".into()),
        );
        let list = if let Some(status) =
            self.status(&self.data.snapshots, colors, Section::Snapshots, cx)
        {
            status
        } else {
            let Some(Ok(snapshots)) = &self.data.snapshots else {
                return div().into_any_element();
            };
            if snapshots.is_empty() {
                kit::empty(
                    "还没有快照",
                    "快照保存所有世界和游戏设置，恢复失败会自动回到恢复前",
                    colors,
                )
                .into_any_element()
            } else {
                let rows: Vec<_> = snapshots
                    .iter()
                    .enumerate()
                    .map(|(row, snapshot)| {
                        let scope = match &snapshot.scope {
                            SnapshotScope::Full => "全部世界与设置".to_owned(),
                            SnapshotScope::World(name) => format!("世界 {name}"),
                        };
                        let trail = h_flex()
                            .gap_1()
                            .child(self.asking(
                                ("snapshot-restore", row),
                                "恢复",
                                Confirm::RestoreSnapshot(snapshot.id.clone()),
                                cx,
                            ))
                            .child(self.asking(
                                ("snapshot-delete", row),
                                "删除",
                                Confirm::DeleteSnapshot(snapshot.id.clone()),
                                cx,
                            ));
                        kit::row(
                            snapshot.label.clone(),
                            format!(
                                "{} · {} · {}",
                                clock(snapshot.created),
                                scope,
                                size_label(snapshot.size)
                            ),
                            None,
                            Some(trail.into_any_element()),
                            colors,
                        )
                    })
                    .collect();
                kit::list(rows, colors).into_any_element()
            }
        };
        v_flex()
            .w_full()
            .gap_3()
            .child(create)
            .child(list)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(folder: &str, name: &str, played: Option<i64>) -> WorldInfo {
        WorldInfo {
            folder: folder.into(),
            name: name.into(),
            last_played_ms: played,
            game_version: None,
            hardcore: false,
            has_icon: false,
            damaged: false,
            lock_touched_ms: None,
        }
    }

    #[test]
    fn worlds_are_searched_by_name_or_folder_and_sorted_by_play_time_or_name() {
        let worlds = vec![
            world("a", "Zeta", Some(30)),
            world("b", "alpha", Some(10)),
            world("c-folder", "Mid", None),
        ];
        let names = |shown: Vec<&WorldInfo>| -> Vec<String> {
            shown.iter().map(|world| world.name.clone()).collect()
        };
        assert_eq!(
            names(ordered_worlds(&worlds, "", 0)),
            ["Zeta", "alpha", "Mid"]
        );
        assert_eq!(
            names(ordered_worlds(&worlds, "", 1)),
            ["alpha", "Mid", "Zeta"]
        );
        assert_eq!(names(ordered_worlds(&worlds, "ALP", 0)), ["alpha"]);
        assert_eq!(names(ordered_worlds(&worlds, " c-fol ", 0)), ["Mid"]);
        assert!(ordered_worlds(&worlds, "nothing", 0).is_empty());
    }

    #[test]
    fn an_export_says_what_was_listed_and_what_was_carried() {
        let path = std::path::Path::new("/tmp/pack.mrpack");
        let report = lumilio_core::ExportReport {
            linked: 3,
            bundled: 2,
            lookup_failed: false,
            size: 10,
        };
        let text = export_notice(&report, path);
        assert!(text.contains("/tmp/pack.mrpack") && text.contains("3 个文件按地址列出"));
        assert!(text.contains("2 个放在包里"));
        let offline = lumilio_core::ExportReport {
            linked: 0,
            bundled: 5,
            lookup_failed: true,
            size: 10,
        };
        let text = export_notice(&offline, path);
        assert!(text.contains("没能连上 Modrinth") && text.contains("5 个文件"));
    }

    #[test]
    fn a_confirmation_names_what_it_will_remove_and_tolerates_a_list_not_loaded() {
        let worlds = [world("w1", "Skyblock", Some(1))];
        let snapshots = [SnapshotInfo {
            id: "s1".into(),
            label: "before update".into(),
            scope: SnapshotScope::Full,
            created: 1,
            size: 10,
        }];
        let (title, _, ok) = Confirm::DeleteWorld("w1".into()).words(&worlds, &snapshots);
        assert!(title.contains("Skyblock") && ok == "删除");
        let (title, description, ok) =
            Confirm::RestoreSnapshot("s1".into()).words(&worlds, &snapshots);
        assert!(title.contains("before update") && ok == "恢复");
        assert!(description.contains("自动存一份"));
        let (title, _, _) = Confirm::DeleteSnapshot("s1".into()).words(&worlds, &snapshots);
        assert!(title.contains("before update"));
        // Not loaded: it still asks, by folder or in general terms.
        assert!(
            Confirm::DeleteWorld("x".into())
                .words(&[], &[])
                .0
                .contains('x')
        );
        assert!(
            Confirm::DeleteSnapshot("?".into())
                .words(&[], &[])
                .0
                .contains("这份快照")
        );
    }

    #[test]
    fn every_problem_with_something_to_do_names_it() {
        use lumilio_core::Loader;
        let cases = [
            (ProblemKind::NotInstalled, ProblemAction::Install),
            (
                ProblemKind::LoaderUnsupported(Loader::Forge),
                ProblemAction::ChangeRuntime,
            ),
            (
                ProblemKind::NoJava { required: Some(21) },
                ProblemAction::InstallJava(Some(21)),
            ),
            (ProblemKind::NoAccount, ProblemAction::Accounts),
            (
                ProblemKind::DamagedFiles { count: 3 },
                ProblemAction::Repair,
            ),
            (
                ProblemKind::WrongLoaderMods { names: vec![] },
                ProblemAction::Content,
            ),
            (
                ProblemKind::DuplicateMods { ids: vec![] },
                ProblemAction::Content,
            ),
            (
                ProblemKind::LowMemory { max_mb: 512 },
                ProblemAction::Settings(3),
            ),
            (
                ProblemKind::LastSessionFailed(SessionOutcome::Crashed),
                ProblemAction::Logs,
            ),
        ];
        for (kind, action) in cases {
            let (found, label) = problem_action(&kind).unwrap();
            assert_eq!(found, action);
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn the_world_being_played_is_the_newest_one_opened_since_the_game_started() {
        let mut worlds = vec![
            world("a", "A", None),
            world("b", "B", None),
            world("c", "C", None),
        ];
        assert_eq!(
            playing_world(&worlds, 1_000),
            None,
            "no lock, nothing opened"
        );
        worlds[0].lock_touched_ms = Some(500); // from an earlier session
        worlds[1].lock_touched_ms = Some(1_200);
        worlds[2].lock_touched_ms = Some(1_900);
        assert_eq!(playing_world(&worlds, 1_000), Some("c"));
        assert_eq!(playing_world(&worlds, 2_000), None);
        assert_eq!(playing_world(&worlds, 0), Some("c"));
    }

    #[test]
    fn the_overview_shows_the_newest_three_of_each_kind_newest_first() {
        let session = |started| HistoryEvent::Session {
            started,
            seconds: 60,
            exit_code: None,
            outcome: SessionOutcome::Clean,
        };
        let change = |at: u64| HistoryEvent::Change {
            at,
            kind: ChangeKind::ContentAdded,
            subject: format!("m{at}"),
        };
        let read = HistoryRead {
            events: vec![
                session(1),
                change(2),
                session(3),
                session(4),
                change(5),
                session(6),
                change(7),
                change(8),
            ],
            skipped: 0,
        };
        let starts: Vec<_> = recent_sessions(&read, 3).iter().map(|s| s.0).collect();
        assert_eq!(starts, [6, 4, 3]);
        let changes: Vec<_> = recent_changes(&read, 3).iter().map(|c| c.0).collect();
        assert_eq!(changes, [8, 7, 5]);
        assert!(recent_sessions(&HistoryRead::default(), 3).is_empty());
    }

    #[test]
    fn durations_and_sizes_read_naturally() {
        assert_eq!(duration_label(10), "不到 1 分钟");
        assert_eq!(duration_label(125), "2 分钟");
        assert_eq!(duration_label(3 * 3600 + 5 * 60), "3 小时 5 分钟");
        assert_eq!(size_label(0), "—");
        assert_eq!(size_label(900), "900 B");
        assert_eq!(size_label(2048), "2 KB");
        assert_eq!(size_label(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn every_outcome_and_change_has_copy_and_unknown_results_say_so() {
        assert!(outcome_label(SessionOutcome::Interrupted).contains("未知"));
        assert_eq!(change_label(ChangeKind::SnapshotRestored), "恢复了快照");
    }

    #[test]
    fn problems_name_their_cause_without_raw_debug_text() {
        let text = |kind| {
            problem_text(&Problem {
                severity: Severity::Warning,
                kind,
            })
        };
        let (title, detail) = text(ProblemKind::NoJava { required: Some(21) });
        assert!(title.contains("Java") && detail.contains("21"));
        let (_, detail) = text(ProblemKind::DamagedFiles { count: 3 });
        assert!(detail.contains('3'));
        let (title, _) = text(ProblemKind::LastSessionFailed(SessionOutcome::Crashed));
        assert!(!title.contains("Crashed"));
    }

    #[test]
    fn store_clears_pending_and_reports_presence() {
        let mut data = Data::default();
        data.pending.push(Section::Worlds);
        assert!(!data.has(Section::Worlds));
        data.store(Arrived::Worlds(Ok(Vec::new())));
        assert!(data.has(Section::Worlds) && data.pending.is_empty());
        data.store(Arrived::Content(ProjectKind::Shader, Err("x".into())));
        assert!(data.has(Section::Content(ProjectKind::Shader)));
        assert!(!data.has(Section::Content(ProjectKind::Mod)));
    }
}
