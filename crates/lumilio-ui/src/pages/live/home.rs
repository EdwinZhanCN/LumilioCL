//! Home's groups below the world that need the library (design language §5):
//! where the continued game left off, its record, and the recent games as
//! the Library's own faceplates.

use super::controls::{LiveCtx, send};
use super::library::card;
use crate::home::{HomePresentation, body_label};
use crate::kit;
use crate::live::{HomePlaces, LibraryCard, LiveIntent};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, px};
use gpui_component::{h_flex, v_flex};

/// Recent games on Home: one row of faceplates at the full column.
pub const HOME_RECENT: usize = 4;

/// What the library adds under the world, or `None` when it knows nothing
/// about the game Home is about (Home then draws its recent entries plainly).
pub fn home_sections(ctx: &LiveCtx, home: &HomePresentation) -> Option<AnyElement> {
    let subject = home.subject()?;
    let colors = ctx.colors;
    // Only the game Home is about: another game's worlds never sit under it.
    let continued = ctx
        .model
        .home_places
        .as_ref()
        .filter(|places| subject.id.as_deref() == Some(places.instance.as_str()))
        .map(|places| continued_group(places, home.can_start(), ctx));
    let cards: Vec<&LibraryCard> = home
        .recent()
        .iter()
        .filter_map(|entry| entry.id.as_deref())
        .filter_map(|id| ctx.model.library.iter().find(|card| card.id == id))
        .take(HOME_RECENT)
        .collect();
    // ia[home]: 打开最近的游戏 | 「最近」里的游戏库同款卡片 | 点卡片进游戏页；卡片上的启动、收藏和 ⋯ 菜单与游戏库相同
    let recent = (!cards.is_empty()).then(|| {
        v_flex()
            .gap_3()
            .child(body_label("最近", colors.muted))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_4()
                    .children(cards.iter().enumerate().map(|(index, item)| {
                        div()
                            .debug_selector(move || format!("home-recent-{index}"))
                            .child(card(index, item, ctx))
                    })),
            )
    });
    if continued.is_none() && recent.is_none() {
        return None;
    }
    Some(
        v_flex()
            .w_full()
            .gap_6()
            .children(continued)
            .children(recent)
            .into_any_element(),
    )
}

/// The game's record as display readings: play time in hours (minutes
/// under one hour), worlds and servers. A new game reads zeros: an
/// instrument at rest, not a missing one.
pub fn record_readings(places: &HomePlaces) -> Vec<kit::Reading> {
    let (time, digits, unit) = if places.play_seconds >= 3600 {
        (places.play_seconds / 3600, 4, "H")
    } else {
        (places.play_seconds / 60, 2, "MIN")
    };
    let count = |label: &'static str, value: usize| kit::Reading {
        label: label.into(),
        value: value as u64,
        digits: 3,
        unit: None,
    };
    vec![
        kit::Reading {
            label: "游玩时间".into(),
            value: time,
            digits,
            unit: Some(unit),
        },
        count("世界", places.worlds),
        count("服务器", places.servers),
    ]
}

/// 接着玩 beside 游戏记录. A game with no worlds or servers yet says where
/// they will appear; one whose version cannot go straight in lists nothing.
fn continued_group(places: &HomePlaces, can_start: bool, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let nothing_yet = places.worlds == 0 && places.servers == 0;
    let list = (nothing_yet || !places.places.is_empty()).then(|| {
        let rows: Vec<gpui::Div> = if nothing_yet {
            vec![
                kit::row(
                    "还没有世界",
                    "按「继续」进入游戏，创建的世界会出现在这里",
                    None,
                    None,
                    colors,
                )
                .debug_selector(|| "home-places-empty".into()),
            ]
        } else {
            places
                .places
                .iter()
                .enumerate()
                .map(|(index, place)| {
                    let enter = send(
                        &ctx.handler,
                        LiveIntent::PlayPlace(places.instance.clone(), place.target.clone()),
                    );
                    // ia[home]: 接着玩：进入世界 / 服务器 | 「接着玩」每行的「进入」（继续的游戏最近玩的 3 个世界，再是服务器列表前 2 个） | 启动并直达该世界或服务器，英雄区进入启动时刻 | 启动中和游戏中置灰；1.20 以前的版本不列出；还没有世界时这里说一句它们会出现在哪
                    let key = kit::ghost(("home-place-enter", index), "进入", enter)
                        .disabled(!can_start)
                        .debug_selector(move || format!("home-place-enter-{index}"));
                    kit::row(
                        place.name.clone(),
                        place.detail.clone(),
                        None,
                        Some(key.into_any_element()),
                        colors,
                    )
                })
                .collect()
        };
        v_flex()
            .flex_1()
            .min_w(px(320.))
            .gap_3()
            .child(body_label("接着玩", colors.muted))
            .child(kit::panel_list(rows, colors))
    });
    let record = v_flex()
        .flex_none()
        .gap_3()
        .child(body_label("游戏记录", colors.muted))
        .child(
            kit::record("home-record", &record_readings(places), colors)
                .debug_selector(|| "home-record".into()),
        );
    h_flex()
        .w_full()
        .flex_wrap()
        .items_start()
        .gap_6()
        .children(list)
        .child(record)
        .into_any_element()
}
