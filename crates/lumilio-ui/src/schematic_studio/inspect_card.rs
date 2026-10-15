//! The inspect card (plan schematic-studio T32, D31): a click in the picture
//! inspects that one block, as Litematica's info overlay does, and this card
//! floats right over it, following it as the camera turns. It is as small as
//! it can be: the name with the ID and position under it, the properties as
//! tags (the `key=value` in their tooltips), and for a container its slots
//! as the game's screen lays them out, each with its item's picture and
//! count. Higher than the room over the block, it scrolls. Choosing a kind
//! belongs to the materials; the card reaches it by one key.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, ClipboardItem, Context, RenderImage, SharedString, div, img, px};
use gpui_component::{Icon, Sizable as _, StyledExt as _, h_flex, v_flex};
use lumilio_schematic::inspect::{Contents, Grid, Stack};

use super::StudioView;
use super::materials::icon_image;
use super::viewport::split_state;
use super::words::{block_name, property_line, stacks};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit::{self, TagKind};
use crate::model_view::{Inspected, TOOL_STRIP_CLEARANCE};
use crate::theme::{self, ShellColors};
use crate::tr;

const CARD_WIDTH: f32 = 216.;
const CARD_ICON: f32 = 20.;
/// A slot and the item picture in it, as the game's 18 and 16.
const SLOT: f32 = 20.;
const SLOT_ICON: f32 = 16.;
const SLOT_GAP: f32 = 2.;
/// Item pictures are drawn at this many pixels: sharp at 16 on a
/// high-density screen.
const ITEM_ICON_PIXELS: u32 = 32;
/// Between the card and the block, and the card and the picture's edges.
const GAP: f32 = 8.;
const MAX_CARD_HEIGHT: f32 = 320.;
/// Less room than this over the block puts the card under it, if there is
/// more there.
const ROOM: f32 = 140.;

/// Where the card sits in the picture, in its pixels from the top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub left: f32,
    /// The card's top edge, when it hangs under the block.
    pub top: Option<f32>,
    /// The distance of its bottom edge from the picture's, when it stands
    /// over the block.
    pub bottom: Option<f32>,
    pub max_height: f32,
}

/// Over the block whose screen box is `rect` (fractions of a picture
/// `size` big), centred on it and kept inside the picture; under it when
/// there is little room over it and more under it (the tool strip takes the
/// bottom). With the block out of view, at the lower left over the tool
/// strip, where the read-out plate is.
pub(crate) fn placement(rect: Option<[f32; 4]>, (width, height): (f32, f32)) -> Placement {
    let corner = Placement {
        left: 12.,
        top: None,
        bottom: Some(TOOL_STRIP_CLEARANCE),
        max_height: (height - TOOL_STRIP_CLEARANCE - GAP).clamp(0., MAX_CARD_HEIGHT),
    };
    let Some([left, top, right, bottom]) = rect else {
        return corner;
    };
    let [left, top, right, bottom] = [left * width, top * height, right * width, bottom * height];
    if right < 0. || left > width || bottom < 0. || top > height {
        return corner;
    }
    let centre = ((left + right) / 2.).clamp(0., width);
    let card_left = (centre - CARD_WIDTH / 2.).clamp(GAP, (width - CARD_WIDTH - GAP).max(GAP));
    let over = top - 2. * GAP;
    let under = height - TOOL_STRIP_CLEARANCE - bottom - GAP;
    if over.max(under) < SLOT * 3. {
        return corner;
    }
    if over >= ROOM || over >= under {
        Placement {
            left: card_left,
            top: None,
            bottom: Some(height - top + GAP),
            max_height: over.min(MAX_CARD_HEIGHT),
        }
    } else {
        Placement {
            left: card_left,
            top: Some(bottom + GAP),
            bottom: None,
            max_height: under.min(MAX_CARD_HEIGHT),
        }
    }
}

/// Each slot's stack: by its slot number, or the first slot for an item a
/// block entity keeps on its own (a jukebox's record).
fn by_slot(stacks: &[Stack]) -> HashMap<u32, &Stack> {
    stacks
        .iter()
        .map(|stack| (stack.slot.map_or(0, |slot| slot.max(0) as u32), stack))
        .collect()
}

impl StudioView {
    /// Asks the viewport's worker for the pictures of the inspected block's
    /// items that are not drawn yet.
    pub(super) fn want_item_icons(&mut self, cx: &mut Context<Self>) {
        let Some(viewer) = self.viewer.clone() else {
            return;
        };
        let wanted: Vec<String> = {
            let viewer = viewer.read(cx);
            let Some(items) = viewer
                .inspected()
                .and_then(|inspected| inspected.contents.as_ref())
                .and_then(|contents| contents.items.as_ref())
            else {
                return;
            };
            items
                .iter()
                .map(|stack| stack.id.clone())
                .filter(|id| {
                    !self.item_icons.contains_key(id) && !self.item_icons_asked.contains(id)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        if wanted.is_empty() {
            return;
        }
        let Some(answer) = viewer.read(cx).item_icons(wanted.clone(), ITEM_ICON_PIXELS) else {
            return;
        };
        self.item_icons_asked.extend(wanted.iter().cloned());
        let file = self.selected.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(frames)) = answer.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if this.selected != file {
                    return;
                }
                for (id, frame) in wanted.into_iter().zip(frames) {
                    if let Some(icon) = frame.and_then(icon_image) {
                        this.item_icons.insert(id, icon);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn inspect_card(
        &self,
        inspected: &Inspected,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let viewer = self.viewer.as_ref().map(|viewer| viewer.read(cx));
        let size = viewer
            .and_then(|viewer| viewer.picture_size())
            .map_or((0., 0.), |size| {
                (f32::from(size.width), f32::from(size.height))
            });
        let place = placement(viewer.and_then(|viewer| viewer.mark_rect()), size);
        let (id, properties) = split_state(&inspected.picked.state);
        let [x, y, z] = inspected.picked.position;
        let state = inspected.picked.state.clone();
        let copy = Key::new("studio-inspect-copy")
            .icon(Icon::new(UiIcon::Copy))
            .ghost()
            .xsmall()
            .tooltip(tr!("studio-inspect-copy"))
            .debug_selector(|| "studio-inspect-copy".into())
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(state.clone()));
            });
        let close = Key::new("studio-inspect-close")
            .icon(Icon::new(UiIcon::Close))
            .ghost()
            .xsmall()
            .tooltip(tr!("common-close"))
            .debug_selector(|| "studio-inspect-close".into())
            .on_click(cx.listener(|view, _, _, cx| {
                if let Some(viewer) = view.viewer.clone() {
                    viewer.update(cx, |viewer, cx| viewer.let_go(cx));
                }
            }));
        let header = h_flex()
            .w_full()
            .gap_1()
            .items_center()
            .child(self.icon(&id, CARD_ICON, colors))
            .child(
                div()
                    .debug_selector(|| "studio-inspect-name".into())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_medium()
                    .text_color(colors.foreground)
                    .child(self.name_of(&id)),
            )
            .child(copy)
            .child(close);
        let caption = div()
            .debug_selector(|| "studio-inspect-id".into())
            .truncate()
            .font_family(theme::mono_font())
            .text_xs()
            .text_color(colors.muted)
            .child(format!("{id} · {x}, {y}, {z}"));
        let tags: Vec<AnyElement> = properties
            .iter()
            .enumerate()
            .map(|(index, (key, value))| {
                let raw = format!("{key}={value}");
                let selector = format!("studio-inspect-property-{index}");
                kit::tag(property_line(key, value), TagKind::Plain, colors)
                    .id(SharedString::from(selector.clone()))
                    .debug_selector(move || selector.clone())
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(raw.clone()).build(window, cx)
                    })
                    .into_any_element()
            })
            .collect();
        let for_kind = id.clone();
        // ia[plugin.litematica]: 从检视卡选中这种方块 | 检视卡底部 ghost「选中这种」 | 材料面板切到「材料」并选中这种方块的格子，视口里这种方块保持原色、其余变淡；检视卡与描边保留 | 在视口里点方块不会改材料面板的选中，这是唯一的桥（D31）
        let kind = Key::new("studio-inspect-kind")
            .label(tr!("studio-inspect-kind"))
            .ghost()
            .xsmall()
            .tooltip(tr!("studio-inspect-kind-tip"))
            .debug_selector(|| "studio-inspect-kind".into())
            .on_click(cx.listener(move |view, _, _, cx| view.choose_block(&for_kind, cx)));
        // ia[plugin.litematica]: 从检视卡替换这个状态 | 检视卡底部 ghost「替换…」 | 打开替换方块，第一行从这个方块的状态开始，每个属性都按它的值精确匹配 | 从材料面板进来是这种方块的任意状态；游戏运行中置灰并说明（T24）
        let exact = inspected.picked.state.clone();
        let blocked = self.operation_blocked();
        let replace = Key::new("studio-inspect-replace")
            .label(tr!("studio-detail-replace"))
            .ghost()
            .xsmall()
            .debug_selector(|| "studio-inspect-replace".into())
            .disabled(blocked.is_some())
            .when_some(blocked, |key, why| key.tooltip(why))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.open_replace_for(&exact, window, cx);
            }));
        let technical = inspected
            .contents
            .as_ref()
            .filter(|contents| !contents.data.is_empty())
            .map(|contents| {
                kit::technical("studio-inspect-data", contents.data.clone())
                    .xsmall()
                    .debug_selector(|| "studio-inspect-data".into())
            });
        // ia[plugin.litematica]: 检视卡 | 浮在被检视的方块正上方（上方放不下时在下方，方块不在画面里时在视口左下），随视角移动 | 图标、名字；等宽的 id 与位置；属性标签（悬停看原文 key=value）；容器按游戏界面的格子排（物品贴图与数量，悬停看名字与组数）；名称、告示牌与命令方块的文字；其余数据在「技术详情」 | 超过方块上方的空间就在卡内滚动；复制键复制完整方块状态（T32、D31）
        div()
            .id("studio-inspect")
            .debug_selector(|| "studio-inspect".into())
            .occlude()
            .absolute()
            .left(px(place.left))
            .when_some(place.top, |card, top| card.top(px(top)))
            .when_some(place.bottom, |card, bottom| card.bottom(px(bottom)))
            .w(px(CARD_WIDTH))
            .max_h(px(place.max_height))
            .overflow_y_scroll()
            .track_scroll(&self.inspect_scroll)
            .p_2()
            .rounded(px(6.))
            .bg(colors.body.panel)
            .border_1()
            .border_color(colors.border)
            .shadow_md()
            .child(
                v_flex()
                    .w_full()
                    .gap(px(6.))
                    .child(v_flex().w_full().child(header).child(caption))
                    .children((!tags.is_empty()).then(|| {
                        h_flex()
                            .debug_selector(|| "studio-inspect-properties".into())
                            .w_full()
                            .flex_wrap()
                            .gap_1()
                            .children(tags)
                    }))
                    .children(
                        inspected
                            .contents
                            .as_ref()
                            .map(|contents| self.contents(contents, colors)),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .child(h_flex().gap_1().child(kind).child(replace))
                            .children(technical),
                    ),
            )
            .into_any_element()
    }

    /// What the block entity holds: its slots, its given name and its text.
    fn contents(&self, contents: &Contents, colors: ShellColors) -> AnyElement {
        v_flex()
            .debug_selector(|| "studio-inspect-contents".into())
            .w_full()
            .gap_1()
            .children(
                contents
                    .grid
                    .zip(contents.items.as_deref())
                    .map(|(grid, items)| self.slots(grid, items, colors)),
            )
            .children(contents.name.as_ref().map(|name| {
                div()
                    .debug_selector(|| "studio-inspect-given-name".into())
                    .truncate()
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(tr!("studio-inspect-quoted", name = name.as_str()))
            }))
            .children((!contents.text.is_empty()).then(|| {
                v_flex()
                    .debug_selector(|| "studio-inspect-text".into())
                    .w_full()
                    .children(contents.text.iter().map(|line| {
                        div()
                            .text_xs()
                            .text_color(colors.foreground)
                            .child(SharedString::from(line.clone()))
                    }))
            }))
            .into_any_element()
    }

    /// The slots in rows as the container's screen has them, each with its
    /// item's picture and, past one, its count.
    fn slots(&self, grid: Grid, items: &[Stack], colors: ShellColors) -> AnyElement {
        let held = by_slot(items);
        let columns = grid.columns.max(1);
        let rows = (0..grid.slots.div_ceil(columns)).map(|row| {
            h_flex()
                .gap(px(SLOT_GAP))
                .children((0..columns).filter_map(|column| {
                    let index = row * columns + column;
                    (index < grid.slots)
                        .then(|| self.slot(index, held.get(&index).copied(), colors))
                }))
        });
        v_flex()
            .debug_selector(|| "studio-inspect-slots".into())
            .gap(px(SLOT_GAP))
            .children(rows)
            .into_any_element()
    }

    fn slot(&self, index: u32, stack: Option<&Stack>, colors: ShellColors) -> AnyElement {
        let selector = format!("studio-inspect-slot-{index}");
        let slot = div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .relative()
            .flex_none()
            .size(px(SLOT))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.))
            .bg(colors.body.key_grey);
        let Some(stack) = stack else {
            return slot.into_any_element();
        };
        let name = block_name(&self.names, &stack.id);
        let count = u64::from(stack.count);
        let tip = if count >= 64 {
            tr!(
                "studio-inspect-stacks",
                name = name,
                count = count,
                stacks = stacks(count)
            )
        } else {
            tr!("studio-inspect-stack", name = name, count = count)
        };
        let picture: AnyElement = match self.item_icons.get(&stack.id) {
            Some(icon) => img(Arc::<RenderImage>::clone(icon))
                .size(px(SLOT_ICON))
                .into_any_element(),
            // Not drawn (yet): a quiet mark that the slot is not empty.
            None => div()
                .size(px(SLOT_ICON * 0.6))
                .rounded(px(2.))
                .bg(colors.muted.opacity(0.5))
                .into_any_element(),
        };
        slot.tooltip(move |window, cx| {
            gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
        })
        .child(picture)
        .children((count > 1).then(|| {
            div()
                .absolute()
                .right(px(1.))
                .bottom(px(-1.))
                .font_family(theme::mono_font())
                .text_size(px(9.))
                .line_height(px(10.))
                .text_color(colors.foreground)
                .child(count.to_string())
        }))
        .into_any_element()
    }
}
