//! The Settings tab of an Instance (`docs/ia/instance/settings.md`): five
//! groups of rows. Each shows the value in effect and where it comes from;
//! anything typed is edited in a dialog, and "恢复默认" removes the override
//! instead of copying the launcher default into the instance.
//!
//! A child module of `instance_detail`, so it shares the view's private state.

mod groups;
mod saves;
mod texts;

#[cfg(test)]
mod tests;

use super::{InstanceDetailView, InstanceIntent};
use crate::settings_dialog::{DialogSpec, SettingsDialog};
use crate::theme::ShellColors;
use crate::{kit, tr, tr_all};
use gpui::prelude::*;
use gpui::{AnyElement, Context, SharedString};
use gpui_component::{h_flex, v_flex};

/// Label lists read through `Deref`, so each read takes the words again and a
/// language switch reaches the segment bars and dropdowns.
pub static SUBTABS: Subtabs = Subtabs;

pub struct Subtabs;

impl std::ops::Deref for Subtabs {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all![
            "instance-settings-tab-game",
            "instance-settings-tab-runtime",
            "instance-settings-tab-java",
            "instance-settings-tab-performance",
            "instance-settings-tab-advanced",
        ]
    }
}

static FULLSCREEN: Fullscreen = Fullscreen;

struct Fullscreen;

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

static AFTER_LAUNCH: AfterLaunch = AfterLaunch;

struct AfterLaunch;

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

static QUICK_PLAY: QuickPlay = QuickPlay;

struct QuickPlay;

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

static FOLLOW_OR_OWN: FollowOrOwn = FollowOrOwn;

struct FollowOrOwn;

impl std::ops::Deref for FollowOrOwn {
    type Target = [&'static str];

    fn deref(&self) -> &'static [&'static str] {
        tr_all!["instance-settings-follow-default", "instance-settings-own"]
    }
}

impl InstanceDetailView {
    fn setting_row(
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

    fn edit_spec(
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
    pub(super) fn settings_tab(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let sub = self.settings_sub.min(SUBTABS.len() - 1);
        let entity = cx.entity().downgrade();
        let segments = kit::segments(
            "instance-settings-sub",
            &SUBTABS,
            sub,
            move |index, _, cx| {
                let _ = entity.update(cx, |view, cx| {
                    view.settings_sub = index;
                    cx.notify();
                });
            },
        );
        let body = match sub {
            0 => self.group_game(colors),
            1 => self.group_runtime(colors, cx),
            2 => self.group_java(colors),
            3 => self.performance(colors, cx),
            _ => self.group_advanced(colors),
        };
        v_flex()
            .w_full()
            .gap_5()
            .child(h_flex().child(segments))
            .child(kit::entrance(body, ("instance-settings-body", sub)))
            .into_any_element()
    }
}
