use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, px};
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};
use lumilio_core::{CONNECTIONS_PER_FILE, FileDownload, SourceSwitch, TaskDownloads};

use super::controls::{LiveCtx, send};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit::{TagKind, Tone, ViewIntent};
use crate::live::{
    ActivityRow, ActivityState, LiveHandler, LiveIntent, eta_text, file_amount, source_host,
};
use crate::theme::ShellColors;
use crate::{kit, tr, tr_all};

/// A task's open detail lists this many files; the rest are counted.
const FILES_SHOWN: usize = 8;
/// The row's lead: room for the small key that opens its files.
const LEAD: f32 = 28.;
/// Where an open detail starts: under the name, past the lead and the gap
/// after it.
const DETAIL_INDENT: f32 = LEAD + 12.;
/// A file's progress strip in an open detail.
const FILE_METER_WIDTH: f32 = 96.;
/// Bars in a file's strip: few enough that each stays a bar.
const FILE_METER_BARS: usize = 12;

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
fn retry_button(index: usize, row: &ActivityRow, handler: &LiveHandler) -> Option<Key> {
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
fn open_button(index: usize, row: &ActivityRow, handler: &LiveHandler, opens: bool) -> Option<Key> {
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

/// 「暂停」 or 「继续」 for a running task that downloads, in one place.
fn pause_button(index: usize, row: &ActivityRow, handler: &LiveHandler) -> Option<Key> {
    let task = row.pause?;
    let (label, intent) = if row.paused {
        (tr!("activity-resume"), LiveIntent::ResumeTask(task))
    } else {
        (tr!("activity-pause"), LiveIntent::PauseTask(task))
    };
    // ia[activity]: 暂停 / 继续 | 正在下载的任务行的「暂停」「继续」（同一个位置） | 暂停后放开连接、保留已下载的部分，状态变成“已暂停”、进度条变灰；继续后从已下载处接着下 | 任务开始下载后才出现
    Some(
        kit::ghost(("live-pause", index), label, send(handler, intent))
            .debug_selector(move || format!("live-pause-{index}")),
    )
}

/// What a task is doing, as an LED and a word beside its name.
fn state_of(row: &ActivityRow) -> (&'static str, Tone) {
    match &row.state {
        ActivityState::Running if row.paused => (tr!("activity-status-paused"), Tone::Off),
        ActivityState::Running => match &row.downloads {
            Some(downloads) if downloads.files.iter().any(|file| file.source.is_some()) => {
                (tr!("activity-status-downloading"), Tone::Lit)
            }
            Some(downloads) if !downloads.files.is_empty() => {
                (tr!("activity-status-waiting"), Tone::Off)
            }
            _ => (tr!("activity-status-running"), Tone::Lit),
        },
        ActivityState::Done => (tr!("activity-status-done"), Tone::Ok),
        ActivityState::Failed(_) => (tr!("activity-status-failed"), Tone::Failure),
        ActivityState::Cancelled => (tr!("activity-status-cancelled"), Tone::Off),
    }
}

/// The reading on a row's trailing edge: for a running task how much is
/// done over how much there is, with the time left under it (the meter
/// already says what share); when a finished one ended.
fn reading(row: &ActivityRow, colors: ShellColors) -> Option<AnyElement> {
    if row.state != ActivityState::Running {
        return row
            .when
            .as_ref()
            .map(|when| kit::reading(when.clone(), colors).into_any_element());
    }
    let (done, total) = row.amount?;
    let amount = match row.unit {
        lumilio_core::ProgressUnit::Bytes => file_amount(done, Some(total)),
        lumilio_core::ProgressUnit::Items => {
            tr!("activity-amount-files", done = done, total = total)
        }
    };
    let left = row
        .rate
        .filter(|_| !row.paused)
        .and_then(|rate| eta_text(total.saturating_sub(done), rate));
    Some(
        v_flex()
            .items_end()
            .gap(px(2.))
            .child(kit::reading(amount, colors).text_color(colors.foreground))
            .children(left.map(|left| kit::reading(tr!("activity-remaining", left = left), colors)))
            .into_any_element(),
    )
}

/// The row's keys: pause and cancel while it runs; what can be done about
/// it once it ended.
fn keys(index: usize, row: &ActivityRow, handler: &LiveHandler, opens: bool) -> gpui::Div {
    let keys = h_flex().flex_none().gap_2().items_center();
    match &row.state {
        ActivityState::Running => keys.children(pause_button(index, row, handler)).children(
            // ia[activity]: 取消 | 运行中任务行的「取消」 | 任务停止，状态“已取消”；已下载的部分留到下次重试时继续 | 留下的部分启动时清理超过 7 天的
            row.cancel.map(|id| {
                kit::ghost(
                    ("live-cancel", index),
                    tr!("common-cancel"),
                    send(handler, LiveIntent::CancelTask(id)),
                )
            }),
        ),
        ActivityState::Failed(message) => keys
            // ia[activity]: 技术详情 | 失败任务行的技术详情 | 失败原因弹窗；下载失败写明每个试过的来源和它的原因
            .child(kit::technical(
                ("live-task-technical", index),
                message.clone(),
            ))
            .children(retry_button(index, row, handler))
            .children(open_button(index, row, handler, opens)),
        ActivityState::Done => keys.children(open_button(index, row, handler, opens)),
        ActivityState::Cancelled => keys.children(retry_button(index, row, handler)),
    }
}

/// The row's lead: the key that opens what a running task downloads, or
/// the same room empty, so every name starts in one column.
fn lead(index: usize, row: &ActivityRow, ctx: &LiveCtx, open: bool) -> AnyElement {
    let slot = div().flex_none().w(px(LEAD));
    let Some(task) = row.task.filter(|_| row.downloads.is_some()) else {
        return slot.into_any_element();
    };
    let (icon, tip) = if open {
        (UiIcon::Expand, tr!("activity-hide-files"))
    } else {
        (UiIcon::Next, tr!("activity-show-files"))
    };
    let emit = ctx.emit.clone();
    // ia[activity]: 看下载详情 | 运行中任务行首的展开键 | 在任务下展开一张表：正在下载的每个文件、来源、几条连接（亮几格）、进度；某来源失败时写在那个文件下面，原因在技术详情里；排队的文件只数个数 | 任务开始下载后才出现
    slot.child(
        Key::new(("live-task-detail", index))
            .icon(Icon::new(icon))
            .ghost()
            .small()
            .tooltip(tip)
            .on_click(move |_, window, cx| emit(ViewIntent::TaskDetail(task), window, cx))
            .debug_selector(move || format!("live-task-detail-{index}")),
    )
    .into_any_element()
}

/// A source given up for a file: which one, and which took its place.
fn switch_line(switch: &SourceSwitch, colors: ShellColors) -> gpui::Div {
    let from = source_host(&switch.from);
    match &switch.to {
        Some(to) => kit::status(
            tr!(
                "activity-source-switched",
                from = from,
                to = source_host(to)
            ),
            Tone::Off,
            colors,
        ),
        None => kit::status(
            tr!("activity-source-exhausted", from = from),
            Tone::Failure,
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

/// One file on its way, in columns: its name (and the attempt when it is not
/// the first), where it comes from, the connections it holds, how far it
/// got; the sources it gave up under it.
fn file_line(
    id: String,
    file: &FileDownload,
    switches: &[&SourceSwitch],
    paused: bool,
    colors: ShellColors,
) -> gpui::Div {
    let fraction = file
        .total
        .filter(|total| *total > 0)
        .map(|total| (file.done as f64 / total as f64).clamp(0., 1.) as f32);
    let name = h_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .items_center()
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(colors.foreground)
                // A download staged under a hidden name is still that file.
                .child(file.name.trim_start_matches('.').to_owned()),
        )
        .when(file.attempt > 1, |name| {
            name.child(kit::tag(
                tr!("activity-file-attempt", attempt = file.attempt),
                TagKind::Plain,
                colors,
            ))
        });
    let source = file.source.as_deref().map(source_host).unwrap_or_default();
    v_flex()
        .w_full()
        .gap(px(4.))
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .items_center()
                .child(name)
                .child(
                    // The source gives way before the file's name does.
                    kit::reading(source, colors)
                        .flex_1()
                        .min_w_0()
                        .max_w(px(180.))
                        .overflow_hidden()
                        .text_ellipsis(),
                )
                .child(kit::connection_cells(
                    file.connections,
                    CONNECTIONS_PER_FILE,
                    colors,
                ))
                .child(
                    div()
                        .flex_none()
                        .w(px(FILE_METER_WIDTH))
                        .children(fraction.map(|fraction| {
                            kit::row_meter(
                                format!("{id}-meter"),
                                fraction,
                                FILE_METER_BARS,
                                paused,
                                colors,
                            )
                        })),
                )
                .child(
                    kit::reading(file_amount(file.done, file.total), colors)
                        .min_w(px(112.))
                        .flex()
                        .justify_end(),
                ),
        )
        .children(
            switches
                .iter()
                .map(|switch| switch_line(switch, colors).pl(px(12.))),
        )
}

/// What an open running task downloads: a table of the files on their way,
/// then the ones waiting for a connection, the retries and the sources given
/// up for files no longer listed.
fn downloads_detail(
    index: usize,
    downloads: &TaskDownloads,
    paused: bool,
    colors: ShellColors,
) -> gpui::Div {
    let mut detail = v_flex()
        .w_full()
        .gap(px(8.))
        .pl(px(DETAIL_INDENT))
        .pb(px(12.));
    let moving: Vec<&FileDownload> = downloads
        .files
        .iter()
        .filter(|file| file.source.is_some())
        .collect();
    let queued = downloads.files.len() - moving.len();
    if moving.is_empty() && queued == 0 {
        detail = detail.child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(tr!("activity-files-none")),
        );
    }
    let shown = &moving[..moving.len().min(FILES_SHOWN)];
    detail = detail.children(shown.iter().enumerate().map(|(position, file)| {
        let switches: Vec<&SourceSwitch> = downloads
            .switches
            .iter()
            .filter(|switch| switch.file == file.name)
            .collect();
        file_line(
            format!("live-file-{index}-{position}"),
            file,
            &switches,
            paused,
            colors,
        )
    }));
    let mut tally = Vec::new();
    let more = moving.len() - shown.len();
    if more > 0 {
        tally.push(tr!("activity-files-more", count = more));
    }
    if queued > 0 {
        tally.push(tr!("activity-files-queued", count = queued));
    }
    if downloads.retries > 0 {
        tally.push(tr!("activity-retries", count = downloads.retries));
    }
    detail = detail.children(downloads.switches.iter().filter_map(|switch| {
        let listed = shown.iter().any(|file| file.name == switch.file);
        (!listed).then(|| switch_line(switch, colors))
    }));
    if !tally.is_empty() || !downloads.switches.is_empty() {
        detail = detail.child(
            h_flex()
                .gap_3()
                .items_center()
                .children(
                    tally
                        .into_iter()
                        .map(|line| div().text_xs().text_color(colors.muted).child(line)),
                )
                .when(!downloads.switches.is_empty(), |line| {
                    line.child(kit::technical(
                        ("live-switch-technical", index),
                        switch_reasons(&downloads.switches),
                    ))
                }),
        );
    }
    detail.debug_selector(move || format!("live-task-files-{index}"))
}

fn activity_row(index: usize, row: &ActivityRow, ctx: &LiveCtx, opens: bool) -> gpui::Div {
    let colors = ctx.colors;
    let open = row
        .task
        .is_some_and(|task| ctx.state.activity_open.contains(&task));
    let (word, tone) = state_of(row);
    // ia[activity]: 看进度 | 任务行：名字旁的状态，名字下的进度条，右侧的已下载/总量与其下的剩余时间 | 下载中、等待连接、进行中、已暂停一眼可分；进度条只会前进，暂停时变灰；剩余时间在有两次读数后出现（下载按字节，安装/修复按文件个数）
    let mut item = kit::Item::new(row.title.clone())
        .lead(lead(index, row, ctx, open))
        .state(word, tone);
    if !row.detail.is_empty() {
        item = item.tag(row.detail.clone(), TagKind::Plain);
    }
    if let (ActivityState::Running, Some(fraction)) = (&row.state, row.fraction) {
        item = item.meter(kit::row_meter(
            ("live-progress", index),
            fraction,
            kit::ROW_METER_BARS,
            row.paused,
            colors,
        ));
    }
    if let Some(reading) = reading(row, colors) {
        item = item.reading(reading);
    }
    let line = item
        .actions(keys(index, row, &ctx.handler, opens))
        .render(colors);
    match row.downloads.as_ref().filter(|_| open) {
        Some(downloads) => v_flex()
            .w_full()
            .child(line)
            .child(downloads_detail(index, downloads, row.paused, colors)),
        None => line,
    }
}

/// The one display on Activity, while a task downloads: the speed of all
/// downloads, a cell per connection the setting allows, and how many tasks
/// run and are paused.
fn transfer_display(ctx: &LiveCtx) -> Option<AnyElement> {
    let running: Vec<&ActivityRow> = ctx
        .model
        .activity
        .iter()
        .filter(|row| row.state == ActivityState::Running)
        .collect();
    if !running.iter().any(|row| row.downloads.is_some()) {
        return None;
    }
    let paused = running.iter().filter(|row| row.paused).count();
    let count = |label: &'static str, value: usize| kit::Reading {
        label: label.into(),
        value: value as u64,
        digits: 2,
        unit: None,
    };
    let traffic = ctx.model.traffic;
    // ia[activity]: 看下载仪表 | 列表上方的显示屏 | 所有下载加起来的速度；每格一条连接，格数是设置里的同时下载数，亮几格就在用几条；进行中与已暂停的任务数 | 有任务在下载时才出现
    Some(
        kit::transfer_display(
            "live-transfer-display",
            kit::TransferDisplay {
                speed: traffic.rate,
                open: traffic.open,
                limit: traffic.limit,
                legends: (
                    tr!("activity-traffic-rate"),
                    tr!("activity-traffic-connections"),
                ),
                counts: vec![
                    count(tr!("activity-status-running"), running.len() - paused),
                    count(tr!("activity-status-paused"), paused),
                ],
            },
            ctx.colors,
        )
        .debug_selector(|| "live-transfer-display".to_owned())
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
            None,
        ))
        .children(transfer_display(ctx))
        .child(kit::entrance(body, ("live-activity-body", tab)))
}
