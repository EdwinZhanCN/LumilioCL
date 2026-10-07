use super::super::panels::{act, clock};
use super::super::{InstanceDetailView, InstanceIntent, Section};
use crate::{kit, theme::ShellColors, toast::Toast};
use gpui::prelude::*;
use gpui::{AnyElement, Context, ScrollStrategy, Window, div, px, uniform_list};
use gpui_component::Sizable as _;
use gpui_component::input::Input;
use gpui_component::select::Select;
use gpui_component::{h_flex, v_flex};
use lumilio_core::{GameLogSource, LogLevel, LogLine, filter_log, log_lines};
use std::rc::Rc;

impl InstanceDetailView {
    pub(super) fn filtered_lines<'a>(
        &self,
        lines: &[LogLine<'a>],
        cx: &gpui::App,
    ) -> Vec<LogLine<'a>> {
        let report = matches!(self.log_source, GameLogSource::Crash(_));
        let lines = filter_log(lines, None, &self.log_query(cx));
        lines
            .into_iter()
            .filter(|line| report || self.log_levels.contains(&line.level))
            .collect()
    }

    pub(in super::super) fn choose_log(
        &mut self,
        source: GameLogSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.log_source == source {
            return;
        }
        self.log_source = source.clone();
        self.crash = None;
        self.log_follow = true;
        self.log_scroll.set_offset(gpui::point(px(0.), px(0.)));
        if source != GameLogSource::Live {
            let name = match &source {
                GameLogSource::Latest => "latest.log".to_owned(),
                GameLogSource::File(name) | GameLogSource::Crash(name) => name.clone(),
                GameLogSource::Live => unreachable!(),
            };
            self.crash = Some((name, None));
            (self.handler)(InstanceIntent::OpenGameLog(source), window, cx);
        }
        cx.notify();
    }

    pub(super) fn log_source_label(&self) -> String {
        match &self.log_source {
            GameLogSource::Live => if self.live_output.is_some() {
                "实时输出"
            } else {
                "最近日志 · latest.log"
            }
            .into(),
            GameLogSource::Latest => "latest.log".into(),
            GameLogSource::File(name) => name.clone(),
            GameLogSource::Crash(name) => format!("崩溃报告 · {name}"),
        }
    }

    pub(super) fn logs_body(
        &mut self,
        window: &mut Window,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(status) = self.status(&self.data.logs, colors, Section::Logs, cx) {
            return status;
        }
        let Some(Ok(logs)) = &self.data.logs else {
            return div().into_any_element();
        };
        let mut sources = vec![(
            GameLogSource::Live,
            if self.live_output.is_some() {
                "实时输出"
            } else {
                "最近日志 · latest.log"
            }
            .to_owned(),
        )];
        if logs.latest.is_some() {
            sources.push((GameLogSource::Latest, "latest.log".into()));
        }
        sources.extend(logs.files.iter().map(|file| {
            (
                GameLogSource::File(file.name.clone()),
                format!("{} · {}", file.name, clock(file.modified)),
            )
        }));
        sources.extend(logs.crashes.iter().map(|file| {
            (
                GameLogSource::Crash(file.file_name.clone()),
                format!("崩溃报告 · {} · {}", file.file_name, clock(file.modified)),
            )
        }));
        let (source_select, level_select) = self.log_selects(sources, window, cx);
        // ia[instance.diagnostics]: 选择日志来源 | 日志工具栏 · 搜索框右侧单选下拉 | 实时输出、latest.log、历史日志（含 .log.gz）、崩溃报告共用一个阅读区；可搜索来源；选择文件后显示加载或读取失败
        let source = div()
            .w(px(180.))
            .flex_none()
            .debug_selector(|| "instance-log-source".into())
            .child(
                Select::new(&source_select)
                    .small()
                    .accessibility_label("日志来源")
                    .menu_width(px(320.))
                    .search_placeholder("搜索日志来源"),
            );
        let report = matches!(self.log_source, GameLogSource::Crash(_));
        // ia[instance.diagnostics]: 按级别筛选 | 日志工具栏 · 来源右侧多选下拉 | 默认全部，点击或 Enter 独立切换错误、警告、信息、调试；菜单保持打开，Escape 关闭并保留选择；堆栈继承上一行级别；崩溃报告禁用级别筛选
        let levels = div()
            .w(px(160.))
            .flex_none()
            .debug_selector(|| "instance-log-levels".into())
            .child(
                Select::new(&level_select)
                    .small()
                    .accessibility_label("日志级别")
                    .title_prefix("级别 · ")
                    .placeholder("未选择级别")
                    .disabled(report),
            );
        let text = self.log_text().map(|text| text.into_owned());
        let has_text = text.as_ref().is_some_and(|text| !text.is_empty());
        let actions = h_flex()
            .flex_none()
            .gap_2()
            // ia[instance.diagnostics]: 崩溃分析 | 日志工具栏 · 按键「崩溃分析…」→ 弹窗 | 分析当前完整来源的快照；显示可能原因、建议、可展开证据；无匹配或失败明确提示
            .child(
                kit::ghost(
                    "instance-log-analysis",
                    "崩溃分析…",
                    act(cx, |view, window, cx| view.open_log_analysis(window, cx)),
                )
                .disabled(!has_text)
                .debug_selector(|| "instance-log-analysis".into()),
            )
            // ia[instance.diagnostics]: 复制日志 | 日志工具栏 · 按键「复制」 | 复制当前来源筛选出的全部行；无匹配时提示
            .child(
                kit::ghost(
                    "instance-log-copy",
                    "复制",
                    act(cx, |view, _, cx| {
                        if let Some(text) = view.visible_log(cx).filter(|text| !text.is_empty()) {
                            crate::toast::copy_text(text, cx);
                            view.toast(Toast::success("已复制日志"), cx);
                        } else {
                            view.toast(Toast::error("没有可复制的日志行"), cx);
                        }
                    }),
                )
                .disabled(!has_text)
                .debug_selector(|| "instance-log-copy".into()),
            )
            // ia[instance.diagnostics]: 导出日志 / 报告 | 日志工具栏 · 按键「导出…」→ 选位置 | 导出当前完整来源，忽略阅读筛选；压缩日志解压为文本；玩家名、UUID 和目录脱敏；实时来源保存点击时的输出快照
            .child(
                kit::ghost(
                    "instance-log-export",
                    "导出…",
                    act(cx, |view, window, cx| {
                        let source = if view.log_source == GameLogSource::Live
                            && view.live_output.is_none()
                        {
                            GameLogSource::Latest
                        } else {
                            view.log_source.clone()
                        };
                        let live = view
                            .log_text()
                            .map(|text| text.into_owned())
                            .unwrap_or_default();
                        (view.handler)(InstanceIntent::ExportGameLog { source, live }, window, cx);
                    }),
                )
                .disabled(!has_text)
                .debug_selector(|| "instance-log-export".into()),
            );
        let tools = h_flex()
            .w_full()
            .min_w(px(680.))
            .items_center()
            .flex_none()
            .gap_2()
            .children(self.fields.as_ref().map(|fields| {
                // ia[instance.diagnostics]: 搜索日志 | 日志工具栏 · 搜索框 | 在当前来源中忽略大小写筛选
                div()
                    .flex_1()
                    .min_w_0()
                    .debug_selector(|| "instance-log-search".into())
                    .child(Input::new(&fields.log_search).small())
            }))
            .child(source)
            .child(levels)
            .child(actions);
        let running = self.log_source == GameLogSource::Live && self.live_output.is_some();
        let base = self.log_list_scroll.0.borrow().base_handle.clone();
        self.log_scroll = base.clone();
        let mut matched_count = 0;
        let body = if let Some(text) = text {
            let lines = log_lines(&text);
            let matched = self.filtered_lines(&lines, cx);
            let rows: Rc<Vec<_>> = Rc::new(
                matched
                    .iter()
                    .map(|line| (line.level, line.text.to_owned()))
                    .collect(),
            );
            let count = rows.len();
            let widest = rows
                .iter()
                .enumerate()
                .max_by_key(|(_, (_, text))| text.chars().count())
                .map(|(index, _)| index);
            matched_count = count;
            if running && self.log_follow && count > 0 {
                self.log_list_scroll
                    .scroll_to_item(count - 1, ScrollStrategy::Bottom);
            }
            let weak = cx.entity().downgrade();
            let wheel = base.clone();
            uniform_list("instance-log-lines", count, move |range, _, _| {
                range
                    .map(|index| {
                        let (level, text) = &rows[index];
                        log_line(
                            &LogLine {
                                level: *level,
                                text,
                            },
                            colors,
                        )
                        .h(px(20.))
                        .px_3()
                        .whitespace_nowrap()
                    })
                    .collect()
            })
            .with_sizing_behavior(gpui::ListSizingBehavior::Auto)
            .with_width_from_item(widest)
            .with_horizontal_sizing_behavior(gpui::ListHorizontalSizingBehavior::Unconstrained)
            .flex_1()
            .min_h_0()
            .w_full()
            .track_scroll(&self.log_list_scroll)
            .debug_selector(|| "instance-log-lines".into())
            .on_scroll_wheel(move |event, _, cx| {
                if wheel.max_offset().y > px(0.) {
                    cx.stop_propagation();
                }
                if running {
                    let _ = weak.update(cx, |view, cx| {
                        if event.delta.pixel_delta(px(20.)).y > px(0.) {
                            view.log_follow = false;
                        }
                        cx.notify();
                    });
                }
            })
            .into_any_element()
        } else {
            let message = match &self.crash {
                Some((_, Some(Err(_)))) => "没有读到这份日志",
                Some((_, None)) => "正在读取日志…",
                _ => "还没有日志",
            };
            v_flex()
                .gap_2()
                .child(kit::empty(
                    message,
                    "选择其他来源，或运行游戏后再查看",
                    colors,
                ))
                .children(
                    self.crash
                        .as_ref()
                        .and_then(|(_, read)| read.as_ref())
                        .and_then(|result| result.as_ref().err())
                        .map(|detail| kit::technical("log-read-error", detail.clone())),
                )
                .into_any_element()
        };
        let note = if !has_text {
            "".to_owned()
        } else if matched_count == 0 {
            "没有符合条件的日志行".into()
        } else {
            format!("{matched_count} 行")
        };
        v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .gap_3()
            .child(
                div()
                    .id("instance-log-toolbar")
                    .w_full()
                    .min_w_0()
                    .flex_none()
                    .overflow_x_scroll()
                    .child(tools),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .items_center()
                    .child(div().text_xs().text_color(colors.muted).child(note))
                    .children((running && !self.log_follow).then(|| {
                        // ia[instance.diagnostics]: 回到底部 | 日志状态行 · 按键 | 回到实时输出末尾并恢复跟随；向上滚动暂停跟随
                        kit::ghost(
                            "instance-log-bottom",
                            "回到底部",
                            act(cx, |view, _, cx| {
                                view.log_follow = true;
                                cx.notify();
                            }),
                        )
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .bg(colors.surface)
                    .rounded(px(8.))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(body),
            )
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
    div()
        .w_full()
        .text_xs()
        .font_family("Menlo")
        .text_color(tone)
        .child(line.text.to_owned())
}
