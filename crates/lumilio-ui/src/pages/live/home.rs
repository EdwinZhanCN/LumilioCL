//! Home's groups below the world that need the library (design language §5):
//! where the continued game left off, and its record.

use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, px};
use gpui_component::{h_flex, v_flex};

use super::controls::{LiveCtx, send};
use crate::home::{HomePresentation, body_label};
use crate::kit;
use crate::live::{HomePlaces, LiveIntent, Place, PlaceKind, PlacePicture};
use crate::theme::ShellColors;

/// What the library adds under the world, or `None` when it knows nothing
/// about the game Home is about.
pub fn home_sections(
    ctx: &LiveCtx,
    home: &HomePresentation,
    can_start: bool,
) -> Option<AnyElement> {
    let subject = home.subject()?;
    // Only the game Home is about: another game's worlds never sit under it.
    ctx.model
        .home_places
        .as_ref()
        .filter(|places| subject.id.as_deref() == Some(places.instance.as_str()))
        .map(|places| continued_group(places, can_start, ctx))
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
            label: crate::tr!("home-record-play-time").into(),
            value: time,
            digits,
            unit: Some(unit),
        },
        count(crate::tr!("home-record-worlds"), places.worlds),
        count(crate::tr!("home-record-servers"), places.servers),
    ]
}

/// A place without its key, as the Worlds tab shows it: a world with its
/// mode, cheats, version and when it was played; a server with its version,
/// address, players, round trip and message.
fn place_item(
    row: usize,
    place: &Place,
    loader: crate::cover::Loader,
    colors: ShellColors,
) -> kit::Item {
    let (item, loader) = match &place.kind {
        PlaceKind::World(world) => (crate::places::world_item(world, false, colors), loader),
        PlaceKind::Server(server, state) => (
            crate::places::server_item(row, server, Some(state), colors),
            crate::cover::Loader::Vanilla,
        ),
    };
    item.lead(kit::cover_tile(
        place.picture.as_ref().map(PlacePicture::source),
        place.seed,
        loader,
        colors,
    ))
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
                    crate::tr!("home-places-empty"),
                    crate::tr!("home-places-empty-help"),
                    None,
                    None,
                    colors,
                )
                .debug_selector(|| "home-places-empty".into()),
            ]
        } else {
            // A world without its own icon gets the game's cover family.
            let loader = ctx
                .model
                .library
                .iter()
                .find(|card| card.id == places.instance)
                .map_or(crate::cover::Loader::Vanilla, |card| {
                    crate::live::cover_loader(card.loader)
                });
            places
                .places
                .iter()
                .enumerate()
                .map(|(index, place)| {
                    let enter = send(
                        &ctx.handler,
                        LiveIntent::PlayPlace(places.instance.clone(), place.target()),
                    );
                    // ia[home]: 接着玩：进入世界 / 服务器 | 「接着玩」每行的「进入」（继续的游戏最近玩的 3 个世界，再是服务器列表前 2 个） | 启动并直达该世界或服务器，英雄区进入启动时刻 | 每行和世界页的行一样（世界的模式、作弊、版本、时间；服务器会被询问，显示人数、信号格和 MOTD）；启动中和游戏中置灰；1.20 以前的版本不列出；还没有世界时这里说一句它们会出现在哪
                    let key = kit::ghost(
                        ("home-place-enter", index),
                        crate::tr!("home-place-enter"),
                        enter,
                    )
                    .disabled(!can_start)
                    .debug_selector(move || format!("home-place-enter-{index}"));
                    place_item(index, place, loader, colors)
                        .actions(key)
                        .render(colors)
                })
                .collect()
        };
        v_flex()
            .flex_1()
            .min_w(px(320.))
            .gap_3()
            .child(body_label(crate::tr!("home-continued"), colors.muted))
            .child(kit::panel_list(rows, colors))
    });
    let record = v_flex()
        .flex_none()
        .gap_3()
        .child(body_label(crate::tr!("home-record"), colors.muted))
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
