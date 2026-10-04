use super::super::panels::{act, act_index, clock, hint_text};
use super::super::{InstanceDetailView, InstanceIntent, Section};
use super::helpers::{LEVEL_LABELS, LOG_LINES_SHOWN, least_level};
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::ShellColors;
use crate::toast::Toast;
use gpui::prelude::*;
use gpui::{AnyElement, Context, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::Input;
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::{LogLevel, LogLine, filter_log, log_lines};

impl InstanceDetailView {
    pub(super) fn logs_body(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
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
}

pub(super) fn log_line(line: &LogLine<'_>, colors: ShellColors) -> gpui::Div {
    let tone = match line.level {
        LogLevel::Error => colors.danger,
        LogLevel::Warn => kit::tone_warn(),
        LogLevel::Info => colors.foreground,
        LogLevel::Debug => colors.muted,
    };
    div().w_full().text_color(tone).child(line.text.to_owned())
}
