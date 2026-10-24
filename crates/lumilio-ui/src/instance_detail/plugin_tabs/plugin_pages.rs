use std::collections::HashMap;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, RenderImage, SharedString, Window, div, px};
use gpui_component::{WindowExt as _, h_flex, v_flex};
use lumilio_core::PluginTab;
use lumilio_plugin_api::{ActionId, View};

use super::super::intent::InstanceIntent;
use super::super::{InstanceDetailView, TAB_SCREENSHOTS, TABS};
use super::images::{Paint, collect_images, collect_models, render_image};
use crate::key::Key;
use crate::theme::ShellColors;
use crate::{kit, theme, tr};

/// Plugin tabs sit right after the built-in 截图 tab, so built-in positions
/// never change.
pub(super) const PLUGIN_TABS_AT: usize = TAB_SCREENSHOTS + 1;
pub(super) const THUMB: f32 = 56.;
pub(super) const COVER: f32 = 120.;

/// What a plugin tab currently has to show.
pub(in super::super) enum PluginPage {
    /// The view, its images and models, in tree order.
    Shown(
        View,
        Vec<Option<Arc<RenderImage>>>,
        Vec<Entity<crate::model_view::ModelView>>,
        Vec<Entity<crate::world_explorer::MapView>>,
        Vec<Entity<crate::schematic_studio::StudioView>>,
    ),
    /// The plugin was switched off or failed while the page was open.
    Unavailable,
    Failed(String),
}

pub(in super::super) type PluginPages = HashMap<String, PluginPage>;

/// Returns a page's pictures to GPUI. Its maps and 3D previews let go of
/// theirs when dropped; the plugin's pictures are bare images whose textures
/// would otherwise stay in the window's atlas.
pub(super) fn release(page: PluginPage, cx: &mut App) {
    let PluginPage::Shown(_, images, ..) = page else {
        return;
    };
    let images: Vec<_> = images.into_iter().flatten().collect();
    if images.is_empty() {
        return;
    }
    // Outside any window update, so every window's atlas is reached.
    cx.defer(move |cx| {
        for image in images {
            cx.drop_image(image, None);
        }
    });
}

impl InstanceDetailView {
    /// Plugin pages exist only while shown: a map holds its tiles and a 3D
    /// preview its renderer, in memory and on the GPU. Every page but
    /// `keep`'s is dropped; a page shown again is asked for again, as opening
    /// its tab always does.
    pub(in super::super) fn drop_plugin_pages(&mut self, keep: Option<&str>, cx: &mut App) {
        let gone: Vec<String> = self
            .plugin_pages
            .keys()
            .filter(|plugin| Some(plugin.as_str()) != keep)
            .cloned()
            .collect();
        for plugin in gone {
            if let Some(page) = self.plugin_pages.remove(&plugin) {
                release(page, cx);
            }
        }
    }

    /// Another page took this one's place on screen. Its plugin pages go; an
    /// open plugin tab asks for its page again once this page is shown.
    pub fn left_screen(&mut self, cx: &mut Context<Self>) {
        if self.plugin_pages.is_empty() {
            return;
        }
        self.drop_plugin_pages(None, cx);
        if self.plugin_open.is_some() {
            self.refresh_plugin_tabs = true;
        }
    }

    /// The game ended and may have written its saves: open maps look at the
    /// world's files again, and open studios at their folder (and whether
    /// they may write).
    pub(in super::super) fn maps_files_changed(&mut self, cx: &mut Context<Self>) {
        for page in self.plugin_pages.values() {
            if let PluginPage::Shown(_, _, _, maps, studios) = page {
                for map in maps {
                    map.update(cx, |map, cx| map.files_changed(cx));
                }
                for studio in studios {
                    studio.update(cx, |studio, cx| studio.files_changed(cx));
                }
            }
        }
    }

    /// The game started: open studios ask again whether they may write.
    pub(in super::super) fn studios_write_changed(&mut self, cx: &mut Context<Self>) {
        for page in self.plugin_pages.values() {
            if let PluginPage::Shown(_, _, _, _, studios) = page {
                for studio in studios {
                    studio.update(cx, |studio, cx| studio.write_access_changed(cx));
                }
            }
        }
    }

    /// Which plugin tabs show for this game. A tab that went away while open
    /// returns the page to a built-in tab.
    pub fn plugin_tabs_arrived(&mut self, tabs: Vec<PluginTab>, cx: &mut Context<Self>) {
        self.plugin_tabs = tabs;
        self.keep_enabled_plugin_tabs(cx);
        let early: Vec<String> = self
            .early_plugin_views
            .keys()
            .filter(|plugin| self.plugin_tabs.iter().any(|tab| &tab.plugin == *plugin))
            .cloned()
            .collect();
        for plugin in early {
            if let Some(result) = self.early_plugin_views.remove(&plugin) {
                self.plugin_view_arrived(plugin, result, cx);
            }
        }
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
            // The page was opened before the tab list came (the review
            // hook, a quick click after opening the game): keep the answer
            // for the list.
            if self.plugin_open.as_deref() == Some(plugin.as_str()) {
                self.early_plugin_views.insert(plugin, result);
            }
            return;
        }
        // A studio answers a plugin action (a CSV export) with the same view;
        // the open studio keeps its file, viewport and inspector.
        if let (
            Ok(Some(View::Studio { folder })),
            Some(PluginPage::Shown(View::Studio { folder: open }, ..)),
        ) = (&result, self.plugin_pages.get(&plugin))
            && folder == open
        {
            cx.notify();
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
                let mut maps = Vec::new();
                fn count_maps(view: &View) -> usize {
                    match view {
                        View::Map => 1,
                        View::Section { children, .. } | View::Detail { children, .. } => {
                            children.iter().map(count_maps).sum()
                        }
                        _ => 0,
                    }
                }
                for _ in 0..count_maps(&view) {
                    let (handler, pop_out) = (self.handler.clone(), self.handler.clone());
                    maps.push(cx.new(|cx| {
                        crate::world_explorer::MapView::new(
                            std::rc::Rc::new(move |request, window, cx| {
                                handler(InstanceIntent::LoadMap { request }, window, cx)
                            }),
                            cx,
                        )
                        .on_pop_out(std::rc::Rc::new(
                            move |handoff, window, cx| {
                                pop_out(InstanceIntent::PopOutMap { handoff }, window, cx)
                            },
                        ))
                    }));
                }
                let mut studios = Vec::new();
                if let View::Studio { folder } = &view {
                    studios.push(self.new_studio(&plugin, folder, cx));
                }
                PluginPage::Shown(view, rendered, models, maps, studios)
            }
            Ok(None) => PluginPage::Unavailable,
            Err(detail) => PluginPage::Failed(detail),
        };
        if self.plugin_open.as_ref() == Some(&plugin) {
            self.plugin_pane_scroll
                .set_offset(gpui::point(px(0.), px(0.)));
        }
        if let Some(old) = self.plugin_pages.insert(plugin, page) {
            release(old, cx);
        }
        cx.notify();
    }

    /// Late results cannot enter a replacement detail, even for the same file.
    pub fn plugin_model_arrived(
        &mut self,
        request: u64,
        result: Result<lumilio_core::ModelPreview, String>,
        cx: &mut Context<Self>,
    ) {
        let mut result = Some(result);
        for page in self.plugin_pages.values() {
            if let PluginPage::Shown(_, _, models, _, studios) = page {
                if let Some(model) = models
                    .iter()
                    .find(|model| model.read(cx).request_id() == Some(request))
                {
                    let result = result.take().expect("answered once");
                    model.update(cx, |model, cx| model.assets(result, cx));
                    return;
                }
                for studio in studios {
                    let asked = studio.read(cx).viewport_request() == Some(request);
                    if asked {
                        let result = result.take().expect("answered once");
                        studio.update(cx, |studio, cx| studio.model_arrived(request, result, cx));
                        return;
                    }
                }
            }
        }
    }

    /// A studio's connection to the application arrived.
    pub fn plugin_studio_arrived(
        &mut self,
        request: u64,
        connection: crate::schematic_studio::Connection,
        cx: &mut Context<Self>,
    ) {
        for page in self.plugin_pages.values() {
            if let PluginPage::Shown(_, _, _, _, studios) = page
                && let Some(studio) = studios
                    .iter()
                    .find(|studio| studio.entity_id().as_u64() == request)
            {
                studio.update(cx, |studio, cx| studio.connect(connection, cx));
                return;
            }
        }
    }

    pub(super) fn new_studio(
        &self,
        plugin: &str,
        folder: &str,
        cx: &mut Context<Self>,
    ) -> Entity<crate::schematic_studio::StudioView> {
        use crate::schematic_studio::{Placement, StudioAction, StudioView};
        let (load, load_model, act, open) = (
            self.handler.clone(),
            self.handler.clone(),
            self.handler.clone(),
            self.handler.clone(),
        );
        let (for_load, for_model, for_act, for_open) = (
            plugin.to_owned(),
            plugin.to_owned(),
            plugin.to_owned(),
            plugin.to_owned(),
        );
        let (folder_name, folder_open) = (folder.to_owned(), folder.to_owned());
        cx.new(|cx| {
            StudioView::new(
                plugin.to_owned(),
                folder.to_owned(),
                Placement::Pane,
                std::rc::Rc::new(move |request, window, cx| {
                    load(
                        InstanceIntent::LoadStudio {
                            plugin: for_load.clone(),
                            folder: folder_name.clone(),
                            request,
                        },
                        window,
                        cx,
                    )
                }),
                std::rc::Rc::new(move |file, request, window, cx| {
                    load_model(
                        InstanceIntent::LoadModel {
                            plugin: for_model.clone(),
                            file,
                            request,
                        },
                        window,
                        cx,
                    )
                }),
                std::rc::Rc::new(move |action, window, cx| {
                    let action = match action {
                        StudioAction::Reveal(file) => format!("reveal:{file}"),
                        StudioAction::ExportCsv(file) => format!("export:{file}"),
                    };
                    act(
                        InstanceIntent::PluginAction {
                            plugin: for_act.clone(),
                            action: ActionId::new(action),
                        },
                        window,
                        cx,
                    )
                }),
                cx,
            )
            .on_open_window(std::rc::Rc::new(move |file, window, cx| {
                open(
                    InstanceIntent::OpenStudioWindow {
                        plugin: for_open.clone(),
                        folder: folder_open.clone(),
                        file,
                    },
                    window,
                    cx,
                )
            }))
        })
    }

    pub fn plugin_map_arrived(
        &mut self,
        request: u64,
        connection: crate::world_explorer::Connection,
        cx: &mut Context<Self>,
    ) {
        for page in self.plugin_pages.values() {
            if let PluginPage::Shown(_, _, _, maps, _) = page
                && let Some(map) = maps.iter().find(|map| map.entity_id().as_u64() == request)
            {
                map.update(cx, |map, cx| map.connect(connection, cx));
                return;
            }
        }
    }

    /// Drops tabs and pages of plugins that are no longer enabled.
    pub(in super::super) fn keep_enabled_plugin_tabs(&mut self, cx: &mut App) {
        if let Some(enabled) = &self.enabled_plugins {
            self.plugin_tabs.retain(|tab| enabled.contains(&tab.plugin));
        }
        let tabs = &self.plugin_tabs;
        let gone: Vec<String> = self
            .plugin_pages
            .keys()
            .filter(|plugin| !tabs.iter().any(|tab| &tab.plugin == *plugin))
            .cloned()
            .collect();
        for plugin in gone {
            if let Some(page) = self.plugin_pages.remove(&plugin) {
                release(page, cx);
            }
        }
        if self
            .plugin_open
            .as_ref()
            .is_some_and(|open| !tabs.iter().any(|tab| &tab.plugin == open))
        {
            self.plugin_open = None;
        }
    }

    /// Whether the tab bar carries the single 插件 tab.
    pub(super) fn has_plugin_tab(&self) -> bool {
        !self.plugin_tabs.is_empty()
    }

    /// The labels of the tab bar: the built-in tabs, with one 插件 tab after
    /// 截图 while any plugin contributes one. The plugins themselves live in
    /// that tab's list, so the bar never grows with them.
    pub(in super::super) fn tab_labels(&self) -> Vec<String> {
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
    pub(in super::super) fn shown_tab(&self) -> usize {
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
    pub(in super::super) fn open_shown(
        &mut self,
        shown: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    /// Opens `plugin`'s pane as its entry in the 插件 list does; the
    /// application's review hook (`LUMILIO_INSTANCE_PLUGIN`) uses it.
    pub fn open_plugin_pane(
        &mut self,
        plugin: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_plugin(plugin, window, cx);
    }

    pub(in super::super) fn open_plugin(
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
        self.drop_plugin_pages(Some(&plugin), cx);
        (self.handler)(InstanceIntent::PluginView(plugin), window, cx);
        cx.notify();
    }

    pub(in super::super) fn plugin_panel(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(plugin) = &self.plugin_open else {
            return div().into_any_element();
        };
        let full_map = matches!(
            self.plugin_pages.get(plugin),
            Some(PluginPage::Shown(View::Map | View::Studio { .. }, ..))
        );
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
            Some(PluginPage::Shown(view, images, models, maps, studios)) => {
                let mut paint = Paint {
                    plugin,
                    images,
                    models,
                    maps,
                    studios,
                    next_map: 0,
                    next_studio: 0,
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
                        SharedString::from(tab.title.get(crate::i18n::locale().tag())),
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
                            .child(
                                div()
                                    .w_full()
                                    // The map fills the pane's height, but stops above
                                    // the floating navigation bar like every other page.
                                    .when(full_map, |pane| pane.h_full().min_h_0())
                                    .pb(theme::BOTTOM_SAFE_AREA)
                                    .child(content),
                            ),
                    )
                    .children((!full_map).then(|| {
                        // ia[instance]: 返回插件内容顶部 | 非地图插件内容 · 右下角悬浮「返回顶部」按钮 | 将详情和材料清单共享的滚动区域移回顶部；地图使用填满高度的视口
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
                    })),
            )
            .into_any_element()
    }

    /// Sends a plugin's action; a destructive one asks first.
    pub(super) fn plugin_act(
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
