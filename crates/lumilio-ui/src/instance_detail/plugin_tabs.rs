//! Tabs that plugins contribute to the instance page.
//!
//! A plugin only describes content (a view tree); this module is the one
//! renderer that turns it into kit components. Actions go out as intents, and
//! the application runs the plugin and answers with the new view.

use std::collections::HashMap;
use std::sync::Arc;

use super::intent::InstanceIntent;
use super::panels::act;
use super::{InstanceDetailView, TAB_SCREENSHOTS, TABS};
use crate::key::Key;
use crate::kit;
use crate::kit::TagKind;
use crate::theme::{self, ShellColors};
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, ObjectFit, RenderImage, ScrollHandle, Window, div, img, px};
use gpui_component::{StyledExt as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::PluginTab;
use lumilio_plugin_api::{ActionId, ImageData, KeyKind, ListItem, Tone, View};

/// Plugin tabs sit right after the built-in 截图 tab, so built-in positions
/// never change.
const PLUGIN_TABS_AT: usize = TAB_SCREENSHOTS + 1;
const THUMB: f32 = 56.;
const COVER: f32 = 120.;
/// A table taller than this scrolls inside the page instead of stretching it.
const TABLE_MAX_HEIGHT: f32 = 360.;

/// What a plugin tab currently has to show.
pub(super) enum PluginPage {
    /// The view, its images and one scroll handle per table, in tree order.
    Shown(View, Vec<Option<Arc<RenderImage>>>, Vec<ScrollHandle>),
    /// The plugin was switched off or failed while the page was open.
    Unavailable,
    Failed(String),
}

pub(super) type PluginPages = HashMap<String, PluginPage>;

impl InstanceDetailView {
    /// Which plugin tabs show for this game. A tab that went away while open
    /// returns the page to a built-in tab.
    pub fn plugin_tabs_arrived(&mut self, tabs: Vec<PluginTab>, cx: &mut Context<Self>) {
        self.plugin_tabs = tabs;
        self.keep_enabled_plugin_tabs();
        cx.notify();
    }

    /// One plugin tab's view arrived (`None`: the plugin is off or failed).
    pub fn plugin_view_arrived(
        &mut self,
        plugin: String,
        result: Result<Option<View>, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.plugin_tabs.iter().any(|tab| tab.plugin == plugin) {
            return;
        }
        let page = match result {
            Ok(Some(view)) => {
                let rendered = {
                    let mut images = Vec::new();
                    collect_images(&view, &mut images);
                    images.into_iter().map(render_image).collect()
                };
                let scrolls = (0..count_tables(&view))
                    .map(|_| ScrollHandle::new())
                    .collect();
                PluginPage::Shown(view, rendered, scrolls)
            }
            Ok(None) => PluginPage::Unavailable,
            Err(detail) => PluginPage::Failed(detail),
        };
        self.plugin_pages.insert(plugin, page);
        cx.notify();
    }

    /// Drops tabs and pages of plugins that are no longer enabled.
    pub(super) fn keep_enabled_plugin_tabs(&mut self) {
        if let Some(enabled) = &self.enabled_plugins {
            self.plugin_tabs.retain(|tab| enabled.contains(&tab.plugin));
        }
        let tabs = &self.plugin_tabs;
        self.plugin_pages
            .retain(|plugin, _| tabs.iter().any(|tab| &tab.plugin == plugin));
        if self
            .plugin_open
            .as_ref()
            .is_some_and(|open| !tabs.iter().any(|tab| &tab.plugin == open))
        {
            self.plugin_open = None;
        }
    }

    /// The labels of the tab bar: the built-in tabs with the plugin tabs
    /// inserted after 截图.
    pub(super) fn tab_labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = TABS[..PLUGIN_TABS_AT]
            .iter()
            .map(|label| (*label).to_owned())
            .collect();
        labels.extend(self.plugin_tabs.iter().map(|tab| tab.title.clone()));
        labels.extend(
            TABS[PLUGIN_TABS_AT..]
                .iter()
                .map(|label| (*label).to_owned()),
        );
        labels
    }

    /// The position of the open tab in [`Self::tab_labels`].
    pub(super) fn shown_tab(&self) -> usize {
        let open = self
            .plugin_open
            .as_ref()
            .and_then(|open| self.plugin_tabs.iter().position(|tab| &tab.plugin == open));
        match open {
            Some(index) => PLUGIN_TABS_AT + index,
            None if self.tab < PLUGIN_TABS_AT => self.tab,
            None => self.tab + self.plugin_tabs.len(),
        }
    }

    /// A click on the tab bar, which counts plugin tabs too.
    pub(super) fn open_shown(&mut self, shown: usize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.plugin_tabs.len();
        if (PLUGIN_TABS_AT..PLUGIN_TABS_AT + count).contains(&shown) {
            let plugin = self.plugin_tabs[shown - PLUGIN_TABS_AT].plugin.clone();
            self.open_plugin(plugin, window, cx);
        } else if shown < PLUGIN_TABS_AT {
            self.open_tab(shown, window, cx);
        } else {
            self.open_tab(shown - count, window, cx);
        }
    }

    pub(super) fn open_plugin(
        &mut self,
        plugin: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm = None;
        // ia[instance]: 打开插件提供的标签 | 游戏页标签栏里「截图」后面的标签 | 读取并显示插件给的内容；插件被关闭或出错时标签消失，回到内置标签 | 插件只描述内容，版式由启动器统一
        self.plugin_open = Some(plugin.clone());
        (self.handler)(InstanceIntent::PluginView(plugin), window, cx);
        cx.notify();
    }

    pub(super) fn plugin_panel(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let Some(plugin) = &self.plugin_open else {
            return div().into_any_element();
        };
        match self.plugin_pages.get(plugin) {
            None => kit::empty("正在读取…", "", colors).into_any_element(),
            Some(PluginPage::Unavailable) => {
                kit::empty("这个标签现在不可用", "插件已关闭，或者出了问题", colors)
                    .into_any_element()
            }
            Some(PluginPage::Failed(detail)) => v_flex()
                .gap_3()
                .child(kit::empty("没有读到这部分内容", "可以重试", colors))
                .child(
                    h_flex()
                        .justify_center()
                        .child(kit::technical("plugin-technical", detail.clone())),
                )
                .into_any_element(),
            Some(PluginPage::Shown(view, images, scrolls)) => {
                let mut paint = Paint {
                    plugin,
                    images,
                    scrolls,
                    next_table: 0,
                    next_image: 0,
                    next_id: 0,
                    colors,
                };
                paint.view(view, cx)
            }
        }
    }

    /// Sends a plugin's action; a destructive one asks first.
    fn plugin_act(
        &mut self,
        plugin: String,
        action: ActionId,
        ask: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let intent = InstanceIntent::PluginAction { plugin, action };
        let Some(label) = ask else {
            (self.handler)(intent, window, cx);
            return;
        };
        let handler = self.handler.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (handler, intent) = (handler.clone(), intent.clone());
            alert
                .title(format!("{label}？"))
                .description("这个操作做了就撤不回来。")
                .ok_text(label.clone())
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text("取消")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    handler(intent.clone(), window, cx);
                    true
                })
        });
    }
}

/// Every image in the tree, in the order the renderer meets them.
fn collect_images<'a>(view: &'a View, out: &mut Vec<&'a ImageData>) {
    match view {
        View::Section { children, .. } => {
            children.iter().for_each(|child| collect_images(child, out))
        }
        View::List { items } => out.extend(items.iter().filter_map(|item| item.image.as_ref())),
        View::Detail {
            image, children, ..
        } => {
            out.extend(image.iter());
            children.iter().for_each(|child| collect_images(child, out));
        }
        View::Image(image) => out.push(image),
        View::Model { .. }
        | View::Table { .. }
        | View::Empty { .. }
        | View::Text { .. }
        | View::Tags(_)
        | View::Key { .. } => {}
    }
}

fn count_tables(view: &View) -> usize {
    match view {
        View::Table { .. } => 1,
        View::Section { children, .. } | View::Detail { children, .. } => {
            children.iter().map(count_tables).sum()
        }
        _ => 0,
    }
}

/// RGBA pixels from a plugin as the BGRA bitmap GPUI uploads; a malformed
/// image is left out rather than trusted.
fn render_image(data: &ImageData) -> Option<Arc<RenderImage>> {
    if !data.is_valid() {
        return None;
    }
    let mut bytes = data.rgba.clone();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(data.width, data.height, bytes)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

struct Paint<'a> {
    plugin: &'a str,
    images: &'a [Option<Arc<RenderImage>>],
    scrolls: &'a [ScrollHandle],
    next_table: usize,
    next_image: usize,
    next_id: usize,
    colors: ShellColors,
}

impl Paint<'_> {
    fn id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }

    fn image(&mut self) -> Option<Arc<RenderImage>> {
        let image = self.images.get(self.next_image).cloned().flatten();
        self.next_image += 1;
        image
    }

    fn view(&mut self, view: &View, cx: &mut Context<InstanceDetailView>) -> AnyElement {
        let colors = self.colors;
        match view {
            View::Section { title, children } => {
                let body = v_flex()
                    .w_full()
                    .gap_3()
                    .children(children.iter().map(|child| self.view(child, cx)));
                kit::section(title.clone(), colors, body).into_any_element()
            }
            View::List { items } => {
                let rows: Vec<AnyElement> = items.iter().map(|item| self.item(item, cx)).collect();
                kit::list(rows, colors).into_any_element()
            }
            View::Detail {
                title,
                subtitle,
                image,
                facts,
                children,
            } => {
                let picture = image.as_ref().and_then(|_| self.image());
                let head = h_flex()
                    .w_full()
                    .gap_4()
                    .items_center()
                    .children(
                        picture
                            .map(|picture| picture_box(picture, COVER, colors).into_any_element()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .text_color(colors.foreground)
                                    .child(title.clone()),
                            )
                            .children(
                                subtitle.clone().map(|text| {
                                    div().text_sm().text_color(colors.muted).child(text)
                                }),
                            ),
                    );
                let rows: Vec<AnyElement> = facts
                    .iter()
                    .map(|(label, value)| {
                        let id = self.id();
                        kit::value_row(
                            ("plugin-fact", id),
                            label.clone(),
                            None,
                            value.clone(),
                            None,
                            colors,
                        )
                        .into_any_element()
                    })
                    .collect();
                // Keys sit right under the title, whatever order the tree has
                // them in: a long table must not push them out of reach.
                let (keys, others): (Vec<&View>, Vec<&View>) = children
                    .iter()
                    .partition(|child| matches!(child, View::Key { .. }));
                let mut column = v_flex().w_full().gap_4().child(head);
                if !keys.is_empty() {
                    column = column.child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .children(keys.into_iter().map(|key| self.view(key, cx))),
                    );
                }
                if !rows.is_empty() {
                    column = column.child(kit::list(rows, colors));
                }
                column = column.children(others.into_iter().map(|child| self.view(child, cx)));
                column.into_any_element()
            }
            View::Table { columns, rows } => {
                let line = |cells: &[String], head: bool| {
                    h_flex()
                        .w_full()
                        .gap_3()
                        .py(px(8.))
                        .children(cells.iter().enumerate().map(|(index, cell)| {
                            let text = div()
                                .flex_1()
                                .min_w_0()
                                .when(index > 0, gpui::Styled::flex_none)
                                .when(index > 0, |cell| cell.w(px(96.)).text_right())
                                .text_xs()
                                .child(cell.clone());
                            if head {
                                text.text_color(colors.muted)
                            } else {
                                text.font_family(theme::MONO_FONT)
                                    .text_color(colors.foreground)
                            }
                            .into_any_element()
                        }))
                };
                let table = self.next_table;
                self.next_table += 1;
                let scroll = self.scrolls.get(table).cloned().unwrap_or_default();
                // The head stays put; only the rows scroll, and the wheel
                // stays with them while they can move (as the game log does).
                let body = div()
                    .id(("plugin-table", table))
                    .debug_selector(move || format!("plugin-table-{table}"))
                    .w_full()
                    .max_h(px(TABLE_MAX_HEIGHT))
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .children(rows.iter().map(|row| {
                        div()
                            .w_full()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(line(row, false))
                    }));
                v_flex()
                    .w_full()
                    .child(kit::list(vec![line(columns, true)], colors))
                    .child(kit::keep_wheel(body, &scroll))
                    .into_any_element()
            }
            View::Empty { title, message } => {
                kit::empty(title.clone(), message.clone(), colors).into_any_element()
            }
            View::Text { text, tone } => {
                let base = div().text_sm().child(text.clone());
                match tone {
                    Tone::Body => base.text_color(colors.foreground),
                    Tone::Secondary => base.text_color(colors.muted),
                    Tone::Mono => base
                        .text_color(colors.foreground)
                        .font_family(theme::MONO_FONT),
                }
                .into_any_element()
            }
            View::Tags(tags) => h_flex()
                .gap_1()
                .flex_wrap()
                .children(
                    tags.iter()
                        .map(|tag| kit::tag(tag.clone(), TagKind::Plain, colors)),
                )
                .into_any_element(),
            View::Image(_) => match self.image() {
                Some(picture) => picture_box(picture, COVER, colors).into_any_element(),
                None => div().into_any_element(),
            },
            View::Model { file } => self.model(file, cx),
            View::Key {
                id,
                label,
                kind,
                destructive,
            } => {
                let key_id = self.id();
                let plugin = self.plugin.to_owned();
                let (action, ask) = (id.clone(), destructive.then(|| label.clone()));
                let key = Key::new(("plugin-key", key_id)).label(label.clone());
                let key = match kind {
                    KeyKind::Primary => key.primary(),
                    KeyKind::Ghost => key.ghost(),
                };
                let on_click = act(cx, move |view, window, cx| {
                    view.plugin_act(plugin.clone(), action.clone(), ask.clone(), window, cx);
                });
                let selector = format!("plugin-key-{}", id.0);
                key.on_click(move |_, window: &mut Window, cx: &mut App| on_click(window, cx))
                    .debug_selector(move || selector.clone())
                    .into_any_element()
            }
        }
    }

    /// A card with the key that opens the preview window.
    fn model(&mut self, file: &str, cx: &mut Context<InstanceDetailView>) -> AnyElement {
        let colors = self.colors;
        let id = self.id();
        let (plugin, file) = (self.plugin.to_owned(), file.to_owned());
        let action: AnyElement = if crate::model_preview::SUPPORTED {
            let on_click = act(cx, move |view, window, cx| {
                (view.handler)(
                    InstanceIntent::PluginModel {
                        plugin: plugin.clone(),
                        file: file.clone(),
                    },
                    window,
                    cx,
                );
            });
            Key::new(("plugin-model", id))
                .label("打开 3D 预览")
                .primary()
                .on_click(move |_, window: &mut Window, cx: &mut App| on_click(window, cx))
                .debug_selector(|| "plugin-model-open".into())
                .into_any_element()
        } else {
            div()
                .text_xs()
                .text_color(colors.muted)
                .child("这个系统上还不能预览 3D")
                .into_any_element()
        };
        kit::surface(colors)
            .w_full()
            .p_4()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(colors.foreground)
                                    .child("3D 预览"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted)
                                    .child("用游戏自己的贴图画出整个投影，可以旋转和缩放"),
                            ),
                    )
                    .child(action),
            )
            .into_any_element()
    }

    fn item(&mut self, item: &ListItem, cx: &mut Context<InstanceDetailView>) -> AnyElement {
        let colors = self.colors;
        let id = self.id();
        let lead = item
            .image
            .as_ref()
            .and_then(|_| self.image())
            .map(|picture| picture_box(picture, THUMB, colors).into_any_element());
        let trail = (!item.tags.is_empty() || item.value.is_some()).then(|| {
            h_flex()
                .gap_2()
                .items_center()
                .children(
                    item.tags
                        .iter()
                        .map(|tag| kit::tag(tag.clone(), TagKind::Plain, colors)),
                )
                .children(item.value.clone().map(|value| {
                    div()
                        .font_family(theme::MONO_FONT)
                        .text_xs()
                        .text_color(colors.muted)
                        .child(value)
                }))
                .into_any_element()
        });
        let row = kit::row(
            item.title.clone(),
            item.subtitle.clone().unwrap_or_default(),
            lead,
            trail,
            colors,
        );
        let selector = format!("plugin-item-{}", item.id);
        let wrapper = div()
            .id(("plugin-item", id))
            .debug_selector(move || selector.clone())
            .child(row);
        match &item.open {
            Some(action) => {
                let (plugin, action) = (self.plugin.to_owned(), action.clone());
                wrapper
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.plugin_act(plugin.clone(), action.clone(), None, window, cx);
                    }))
                    .into_any_element()
            }
            None => wrapper.into_any_element(),
        }
    }
}

fn picture_box(picture: Arc<RenderImage>, size: f32, colors: ShellColors) -> gpui::Div {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(6.))
        .overflow_hidden()
        .bg(colors.surface_subtle)
        .child(img(picture).size_full().object_fit(ObjectFit::Cover))
}
