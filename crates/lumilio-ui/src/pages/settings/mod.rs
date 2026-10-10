//! Settings (`docs/ia/settings.md`): launcher preferences and the defaults
//! every game starts from. The page shows read-only values and quick
//! controls; anything typed is edited in a dialog (design language §10).

mod about;
mod appearance;
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
use self::appearance::appearance;
use self::game_defaults::game_defaults;
use self::general::general;
use self::java::java;
use self::storage::downloads;
use super::live::LiveCtx;
use crate::kit;
use crate::kit::ViewIntent;
use crate::{tr, tr_all};
use gpui::IntoElement;
use gpui::prelude::*;
use gpui_component::v_flex;

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

/// The view-state group that remembers the chosen tab.
pub const TAB_GROUP: u8 = 201;

fn fullscreen_choices() -> &'static [&'static str] {
    tr_all![
        "settings-fullscreen-off",
        "settings-fullscreen-on",
        "settings-fullscreen-unset",
    ]
}

/// The 插件 tab: its list and its detail scroll on their own.
const PLUGINS_TAB: usize = 6;

/// The page is an app shell: the header and tabs stay put and only the body
/// below them scrolls. The 插件 tab owns its scrolling instead (master–detail).
pub fn render(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let tabs = tabs();
    let tab = ctx.state.choice(TAB_GROUP, 0).min(tabs.len() - 1);
    let body = match &ctx.model.settings {
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
    kit::fixed_page("live-settings", head, body)
}
