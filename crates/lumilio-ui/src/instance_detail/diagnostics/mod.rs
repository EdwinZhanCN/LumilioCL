//! The Instance page's Diagnostics tab (IA `instance/diagnostics.md`):
//! problems, the game log with a level filter and search, crash reports, and
//! a read-only browser of the game directory.
//!
//! A child module of `instance_detail`, so it shares the view's private state.

mod files;
mod helpers;
mod logs;

#[cfg(test)]
mod tests;

pub use self::helpers::{DIAGNOSTIC_LABELS, crash_for_session, least_level};

use super::panels::{act_index, problem_text, problem_tone};
use super::{InstanceDetailView, InstanceIntent, Section};
use crate::kit;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div, px};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{filter_log, log_lines};

impl InstanceDetailView {
    /// Opening the tab loads what its current part shows.
    pub(super) fn open_diagnostics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let section = match self.diag_sub {
            0 => Section::Problems,
            1 => Section::Logs,
            _ => Section::Files,
        };
        self.ensure(section, window, cx);
    }

    /// Opens 诊断 on its log part for a session that went wrong. If the game
    /// left a crash report from that time, it is opened too (once the list of
    /// reports is known).
    pub(super) fn inspect_session(
        &mut self,
        started: u64,
        seconds: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.diag_sub = 1;
        self.inspect = Some((started, seconds));
        self.open_tab(super::TAB_DIAGNOSTICS, window, cx);
        self.open_inspected_crash(window, cx);
    }

    /// Opens the crash report that belongs to the session being inspected,
    /// when the reports are known. Called again when they arrive.
    pub(super) fn open_inspected_crash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((started, seconds)) = self.inspect else {
            return;
        };
        let Some(Ok(logs)) = &self.data.logs else {
            return;
        };
        self.inspect = None;
        if let Some(report) = crash_for_session(&logs.crashes, started, seconds) {
            let file = report.file_name.clone();
            self.crash = Some((file.clone(), None));
            cx.notify();
            (self.handler)(InstanceIntent::OpenCrash(file), window, cx);
        }
    }

    /// Lists another folder of the game directory; one request at a time.
    pub(super) fn open_folder(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.data.pending.contains(&Section::Files) {
            return;
        }
        self.data.pending.push(Section::Files);
        (self.handler)(InstanceIntent::OpenFolder(path), window, cx);
    }

    fn log_query(&self, cx: &gpui::App) -> String {
        self.fields
            .as_ref()
            .map(|fields| fields.log_search.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// The log being read: the game's own output while it runs, else the tail
    /// of `latest.log`.
    fn log_text(&self) -> Option<std::borrow::Cow<'_, str>> {
        if let Some(live) = &self.live_output {
            return Some(std::borrow::Cow::Owned(live.join("\n")));
        }
        let Some(Ok(logs)) = &self.data.logs else {
            return None;
        };
        logs.latest.as_deref().map(std::borrow::Cow::Borrowed)
    }

    /// The log lines that match the level and search, as one text.
    pub(super) fn visible_log(&self, cx: &gpui::App) -> Option<String> {
        let text = self.log_text()?;
        let lines = log_lines(&text);
        let shown = filter_log(&lines, least_level(self.log_level), &self.log_query(cx));
        Some(
            shown
                .iter()
                .map(|line| line.text)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    pub(super) fn diagnostics_panel(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sub = self.diag_sub.min(DIAGNOSTIC_LABELS.len() - 1);
        let segments = kit::segments(
            "instance-diagnostic-parts",
            &DIAGNOSTIC_LABELS,
            sub,
            act_index(cx, |view, index: usize, window, cx| {
                view.diag_sub = index;
                cx.notify();
                view.open_diagnostics(window, cx);
            }),
        );
        let body = match sub {
            0 => self.problems_body(colors, cx),
            1 => self.logs_body(colors, cx),
            _ => self.files_body(colors, cx),
        };
        v_flex()
            .w_full()
            .gap_4()
            .child(h_flex().child(segments))
            .child(body)
            .into_any_element()
    }

    // ia[instance.diagnostics]: 看问题、执行修复 | 诊断 · 问题分段 | 每个问题一个操作（与概览相同）
    fn problems_body(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        if let Some(status) = self.status(&self.data.problems, colors, Section::Problems, cx) {
            return status;
        }
        let Some(Ok(problems)) = &self.data.problems else {
            return div().into_any_element();
        };
        if problems.is_empty() {
            return kit::empty("没有发现问题", "游戏现在看起来一切正常", colors).into_any_element();
        }
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
        kit::list(rows, colors).into_any_element()
    }
}
