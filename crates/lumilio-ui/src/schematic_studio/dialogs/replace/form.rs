use gpui::prelude::*;
use gpui::{Anchor, AnyElement, App, Context, Entity, Focusable as _, SharedString, Window, div};
use gpui_component::h_flex;
use gpui_component::popover::Popover;
use lumilio_core::SchematicOperation;

use super::super::{DialogForm, Submission};
use super::{Preview, ReplaceDialog};
use crate::key::Key;
use crate::schematic_studio::tree;
use crate::schematic_studio::writes::Edited;
use crate::theme::ShellColors;
use crate::{kit, tr};

impl DialogForm for ReplaceDialog {
    fn status(&self) -> &Entity<Submission> {
        &self.status
    }

    fn ready(&self, _: &App) -> bool {
        matches!(self.preview, Preview::Count(count, _) if count > 0)
    }

    fn commit_label(&self) -> SharedString {
        match self.preview {
            Preview::Count(count, _) if count > 0 => {
                tr!("studio-replace-commit-count", count = count).into()
            }
            _ => tr!("studio-replace-commit").into(),
        }
    }

    fn width(&self) -> f32 {
        640.
    }

    fn footer_start(&mut self, _: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let colors = ShellColors::current(cx);
        let busy = self.status.read(cx).busy;
        // ia[plugin.litematica]: 套用方案 | 替换对话框底栏左侧「套用方案…」（存过方案时才有）→ 菜单列出方案名 | 把表格换成方案的行，这份投影没有的方块那一行不替换任何东西；底栏写「方案「名」」直到表格被改动；用方案替换时历史行写方案名 | 方案存在启动器数据里，所有实例共用（T24）
        let apply = (!self.schemes.schemes.is_empty()).then(|| {
            kit::with_menu(
                Key::new("studio-replace-scheme-pick")
                    .label(tr!("studio-replace-scheme-pick"))
                    .white()
                    .disabled(busy)
                    .debug_selector(|| "studio-replace-scheme-pick".into()),
                self.scheme_entries(cx),
                Anchor::BottomLeft,
                colors,
            )
            .into_any_element()
        });
        let save_key = Key::new("studio-replace-scheme-save")
            .label(tr!("studio-replace-scheme-save"))
            .white()
            .debug_selector(|| "studio-replace-scheme-save".into());
        // ia[plugin.litematica]: 存为方案 | 替换对话框底栏左侧「存为方案…」→ 弹出框里的方案名与「存」（回车也行） | 把表格存成这个名字的方案（同名覆盖），弹出框关上，底栏写「方案「名」」；之后在任何投影的替换对话框里都能套用 | 每一行都选好目标方块之前置灰并说明；每次替换也记下用过的目标方块，下次排在选择器最前（T24）
        let save = if self.can_save(cx) && !busy {
            let (this, panel) = (cx.weak_entity(), cx.weak_entity());
            Popover::new("studio-replace-scheme-popover")
                .anchor(Anchor::BottomLeft)
                .open(self.naming)
                .on_open_change(move |open, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.naming = *open;
                        cx.notify();
                    });
                })
                .track_focus(&self.scheme_name.read(cx).focus_handle(cx))
                .trigger(save_key)
                .content(move |_, _, cx| {
                    panel
                        .update(cx, |this, cx| this.naming_panel(cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .into_any_element()
        } else {
            save_key
                .disabled(true)
                .tooltip(tr!("studio-replace-scheme-unready"))
                .into_any_element()
        };
        let from = self.from_scheme.as_deref().map(|name| {
            div()
                .debug_selector(|| "studio-replace-scheme-current".into())
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(colors.muted)
                .child(tr!("studio-replace-scheme-current", name = name))
        });
        Some(
            h_flex()
                .min_w_0()
                .gap_2()
                .items_center()
                .children(apply)
                .child(save)
                .children(from)
                .into_any_element(),
        )
    }

    fn commit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let (Some(plan), Some(saved)) = (self.plan(cx), self.scheme_rows(cx)) else {
            return;
        };
        let catalog = self.catalog();
        let summary_rows: Vec<(String, String)> = saved
            .iter()
            .map(|row| {
                let source = if row.properties.is_empty() {
                    row.source.clone()
                } else {
                    let inner: Vec<String> = row
                        .properties
                        .iter()
                        .map(|(key, value)| format!("{key}={value}"))
                        .collect();
                    format!("{}[{}]", row.source, inner.join(","))
                };
                (source, row.target.clone())
            })
            .collect();
        let scheme = self.from_scheme.clone();
        let name = tree::stem(&self.file).to_owned();
        let status = self.status.clone();
        let mut schemes = self.schemes.clone();
        schemes.used(saved.iter().map(|row| row.target.clone()));
        let _ = self
            .studio
            .update(cx, |studio, _| studio.save_schemes(schemes));
        let edit = move |bytes: &[u8]| {
            let mut schematic =
                lumilio_schematic::load_litematic(bytes).map_err(|error| error.to_string())?;
            let count = lumilio_schematic::replace::apply_mapping(
                &mut schematic,
                &plan,
                catalog.as_deref(),
            );
            let bytes = lumilio_schematic::to_litematic_bytes(&schematic)
                .map_err(|error| error.to_string())?;
            Ok(Edited { bytes, count })
        };
        let describe = Box::new(move |count: u64| {
            let summary = match (&scheme, summary_rows.as_slice()) {
                (Some(scheme), _) => tr!(
                    "studio-replace-summary-scheme",
                    name = scheme.as_str(),
                    count = count
                ),
                (None, [(source, target)]) => tr!(
                    "studio-replace-summary",
                    source = source.as_str(),
                    target = target.as_str(),
                    count = count
                ),
                (None, rows) => tr!(
                    "studio-replace-summary-rows",
                    rows = rows.len(),
                    count = count
                ),
            };
            (
                SchematicOperation {
                    kind: "replace".into(),
                    summary,
                },
                tr!("studio-replaced", count = count, file = name.as_str()),
            )
        });
        let _ = self.studio.update(cx, |studio, cx| {
            studio.submit(status, edit, describe, cx);
        });
    }
}
