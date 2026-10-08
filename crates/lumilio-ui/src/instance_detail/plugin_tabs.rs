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
use crate::tr;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, ObjectFit, RenderImage, SharedString, Window, div, img, px,
};
use gpui_component::{StyledExt as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::PluginTab;
use lumilio_plugin_api::{ActionId, ImageData, KeyKind, ListItem, Tone, View};

/// Plugin tabs sit right after the built-in 截图 tab, so built-in positions
/// never change.
const PLUGIN_TABS_AT: usize = TAB_SCREENSHOTS + 1;
const THUMB: f32 = 56.;
const COVER: f32 = 120.;

/// What a plugin tab currently has to show.
pub(super) enum PluginPage {
    /// The view, its images and models, in tree order.
    Shown(
        View,
        Vec<Option<Arc<RenderImage>>>,
        Vec<Entity<crate::model_view::ModelView>>,
    ),
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
                let mut files = Vec::new();
                collect_models(&view, &mut files);
                let mut models = Vec::new();
                for file in files {
                    let (plugin, file, handler) =
                        (plugin.clone(), file.to_owned(), self.handler.clone());
                    models.push(cx.new(|cx| {
                        crate::model_view::ModelView::new(
                            std::rc::Rc::new(move |request, window, cx| {
                                handler(
                                    InstanceIntent::LoadModel {
                                        plugin: plugin.clone(),
                                        file: file.clone(),
                                        request,
                                    },
                                    window,
                                    cx,
                                );
                            }),
                            cx,
                        )
                    }));
                }
                PluginPage::Shown(view, rendered, models)
            }
            Ok(None) => PluginPage::Unavailable,
            Err(detail) => PluginPage::Failed(detail),
        };
        if self.plugin_open.as_ref() == Some(&plugin) {
            self.plugin_pane_scroll
                .set_offset(gpui::point(px(0.), px(0.)));
        }
        self.plugin_pages.insert(plugin, page);
        cx.notify();
    }

    /// Late results cannot enter a replacement detail, even for the same file.
    pub fn plugin_model_arrived(
        &mut self,
        request: u64,
        result: Result<lumilio_core::ModelPreview, String>,
        cx: &mut Context<Self>,
    ) {
        for page in self.plugin_pages.values() {
            if let PluginPage::Shown(_, _, models) = page
                && let Some(model) = models
                    .iter()
                    .find(|model| model.read(cx).request_id() == Some(request))
            {
                model.update(cx, |model, cx| model.assets(result, cx));
                return;
            }
        }
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

    /// Whether the tab bar carries the single 插件 tab.
    fn has_plugin_tab(&self) -> bool {
        !self.plugin_tabs.is_empty()
    }

    /// The labels of the tab bar: the built-in tabs, with one 插件 tab after
    /// 截图 while any plugin contributes one. The plugins themselves live in
    /// that tab's list, so the bar never grows with them.
    pub(super) fn tab_labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = TABS[..PLUGIN_TABS_AT]
            .iter()
            .map(|label| (*label).to_owned())
            .collect();
        if self.has_plugin_tab() {
            labels.push(tr!("settings-tab-plugins").to_owned());
        }
        labels.extend(
            TABS[PLUGIN_TABS_AT..]
                .iter()
                .map(|label| (*label).to_owned()),
        );
        labels
    }

    /// The position of the open tab in [`Self::tab_labels`].
    pub(super) fn shown_tab(&self) -> usize {
        if self.plugin_open.is_some() && self.has_plugin_tab() {
            return PLUGIN_TABS_AT;
        }
        if self.tab < PLUGIN_TABS_AT || !self.has_plugin_tab() {
            self.tab
        } else {
            self.tab + 1
        }
    }

    /// A click on the tab bar. Selecting 插件 opens its list; which plugin is
    /// shown is chosen there, not on the bar.
    pub(super) fn open_shown(&mut self, shown: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_plugin_tab() && shown == PLUGIN_TABS_AT {
            let plugin = self
                .plugin_open
                .clone()
                .filter(|open| self.plugin_tabs.iter().any(|tab| &tab.plugin == open))
                .or_else(|| self.plugin_tabs.first().map(|tab| tab.plugin.clone()));
            if let Some(plugin) = plugin {
                self.open_plugin(plugin, window, cx);
            }
        } else if shown < PLUGIN_TABS_AT || !self.has_plugin_tab() {
            self.open_tab(shown, window, cx);
        } else {
            self.open_tab(shown - 1, window, cx);
        }
    }

    pub(super) fn open_plugin(
        &mut self,
        plugin: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm = None;
        self.plugin_pane_scroll
            .set_offset(gpui::point(px(0.), px(0.)));
        // ia[instance]: 打开插件标签 | 游戏页「插件」标签内的左侧列表 | 读取并显示插件给的内容；插件被关闭或出错时标签消失，回到内置标签 | 插件只描述内容，版式由启动器统一
        self.plugin_open = Some(plugin.clone());
        (self.handler)(InstanceIntent::PluginView(plugin), window, cx);
        cx.notify();
    }

    pub(super) fn plugin_panel(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let Some(plugin) = &self.plugin_open else {
            return div().into_any_element();
        };
        let content = match self.plugin_pages.get(plugin) {
            None => kit::empty(tr!("library-loading"), "", colors).into_any_element(),
            Some(PluginPage::Unavailable) => kit::empty(
                tr!("instance-plugin-unavailable-title"),
                tr!("instance-plugin-unavailable-help"),
                colors,
            )
            .into_any_element(),
            Some(PluginPage::Failed(detail)) => v_flex()
                .gap_3()
                .child(kit::empty(
                    tr!("instance-section-read-failed"),
                    tr!("instance-section-retry-help"),
                    colors,
                ))
                .child(
                    h_flex()
                        .justify_center()
                        .child(kit::technical("plugin-technical", detail.clone())),
                )
                .into_any_element(),
            Some(PluginPage::Shown(view, images, models)) => {
                let mut paint = Paint {
                    plugin,
                    images,
                    models,
                    next_table: 0,
                    next_model: 0,
                    next_image: 0,
                    next_id: 0,
                    colors,
                };
                paint.view(view, cx)
            }
        };
        // ia[instance]: 切换插件标签 | 游戏页「插件」标签内的左侧列表 | 右侧显示所选插件的内容
        let entity = cx.entity().downgrade();
        let list = kit::keep_wheel(
            v_flex()
                .id("instance-plugin-list")
                .w(px(180.))
                .flex_none()
                .h_full()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.plugin_list_scroll)
                .children(self.plugin_tabs.iter().map(|tab| {
                    let chosen = &tab.plugin == plugin;
                    let target = tab.plugin.clone();
                    let entity = entity.clone();
                    kit::led_option(
                        SharedString::from(format!("instance-plugin-tab-{}", tab.plugin)),
                        tab.title.clone(),
                        chosen,
                        colors,
                        move |window, cx| {
                            let _ = entity.update(cx, |view, cx| {
                                view.open_plugin(target.clone(), window, cx)
                            });
                        },
                    )
                })),
            &self.plugin_list_scroll,
        )
        .into_any_element();
        // The list is pinned; only the pane beside it moves, so it stays in
        // view however long the plugin's content is.
        h_flex()
            .w_full()
            .h_full()
            .min_h_0()
            .gap_4()
            .child(list)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .min_h_0()
                    .child(
                        div()
                            .id("instance-plugin-pane")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.plugin_pane_scroll)
                            .child(div().w_full().pb(theme::BOTTOM_SAFE_AREA).child(content)),
                    )
                    .child({
                        // ia[instance]: 返回插件内容顶部 | 右下角悬浮「返回顶部」按钮 | 将详情和材料清单共享的滚动区域移回顶部
                        div()
                            .absolute()
                            .right_3()
                            .bottom(theme::BOTTOM_SAFE_AREA)
                            .child(
                                Key::new("plugin-back-to-top")
                                    .label(tr!("instance-back-to-top"))
                                    .white()
                                    .debug_selector(|| "plugin-back-to-top".into())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.plugin_pane_scroll
                                            .set_offset(gpui::point(px(0.), px(0.)));
                                        cx.notify();
                                    })),
                            )
                    }),
            )
            .into_any_element()
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
                .title(tr!("instance-plugin-confirm-title", label = label.as_str()))
                .description(tr!("instance-plugin-confirm-body"))
                .ok_text(label.clone())
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text(tr!("common-cancel"))
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
        View::Map
        | View::Model { .. }
        | View::Table { .. }
        | View::Empty { .. }
        | View::Text { .. }
        | View::Tags(_)
        | View::Key { .. } => {}
    }
}

fn collect_models<'a>(view: &'a View, out: &mut Vec<&'a str>) {
    match view {
        View::Model { file } => out.push(file),
        View::Section { children, .. } | View::Detail { children, .. } => {
            children.iter().for_each(|child| collect_models(child, out));
        }
        _ => {}
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
    models: &'a [Entity<crate::model_view::ModelView>],
    next_model: usize,
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
                // Consume the original tree order before moving entry keys into
                // the header, so nested images/models keep their asset identity.
                let mut actions = Vec::new();
                let mut body = Vec::new();
                for child in children {
                    let element = self.view(child, cx);
                    if matches!(child, View::Key { .. } | View::Model { .. }) {
                        actions.push(element);
                    } else {
                        body.push(element);
                    }
                }
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
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id(("plugin-detail-title", self.id()))
                                            .debug_selector(|| "plugin-detail-title".into())
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_lg()
                                            .font_semibold()
                                            .text_color(colors.foreground)
                                            .child(title.clone()),
                                    )
                                    .child(h_flex().flex_none().gap_2().children(actions)),
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
                let mut column = v_flex().w_full().gap_4().child(head);
                if !rows.is_empty() {
                    column = column.child(kit::list(rows, colors));
                }
                column = column.children(body);
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
                // Header and material rows follow the detail pane's scroll.
                let body = div()
                    .id(("plugin-table", table))
                    .debug_selector(move || format!("plugin-table-{table}"))
                    .w_full()
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
                    .child(body)
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
            View::Map => div().into_any_element(),
            View::Model { .. } => {
                let model = self.models.get(self.next_model);
                self.next_model += 1;
                model.map_or_else(
                    || div().into_any_element(),
                    |model| model.clone().into_any_element(),
                )
            }
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
