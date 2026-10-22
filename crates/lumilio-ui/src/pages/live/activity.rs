use super::controls::{LiveCtx, send};
use crate::kit;
use crate::kit::{Emit, ViewIntent};
use crate::live::{
    ActivityRow, ActivityState, LiveHandler, LiveIntent, TrafficReading, file_amount, source_host,
};
use crate::theme::ShellColors;
use crate::{tr, tr_all};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, px};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{FileDownload, SourceSwitch, TaskDownloads};

/// A task's open detail lists this many files; the rest are counted.
const FILES_SHOWN: usize = 8;
/// The open detail starts under the title: past the row's state dot and the
/// gap after it.
const DETAIL_INDENT: f32 = 8. + 12.;

/// The Activity tabs, as translated labels.
#[allow(non_snake_case)]
pub fn ACTIVITY_TABS() -> &'static [&'static str] {
    tr_all![
        "activity-tab-all",
        "activity-tab-download",
        "activity-tab-install",
        "activity-tab-update",
        "activity-tab-repair",
    ]
}

/// 「重试」 for a finished task that remembers its input.
// ia[activity]: 失败重试 | 失败或已取消任务行的「重试」 | 用当时的输入重新发起，作为新任务出现，旧条目保留；中途停下的下载接着已下载的部分继续 | 升级前的旧记录没有输入，不显示
pub(super) fn retry_button(
    index: usize,
    row: &ActivityRow,
    handler: &LiveHandler,
) -> Option<impl IntoElement> {
    let action = row.retry.clone()?;
    Some(
        kit::ghost(
            ("live-retry", index),
            tr!("common-retry"),
            send(handler, LiveIntent::RetryTask(action)),
        )
        .debug_selector(move || format!("live-retry-{index}")),
    )
}

/// 「打开」 for a task about a game that is still in the library.
// ia[activity]: 打开结果 | 完成或失败任务行的「打开」 | 任务关联到仍在游戏库里的游戏时跳到游戏页；整合包安装/导入完成后关联到新建的游戏
pub(super) fn open_button(
    index: usize,
    row: &ActivityRow,
    handler: &LiveHandler,
    opens: bool,
) -> Option<impl IntoElement> {
    let id = row.instance.clone().filter(|_| opens)?;
    Some(
        kit::ghost(
            ("live-open", index),
            tr!("common-open"),
            send(handler, LiveIntent::OpenInstance(id)),
        )
        .debug_selector(move || format!("live-open-{index}")),
    )
}

/// How fast a running task goes and how long is left, when that is known;
/// a paused one says so instead.
pub(super) fn speed_line(row: &ActivityRow, colors: ShellColors) -> Option<AnyElement> {
    if row.paused {
        return Some(
            kit::status(tr!("activity-status-paused"), kit::Tone::Off, colors).into_any_element(),
        );
    }
    let rate = row.rate?;
    let mut parts = vec![crate::live::rate_text(row.unit, rate)];
    if let Some((done, total)) = row.amount
        && let Some(left) = crate::live::eta_text(total.saturating_sub(done), rate)
    {
        parts.push(tr!("activity-remaining", left = left));
    }
    Some(
        div()
            .text_xs()
            .text_color(colors.muted)
            .child(parts.join(" · "))
            .into_any_element(),
    )
}

/// 「暂停」 or 「继续」 for a running task that downloads.
fn pause_button(
    index: usize,
    row: &ActivityRow,
    handler: &LiveHandler,
) -> Option<impl IntoElement> {
    let task = row.pause?;
    let (label, intent) = if row.paused {
        (tr!("activity-resume"), LiveIntent::ResumeTask(task))
    } else {
        (tr!("activity-pause"), LiveIntent::PauseTask(task))
    };
    // ia[activity]: 暂停 / 继续 | 正在下载的任务行的「暂停」「继续」 | 暂停后放开连接、保留已下载的部分，行上显示“已暂停”；继续后从已下载处接着下 | 任务开始下载后才出现
    Some(
        kit::ghost(("live-pause", index), label, send(handler, intent))
            .debug_selector(move || format!("live-pause-{index}")),
    )
}

/// The line under a running task's title: its game, how fast it goes (or
/// that it is paused), and 「详情」 when it has downloads to show. They sit
/// here rather than on the trailing edge, so the title keeps its room in a
/// narrow window.
fn running_detail(
    index: usize,
    row: &ActivityRow,
    emit: &Emit,
    open: bool,
    colors: ShellColors,
) -> AnyElement {
    let toggle = row.task.filter(|_| row.downloads.is_some()).map(|task| {
        let emit = emit.clone();
        let label = if open {
            tr!("activity-hide-files")
        } else {
            tr!("activity-show-files")
        };
        // ia[activity]: 看下载详情 | 运行中任务行标题下的「详情」「收起」 | 展开列出正在下载的文件：来源、第几次尝试、几个连接、已下载多少；下面是重试次数和“某来源失败，已改用另一个”，失败原因在技术详情里 | 任务开始下载后才出现
        kit::ghost(("live-task-detail", index), label, move |window, cx| {
            emit(ViewIntent::TaskDetail(task), window, cx);
        })
        .flush_start()
        .debug_selector(move || format!("live-task-detail-{index}"))
    });
    // A narrow window puts 「详情」 on a line of its own instead of under the
    // keys beside it.
    h_flex()
        .flex_wrap()
        .gap_x_2()
        .gap_y(px(2.))
        .items_center()
        .min_w_0()
        .when(!row.detail.is_empty(), |line| {
            line.child(
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .truncate()
                    .child(row.detail.clone()),
            )
        })
        .children(speed_line(row, colors))
        .children(toggle)
        .into_any_element()
}

/// One file on its way: its name, where it comes from, the attempt and the
/// connections when they say something, and how far it got.
fn file_line(file: &FileDownload, colors: ShellColors) -> gpui::Div {
    let mut line = h_flex().w_full().gap_3().items_center().child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_xs()
            .text_color(colors.foreground)
            // A download staged under a hidden name is still that file.
            .child(file.name.trim_start_matches('.').to_owned()),
    );
    let Some(source) = &file.source else {
        return line.child(kit::status(
            tr!("activity-file-waiting"),
            kit::Tone::Off,
            colors,
        ));
    };
    if file.attempt > 1 {
        line = line.child(kit::tag(
            tr!("activity-file-attempt", attempt = file.attempt),
            kit::TagKind::Plain,
            colors,
        ));
    }
    if file.connections > 1 {
        line = line.child(kit::tag(
            tr!("activity-file-connections", count = file.connections),
            kit::TagKind::Plain,
            colors,
        ));
    }
    line.child(kit::reading(source_host(source), colors)).child(
        kit::reading(file_amount(file.done, file.total), colors)
            .min_w(px(132.))
            .flex()
            .justify_end(),
    )
}

/// A source given up: which one, and which took its place.
fn switch_line(switch: &SourceSwitch, colors: ShellColors) -> gpui::Div {
    let from = source_host(&switch.from);
    match &switch.to {
        Some(to) => kit::status(
            tr!(
                "activity-source-switched",
                from = from,
                to = source_host(to)
            ),
            kit::Tone::Off,
            colors,
        ),
        None => kit::status(
            tr!("activity-source-exhausted", from = from),
            kit::Tone::Failure,
            colors,
        ),
    }
}

/// The raw reasons behind the sources given up, for 技术详情.
fn switch_reasons(switches: &[SourceSwitch]) -> String {
    switches
        .iter()
        .map(|switch| {
            format!(
                "{}: {} -> {}: {}",
                switch.file,
                switch.from,
                switch.to.as_deref().unwrap_or("-"),
                switch.reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What an open running task downloads.
fn downloads_detail(index: usize, downloads: &TaskDownloads, colors: ShellColors) -> gpui::Div {
    let mut detail = v_flex()
        .w_full()
        .gap(px(6.))
        .pl(px(DETAIL_INDENT))
        .pb(px(11.));
    if downloads.files.is_empty() {
        detail = detail.child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(tr!("activity-files-none")),
        );
    }
    detail = detail.children(
        downloads
            .files
            .iter()
            .take(FILES_SHOWN)
            .map(|file| file_line(file, colors)),
    );
    let more = downloads.files.len().saturating_sub(FILES_SHOWN);
    if more > 0 {
        detail = detail.child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(tr!("activity-files-more", count = more)),
        );
    }
    if downloads.retries > 0 {
        detail = detail.child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(tr!("activity-retries", count = downloads.retries)),
        );
    }
    if !downloads.switches.is_empty() {
        detail = detail.child(
            h_flex()
                .gap_3()
                .items_start()
                .child(
                    v_flex().flex_1().min_w_0().gap(px(4.)).children(
                        downloads
                            .switches
                            .iter()
                            .map(|switch| switch_line(switch, colors)),
                    ),
                )
                .child(kit::technical(
                    ("live-switch-technical", index),
                    switch_reasons(&downloads.switches),
                )),
        );
    }
    detail.debug_selector(move || format!("live-task-files-{index}"))
}

pub(super) fn activity_row(
    index: usize,
    row: &ActivityRow,
    ctx: &LiveCtx,
    opens: bool,
) -> gpui::Div {
    let (handler, colors) = (&ctx.handler, ctx.colors);
    let (tone, trail) = match &row.state {
        ActivityState::Running => (
            if row.paused {
                colors.muted
            } else {
                colors.foreground
            },
            h_flex()
                .gap_3()
                .items_center()
                // ia[activity]: 看进度 | 任务行右侧进度条和百分比，标题下的速度和剩余时间 | 实时进度；速度和剩余时间在有两次读数后出现（下载按字节，安装/修复按文件个数）；暂停时那里显示“已暂停”
                .child(match row.fraction {
                    Some(fraction) => h_flex()
                        .gap_3()
                        .items_center()
                        .child(kit::progress(("live-progress", index), fraction, colors))
                        .child(
                            div()
                                .w(px(36.))
                                .text_xs()
                                .text_color(colors.muted)
                                .child(format!("{}%", (fraction * 100.) as u32)),
                        )
                        .into_any_element(),
                    None => {
                        kit::chip(tr!("activity-status-running"), None, colors).into_any_element()
                    }
                })
                .children(pause_button(index, row, handler))
                // ia[activity]: 取消 | 运行中任务行的「取消」 | 任务停止，状态“已取消”；已下载的部分留到下次重试时继续 | 留下的部分启动时清理超过 7 天的
                .children(row.cancel.map(|id| {
                    kit::ghost(
                        ("live-cancel", index),
                        tr!("common-cancel"),
                        send(handler, LiveIntent::CancelTask(id)),
                    )
                }))
                .into_any_element(),
        ),
        ActivityState::Done => (
            kit::tone_ok(),
            h_flex()
                .gap_2()
                .items_center()
                .children(open_button(index, row, handler, opens))
                .child(kit::chip(
                    tr!("activity-status-done"),
                    Some(kit::tone_ok()),
                    colors,
                ))
                .into_any_element(),
        ),
        ActivityState::Failed(message) => (
            colors.danger,
            h_flex()
                .gap_2()
                .items_center()
                // ia[activity]: 技术详情 | 失败任务行的技术详情 | 失败原因弹窗；下载失败写明每个试过的来源和它的原因
                .child(kit::technical(
                    ("live-task-technical", index),
                    message.clone(),
                ))
                .children(retry_button(index, row, handler))
                .children(open_button(index, row, handler, opens))
                .child(kit::chip(
                    tr!("activity-status-failed"),
                    Some(colors.danger),
                    colors,
                ))
                .into_any_element(),
        ),
        ActivityState::Cancelled => (
            colors.muted,
            h_flex()
                .gap_2()
                .items_center()
                .children(retry_button(index, row, handler))
                .child(kit::chip(tr!("activity-status-cancelled"), None, colors))
                .into_any_element(),
        ),
    };
    let lead = Some(
        div()
            .size(px(8.))
            .rounded_full()
            .bg(tone)
            .into_any_element(),
    );
    if row.state != ActivityState::Running {
        return kit::row(
            row.title.clone(),
            row.detail.clone(),
            lead,
            Some(trail),
            colors,
        );
    }
    let open = row
        .task
        .is_some_and(|task| ctx.state.activity_open.contains(&task));
    let line = kit::row_with(
        row.title.clone(),
        running_detail(index, row, &ctx.emit, open, colors),
        lead,
        Some(trail),
        colors,
    );
    match row.downloads.as_ref().filter(|_| open) {
        Some(downloads) => v_flex()
            .w_full()
            .child(line)
            .child(downloads_detail(index, downloads, colors)),
        None => line,
    }
}

/// The speed of all downloads together and the connections they hold, on
/// the toolbar's trailing edge while anything downloads.
fn traffic_readings(traffic: TrafficReading, colors: ShellColors) -> Option<AnyElement> {
    if traffic.open == 0 && traffic.rate.is_none() {
        return None;
    }
    let legend = |text: &'static str| {
        div()
            .text_xs()
            .text_color(colors.muted)
            .whitespace_nowrap()
            .child(text)
    };
    // ia[activity]: 看总速度 | L3 工具栏右侧的速度与连接读数 | 所有下载加起来的速度，和正在用的连接数 / 上限（设置里的同时下载数） | 有下载时才出现
    Some(
        h_flex()
            .gap_3()
            .items_center()
            .children(traffic.rate.map(|rate| {
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(legend(tr!("activity-traffic-rate")))
                    .child(
                        kit::reading(
                            crate::live::rate_text(lumilio_core::ProgressUnit::Bytes, rate),
                            colors,
                        )
                        .text_color(colors.foreground),
                    )
            }))
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(legend(tr!("activity-traffic-connections")))
                    .child(kit::count_of(
                        traffic.open as u64,
                        Some(traffic.limit as u64),
                        colors,
                    )),
            )
            .debug_selector(|| "live-traffic".to_owned())
            .into_any_element(),
    )
}

pub fn activity(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let running = ctx.model.active_tasks();
    let tab = ctx.state.activity_tab.min(ACTIVITY_TABS().len() - 1);
    let shown = crate::live::activity_in_tab(&ctx.model.activity, tab);
    let finished = ctx
        .model
        .activity
        .iter()
        .any(|row| row.state != ActivityState::Running);
    let body = if shown.is_empty() {
        if ctx.model.activity.is_empty() {
            kit::empty(
                tr!("activity-empty-title"),
                tr!("activity-empty-help"),
                colors,
            )
            .into_any_element()
        } else {
            kit::empty(tr!("activity-tab-empty"), "", colors).into_any_element()
        }
    } else {
        kit::panel_list(
            shown
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    let opens = row
                        .instance
                        .as_ref()
                        .is_some_and(|id| ctx.model.library.iter().any(|card| &card.id == id));
                    activity_row(index, row, ctx, opens)
                })
                .collect(),
            colors,
        )
        .into_any_element()
    };
    let actions = kit::PageActions::new("live-activity-actions").secondary(
        // ia[activity]: 清除已完成 | L2 次要「清除已完成」 | 清空结束的条目；没有结束的条目时禁用
        kit::action(
            "live-clear-finished",
            tr!("activity-clear-finished"),
            None,
            false,
            send(&ctx.handler, LiveIntent::ClearFinished),
        )
        .disabled(!finished),
    );
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            tr!("route-activity"),
            match running {
                0 => tr!("activity-none-running").to_owned(),
                count => tr!("activity-running-count", count = count),
            },
            actions.render(colors),
            colors,
        ))
        .child(kit::toolbar(
            Some(
                // ia[activity]: 分类筛选 | L3 标签：全部 / 下载 / 安装 / 更新 / 修复 | 按任务类别过滤
                kit::tabs("live-activity-tabs", ACTIVITY_TABS(), tab, {
                    let emit = ctx.emit.clone();
                    move |index, window, app| emit(ViewIntent::ActivityTab(index), window, app)
                })
                .into_any_element(),
            ),
            traffic_readings(ctx.model.traffic, colors),
        ))
        .child(kit::entrance(body, ("live-activity-body", tab)))
}
