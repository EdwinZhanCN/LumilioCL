use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, Render, Window};
use gpui_component::IndexPath;
use gpui_component::select::{SearchableVec, SelectState};
use gpui_component::v_flex;

use super::about::about;
use super::appearance::appearance;
use super::game_defaults::game_defaults;
use super::general::general;
use super::java::java;
use super::plugins;
use super::settings_actions::Settings;
use super::storage::downloads;
use crate::kit::{Emit, ViewIntent};
use crate::live::{LiveHandler, SettingsView, UpdateStatus};
use crate::theme::ShellColors;
use crate::{kit, tr, tr_all};

// ia[settings]: 打开插件 | L3 设置 tab「插件」 | 显示核心插件、权限、运行状态和各插件的声明式设置；没有插件时显示空列表
#[must_use]
pub fn tabs() -> &'static [&'static str] {
    tr_all![
        "settings-tab-general",
        "settings-tab-appearance",
        "settings-tab-game-defaults",
        "settings-tab-java",
        "settings-tab-downloads",
        "settings-tab-about",
        "settings-tab-plugins",
    ]
}

/// The choice group of the page's tab.
pub const TAB_GROUP: u8 = 201;
/// The choice group of the plugin the 插件 tab shows.
pub const PLUGIN_GROUP: u8 = 202;

pub(super) fn fullscreen_choices() -> &'static [&'static str] {
    tr_all![
        "settings-fullscreen-off",
        "settings-fullscreen-on",
        "settings-fullscreen-unset",
    ]
}

/// The 插件 tab: its list and its detail scroll on their own.
pub(super) const PLUGINS_TAB: usize = 6;

/// What the page shows of the rest of the launcher, as the shell knows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsInputs {
    /// Where the launcher's own update stands, for the 关于 tab.
    pub update_status: UpdateStatus,
}

/// The page's tab and the plugin its 插件 tab shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettingsChoices {
    pub tab: usize,
    pub plugin: usize,
}

impl SettingsChoices {
    pub(super) fn apply(&mut self, intent: ViewIntent) {
        match intent {
            ViewIntent::Choose(TAB_GROUP, tab) => self.tab = tab,
            ViewIntent::Choose(PLUGIN_GROUP, plugin) => self.plugin = plugin,
            _ => {}
        }
    }
}

/// The inputs the page owns. Made the first time it draws, because they
/// need a window.
pub struct SettingsControls {
    /// The translation provider; Microsoft Edge is the only one.
    pub translation_provider: Entity<SelectState<SearchableVec<String>>>,
    /// The plugin list's scroll, so it keeps the wheel while it can scroll.
    pub plugin_scroll: gpui::ScrollHandle,
}

impl SettingsControls {
    fn new(window: &mut Window, cx: &mut gpui::App) -> Self {
        Self {
            translation_provider: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec!["Microsoft Edge".to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            plugin_scroll: gpui::ScrollHandle::new(),
        }
    }
}

/// What the tabs draw from.
pub(super) struct SettingsCtx<'a> {
    pub colors: ShellColors,
    pub choices: SettingsChoices,
    pub emit: Emit,
    pub handler: LiveHandler,
    pub controls: &'a SettingsControls,
    pub update_status: &'a UpdateStatus,
    /// The look in force and the themes found, for the 外观 tab.
    pub look: Option<&'a crate::theme::Look>,
    pub catalog: Option<&'a crate::theme::Catalog>,
    pub wallpaper: Option<&'a crate::theme::Wallpaper>,
}

impl Settings {
    /// Where the page sends what it asks for: the shell's handler, which
    /// passes this page's own intents back to it.
    pub fn set_handler(&mut self, handler: LiveHandler) {
        self.handler = Some(handler);
    }

    /// What the page shows, as the shell knows it.
    pub fn set_inputs(&mut self, inputs: SettingsInputs, cx: &mut Context<Self>) {
        if self.inputs != inputs {
            self.inputs = inputs;
            cx.notify();
        }
    }

    #[must_use]
    pub fn choices(&self) -> SettingsChoices {
        self.choices
    }

    /// The settings last read, if they have been.
    #[must_use]
    pub fn settings(&self) -> Option<&SettingsView> {
        self.view.as_ref()
    }

    /// Shows these settings as if just read, for design review and tests.
    pub fn show_settings(&mut self, view: SettingsView, cx: &mut Context<Self>) {
        self.view = Some(view);
        cx.notify();
    }

    /// Opens a tab directly, for design review and tests.
    pub fn show_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.choices.tab = tab;
        cx.notify();
    }

    /// Applies a choice made on the page: a tab, or a plugin to show.
    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.choices.apply(intent);
        cx.notify();
    }
}

impl Render for Settings {
    /// The page is an app shell: the header and tabs stay put and only the
    /// body below them scrolls. The 插件 tab owns its scrolling instead
    /// (master–detail).
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.controls.is_none() {
            self.controls = Some(SettingsControls::new(window, cx));
        }
        let this = cx.entity().downgrade();
        let emit: Emit = Rc::new(move |intent, _, cx| {
            let _ = this.update(cx, |settings, cx| settings.apply_view_intent(intent, cx));
        });
        let ctx = SettingsCtx {
            colors: ShellColors::current(cx),
            choices: self.choices,
            emit,
            handler: self
                .handler
                .clone()
                .unwrap_or_else(|| Rc::new(|_, _, _| {})),
            controls: self
                .controls
                .as_ref()
                .expect("controls are made before the page draws"),
            update_status: &self.inputs.update_status,
            look: cx.try_global::<crate::theme::Look>(),
            catalog: cx.try_global::<crate::theme::Catalog>(),
            wallpaper: cx.try_global::<crate::theme::Wallpaper>(),
        };
        page(self.view.as_ref(), &ctx)
    }
}

fn page(view: Option<&SettingsView>, ctx: &SettingsCtx) -> AnyElement {
    let colors = ctx.colors;
    let tabs = tabs();
    let tab = ctx.choices.tab.min(tabs.len() - 1);
    let body = match view {
        None => kit::scroll_body(
            "settings-scroll",
            kit::empty(tr!("settings-loading"), "", colors),
        )
        .into_any_element(),
        Some(view) if tab == PLUGINS_TAB => {
            kit::pane_body(plugins::render(view, ctx)).into_any_element()
        }
        Some(view) => {
            let content = match tab {
                0 => general(view, ctx),
                1 => appearance(view, ctx),
                2 => game_defaults(view, ctx),
                3 => java(view, ctx),
                4 => downloads(view, ctx),
                _ => about(view, ctx),
            };
            kit::scroll_body(
                "settings-scroll",
                kit::entrance(content, ("settings-body", tab)),
            )
            .into_any_element()
        }
    };
    let head = v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            tr!("settings-title"),
            tr!("settings-subtitle"),
            None,
            colors,
        ))
        .child(kit::toolbar(
            Some(
                kit::tabs("settings-tabs", tabs, tab, {
                    let emit = ctx.emit.clone();
                    move |index, window, app| {
                        emit(ViewIntent::Choose(TAB_GROUP, index), window, app)
                    }
                })
                .into_any_element(),
            ),
            None,
        ));
    kit::fixed_page("live-settings", head, body).into_any_element()
}
