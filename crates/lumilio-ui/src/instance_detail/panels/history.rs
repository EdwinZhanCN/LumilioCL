use super::super::{InstanceDetailView, Section};
use super::HISTORY_LABELS;
use super::data::Confirm;
use super::helpers::{act, act_index, clock};
use super::labels::{change_label, duration_label, outcome_label, size_label};
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{AnyElement, Context, div, px};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{HistoryEvent, SessionOutcome, SnapshotScope};

impl InstanceDetailView {
    pub(in super::super) fn history_panel(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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

    // ia[instance.history]: 看变更 | 历史 · 变更分段 | 只读时间线（内容、世界、设置、安装等）
    // ia[instance.history]: 看游玩记录 | 历史 · 游玩记录分段 | 只读；崩溃或没能启动的会话有「查看日志」，跳到诊断·日志，崩溃报告按时间对应（会话开始到结束后 2 分钟内写下、离结束最近的那份），找到就直接打开
    pub(super) fn events_body(
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

    pub(super) fn snapshots_body(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy;
        let create = h_flex().child(
            // ia[instance.history]: 创建快照 | 历史 · 快照分段「现在创建快照」→ 弹窗（备注、范围） | 后台创建 → toast
            kit::action(
                "snapshot-create",
                "现在创建快照",
                Some(UiIcon::Plus),
                true,
                act(cx, |view, window, cx| {
                    view.open_editor(super::super::editors::Editor::Snapshot, window, cx)
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
                            // ia[instance.history]: 恢复快照 | 快照行「恢复」→ 警告弹窗“恢复会用快照替换当前的 X，当前状态会先自动存一份” | 后台恢复 → toast；失败自动回到恢复前
                            .child(self.asking(
                                ("snapshot-restore", row),
                                "恢复",
                                Confirm::RestoreSnapshot(snapshot.id.clone()),
                                cx,
                            ))
                            // ia[instance.history]: 删除快照 | 快照行「删除」→ 警告弹窗 | 删除快照文件
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
