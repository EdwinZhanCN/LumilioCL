//! Truthful page bodies for global routes whose domain slices are not wired yet.

use gpui::{AnyElement, IntoElement, div, prelude::*, px};
use gpui_component::{Icon, Sizable as _, StyledExt as _, v_flex};

use crate::tr;
use crate::{assets::LandmarkIcon, route::Route, theme::ShellColors};

pub fn render(route: Route, colors: ShellColors) -> AnyElement {
    let (eyebrow, title, body, icon) = match route {
        Route::Library => (
            tr!("route-library"),
            tr!("library-empty"),
            tr!("placeholder-library-body"),
            LandmarkIcon::Library,
        ),
        Route::Discover => (
            tr!("route-discover"),
            tr!("placeholder-discover-title"),
            tr!("placeholder-discover-body"),
            LandmarkIcon::Discover,
        ),
        Route::Activity => (
            tr!("route-activity"),
            tr!("placeholder-activity-title"),
            tr!("placeholder-activity-body"),
            LandmarkIcon::Activity,
        ),
        Route::Accounts => (
            tr!("route-accounts"),
            tr!("account-empty-title"),
            tr!("placeholder-accounts-body"),
            LandmarkIcon::Accounts,
        ),
        Route::Settings => (
            tr!("route-settings"),
            tr!("placeholder-settings-title"),
            tr!("placeholder-settings-body"),
            LandmarkIcon::Settings,
        ),
        Route::Home => (tr!("route-home"), "", "", LandmarkIcon::Home),
    };

    v_flex()
        .items_start()
        .gap_3()
        .child(Icon::new(icon).with_size(px(28.)).text_color(colors.muted))
        .child(div().text_sm().text_color(colors.muted).child(eyebrow))
        .child(
            div()
                .text_size(crate::theme::font_px(32.))
                .font_semibold()
                .child(title),
        )
        .child(div().text_base().text_color(colors.muted).child(body))
        .into_any_element()
}
