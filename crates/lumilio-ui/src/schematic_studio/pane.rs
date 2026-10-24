//! The catalogue (plan schematic-studio, T29; D22, D27): the studio in the
//! game page's plugin pane. Every schematic of the folder is a card, its
//! thumbnail over its name, size and block count, in a section per folder.
//! A click chooses a card; a double click or 打开工作室 opens the workbench
//! window on it. No viewport and no operations live here.

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, ObjectFit, SharedString, div, img, px,
};
use gpui_component::input::Input;
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};

use super::{Loaded, StudioView, muted_line, tree};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme::{ShellColors, motion};
use crate::{kit, theme, tr};

/// A card's width; the pane fits as many as it can on a row.
const CARD_WIDTH: f32 = 176.;
/// The thumbnail's height, 4:3 at the card's inner width.
const THUMB_HEIGHT: f32 = 123.;

impl StudioView {
    pub(super) fn pane(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let head = h_flex()
            .debug_selector(|| "studio-head".into())
            .w_full()
            .items_center()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .truncate()
                            .text_lg()
                            .text_color(colors.foreground)
                            .child(tr!("studio-title")),
                    )
                    .child(
                        div()
                            .truncate()
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.muted)
                            .child(tr!("studio-caption", count = self.tree.files().len())),
                    ),
            )
            // ia[plugin.litematica]: 搜索投影 | 「投影」面板头部的搜索字段 | 按文件名与文件夹筛选卡片，空了的文件夹一节不显示
            .children(self.search.as_ref().map(|search| {
                div().w(px(220.)).flex_none().child(
                    Input::new(search).small().cleanable(true).prefix(
                        Icon::new(UiIcon::Search)
                            .size(px(14.))
                            .text_color(colors.muted),
                    ),
                )
            }))
            .child(self.open_key(cx));
        v_flex()
            .id("schematic-studio")
            .debug_selector(|| "schematic-studio".into())
            .w_full()
            .h_full()
            .min_h_0()
            .gap_3()
            .child(head)
            .child(kit::keep_wheel(
                div()
                    .id("studio-catalogue-scroll")
                    .debug_selector(|| "studio-catalogue".into())
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.catalogue_scroll)
                    .child(self.catalogue(colors, cx)),
                &self.catalogue_scroll,
            ))
            .into_any_element()
    }

    /// The white key that opens the window on the chosen file. It is the
    /// pane's one page action; the pane has no More (D22).
    fn open_key(&self, cx: &mut Context<Self>) -> Key {
        // ia[plugin.litematica]: 打开工作室 | 「投影」面板右上的白键，或双击一张卡片 | 为这个游戏打开投影工作室窗口并打开选中的投影；窗口已开着就带到前面并切到它。窗口里有常驻 3D 视口、材料面板与全部操作 | 一个游戏一个工作室窗口（D22、D27）
        Key::new("studio-open-window")
            .icon(Icon::new(UiIcon::Maximize))
            .label(tr!("studio-open-window"))
            .white()
            .tooltip(tr!("studio-open-window-tip"))
            .debug_selector(|| "studio-open-window".into())
            .on_click(cx.listener(|view, _, window, cx| view.open_in_window(window, cx)))
    }

    /// The cards, a section per folder, or the sentence that stands in.
    fn catalogue(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        match &self.listing {
            Loaded::Loading => return muted_line(tr!("studio-listing"), colors).into_any_element(),
            Loaded::Failed(detail) => {
                return v_flex()
                    .gap_2()
                    .child(muted_line(tr!("studio-list-failed"), colors))
                    .child(kit::technical("studio-list-technical", detail.clone()))
                    .into_any_element();
            }
            Loaded::Ready(()) if self.tree.files().is_empty() => {
                return muted_line(tr!("studio-list-empty"), colors).into_any_element();
            }
            Loaded::Ready(()) => {}
        }
        let query = self
            .search
            .as_ref()
            .map(|search| search.read(cx).value().to_string())
            .unwrap_or_default();
        let sections = self.tree.sections(&self.folder, &query);
        if sections.is_empty() {
            return muted_line(tr!("studio-search-none"), colors).into_any_element();
        }
        let groups = self.tree.duplicates();
        v_flex()
            .w_full()
            .gap_5()
            .children(sections.into_iter().map(|(folder, files)| {
                let cards = files
                    .iter()
                    .map(|file| {
                        let duplicate = groups
                            .iter()
                            .position(|group| group.contains(file))
                            .map(|index| (index + 1, groups[index].len() - 1));
                        self.card(file, duplicate, colors, cx)
                    })
                    .collect::<Vec<_>>();
                let grid = div().w_full().flex().flex_wrap().gap_3().children(cards);
                if folder.is_empty() {
                    grid.into_any_element()
                } else {
                    kit::section(folder, colors, grid).into_any_element()
                }
            }))
            .into_any_element()
    }

    fn card(
        &self,
        file: &str,
        duplicate: Option<(usize, usize)>,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chosen = self.selected.as_deref() == Some(file);
        let target = file.to_owned();
        let selector = format!("studio-file-{file}");
        // Compact, so a large build's size and count fit the card's width.
        let meta = self.summaries.get(file).map(|info| {
            let (x, y, z) = info.size;
            tr!(
                "studio-card-meta",
                size = format!("{x}×{y}×{z}"),
                count = info.block_count
            )
        });
        let picture = self.thumbnails.get(file).map(|thumbnail| {
            img(thumbnail.clone())
                .size_full()
                .object_fit(ObjectFit::Contain)
                .with_animation(
                    SharedString::from(format!("studio-thumbnail-{file}")),
                    Animation::new(motion::QUICK),
                    |picture, delta| picture.opacity(delta),
                )
        });
        // ia[plugin.litematica]: 选择一份投影 | 「投影」面板的卡片（缩略图、名字、尺寸与方块数；内容重复的带 ≡ 编号） | 单击描边选中，双击在工作室窗口里打开它；缩略图没画好时先是一块面板色，画好后淡入 | 重复按方块内容判断，不看文件名与元数据（T16、T29）
        v_flex()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .flex_none()
            .w(px(CARD_WIDTH))
            .gap(px(6.))
            .p(px(6.))
            .rounded(px(6.))
            .border_1()
            .border_color(if chosen {
                colors.foreground
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .when(!chosen, |card| {
                card.hover(move |card| card.bg(colors.body.panel))
            })
            .on_click(
                cx.listener(move |view, event: &gpui::ClickEvent, window, cx| {
                    view.select(target.clone(), window, cx);
                    if event.click_count() == 2 {
                        view.open_in_window(window, cx);
                    }
                }),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(THUMB_HEIGHT))
                    .rounded(px(4.))
                    .overflow_hidden()
                    .bg(colors.surface)
                    .children(picture)
                    .children(duplicate.map(|(group, others)| {
                        div()
                            .id(SharedString::from(format!("studio-duplicate-{file}")))
                            .absolute()
                            .top_1()
                            .right_1()
                            .px(px(4.))
                            .rounded(px(2.))
                            .bg(colors.body.key_grey)
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.foreground)
                            .tooltip(move |window, cx| {
                                gpui_component::tooltip::Tooltip::new(tr!(
                                    "studio-duplicate-tip",
                                    count = others
                                ))
                                .build(window, cx)
                            })
                            .child(format!("≡{group}"))
                    })),
            )
            .child(
                div()
                    .w_full()
                    .text_sm()
                    .line_clamp(2)
                    .text_color(colors.foreground)
                    .child(tree::stem(file).to_owned()),
            )
            .children(meta.map(|meta| {
                div()
                    .truncate()
                    .font_family(theme::mono_font())
                    .text_xs()
                    .text_color(colors.muted)
                    .child(meta)
            }))
            .into_any_element()
    }
}
