//! Settings (`docs/ia/settings.md`): launcher preferences and the defaults
//! every game starts from. The page shows read-only values and quick
//! controls; anything typed is edited in a dialog (design language §10).

mod about;
mod game_defaults;
mod general;
mod java;
mod plugins;
mod rows;
mod storage;
mod text;

#[cfg(test)]
mod tests;

pub use self::text::{bytes_text, commands_text, list_text, memory_text, window_text};

use self::about::about;
use self::game_defaults::game_defaults;
use self::general::general;
use self::java::java;
use self::storage::downloads;
use super::live::LiveCtx;
use crate::kit;
use crate::kit::ViewIntent;
use gpui::IntoElement;
use gpui::prelude::*;
use gpui_component::v_flex;

// ia[settings]: 打开插件 | L3 设置 tab「插件」 | 显示核心插件、权限、运行状态和各插件的声明式设置；没有插件时显示空列表
pub const TABS: [&str; 6] = ["通用", "游戏默认", "Java", "下载与存储", "关于", "插件"];

/// The view-state group that remembers the chosen tab.
pub const TAB_GROUP: u8 = 201;

const APPEARANCES: [&str; 3] = ["跟随系统", "浅色", "深色"];
const AFTER_LAUNCH: [&str; 2] = ["保持", "隐藏启动器"];
const MOTION: [&str; 3] = ["跟随系统", "减少", "完整"];
const FULLSCREEN: [&str; 3] = ["关", "开", "不设置"];

pub fn render(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let tab = ctx.state.choice(TAB_GROUP, 0).min(TABS.len() - 1);
    let body = match &ctx.model.settings {
        None => kit::empty("正在读取设置…", "", colors).into_any_element(),
        Some(view) => match tab {
            0 => general(view, ctx),
            1 => game_defaults(view, ctx),
            2 => java(view, ctx),
            3 => downloads(view, ctx),
            5 => plugins::render(view, ctx),
            _ => about(view, ctx),
        },
    };
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "设置",
            "启动器本身，以及每个游戏默认使用的值",
            None,
            colors,
        ))
        .child(kit::toolbar(
            Some(
                kit::tabs("settings-tabs", &TABS, tab, {
                    let emit = ctx.emit.clone();
                    move |index, window, app| {
                        emit(ViewIntent::Choose(TAB_GROUP, index), window, app)
                    }
                })
                .into_any_element(),
            ),
            None,
        ))
        .child(kit::entrance(body, ("settings-body", tab)))
}
