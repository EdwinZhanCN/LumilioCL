//! The workbench's trailing column (D27): port tabs 材料 / 信息 / 历史 over
//! the open file. 材料 is the block tiles (`materials.rs`); 信息 is what the
//! file says about itself, its entities and duplicates, with the facts for
//! troubleshooting folded under 技术信息; 历史 is the operation log. Counts
//! are mono values in tiles and hairline tables (design language §12), not
//! on a display: D7 keeps displays for progress and the play record.

use gpui::prelude::*;
use gpui::{AnyElement, Context, Pixels, SharedString, div, px};
use gpui_component::input::Input;
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};

use super::words::entity_name;
use super::{Analysis, Inspection, InspectorTab, StudioView, muted_line, tree};
use crate::assets::UiIcon;
use crate::controls::PortTabs;
use crate::key::Key;
use crate::kit;
use crate::theme::{self, ShellColors};
use crate::tr;

/// One line of a hairline table: the label leading, the value trailing in mono.
pub(super) fn line(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(id)
        .w_full()
        .min_h(px(28.))
        .py(px(5.))
        .gap_2()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(colors.foreground)
                .child(label.into()),
        )
        .child(
            div()
                .flex_none()
                .font_family(theme::mono_font())
                .text_xs()
                .text_color(colors.foreground)
                .child(value.into()),
        )
}

pub(super) fn size_text((x, y, z): (i32, i32, i32)) -> String {
    format!("{x} × {y} × {z}")
}

/// The history's words for an operation the core logged by name.
pub(super) fn operation_label(kind: &str) -> String {
    match kind {
        "replace" => tr!("studio-op-replace").to_owned(),
        "convert" => tr!("studio-op-convert").to_owned(),
        "metadata" => tr!("studio-op-metadata").to_owned(),
        "restore" => tr!("studio-op-restore").to_owned(),
        super::writes::DELETE => tr!("studio-op-delete").to_owned(),
        super::writes::DEDUPE => tr!("studio-op-dedupe").to_owned(),
        other => other.to_owned(),
    }
}

impl StudioView {
    pub(super) fn inspector_column(
        &self,
        width: Pixels,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity().downgrade();
        let active = InspectorTab::ALL
            .iter()
            .position(|tab| *tab == self.tab)
            .unwrap_or(0);
        // ia[plugin.litematica]: 切换材料面板标签 | 工作室右栏端口标签：材料 / 信息 / 历史 | 换成对应内容；窄窗口里标签不截断（D27）
        let tabs = PortTabs::new(
            "studio-inspector",
            super::tab_labels(),
            active,
            move |index, _, cx| {
                let _ = entity.update(cx, |view, cx| {
                    view.tab = InspectorTab::ALL[index.min(InspectorTab::ALL.len() - 1)];
                    view.inspector_scroll
                        .set_offset(gpui::point(px(0.), px(0.)));
                    cx.notify();
                });
            },
        )
        .wrap();
        let materials = self.tab == InspectorTab::Materials;
        let body = match self.tab {
            InspectorTab::Materials => self.materials_tab(colors, cx),
            InspectorTab::Info => self.info_tab(colors, cx),
            InspectorTab::History => self.history_tab(colors, cx),
        };
        let summary = match (&self.inspection, materials) {
            (Inspection::Ready(analysis), true) => Some(
                div()
                    .debug_selector(|| "studio-materials-summary".into())
                    .flex_none()
                    .font_family(theme::mono_font())
                    .text_xs()
                    .text_color(colors.muted)
                    .child(tr!(
                        "studio-materials-summary",
                        kinds = analysis.stats.blocks.len(),
                        count = analysis.stats.block_count
                    )),
            ),
            _ => None,
        };
        v_flex()
            .debug_selector(|| "studio-inspector".into())
            .flex_none()
            .w(width)
            .h_full()
            .min_h_0()
            .gap_2()
            .child(tabs)
            .children(materials.then(|| {
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    // ia[plugin.litematica]: 搜索方块 | 材料面板「材料」顶部搜索字段 | 按方块名字或 id 筛选格子；旁边写共有几种、多少个方块
                    .children(self.block_search.as_ref().map(|search| {
                        div().flex_1().min_w_0().child(
                            Input::new(search).small().cleanable(true).prefix(
                                Icon::new(UiIcon::Search)
                                    .size(px(14.))
                                    .text_color(colors.muted),
                            ),
                        )
                    }))
                    .children(summary)
            }))
            .child(kit::keep_wheel(
                div()
                    .id("studio-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.inspector_scroll)
                    // The window has no floating navigation to clear (D22).
                    .child(div().w_full().child(body)),
                &self.inspector_scroll,
            ))
            .children(
                materials
                    .then(|| self.material_detail(colors, cx))
                    .flatten(),
            )
            .into_any_element()
    }

    /// The chosen file's analysis, or the sentence that stands in for it.
    pub(super) fn analysis(&self, colors: ShellColors) -> Result<&Analysis, AnyElement> {
        match &self.inspection {
            Inspection::Ready(analysis) => Ok(analysis),
            Inspection::Empty => {
                Err(muted_line(tr!("studio-pick-first"), colors).into_any_element())
            }
            Inspection::Loading => {
                Err(muted_line(tr!("studio-reading"), colors).into_any_element())
            }
            Inspection::Failed(detail) => Err(v_flex()
                .debug_selector(|| "studio-read-failed".into())
                .gap_2()
                .child(muted_line(tr!("studio-read-failed"), colors))
                .child(kit::technical("studio-read-technical", detail.clone()))
                .into_any_element()),
        }
    }

    /// 信息: what the file says about itself, then its entities and
    /// duplicates, then 技术信息, folded.
    fn info_tab(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let mut column = v_flex().w_full().gap_4();
        match self.analysis(colors) {
            Ok(analysis) => {
                column = column.child(self.metadata(analysis, colors, cx));
                let stats = &analysis.stats;
                if stats.regions.len() > 1 {
                    let rows = stats
                        .regions
                        .iter()
                        .map(|region| {
                            line(
                                SharedString::from(format!("studio-region-{}", region.name)),
                                region.name.clone(),
                                format!("{} · {}", size_text(region.size), region.block_count),
                                colors,
                            )
                        })
                        .collect();
                    column = column.child(kit::section(
                        tr!("studio-regions"),
                        colors,
                        kit::list(rows, colors),
                    ));
                }
                if !stats.entities.is_empty() {
                    let rows = stats
                        .entities
                        .iter()
                        .map(|count| {
                            line(
                                SharedString::from(format!("studio-entity-{}", count.id)),
                                entity_name(&self.names, &count.id),
                                count.count.to_string(),
                                colors,
                            )
                        })
                        .collect();
                    column = column.child(kit::section(
                        tr!("studio-entities"),
                        colors,
                        kit::list(rows, colors),
                    ));
                }
            }
            Err(stand_in) => column = column.child(stand_in),
        }
        if let Some(duplicates) = self.duplicates_section(colors, cx) {
            column = column.child(duplicates);
        }
        column.child(self.technical(colors, cx)).into_any_element()
    }

    fn metadata(
        &self,
        analysis: &Analysis,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let info = &analysis.info;
        let or_none = |text: &str| {
            if text.trim().is_empty() {
                tr!("studio-none").to_owned()
            } else {
                text.to_owned()
            }
        };
        let when = |millis: Option<u64>| {
            millis.map_or_else(
                || tr!("studio-none").to_owned(),
                |millis| crate::live::relative_time(millis / 1000, super::history::now()),
            )
        };
        let rows = vec![
            line(
                "studio-meta-name",
                tr!("studio-meta-name"),
                or_none(&info.name),
                colors,
            ),
            line(
                "studio-meta-author",
                tr!("studio-meta-author"),
                or_none(&info.author),
                colors,
            ),
            line(
                "studio-meta-description",
                tr!("studio-meta-description"),
                or_none(&info.description),
                colors,
            ),
            line(
                "studio-meta-created",
                tr!("studio-meta-created"),
                when(info.created),
                colors,
            ),
            line(
                "studio-meta-modified",
                tr!("studio-meta-modified"),
                when(info.modified),
                colors,
            ),
        ];
        let blocked = self.operation_blocked();
        // ia[plugin.litematica]: 编辑信息… | 材料面板「信息」的 ghost「编辑…」，或更多菜单的「编辑信息…」 → 对话框（名称、作者、描述） | 保存后经备份写入并更新修改时间，记进历史，方块数据不动；名称为空时留在对话框里说明 | 游戏运行中置灰并说明（T15）
        let edit = Key::new("studio-meta-edit")
            .label(tr!("studio-meta-edit"))
            .ghost()
            .small()
            .debug_selector(|| "studio-meta-edit".into())
            .disabled(blocked.is_some())
            .when_some(blocked, |key, why| key.tooltip(why))
            .on_click(cx.listener(|view, _, window, cx| view.open_metadata(window, cx)));
        v_flex()
            .w_full()
            .gap_2()
            .child(h_flex().w_full().justify_end().child(edit))
            .child(kit::list(rows, colors))
            .into_any_element()
    }

    /// 重复: groups of files with the same blocks, each line with
    /// 只保留这个; `None` when every file is different.
    fn duplicates_section(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let groups = self.tree.duplicates();
        if groups.is_empty() {
            return None;
        }
        let blocked = self.write_blocked();
        let rows: Vec<AnyElement> = groups
            .iter()
            .enumerate()
            .flat_map(|(group, files)| files.iter().map(move |file| (group, file, files)))
            .map(|(group, file, files)| {
                let target = file.clone();
                let others: Vec<String> = files
                    .iter()
                    .filter(|other| *other != file)
                    .cloned()
                    .collect();
                let unread = others.iter().any(|other| !self.infos.contains_key(other));
                let why = blocked.or(unread.then(|| tr!("studio-reading")));
                let keep = file.clone();
                // ia[plugin.litematica]: 只保留这个 | 材料面板「信息」的「重复」一节每行的 ghost「只保留这个」 → 确认框 | 确认后把同组其余文件移进启动器的备份（不删除），记进历史，可逐条恢复 | 游戏运行中置灰并说明（T16）
                let keep_only = Key::new(SharedString::from(format!("studio-keep-{file}")))
                    .label(tr!("studio-keep-only"))
                    .ghost()
                    .small()
                    .debug_selector({
                        let file = file.clone();
                        move || format!("studio-keep-{file}")
                    })
                    .disabled(why.is_some())
                    .when_some(why, |key, why| key.tooltip(why))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        cx.stop_propagation();
                        view.ask_keep_only(keep.clone(), others.clone(), window, cx);
                    }));
                // ia[plugin.litematica]: 查看重复的投影 | 材料面板「信息」的「重复」一节 | 按 ≡ 编号分组列出内容相同的文件，点一行就打开它 | 只是标出来，绝不自动删除（T16）
                line(
                    SharedString::from(format!("studio-duplicate-row-{file}")),
                    tree::stem(file).to_owned(),
                    format!("≡{}", group + 1),
                    colors,
                )
                .debug_selector({
                    let file = file.clone();
                    move || format!("studio-duplicate-row-{file}")
                })
                .cursor_pointer()
                .hover(move |row| row.bg(colors.body.panel))
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.select(target.clone(), window, cx);
                }))
                // The key acts alone; a click on it never selects the row.
                .child(
                    div()
                        .id(SharedString::from(format!("studio-keep-slot-{file}")))
                        .flex_none()
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(keep_only),
                )
                .into_any_element()
            })
            .collect();
        Some(
            kit::section(tr!("studio-duplicates"), colors, kit::list(rows, colors))
                .into_any_element(),
        )
    }

    /// 技术信息, folded until opened: the facts that help when something
    /// looks wrong, and the asset sources.
    fn technical(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let open = self.technical_open;
        // ia[plugin.litematica]: 查看技术信息 | 材料面板「信息」最后的「技术信息」折叠行 | 展开后列出体积、方块实体、数据版本、文件路径、这个游戏版本画不出的方块与实体，以及资源来源 | 默认折叠：排错时才需要（D27）
        let head = h_flex()
            .id("studio-technical")
            .debug_selector(|| "studio-technical".into())
            .w_full()
            .h(px(28.))
            .gap_1()
            .items_center()
            .cursor_pointer()
            .on_click(cx.listener(|view, _, _, cx| {
                view.technical_open = !view.technical_open;
                cx.notify();
            }))
            .child(
                Icon::new(if open { UiIcon::Expand } else { UiIcon::Next })
                    .size(px(12.))
                    .text_color(colors.muted),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(tr!("studio-technical")),
            );
        let mut column = v_flex().w_full().gap_3().child(head);
        if !open {
            return column.into_any_element();
        }
        if let Inspection::Ready(analysis) = &self.inspection {
            let stats = &analysis.stats;
            let block_entities: u64 = stats.block_entities.iter().map(|count| count.count).sum();
            let mut rows = vec![
                line(
                    "studio-volume",
                    tr!("studio-volume"),
                    stats.volume.to_string(),
                    colors,
                )
                .into_any_element(),
                line(
                    "studio-block-entity-count",
                    tr!("studio-block-entity-count"),
                    block_entities.to_string(),
                    colors,
                )
                .into_any_element(),
                line(
                    "studio-meta-data-version",
                    tr!("studio-meta-data-version"),
                    analysis.info.data_version.map_or_else(
                        || tr!("studio-version-unknown").to_owned(),
                        |version| version.to_string(),
                    ),
                    colors,
                )
                .into_any_element(),
                line(
                    "studio-meta-file",
                    tr!("studio-meta-file"),
                    self.selected.clone().unwrap_or_default(),
                    colors,
                )
                .into_any_element(),
            ];
            if let Some(viewer) = &self.viewer {
                let viewer = viewer.read(cx);
                for (key, label, ids) in [
                    (
                        "studio-undrawable-blocks",
                        tr!("studio-undrawable-blocks"),
                        viewer.undrawable(),
                    ),
                    (
                        "studio-undrawable-entities",
                        tr!("studio-undrawable-entities"),
                        viewer.undrawable_entities(),
                    ),
                ] {
                    if ids.is_empty() {
                        continue;
                    }
                    rows.push(
                        line(key, label, ids.len().to_string(), colors)
                            .child(kit::technical(
                                SharedString::from(format!("{key}-ids")),
                                ids.join("\n"),
                            ))
                            .into_any_element(),
                    );
                }
            }
            column = column.child(kit::list(rows, colors));
        }
        column
            .child(self.sources_section(colors, cx))
            .into_any_element()
    }
}
