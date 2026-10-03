//! Truthful page bodies for global routes whose domain slices are not wired yet.

use gpui::{AnyElement, IntoElement, div, prelude::*, px};
use gpui_component::{Icon, Sizable as _, StyledExt as _, v_flex};

use crate::{assets::LandmarkIcon, route::Route, theme::ShellColors};

pub fn render(route: Route, colors: ShellColors) -> AnyElement {
    let (eyebrow, title, body, icon) = match route {
        Route::Library => (
            "游戏库",
            "还没有游戏",
            "游戏会在这里安静地聚在一起。",
            LandmarkIcon::Library,
        ),
        Route::Discover => (
            "发现",
            "还没有发现",
            "Mod、资源包和光影会在这里出现。",
            LandmarkIcon::Discover,
        ),
        Route::Activity => (
            "动态",
            "没有进行中的事情",
            "下载、安装和修复完成后，会在这里留下痕迹。",
            LandmarkIcon::Activity,
        ),
        Route::Accounts => (
            "账户",
            "还没有账户",
            "添加一个离线账户，就能进入游戏。",
            LandmarkIcon::Accounts,
        ),
        Route::Settings => (
            "设置",
            "设置正在加载",
            "启动器与游戏的默认值会在这里。",
            LandmarkIcon::Settings,
        ),
        Route::Home => ("首页", "", "", LandmarkIcon::Home),
    };

    v_flex()
        .items_start()
        .gap_3()
        .child(Icon::new(icon).with_size(px(28.)).text_color(colors.muted))
        .child(div().text_sm().text_color(colors.muted).child(eyebrow))
        .child(div().text_size(px(32.)).font_semibold().child(title))
        .child(div().text_base().text_color(colors.muted).child(body))
        .into_any_element()
}
