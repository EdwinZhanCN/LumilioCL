use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div, px};
use gpui_component::input::Input;
use gpui_component::select::Select;
use gpui_component::{Sizable as _, h_flex, v_flex};

use super::super::{field, note};
use super::{BLOCK_MENU, CHIP_WIDTH, Preview, ReplaceDialog, Row};
use crate::controls::Fader;
use crate::key::{Key, KeySize};
use crate::schematic_studio::Loaded;
use crate::theme::ShellColors;
use crate::{theme, tr};

impl ReplaceDialog {
    /// 存为方案…'s popover: the name and the key that keeps it.
    pub(super) fn naming_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = ShellColors::current(cx);
        let can_save = !self.scheme_name.read(cx).value().trim().is_empty() && self.can_save(cx);
        v_flex()
            .debug_selector(|| "studio-replace-scheme-naming".into())
            .w(px(260.))
            .gap_2()
            .child(field(
                tr!("studio-replace-scheme-name"),
                div()
                    .w_full()
                    .debug_selector(|| "studio-replace-scheme-name".into())
                    .child(Input::new(&self.scheme_name).small()),
                colors,
            ))
            .child(
                h_flex().w_full().justify_end().child(
                    Key::new("studio-replace-scheme-keep")
                        .label(tr!("studio-replace-scheme-keep"))
                        .white()
                        .small()
                        .disabled(!can_save)
                        .debug_selector(|| "studio-replace-scheme-keep".into())
                        .on_click(cx.listener(|this, _, _, cx| this.save_scheme(cx))),
                ),
            )
            .into_any_element()
    }

    pub(super) fn row_view(
        &self,
        index: usize,
        row: &Row,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.status.read(cx).busy;
        let count = match &self.preview {
            Preview::Count(_, rows) => rows
                .get(index)
                .map(|count| tr!("studio-replace-row-count", count = *count)),
            _ => None,
        };
        let this = cx.weak_entity();
        let remove = (self.rows.len() > 1).then(|| {
            Key::new(("studio-replace-remove", index))
                .icon(gpui_component::Icon::new(crate::assets::UiIcon::Close))
                .ghost()
                .small()
                .tooltip(tr!("studio-replace-remove-row"))
                .debug_selector(move || format!("studio-replace-remove-{index}"))
                .on_click(move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if index < this.rows.len() && this.rows.len() > 1 {
                            this.rows.remove(index);
                            this.edited(cx);
                        }
                    });
                })
        });
        let this = cx.weak_entity();
        let any = Fader::new(
            ("studio-replace-any", index),
            row.any,
            tr!("studio-replace-any-props"),
            move |_, cx| {
                let _ = this.update(cx, |this, cx| {
                    let any = this.rows.get(index).is_some_and(|row| !row.any);
                    this.set_any(index, any, cx);
                });
            },
        )
        .shown();
        let chips: Vec<AnyElement> = if row.any {
            Vec::new()
        } else {
            row.chips
                .iter()
                .enumerate()
                .map(|(slot, chip)| {
                    // One line each, the name leading and the dropdown on
                    // the column's trailing edge, so the dropdowns stand in
                    // one column whatever the names' lengths.
                    h_flex()
                        .debug_selector(move || format!("studio-replace-chip-{index}-{slot}"))
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_family(theme::mono_font())
                                .text_xs()
                                .text_color(colors.muted)
                                .child(chip.name.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(CHIP_WIDTH))
                                .child(Select::new(&chip.pick.dropdown).small().disabled(busy)),
                        )
                        .into_any_element()
                })
                .collect()
        };
        // The arrow sits in a lane a small control tall, on the pickers'
        // line. The target picker runs to the table's trailing edge, or to
        // ✕, which every row has once there are two; the row's count is
        // its caption, as the chips are the source's.
        let lane = || {
            h_flex()
                .flex_none()
                .h(px(KeySize::Small.height()))
                .items_center()
        };
        h_flex()
            .debug_selector(move || format!("studio-replace-row-{index}"))
            .w_full()
            .gap_2()
            .items_start()
            .py(px(8.))
            .when(index > 0, |row| {
                row.border_t_1().border_color(colors.border)
            })
            .child(
                // The source, and under it what it matches: its properties
                // stay in the source's column.
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .debug_selector(move || format!("studio-replace-source-{index}"))
                            .child(
                                Select::new(&row.source)
                                    .small()
                                    .disabled(busy)
                                    .menu_width(px(BLOCK_MENU))
                                    .search_placeholder(tr!("studio-blocks-placeholder")),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("studio-replace-any-{index}"))
                            .child(any),
                    )
                    .children(chips),
            )
            .child(lane().text_sm().text_color(colors.muted).child("→"))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .debug_selector(move || format!("studio-replace-target-{index}"))
                            .child(
                                Select::new(&row.target)
                                    .small()
                                    .disabled(busy)
                                    .menu_width(px(BLOCK_MENU))
                                    .placeholder(tr!("studio-replace-target-pick"))
                                    .search_placeholder(tr!("studio-blocks-placeholder")),
                            ),
                    )
                    .children(count.map(|count| {
                        div()
                            .debug_selector(move || format!("studio-replace-count-{index}"))
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.foreground)
                            .child(count)
                    })),
            )
            .children(remove)
            .into_any_element()
    }
}

impl Render for ReplaceDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.refill) && self.refill_targets(window, cx) {
            self.refresh(cx);
        }
        let colors = ShellColors::current(cx);
        let busy = self.status.read(cx).busy;
        let rows: Vec<AnyElement> = (0..self.rows.len())
            .map(|index| {
                let row = &self.rows[index];
                self.row_view(index, row, colors, cx)
            })
            .collect();
        let this = cx.weak_entity();
        // ia[plugin.litematica]: 加一行映射 | 替换对话框表格下的「加一行」 | 多一行「从 → 到」，从文件里下一种还没有行的方块开始；每行旁边写它会替换多少个，确认键写总数 | 各行一起按替换前的样子匹配，不会一行接着一行替换（T24）
        let add = Key::new("studio-replace-add-row")
            .label(tr!("studio-replace-add-row"))
            .ghost()
            .small()
            .flush_start()
            .disabled(busy)
            .debug_selector(|| "studio-replace-add-row".into())
            .on_click(move |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    let used: Vec<String> = this
                        .rows
                        .iter()
                        .filter_map(|row| row.source.read(cx).selected_value().cloned())
                        .collect();
                    let next = this
                        .blocks
                        .iter()
                        .map(|block| block.id.clone())
                        .find(|id| !used.contains(id))
                        .or_else(|| this.blocks.first().map(|block| block.id.clone()));
                    if let Some(next) = next {
                        this.add_row(&next, None, window, cx);
                        this.edited(cx);
                    }
                });
            });
        // The table is the dialog's subject, named by its title: each row's
        // arrow and the target's placeholder say which side is which.
        let mut column = v_flex().w_full().gap_3().child(
            v_flex()
                .w_full()
                .gap_1()
                .child(
                    v_flex()
                        .debug_selector(|| "studio-replace-rows".into())
                        .w_full()
                        .children(rows),
                )
                .child(h_flex().child(add)),
        );
        match &self.catalog {
            Loaded::Loading => {
                column = column.child(note(tr!("studio-replace-catalog-loading"), colors));
            }
            Loaded::Failed(_) => {
                column = column.child(
                    note(tr!("studio-replace-catalog-failed"), colors)
                        .debug_selector(|| "studio-replace-catalog-failed".into()),
                );
            }
            Loaded::Ready(_) => {}
        }
        if let Some(scope) = &self.scope_pick {
            // ia[plugin.litematica]: 选替换范围 | 替换对话框「范围」下拉（全部区域 / 各区域名 / 当前切片 / 选区） | 计数与替换只算范围里的方块 | 只有整份文件时不出现；有选区时默认选区；当前切片是视口正在看的那一层（T24、T25）
            column = column.child(field(
                tr!("studio-replace-scope"),
                div()
                    .w(px(280.))
                    .debug_selector(|| "studio-replace-scope".into())
                    .child(Select::new(scope).small().disabled(busy)),
                colors,
            ));
        }
        // What the count stands at, when the rows' counts and the commit
        // key's total do not already say it.
        let preview = match (&self.schematic, &self.preview) {
            (Loaded::Failed(_), _) => Some(note(tr!("studio-read-failed"), colors)),
            (Loaded::Loading, _) | (_, Preview::Counting) => {
                Some(note(tr!("studio-replace-counting"), colors))
            }
            (_, Preview::Waiting) => Some(note(tr!("studio-replace-waiting"), colors)),
            (_, Preview::Count(0, _)) => Some(note(tr!("studio-replace-none"), colors)),
            (_, Preview::Count(..)) => None,
        };
        column.children(
            preview.map(|preview| preview.debug_selector(|| "studio-replace-preview".into())),
        )
    }
}
