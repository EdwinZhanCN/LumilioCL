use super::super::{InstanceDetailView, InstanceIntent, Section};
use super::helpers::{act, clock};
use super::labels::{change_label, duration_label, outcome_label};
use crate::theme::ShellColors;
use crate::{kit, live, tr};
use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, Window, div, px};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{
    ChangeKind, HistoryEvent, HistoryRead, Problem, ProblemKind, SessionOutcome, Severity,
};

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
        ProblemKind::NotInstalled => (ProblemAction::Install, tr!("instance-action-install")),
        ProblemKind::LoaderUnsupported(_) => {
            (ProblemAction::ChangeRuntime, tr!("instance-action-change"))
        }
        ProblemKind::NoJava { required } => (
            ProblemAction::InstallJava(*required),
            tr!("instance-action-install-java"),
        ),
        ProblemKind::NoAccount => (ProblemAction::Accounts, tr!("instance-action-add-account")),
        ProblemKind::DamagedFiles { .. } => (ProblemAction::Repair, tr!("instance-action-repair")),
        ProblemKind::WrongLoaderMods { .. } | ProblemKind::DuplicateMods { .. } => {
            (ProblemAction::Content, tr!("instance-action-go-content"))
        }
        ProblemKind::LowMemory { .. } => {
            (ProblemAction::Settings(3), tr!("instance-action-go-adjust"))
        }
        ProblemKind::LastSessionFailed(_) | ProblemKind::Finding(_) => {
            (ProblemAction::Logs, tr!("instance-view-logs"))
        }
    })
}

/// One line per problem; the headline is calm, the cause stays short.
pub fn problem_text(problem: &Problem) -> (String, String) {
    match &problem.kind {
        ProblemKind::NotInstalled => (
            tr!("instance-problem-not-installed").into(),
            tr!("instance-problem-not-installed-help").into(),
        ),
        ProblemKind::LoaderUnsupported(loader) => (
            tr!(
                "instance-problem-loader-unsupported",
                loader = live::loader_label(*loader)
            ),
            tr!("instance-problem-loader-unsupported-help").into(),
        ),
        ProblemKind::NoJava { required } => (
            tr!("instance-problem-no-java").into(),
            match required {
                Some(major) => tr!("instance-problem-java-required", major = *major),
                None => tr!("instance-problem-install-java-help").into(),
            },
        ),
        ProblemKind::NoAccount => (
            tr!("instance-problem-no-account").into(),
            tr!("instance-problem-no-account-help").into(),
        ),
        ProblemKind::DamagedFiles { count } => (
            tr!("instance-problem-damaged").into(),
            tr!("instance-problem-damaged-help", count = *count),
        ),
        ProblemKind::WrongLoaderMods { names } => (
            tr!("instance-problem-wrong-loader").into(),
            names.join(tr!("common-list-separator")),
        ),
        ProblemKind::DuplicateMods { ids } => (
            tr!("instance-problem-duplicate-mods").into(),
            ids.join(tr!("common-list-separator")),
        ),
        ProblemKind::LowMemory { max_mb } => (
            tr!("instance-problem-low-memory").into(),
            tr!("instance-problem-low-memory-help", max = *max_mb),
        ),
        ProblemKind::LastSessionFailed(outcome) => (
            tr!("instance-problem-last-session").into(),
            outcome_label(*outcome).into(),
        ),
        ProblemKind::Finding(result) => {
            (result.finding.title.clone(), result.finding.advice.clone())
        }
    }
}

pub fn problem_tone(severity: Severity, colors: ShellColors) -> gpui::Hsla {
    match severity {
        Severity::Error => colors.danger,
        Severity::Warning => kit::tone_warn(),
        Severity::Info => colors.muted,
    }
}

impl InstanceDetailView {
    /// The button next to a problem, if there is something to do about it.
    pub(in super::super) fn problem_button(
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
    pub(in super::super) fn run_problem(
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
                self.open_tab(super::super::TAB_SETTINGS, window, cx);
            }
            ProblemAction::Content => self.open_tab(super::super::TAB_CONTENT, window, cx),
            ProblemAction::Logs => {
                self.diag_sub = 1;
                self.open_tab(super::super::TAB_DIAGNOSTICS, window, cx);
            }
        }
    }

    /// The latest sessions and changes (three each), each with a way to the
    /// whole history. Nothing at all while the history is unread or empty.
    // ia[instance.overview]: 最近游玩 / 变更 | 概览里的两个只读摘要，各 3 条 | “查看全部”进历史分段
    pub(in super::super) fn recent_block(
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
                                tr!("instance-history-view-all"),
                                act(cx, move |view, window, cx| {
                                    view.history_sub = sub;
                                    view.open_tab(super::super::TAB_HISTORY, window, cx);
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
                    tr!("library-sort-recent"),
                    "overview-all-sessions",
                    1,
                    session_rows,
                    cx,
                ))
                .children(part(
                    tr!("instance-overview-recent-changes"),
                    "overview-all-changes",
                    0,
                    change_rows,
                    cx,
                ))
                .into_any_element(),
        )
    }

    // ia[instance.overview]: 解决问题 | 概览「需要留意」里每个问题行的按钮 | 安装/修复/更换版本，或跳到设置·Java、设置·性能、内容、账户、诊断·日志
    pub(in super::super) fn problems_block(
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
                tr!("instance-problems-status"),
                colors,
                kit::surface(colors).p_4().child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(tr!("instance-problems-none")),
                ),
            )
            .into_any_element()
        } else {
            kit::section(tr!("home-attention"), colors, kit::list(rows, colors)).into_any_element()
        })
    }
}
