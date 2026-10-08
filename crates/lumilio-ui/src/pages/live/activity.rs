use super::controls::{LiveCtx, send};
use crate::kit;
use crate::kit::ViewIntent;
use crate::live::{ActivityRow, ActivityState, LiveHandler, LiveIntent};
use crate::theme::ShellColors;
use crate::{tr, tr_all};
use gpui::prelude::*;
use gpui::{IntoElement, div, px};
use gpui_component::{h_flex, v_flex};

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
// ia[activity]: 失败重试 | 失败或已取消任务行的「重试」 | 用当时的输入重新发起，作为新任务出现，旧条目保留 | 升级前的旧记录没有输入，不显示
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

/// How fast a running task goes and how long is left, when that is known.
pub(super) fn speed_line(row: &ActivityRow, colors: ShellColors) -> Option<gpui::Div> {
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
            .child(parts.join(" · ")),
    )
}

pub(super) fn activity_row(
    index: usize,
    row: &ActivityRow,
    handler: &LiveHandler,
    colors: ShellColors,
    opens: bool,
) -> gpui::Div {
    let (tone, trail) = match &row.state {
        ActivityState::Running => (
            colors.foreground,
            h_flex()
                .gap_3()
                .items_center()
                // ia[activity]: 看进度 | 任务行右侧进度条和百分比 | 实时进度；速度和剩余时间在有两次读数后出现（下载按字节，安装/修复按文件个数）
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
                .children(speed_line(row, colors))
                // ia[activity]: 取消 | 运行中任务行的「取消」 | 任务停止，状态“已取消”
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
                // ia[activity]: 技术详情 | 失败任务行的技术详情 | 失败原因弹窗
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
    let detail = row.detail.clone();
    kit::row(
        row.title.clone(),
        detail,
        Some(
            div()
                .size(px(8.))
                .rounded_full()
                .bg(tone)
                .into_any_element(),
        ),
        Some(trail),
        colors,
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
                    activity_row(index, row, &ctx.handler, colors, opens)
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
        .child(kit::entrance(body, ("live-activity-body", tab)))
}
