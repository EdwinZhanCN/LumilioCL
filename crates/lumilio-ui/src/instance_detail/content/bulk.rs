use super::super::panels::CONTENT_KINDS;
use super::super::{InstanceDetailView, InstanceIntent};
use super::model::{BAR_HEIGHT, BulkAction};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div, px};
use gpui_component::Sizable as _;
use gpui_component::StyledExt as _;
use gpui_component::WindowExt as _;
use gpui_component::{Icon, h_flex};
use lumilio_core::ProjectKind;
use std::rc::Rc;

impl InstanceDetailView {
    /// P-BULK: what can be done to every selected file at once.
    // ia[instance.content]: 批量启用 / 停用 / 删除 | 选中行后出现的批量栏 | 逐项结果；删除先确认 | H-CONTENT-03/04
    pub(super) fn bulk_bar(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy;
        let kind = CONTENT_KINDS[self.content_index()];
        let files: Vec<String> = self.selected.iter().cloned().collect();
        let view = cx.entity().downgrade();
        let action = |id: &'static str, label: &'static str, run: BulkAction| {
            let view = view.clone();
            theme::clickable(
                Key::new(id)
                    .label(label)
                    .ghost()
                    .small()
                    .disabled(busy)
                    .debug_selector(move || id.into()),
                !busy,
            )
            .on_click(move |_, window, cx| {
                let run = run.clone();
                let _ = view.update(cx, |view, cx| run(view, window, cx));
            })
        };
        let enable = {
            let files = files.clone();
            action(
                "content-bulk-enable",
                "启用",
                Rc::new(move |view, window, cx| {
                    view.set_content_enabled(files.clone(), true, window, cx)
                }),
            )
        };
        let disable = {
            let files = files.clone();
            action(
                "content-bulk-disable",
                "停用",
                Rc::new(move |view, window, cx| {
                    view.set_content_enabled(files.clone(), false, window, cx)
                }),
            )
        };
        let delete = {
            let files = files.clone();
            let view = view.clone();
            theme::clickable(
                Key::new("content-bulk-delete")
                    .icon(Icon::new(UiIcon::Trash))
                    .label("删除")
                    .danger()
                    .small()
                    .disabled(busy),
                !busy,
            )
            .on_click(move |_, window, cx| {
                let files = files.clone();
                let _ = view.update(cx, |view, cx| {
                    view.confirm_delete_content(kind, files, window, cx)
                });
            })
        };
        let clear = action(
            "content-bulk-clear",
            "清除选择",
            Rc::new(|view, _, cx| {
                view.selected.clear();
                cx.notify();
            }),
        );
        h_flex()
            .w_full()
            .h(BAR_HEIGHT)
            .px_3()
            .items_center()
            .justify_between()
            .rounded(px(10.))
            .bg(colors.surface_active)
            .debug_selector(|| "content-bulk".into())
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(format!("已选 {} 项", files.len())),
                    )
                    .child(clear),
            )
            .child(h_flex().gap_1().child(enable).child(disable).child(delete))
            .into_any_element()
    }

    pub(super) fn confirm_update_all(
        &mut self,
        kind: ProjectKind,
        updates: Vec<(String, String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || updates.is_empty() {
            return;
        }
        let view = cx.entity().downgrade();
        let count = updates.len();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let updates = updates.clone();
            alert
                .title(format!("更新 {count} 个文件？"))
                .description(
                    "每个都会换成兼容这个游戏的最新版本；换版本可能让游戏出问题，必要时先建快照。",
                )
                .ok_text("全部更新")
                .cancel_text("取消")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let updates = updates.clone();
                    let _ = view.update(cx, |view, cx| {
                        view.send(InstanceIntent::UpdateContent { kind, updates }, window, cx);
                    });
                    true
                })
        });
    }
}
