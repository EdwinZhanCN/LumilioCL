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
use crate::kit;
use crate::settings_dialog::{DialogSpec, SettingsDialog};
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{AnyElement, Context, SharedString};
use gpui_component::{h_flex, v_flex};

pub const SUBTABS: [&str; 5] = ["游戏", "运行时", "Java", "性能", "高级"];

const FULLSCREEN: [&str; 3] = ["跟随默认", "开", "关"];
const AFTER_LAUNCH: [&str; 3] = ["跟随默认", "保持", "隐藏启动器"];
const QUICK_PLAY: [&str; 3] = ["无", "世界", "服务器"];
const FOLLOW_OR_OWN: [&str; 2] = ["跟随默认", "自己设置"];

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
            kit::ghost(id, "编辑", move |window, cx| {
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
