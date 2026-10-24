use gpui::prelude::*;
use gpui::{AnyElement, Context, SharedString};
use gpui_component::{h_flex, v_flex};

use super::super::{InstanceDetailView, InstanceIntent};
use crate::settings_dialog::{DialogSpec, SettingsDialog};
use crate::theme::ShellColors;
use crate::{kit, tr, tr_all};

/// Label lists read through `Deref`, so each read takes the words again and a
/// language switch reaches the segment bars and dropdowns.
pub static SUBTABS: Subtabs = Subtabs;

pub struct Subtabs;

impl std::ops::Deref for Subtabs {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "instance-settings-tab-game",
            "instance-settings-tab-options",
            "instance-settings-tab-runtime",
            "instance-settings-tab-java",
            "instance-settings-tab-performance",
            "instance-settings-tab-advanced",
        ]
    }
}

/// The groups of the Settings tab, in [`SUBTABS`] order.
pub const SETTINGS_GAME: usize = 0;
pub const SETTINGS_OPTIONS: usize = 1;
pub const SETTINGS_RUNTIME: usize = 2;
pub const SETTINGS_JAVA: usize = 3;
pub const SETTINGS_PERFORMANCE: usize = 4;
pub const SETTINGS_ADVANCED: usize = 5;

pub(super) static FULLSCREEN: Fullscreen = Fullscreen;

pub(super) struct Fullscreen;

impl std::ops::Deref for Fullscreen {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "instance-settings-follow-default",
            "settings-fullscreen-on",
            "settings-fullscreen-off",
        ]
    }
}

pub(super) static AFTER_LAUNCH: AfterLaunch = AfterLaunch;

pub(super) struct AfterLaunch;

impl std::ops::Deref for AfterLaunch {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "instance-settings-follow-default",
            "settings-after-launch-keep",
            "settings-after-launch-hide",
        ]
    }
}

pub(super) static QUICK_PLAY: QuickPlay = QuickPlay;

pub(super) struct QuickPlay;

impl std::ops::Deref for QuickPlay {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "common-none",
            "instance-worlds-world",
            "instance-worlds-server",
        ]
    }
}

pub(super) static FOLLOW_OR_OWN: FollowOrOwn = FollowOrOwn;

pub(super) struct FollowOrOwn;

impl std::ops::Deref for FollowOrOwn {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all!["instance-settings-follow-default", "instance-settings-own"]
    }
}

impl InstanceDetailView {
    pub(super) fn setting_row(
        &self,
        id: &'static str,
        label: &'static str,
        help: Option<String>,
        value: impl Into<SharedString>,
        control: Option<AnyElement>,
        colors: ShellColors,
    ) -> gpui::Stateful<gpui::Div> {
        kit::value_row(id, label, help.map(Into::into), value, control, colors)
            .debug_selector(move || id.to_owned())
    }

    pub(super) fn edit_spec(
        &self,
        id: &'static str,
        spec: impl Fn() -> DialogSpec<InstanceIntent> + 'static,
    ) -> AnyElement {
        let handler = self.handler.clone();
        let busy = self.busy;
        crate::theme::clickable(
            kit::ghost(id, tr!("common-edit"), move |window, cx| {
                SettingsDialog::open(spec(), handler.clone(), window, cx);
            })
            .disabled(busy)
            .debug_selector(move || id.to_owned()),
            !busy,
        )
        .into_any_element()
    }

    /// The Settings tab: a second-level choice and that group's rows.
    pub(in super::super) fn settings_tab(
        &mut self,
        window: &mut gpui::Window,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sub = self.settings_sub.min(SUBTABS.len() - 1);
        let entity = cx.entity().downgrade();
        let segments = kit::segments(
            "instance-settings-sub",
            &SUBTABS,
            sub,
            move |index, window, cx| {
                let _ = entity.update(cx, |view, cx| {
                    view.settings_sub = index;
                    if index == SETTINGS_OPTIONS {
                        view.request(super::super::Section::GameOptions, window, cx);
                    }
                    cx.notify();
                });
            },
        );
        let body = match sub {
            SETTINGS_GAME => self.group_game(colors),
            SETTINGS_OPTIONS => self.group_options(colors, cx),
            SETTINGS_RUNTIME => self.group_runtime(colors, cx),
            SETTINGS_JAVA => self.group_java(window, colors, cx),
            SETTINGS_PERFORMANCE => self.performance(colors, cx),
            _ => self.group_advanced(colors),
        };
        v_flex()
            .w_full()
            .gap_5()
            // ia[instance.settings]: 选项 | 设置 tab 的分段「选项」 | 显示游戏内「选项」里的设置（按键、音量、画面、聊天、辅助功能、语言），游戏关着时可在这里改
            .child(h_flex().child(segments))
            .child(kit::entrance(body, ("instance-settings-body", sub)))
            .into_any_element()
    }
}
