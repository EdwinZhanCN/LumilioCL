//! The Instance page's Diagnostics tab (IA `instance/diagnostics.md`):
//! problems, the game log with a level filter and search, crash reports, and
//! a read-only browser of the game directory.
//!
//! A child module of `instance_detail`, so it shares the view's private state.

use gpui::{AnyElement, Context, IntoElement, Window, div, prelude::*, px};
use gpui_component::input::Input;
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};
use lumilio_core::{FileEntry, LogLevel, LogLine, filter_log, log_lines};

use super::panels::{act, act_index, clock, hint_text, problem_text, problem_tone, size_label};
use super::{InstanceDetailView, InstanceIntent, Section};
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::ShellColors;
use crate::toast::Toast;

pub const DIAGNOSTIC_LABELS: [&str; 3] = ["问题", "日志", "文件"];
pub const LEVEL_LABELS: [&str; 4] = ["全部", "错误", "警告", "信息"];
/// The log view stays light: the newest lines of what matches.
const LOG_LINES_SHOWN: usize = 300;

/// The least serious level a [`LEVEL_LABELS`] choice still shows.
pub fn least_level(choice: usize) -> Option<LogLevel> {
    match choice {
        1 => Some(LogLevel::Error),
        2 => Some(LogLevel::Warn),
        3 => Some(LogLevel::Info),
        _ => None,
    }
}

/// The entries whose name contains `query` (ignoring case); all when empty.
pub fn files_matching<'a>(entries: &'a [FileEntry], query: &str) -> Vec<&'a FileEntry> {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
        .collect()
}

/// How long after a session's end a crash report still belongs to it.
const CRASH_GRACE: u64 = 120;

/// The crash report written during a session: the one modified closest to
/// its end, between its start and shortly after.
pub fn crash_for_session(
    reports: &[lumilio_core::CrashReport],
    started: u64,
    seconds: u64,
) -> Option<&lumilio_core::CrashReport> {
    let end = started.saturating_add(seconds);
    reports
        .iter()
        .filter(|report| report.modified >= started && report.modified <= end + CRASH_GRACE)
        .min_by_key(|report| report.modified.abs_diff(end))
}

/// `folder` and `name` as one path below the game directory.
pub fn join_path(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    }
}

/// The folder one level up (`""` at the top).
pub fn parent_path(folder: &str) -> String {
    folder
        .rsplit_once('/')
        .map_or_else(String::new, |(parent, _)| parent.to_owned())
}

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

    // ia[instance.diagnostics]: 看问题、执行修复 | 诊断 · 问题分段 | 每个问题一个操作（与概览相同） | L-DIAG-01
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

    fn logs_body(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        if let Some(status) = self.status(&self.data.logs, colors, Section::Logs, cx) {
            return status;
        }
        let Some(Ok(logs)) = &self.data.logs else {
            return div().into_any_element();
        };
        let mono = |text: String| {
            div()
                .w_full()
                .max_h(px(320.))
                .overflow_hidden()
                .text_xs()
                .font_family("Menlo")
                .text_color(colors.muted)
                .child(text)
        };
        // ia[instance.diagnostics]: 实时日志（游戏运行中） | 诊断 · 日志分段 | 本启动器启动的游戏运行时显示它正在输出的内容（最近 2000 行），自动跟随末尾；游戏结束后改读 latest.log；启动器之外启动的游戏没有实时输出 | H-PLAY-09
        let running = self.live_output.is_some();
        if running {
            // Following the end of the output.
            self.log_scroll.scroll_to_bottom();
        }
        let latest = match self.log_text() {
            Some(text) => {
                let lines = log_lines(&text);
                let matched = filter_log(&lines, least_level(self.log_level), &self.log_query(cx));
                let start = matched.len().saturating_sub(LOG_LINES_SHOWN);
                let note = if matched.is_empty() {
                    "没有符合条件的日志行".to_owned()
                } else if start > 0 {
                    format!(
                        "符合条件的共 {} 行，显示最后 {LOG_LINES_SHOWN} 行",
                        matched.len()
                    )
                } else {
                    format!("共 {} 行", matched.len())
                };
                kit::surface(colors)
                    .p_4()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(kit::keep_wheel(
                                div()
                                    .id("instance-log-lines")
                                    .debug_selector(|| "instance-log-lines".into())
                                    .w_full()
                                    .max_h(px(360.))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.log_scroll)
                                    .text_xs()
                                    .font_family("Menlo")
                                    .children(
                                        matched[start..].iter().map(|line| log_line(line, colors)),
                                    ),
                                &self.log_scroll,
                            ))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .children(running.then(|| {
                                        kit::chip(
                                            "正在运行 · 跟随输出",
                                            Some(kit::tone_ok()),
                                            colors,
                                        )
                                    }))
                                    .child(div().text_xs().text_color(colors.muted).child(note)),
                            ),
                    )
                    .into_any_element()
            }
            None => kit::empty("还没有日志", "游戏运行过一次之后会出现在这里", colors)
                .into_any_element(),
        };
        let controls = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            // ia[instance.diagnostics]: 按级别筛选 | 日志分段 · 分段 | 视图状态；级别＝该级别及更严重，堆栈行跟随上一行的级别 | H-PLAY-09
            .child(kit::segments(
                "instance-log-levels",
                &LEVEL_LABELS,
                self.log_level.min(LEVEL_LABELS.len() - 1),
                act_index(cx, |view, index: usize, _, cx| {
                    view.log_level = index;
                    cx.notify();
                }),
            ))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .children(self.fields.as_ref().map(|fields| {
                        // ia[instance.diagnostics]: 搜索日志 | 日志分段 · 搜索框 | 视图状态 | H-PLAY-09
                        div().w(px(220.)).child(
                            Input::new(&fields.log_search).small().prefix(
                                Icon::new(UiIcon::Search)
                                    .size(px(14.))
                                    .text_color(colors.muted),
                            ),
                        )
                    }))
                    .child(
                        // ia[instance.diagnostics]: 复制日志 | 日志分段 · 按键「复制」 | 复制当前筛选出的行；没有行时提示 | H-PLAY-09
                        kit::ghost(
                            "instance-log-copy",
                            "复制",
                            act(cx, |view, _, cx| {
                                let Some(text) = view.visible_log(cx).filter(|t| !t.is_empty())
                                else {
                                    view.toast(Toast::error("没有可复制的日志行"), cx);
                                    return;
                                };
                                crate::toast::copy_text(text, cx);
                                view.toast(Toast::success("已复制日志"), cx);
                            }),
                        )
                        .disabled(!running && logs.latest.is_none()),
                    )
                    .child(
                        // ia[instance.diagnostics]: 导出日志 | 日志分段 · 按键「导出…」→ 选位置 | 保存最新日志或打开着的崩溃报告；玩家名、UUID、启动器目录、游戏目录、用户主目录都换成占位符 | H-PLAY-09、H-SET-15
                        kit::ghost(
                            "instance-log-export",
                            "导出…",
                            act(cx, |view, window, cx| {
                                (view.handler)(InstanceIntent::ExportLog(None), window, cx)
                            }),
                        )
                        .disabled(logs.latest.is_none())
                        .debug_selector(|| "instance-log-export".into()),
                    ),
            );
        let crashes: Vec<_> = logs
            .crashes
            .iter()
            .enumerate()
            .map(|(row, report)| {
                let file = report.file_name.clone();
                kit::row(
                    file.clone(),
                    clock(report.modified),
                    None,
                    Some(
                        kit::ghost(
                            ("crash-open", row),
                            "查看",
                            act(cx, move |view, window, cx| {
                                view.crash = Some((file.clone(), None));
                                cx.notify();
                                (view.handler)(InstanceIntent::OpenCrash(file.clone()), window, cx);
                            }),
                        )
                        .debug_selector(|| "crash-open".into())
                        .into_any_element(),
                    ),
                    colors,
                )
            })
            .collect();
        let opened = self.crash.as_ref().map(|(file, read)| match read {
            None => div()
                .text_sm()
                .text_color(colors.muted)
                .child(format!("正在读取 {file}…"))
                .into_any_element(),
            Some(Err(detail)) => h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child("没有读到这份崩溃报告"),
                )
                .child(kit::technical("crash-technical", detail.clone()))
                .into_any_element(),
            Some(Ok((text, hints))) => v_flex()
                .gap_3()
                .child({
                    let file = file.clone();
                    h_flex().child(
                        kit::ghost(
                            "crash-export",
                            "导出这份报告…",
                            act(cx, move |view, window, cx| {
                                (view.handler)(
                                    InstanceIntent::ExportLog(Some(file.clone())),
                                    window,
                                    cx,
                                )
                            }),
                        )
                        .debug_selector(|| "crash-export".into()),
                    )
                })
                .children(hints.iter().map(|hint| {
                    kit::surface(colors).p_3().child(
                        div()
                            .text_sm()
                            .text_color(colors.foreground)
                            // ia[instance.diagnostics]: 崩溃原因识别 | 崩溃报告上方的原因卡片 | 原因 + 建议操作（CrashHint） | H-PLAY-10
                            .child(hint_text(*hint)),
                    )
                }))
                .child(
                    kit::surface(colors)
                        .p_4()
                        .child(mono(text.lines().take(60).collect::<Vec<_>>().join("\n"))),
                )
                .into_any_element(),
        });
        v_flex()
            .w_full()
            .gap_5()
            .child(kit::section(
                // ia[instance.diagnostics]: 看最新日志 / 崩溃报告 | 日志分段 · 来源选择：最近的日志 / 崩溃报告列表 | 选哪份看哪份；崩溃报告「查看」打开 | H-PLAY-10
                "最近的日志",
                colors,
                v_flex().gap_3().child(controls).child(latest),
            ))
            .child(kit::section(
                "崩溃报告",
                colors,
                if crashes.is_empty() {
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child("没有崩溃报告")
                        .into_any_element()
                } else {
                    kit::list(crashes, colors).into_any_element()
                },
            ))
            .children(opened)
            .into_any_element()
    }

    // ia[instance.diagnostics]: 浏览文件 | 诊断 · 文件分段 | 逐级打开的只读文件夹列表，只在游戏目录内；右上角按名字搜当前文件夹 | H-INSTANCE-10
    fn files_body(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        if let Some(status) = self.status(&self.data.files, colors, Section::Files, cx) {
            return status;
        }
        let Some(Ok((folder, entries))) = &self.data.files else {
            return div().into_any_element();
        };
        let trail = if folder.is_empty() {
            "游戏目录".to_owned()
        } else {
            format!("游戏目录 / {}", folder.replace('/', " / "))
        };
        let (here, up) = (folder.clone(), parent_path(folder));
        let header = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .children((!folder.is_empty()).then(|| {
                        kit::ghost(
                            "instance-files-up",
                            "上一级",
                            act(cx, move |view, window, cx| {
                                view.open_folder(up.clone(), window, cx)
                            }),
                        )
                    }))
                    .child(div().text_sm().text_color(colors.muted).child(trail)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .children(self.fields.as_ref().map(|fields| {
                        div().w(px(200.)).child(
                            Input::new(&fields.file_search).small().prefix(
                                Icon::new(UiIcon::Search)
                                    .size(px(14.))
                                    .text_color(colors.muted),
                            ),
                        )
                    }))
                    // ia[instance.diagnostics]: 在访达中显示 | 文件分段 · 按键 | 当前文件夹或所选文件 | H-INSTANCE-10
                    .child(kit::ghost(
                        "instance-files-reveal",
                        "在访达中显示",
                        act(cx, move |view, window, cx| {
                            (view.handler)(InstanceIntent::RevealPath(here.clone()), window, cx)
                        }),
                    )),
            );
        let query = self
            .fields
            .as_ref()
            .map(|fields| fields.file_search.read(cx).value().to_string())
            .unwrap_or_default();
        let rows: Vec<_> = files_matching(entries, &query)
            .into_iter()
            .enumerate()
            .map(|(row, entry)| file_row(row, folder, entry, colors, cx))
            .collect();
        v_flex()
            .w_full()
            .gap_3()
            .child(header)
            .child(if rows.is_empty() {
                if entries.is_empty() {
                    kit::empty("这个文件夹是空的", "", colors).into_any_element()
                } else {
                    kit::empty("没有匹配的文件", "换个关键词试试", colors).into_any_element()
                }
            } else {
                kit::list(rows, colors).into_any_element()
            })
            .into_any_element()
    }
}

fn log_line(line: &LogLine<'_>, colors: ShellColors) -> gpui::Div {
    let tone = match line.level {
        LogLevel::Error => colors.danger,
        LogLevel::Warn => kit::tone_warn(),
        LogLevel::Info => colors.foreground,
        LogLevel::Debug => colors.muted,
    };
    div().w_full().text_color(tone).child(line.text.to_owned())
}

fn file_row(
    row: usize,
    folder: &str,
    entry: &FileEntry,
    colors: ShellColors,
    cx: &mut Context<InstanceDetailView>,
) -> gpui::Div {
    let path = join_path(folder, &entry.name);
    let detail = if entry.is_dir {
        format!("文件夹 · {}", clock(entry.modified))
    } else {
        format!("{} · {}", size_label(entry.size), clock(entry.modified))
    };
    let action = if entry.is_dir {
        kit::ghost(
            ("file-open", row),
            "打开",
            act(cx, move |view, window, cx| {
                view.open_folder(path.clone(), window, cx)
            }),
        )
        .debug_selector(move || format!("file-open-{row}"))
    } else {
        kit::ghost(
            ("file-reveal", row),
            "显示",
            act(cx, move |view, window, cx| {
                (view.handler)(InstanceIntent::RevealPath(path.clone()), window, cx)
            }),
        )
        .debug_selector(move || format!("file-reveal-{row}"))
    };
    kit::row(
        entry.name.clone(),
        detail,
        None,
        Some(action.into_any_element()),
        colors,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_join_and_climb_one_folder_at_a_time() {
        assert_eq!(join_path("", "config"), "config");
        assert_eq!(join_path("config", "sodium.json"), "config/sodium.json");
        assert_eq!(parent_path("config/sodium"), "config");
        assert_eq!(parent_path("config"), "");
        assert_eq!(parent_path(""), "");
    }

    #[test]
    fn the_file_search_matches_names_ignoring_case_and_empty_shows_all() {
        let entry = |name: &str| FileEntry {
            name: name.into(),
            is_dir: false,
            size: 0,
            modified: 0,
        };
        let entries = [
            entry("Sodium.json"),
            entry("options.txt"),
            entry("sodium-extra.toml"),
        ];
        assert_eq!(files_matching(&entries, "").len(), 3);
        assert_eq!(files_matching(&entries, " SODIUM ").len(), 2);
        assert!(files_matching(&entries, "zzz").is_empty());
    }

    #[test]
    fn a_crash_report_belongs_to_the_session_it_was_written_in() {
        let report = |name: &str, modified| lumilio_core::CrashReport {
            file_name: name.into(),
            modified,
        };
        let reports = [
            report("before.txt", 90),
            report("during.txt", 150),
            report("end.txt", 198),
            report("late.txt", 400),
        ];
        // A session from 100 to 200.
        assert_eq!(
            crash_for_session(&reports, 100, 100).map(|r| r.file_name.as_str()),
            Some("end.txt"),
            "the one nearest the end"
        );
        assert!(crash_for_session(&reports, 1000, 50).is_none());
        assert_eq!(
            crash_for_session(&[report("a.txt", 250)], 100, 100).map(|r| r.file_name.as_str()),
            Some("a.txt"),
            "a little after the end still counts"
        );
        assert!(crash_for_session(&[report("a.txt", 400)], 100, 100).is_none());
    }

    #[test]
    fn a_level_choice_shows_that_level_and_worse() {
        assert_eq!(least_level(0), None);
        assert_eq!(least_level(1), Some(LogLevel::Error));
        assert_eq!(least_level(2), Some(LogLevel::Warn));
        assert_eq!(least_level(3), Some(LogLevel::Info));
    }
}
