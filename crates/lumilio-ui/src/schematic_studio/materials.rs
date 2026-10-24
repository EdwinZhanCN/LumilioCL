//! 材料, the workbench's first tab (plan schematic-studio, T30; D27): every
//! block kind of the open file as a tile, its icon over its count, the most
//! numerous first. A tile chooses a kind (a click in the picture inspects
//! one block instead, D31); the chosen kind's detail sits at the column's
//! bottom with what can be done to it. Icons come from the viewport's
//! worker (T27); names from the game (T26).

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, ClipboardItem, Context, RenderImage, SharedString, div, img, px};
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};
use lumilio_schematic::stats::BlockCount;
use lumilio_schematic_render::{Focus, Frame};

use super::words::{block_name, property_words, stacks};
use super::{Inspection, InspectorTab, StudioView, muted_line};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit::TagKind;
use crate::theme::ShellColors;
use crate::{kit, theme, tr};

/// A tile's side; four fit the narrowest column.
const TILE: f32 = 60.;
const TILE_ICON: f32 = 32.;
const DETAIL_ICON: f32 = 48.;
/// Icons are drawn once at this many pixels, enough for the detail's icon
/// on a high-density screen.
const ICON_PIXELS: u32 = 96;

/// The slot behind block icons: key grey by day and the graphite key by
/// night, so the slot is always set off from the panel, and lighter than
/// it at night as the game's slots are lighter than its darkest blocks.
pub(super) fn slot_fill(body: crate::theme::Body) -> gpui::Hsla {
    if body.dark {
        body.key_white
    } else {
        body.key_grey
    }
}

/// `minecraft:oak_stairs` with `[facing=east,half=top]`, as the mesher
/// reads a state.
pub(super) fn state_string(id: &str, properties: &[(String, String)]) -> String {
    if properties.is_empty() {
        return id.to_owned();
    }
    let properties: Vec<String> = properties
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    format!("{id}[{}]", properties.join(","))
}

/// A drawn icon as GPUI shows it; the frame is blue first already.
pub(super) fn icon_image(frame: Frame) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

impl StudioView {
    /// Asks the viewport's worker for one icon per block kind of the open
    /// file, drawn from its most common state, once both the file's
    /// statistics and the viewport's scene are there.
    pub(super) fn want_icons(&mut self, cx: &mut Context<Self>) {
        if self.icons_asked {
            return;
        }
        let (Some(viewer), Inspection::Ready(analysis), Some(file)) =
            (&self.viewer, &self.inspection, &self.selected)
        else {
            return;
        };
        let blocks: Vec<&BlockCount> = analysis.stats.blocks.iter().collect();
        let ids: Vec<String> = blocks.iter().map(|block| block.id.clone()).collect();
        let states: Vec<String> = blocks
            .iter()
            .map(|block| {
                let common = block
                    .states
                    .first()
                    .map_or(&[][..], |state| &state.properties[..]);
                state_string(&block.id, common)
            })
            .collect();
        let Some(answer) = viewer.read(cx).icons(states, ICON_PIXELS) else {
            return;
        };
        self.icons_asked = true;
        let file = file.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(frames)) = answer.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if this.selected.as_deref() != Some(file.as_str()) {
                    return;
                }
                for (id, frame) in ids.into_iter().zip(frames) {
                    if let Some(icon) = frame.and_then(icon_image)
                        && let Some(old) = this.icons.insert(id, icon)
                    {
                        this.old_icons.push(old);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Chooses block kind `id`: its tile is ringed, its detail shows, and
    /// 材料 comes forward. A click on the tile does this, the inspect card's
    /// 选中这种 too, and the application's review hook.
    pub fn choose_block(&mut self, id: &str, cx: &mut Context<Self>) {
        if !matches!(self.inspection, Inspection::Ready(_)) {
            // Chosen before the file is read (the window opening on it):
            // applied once its statistics are in.
            self.pending_block = Some(id.to_owned());
            return;
        }
        if self.chosen.as_deref() != Some(id) {
            self.isolate = false;
        }
        self.chosen = Some(id.to_owned());
        self.tab = InspectorTab::Materials;
        self.sync_focus(cx);
        cx.notify();
    }

    /// Inspects the block at `position`, as a click on it does (T32): the
    /// application's review hook. Before the file's viewport exists, it is
    /// handed over once there is one.
    pub fn inspect_block(&mut self, position: [i32; 3], cx: &mut Context<Self>) {
        match self.viewer.clone() {
            Some(viewer) => viewer.update(cx, |viewer, cx| viewer.inspect_cell(position, cx)),
            None => self.pending_inspect = Some(position),
        }
    }

    /// What the picture keeps in colour (T31, D30): the hovered tile's kind,
    /// else the chosen one, faded around; only the chosen kind with
    /// 只看这种; every block otherwise.
    pub(super) fn focus(&self) -> Focus {
        match (&self.tile_hover, &self.chosen) {
            (_, Some(chosen)) if self.isolate => Focus::Only(chosen.clone()),
            (Some(hovered), _) => Focus::Fade(hovered.clone()),
            (None, Some(chosen)) => Focus::Fade(chosen.clone()),
            (None, None) => Focus::All,
        }
    }

    /// Hands the focus to the viewport; it draws again only on a change.
    pub(super) fn sync_focus(&mut self, cx: &mut Context<Self>) {
        let focus = self.focus();
        if let Some(viewer) = &self.viewer {
            viewer.update(cx, |viewer, _| viewer.set_focus(focus));
        }
    }

    /// The kind's icon at `side` in a slot a little larger, as the game
    /// draws items on its inventory slots: dark or see-through blocks
    /// (black glass, tinted glass, black concrete) stay visible on a dark
    /// panel. The slot is empty while the icon is not drawn.
    pub(super) fn icon(&self, id: &str, side: f32, colors: ShellColors) -> AnyElement {
        let pad = (side / 8.).round().max(2.);
        div()
            .debug_selector(|| "studio-block-slot".into())
            .flex_none()
            .size(px(side + 2. * pad))
            .rounded(px(4.))
            .bg(slot_fill(colors.body))
            .flex()
            .items_center()
            .justify_center()
            .children(
                self.icons
                    .get(id)
                    .map(|icon| img(icon.clone()).flex_none().size(px(side))),
            )
            .into_any_element()
    }

    pub(super) fn name_of(&self, id: &str) -> String {
        block_name(&self.names, id)
    }

    /// The tiles, filtered by the search field (names and IDs): the
    /// selection's blocks while there is one, else the slice's layer's,
    /// else the file's (T25), under a line saying which.
    pub(super) fn materials_tab(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let analysis = match self.analysis(colors) {
            Ok(analysis) => analysis,
            Err(stand_in) => return stand_in,
        };
        let query = self
            .block_search
            .as_ref()
            .map(|search| search.read(cx).value().trim().to_lowercase())
            .unwrap_or_default();
        let (scope, counts) = self.scope(colors, cx);
        let counts = counts.unwrap_or_else(|| {
            analysis
                .stats
                .blocks
                .iter()
                .map(|block| (block.id.clone(), block.count))
                .collect()
        });
        let missing = self.missing_blocks();
        let only_missing = self.only_incompatible && !missing.is_empty();
        let tiles: Vec<AnyElement> = counts
            .iter()
            .filter(|(id, _)| !only_missing || missing.contains(id))
            .filter(|(id, _)| {
                query.is_empty()
                    || id.to_lowercase().contains(&query)
                    || self.name_of(id).to_lowercase().contains(&query)
            })
            .map(|(id, count)| self.tile(id, *count, colors, cx))
            .collect();
        // ia[plugin.litematica]: 只看不兼容 | 工作室材料面板「材料」顶部的「只看不兼容」开关（游戏版本缺方块时才有） | 打开后格子只剩游戏版本没有的方块，每个格子右上角有 ✕，悬停说明「y 没有这种方块」 | 与搜索一起筛（T23）
        let incompatible = (!missing.is_empty()).then(|| {
            let this = cx.weak_entity();
            div()
                .debug_selector(|| "studio-only-incompatible".into())
                .child(
                    crate::controls::Fader::new(
                        "studio-only-incompatible",
                        self.only_incompatible,
                        tr!("studio-only-incompatible"),
                        move |_, cx| {
                            let _ = this.update(cx, |this, cx| {
                                this.only_incompatible = !this.only_incompatible;
                                cx.notify();
                            });
                        },
                    )
                    .shown(),
                )
                .into_any_element()
        });
        let body = if tiles.is_empty() {
            muted_line(
                if !counts.is_empty() {
                    tr!("studio-search-none")
                } else if scope.is_some() {
                    tr!("studio-scope-empty")
                } else {
                    tr!("studio-blocks-none")
                },
                colors,
            )
            .into_any_element()
        } else {
            div()
                .debug_selector(|| "studio-tiles".into())
                .w_full()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .children(tiles)
                .into_any_element()
        };
        if scope.is_none() && incompatible.is_none() {
            return body;
        }
        v_flex()
            .w_full()
            .gap_2()
            .children(scope)
            .children(incompatible)
            .child(body)
            .into_any_element()
    }

    /// Which blocks 材料 counts (T25): the line saying so and their counts,
    /// or neither for the whole file.
    fn scope(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> (Option<AnyElement>, Option<Vec<(String, u64)>>) {
        let Some(viewer) = self.viewer.clone() else {
            return (None, None);
        };
        let (selection, slice, counts) = {
            let viewer = viewer.read(cx);
            let counts = if viewer.selection().is_some() {
                viewer.selection_blocks().to_vec()
            } else {
                viewer.layer_blocks().to_vec()
            };
            (viewer.selection(), viewer.slice(), counts)
        };
        let total: u64 = counts.iter().map(|(_, count)| count).sum();
        let line = |text: String| {
            div()
                .debug_selector(|| "studio-scope".into())
                .font_family(theme::mono_font())
                .text_xs()
                .text_color(colors.foreground)
                .child(text)
        };
        if let Some((min, max)) = selection {
            let side = |axis: usize| max[axis] - min[axis] + 1;
            // ia[plugin.litematica]: 看选区的材料 | 工作室视口 Ctrl 拖拽框选后，材料面板「材料」顶部一行 | 写「选区 宽×高×长 · 方块数」，格子只数选区里的方块；「清除选区」或视口里 Esc 回到整份 | 切片时没有选区则数当前层（T25）
            let clear = Key::new("studio-scope-clear")
                .label(tr!("studio-scope-clear"))
                .ghost()
                .small()
                .debug_selector(|| "studio-scope-clear".into())
                .on_click(move |_, _, cx| {
                    viewer.update(cx, |viewer, cx| viewer.set_selection(None, cx));
                });
            let text = tr!(
                "studio-scope-selection",
                x = side(0),
                y = side(1),
                z = side(2),
                count = total
            );
            let row = h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .justify_between()
                .child(line(text))
                .child(clear);
            return (Some(row.into_any_element()), Some(counts));
        }
        if let Some(slice) = slice {
            let text = tr!(
                "studio-scope-layer",
                axis = ["X", "Y", "Z"][slice.axis.index()],
                layer = slice.layer,
                count = total
            );
            return (Some(line(text).into_any_element()), Some(counts));
        }
        (None, None)
    }

    fn tile(
        &self,
        id: &str,
        count: u64,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = id.to_owned();
        let chosen = self.chosen.as_deref() == Some(id.as_str());
        let tip = tr!(
            "studio-tile-tip",
            name = self.name_of(&id),
            count = count,
            stacks = stacks(count)
        );
        let selector = format!("studio-tile-{id}");
        let (target, hovered) = (id.clone(), id.clone());
        let missing = self
            .compat_target()
            .filter(|_| self.missing_blocks().contains(&id))
            .map(str::to_owned);
        // ia[plugin.litematica]: 指着一格看它在哪 | 工作室材料面板「材料」的方块格子 · 悬停 | 视口里这种方块保持原色，其余方块变灰变淡；移开恢复（选中时保持选中的那种） | 只换遮罩，不重新生成网格（T31、D30）
        // ia[plugin.litematica]: 选中一种方块 | 工作室材料面板「材料」的方块格子（图标与数量，悬停提示名字与组数） | 格子描边，视口里这种方块保持原色、其余变淡，面板底部出现这种方块的详情：名字、id、数量与组数、各状态的数量、替换…、只看这种 | 在视口里点方块是检视那一个，不改这里的选中；检视卡的「选中这种」会（D27、D31）
        v_flex()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .flex_none()
            .w(px(TILE))
            .h(px(TILE))
            .items_center()
            .justify_center()
            .gap(px(2.))
            .rounded(px(4.))
            .border_1()
            .border_color(if chosen {
                colors.foreground
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .when(!chosen, |tile| {
                tile.hover(move |tile| tile.bg(colors.body.panel))
            })
            .tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
            })
            .on_click(cx.listener(move |view, _, _, cx| view.choose_block(&target, cx)))
            .on_hover(cx.listener(move |view, over: &bool, _, cx| {
                if *over {
                    view.tile_hover = Some(hovered.clone());
                } else if view.tile_hover.as_deref() == Some(hovered.as_str()) {
                    view.tile_hover = None;
                }
                view.sync_focus(cx);
            }))
            .child(self.icon(&id, TILE_ICON, colors))
            .child(
                div()
                    .font_family(theme::mono_font())
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(count.to_string()),
            )
            .relative()
            .children(missing.map(|target| {
                let tip = tr!("studio-tile-missing", version = target.as_str());
                let marker = format!("studio-tile-missing-{id}");
                div()
                    .id(SharedString::from(marker.clone()))
                    .debug_selector(move || marker.clone())
                    .absolute()
                    .top(px(2.))
                    .right(px(4.))
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.body.orange)
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                    })
                    .child("✕")
            }))
            .into_any_element()
    }

    /// The chosen kind's detail, pinned under the tiles; `None` with no
    /// kind chosen or one the file does not have.
    pub(super) fn material_detail(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let id = self.chosen.clone()?;
        let Inspection::Ready(analysis) = &self.inspection else {
            return None;
        };
        let block = analysis.stats.blocks.iter().find(|block| block.id == id)?;
        let undrawable = self
            .viewer
            .as_ref()
            .is_some_and(|viewer| viewer.read(cx).undrawable().contains(&id));
        let states: Vec<AnyElement> = block
            .states
            .iter()
            .enumerate()
            .map(|(index, state)| {
                let words = property_words(&state.properties);
                let words = if words.is_empty() {
                    tr!("studio-state-default").to_owned()
                } else {
                    words.join(tr!("common-list-separator"))
                };
                let selector = format!("studio-state-{id}-{index}");
                kit::tag(
                    tr!("studio-state-count", words = words, count = state.count),
                    TagKind::Plain,
                    colors,
                )
                .debug_selector(move || selector.clone())
                .into_any_element()
            })
            .collect();
        let for_copy = id.clone();
        let copy = Key::new("studio-detail-copy")
            .icon(Icon::new(UiIcon::Copy))
            .ghost()
            .small()
            .tooltip(tr!("studio-detail-copy"))
            .debug_selector(|| "studio-detail-copy".into())
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(for_copy.clone()));
            });
        let close = Key::new("studio-detail-close")
            .icon(Icon::new(UiIcon::Close))
            .ghost()
            .small()
            .tooltip(tr!("common-close"))
            .debug_selector(|| "studio-detail-close".into())
            .on_click(cx.listener(|view, _, _, cx| {
                view.chosen = None;
                view.isolate = false;
                view.sync_focus(cx);
                cx.notify();
            }));
        // ia[plugin.litematica]: 只看这种方块 | 材料详情的 ghost「只看这种」/「显示全部」 | 视口里只画这种方块，再点恢复全部；换一种方块或关掉详情也恢复 | 只换遮罩，不重新生成网格（T31、D30）
        let only = Key::new("studio-detail-only")
            .label(if self.isolate {
                tr!("studio-detail-all")
            } else {
                tr!("studio-detail-only")
            })
            .ghost()
            .small()
            .debug_selector(|| "studio-detail-only".into())
            .on_click(cx.listener(|view, _, _, cx| {
                view.isolate = !view.isolate;
                view.sync_focus(cx);
                cx.notify();
            }));
        let blocked = self.operation_blocked();
        let for_replace = id.clone();
        // ia[plugin.litematica]: 替换这种方块… | 材料详情的 ghost「替换…」 | 打开替换对话框，源方块是这种方块、属性不限，只需选目标 | 游戏运行中或文件未读完时置灰并说明（D27）
        let replace = Key::new("studio-detail-replace")
            .label(tr!("studio-detail-replace"))
            .ghost()
            .small()
            .debug_selector(|| "studio-detail-replace".into())
            .disabled(blocked.is_some())
            .when_some(blocked, |key, why| key.tooltip(why))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.open_replace_for(&for_replace, window, cx);
            }));
        Some(
            v_flex()
                .id("studio-detail")
                .debug_selector(|| "studio-detail".into())
                .flex_none()
                .w_full()
                .gap_2()
                .pt_3()
                .border_t_1()
                .border_color(colors.foreground)
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .child(self.icon(&id, DETAIL_ICON, colors))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .debug_selector(|| "studio-detail-name".into())
                                        .truncate()
                                        .text_base()
                                        .text_color(colors.foreground)
                                        .child(self.name_of(&id)),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .font_family(theme::mono_font())
                                        .text_xs()
                                        .text_color(colors.muted)
                                        .child(id.clone()),
                                ),
                        )
                        .child(copy)
                        .child(close),
                )
                .child(
                    div()
                        .debug_selector(|| "studio-detail-count".into())
                        .font_family(theme::mono_font())
                        .text_xs()
                        .text_color(colors.foreground)
                        .child(tr!(
                            "studio-detail-count",
                            count = block.count,
                            stacks = stacks(block.count)
                        )),
                )
                .children(undrawable.then(|| {
                    div()
                        .debug_selector(|| "studio-detail-undrawable".into())
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!("studio-card-undrawable"))
                }))
                .child(h_flex().w_full().flex_wrap().gap_1().children(states))
                .child(h_flex().w_full().gap_2().child(replace).child(only))
                .into_any_element(),
        )
    }
}
