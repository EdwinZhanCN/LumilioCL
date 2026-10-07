use super::controls::{LiveCtx, send};
use super::discover::toolbar_select;
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit::ViewIntent;
use crate::live::{LibraryCard, LiveIntent, cover_loader};
use crate::{kit, theme};
use crate::{tr, tr_all};
use gpui::prelude::*;
use gpui::{App, ClickEvent, IntoElement, Window, div, px};
use gpui_component::Sizable as _;
use gpui_component::{Icon, h_flex, v_flex};

#[must_use]
pub fn library_tabs() -> &'static [&'static str] {
    tr_all![
        "library-tab-all",
        "library-tab-favorites",
        "library-tab-collections"
    ]
}
/// The tab that shows the person's collections.
pub const COLLECTIONS_TAB: usize = 2;

/// View-state groups of the Library's ordering and loader filter.
pub const LIBRARY_SORT: u8 = 210;
pub const LIBRARY_LOADER: u8 = 211;
#[must_use]
pub fn sort_labels() -> &'static [&'static str] {
    tr_all![
        "library-sort-recent",
        "library-sort-name",
        "library-sort-created"
    ]
}

/// The number a loader is remembered by: 1 vanilla, 2 Fabric, 3 Forge,
/// 4 NeoForge, 5 Quilt (0 is every loader).
#[must_use]
pub const fn loader_code(loader: lumilio_core::Loader) -> usize {
    match loader {
        lumilio_core::Loader::Vanilla => 1,
        lumilio_core::Loader::Fabric => 2,
        lumilio_core::Loader::Forge => 3,
        lumilio_core::Loader::NeoForge => 4,
        lumilio_core::Loader::Quilt => 5,
    }
}

/// The loaders the library has, in a steady order, for the filter.
#[must_use]
pub fn present_loaders(cards: &[LibraryCard]) -> Vec<lumilio_core::Loader> {
    const ORDER: [lumilio_core::Loader; 5] = [
        lumilio_core::Loader::Vanilla,
        lumilio_core::Loader::Fabric,
        lumilio_core::Loader::Forge,
        lumilio_core::Loader::NeoForge,
        lumilio_core::Loader::Quilt,
    ];
    ORDER
        .into_iter()
        .filter(|loader| cards.iter().any(|card| card.loader == *loader))
        .collect()
}

/// The cards in the chosen order (0 as given — most recently played, favourites
/// first; 1 by name; 2 newest first) and only those with the loader, if any.
#[must_use]
pub fn arranged(
    mut cards: Vec<&LibraryCard>,
    sort: usize,
    loader: Option<lumilio_core::Loader>,
) -> Vec<&LibraryCard> {
    cards.retain(|card| loader.is_none_or(|loader| card.loader == loader));
    match sort {
        1 => cards.sort_by_key(|card| card.name.to_lowercase()),
        2 => cards.sort_by_key(|card| std::cmp::Reverse(card.created)),
        _ => {}
    }
    cards
}
/// The card's ⋯ menu (IA `library.md`): everything about a game that is not
/// worth a button of its own.
pub(super) fn card_menu(card: &LibraryCard, ctx: &LiveCtx) -> Vec<kit::MenuEntry> {
    let go = |label: &'static str, intent: LiveIntent| {
        kit::MenuEntry::new(label, send(&ctx.handler, intent))
    };
    let id = || card.id.clone();
    // ia[library]: 删除游戏 | 卡片 ⋯ 菜单 → 警告弹窗 | 先在库里确认，再打开游戏页执行删除
    let delete = {
        let handler = ctx.handler.clone();
        let (id, name) = (card.id.clone(), card.name.clone());
        kit::MenuEntry::new(tr!("library-delete"), move |window, cx| {
            // Asked here, before the game's page is opened for it.
            let (handler, id) = (handler.clone(), id.clone());
            crate::collections::confirm_game_delete(
                &name,
                move |window, cx| handler(LiveIntent::DeleteGameOf(id.clone()), window, cx),
                window,
                cx,
            );
        })
        .danger()
    };
    // ia[library]: 菜单：开始游戏 | 卡片 ⋯ 菜单 | 与卡片上的「启动」相同
    // ia[library]: 菜单：打开 | 卡片 ⋯ 菜单 | 游戏页（进历史）
    // ia[library]: 设为当前游戏 | 卡片 ⋯ 菜单 | 首页、发现页的安装目标和右下角芯片跟着换
    // ia[library]: 合集：加入 / 移出 | 卡片 ⋯ 菜单「加入合集…」→ 多选列表 | 勾选即加入或移出，可顺手新建；合集是标签，不移动文件
    // ia[library]: 复制游戏 | 卡片 ⋯ 菜单 → 复制弹窗（名称、是否复制存档） | 先打开游戏页再弹窗，之后同游戏页
    // ia[library]: 在访达中显示 | 卡片 ⋯ 菜单 | 打开游戏目录
    // ia[library]: 导出整合包 | 卡片 ⋯ 菜单 → 导出弹窗 | 先打开游戏页再弹出导出，之后同游戏页
    vec![
        go(tr!("library-menu-play"), LiveIntent::Play(id())),
        go(tr!("library-menu-open"), LiveIntent::OpenInstance(id())),
        go(
            tr!("library-menu-make-current"),
            LiveIntent::InstallTarget(id()),
        ),
        go(
            tr!("library-menu-collections"),
            LiveIntent::EditCollections(id()),
        ),
        go(tr!("library-menu-copy"), LiveIntent::CopyGameOf(id())),
        go(
            crate::platform::reveal_label(),
            LiveIntent::RevealGame(id()),
        ),
        go(tr!("library-menu-export"), LiveIntent::ExportPackOf(id())),
        delete,
    ]
}

pub(super) fn card(index: usize, card: &LibraryCard, ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let open = send(&ctx.handler, LiveIntent::OpenInstance(card.id.clone()));
    let play = send(&ctx.handler, LiveIntent::Play(card.id.clone()));
    let favorite_label = if card.favorite {
        tr!("library-unfavorite")
    } else {
        tr!("library-favorite")
    };
    let favorite = {
        let handler = ctx.handler.clone();
        let id = card.id.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            handler(LiveIntent::ToggleFavorite(id.clone()), window, cx);
        }
    };
    // ia[library]: 打开游戏 | 点卡片 | 游戏页（进历史）
    kit::faceplate(("live-card", index), colors)
        .debug_selector(|| format!("live-card-{index}"))
        .on_click(move |_, window, cx| open(window, cx))
        .child(kit::faceplate_head(
            card.name.clone(),
            cover_loader(card.loader).label(),
            colors,
        ))
        .child(kit::display_window(
            kit::faceplate_cover(card.seed, cover_loader(card.loader), card.world, colors),
            colors,
        ))
        .child(
            h_flex().justify_between().items_center().gap(px(8.)).child(
                div()
                    .min_w_0()
                    .font_family(theme::MONO_FONT)
                    .text_size(px(10.))
                    .text_color(colors.muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(card.meta.clone()),
            ),
        )
        .child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(card.played.clone()),
        )
        .child(
            h_flex()
                .gap_2()
                // ia[library]: 直接启动 | 卡片按键行里的「启动」 | 首页启动时刻接管；不打开游戏页
                .child(
                    kit::action(
                        ("live-play", index),
                        tr!("library-play"),
                        Some(UiIcon::Play),
                        false,
                        move |window, cx| {
                            cx.stop_propagation();
                            play(window, cx);
                        },
                    )
                    .small()
                    .debug_selector(move || format!("live-play-{index}")),
                )
                // ia[library]: 收藏 / 取消 | 卡片按键行里的星（图标颜色表示状态） | 立即切换；收藏页同步；不打开游戏页
                .child(
                    Key::new(("live-favorite", index))
                        .icon(Icon::new(if card.favorite {
                            UiIcon::StarFilled
                        } else {
                            UiIcon::Star
                        }))
                        .icon_color(if card.favorite {
                            colors.primary
                        } else {
                            colors.muted
                        })
                        .white()
                        .small()
                        .tooltip(favorite_label)
                        .on_click(favorite)
                        .debug_selector(move || format!("live-favorite-{index}")),
                )
                .child(
                    div()
                        .debug_selector(move || format!("live-card-more-{index}"))
                        .child(kit::small_more_menu(
                            ("live-card-more", index),
                            card_menu(card, ctx),
                            colors,
                        )),
                ),
        )
}

/// The Collections tab: one section per collection, each a row of cards.
pub(super) fn collections_body(ctx: &LiveCtx, needle: &str) -> gpui::AnyElement {
    let colors = ctx.colors;
    if ctx.model.collections.is_empty() {
        return v_flex()
            .w_full()
            .gap_3()
            .child(kit::empty(
                tr!("library-collections-none"),
                tr!("library-collections-none-help"),
                colors,
            ))
            .child(h_flex().justify_center().child(kit::action(
                "live-collection-empty-new",
                tr!("library-collection-new"),
                Some(UiIcon::Plus),
                true,
                send(&ctx.handler, LiveIntent::NewCollection),
            )))
            .into_any_element();
    }
    let mut next = 0;
    let mut sections = Vec::new();
    for (section, collection) in ctx.model.collections.iter().enumerate() {
        let cards: Vec<&LibraryCard> = ctx
            .model
            .collection_cards(collection)
            .into_iter()
            .filter(|card| needle.is_empty() || card.name.to_lowercase().contains(needle))
            .collect();
        // ia[library]: 合集：改名 / 删除 | 合集节标题 ⋯ 菜单 | 删除先确认，游戏不受影响
        let menu = kit::more_menu(
            ("live-collection-more", section),
            vec![
                kit::MenuEntry::new(
                    tr!("library-collection-rename"),
                    send(
                        &ctx.handler,
                        LiveIntent::RenameCollection(collection.name.clone()),
                    ),
                ),
                kit::MenuEntry::new(tr!("library-collection-delete"), {
                    let handler = ctx.handler.clone();
                    let name = collection.name.clone();
                    move |window, cx| {
                        let (handler, doomed) = (handler.clone(), name.clone());
                        crate::collections::confirm_delete(
                            &name,
                            move |window, cx| {
                                handler(LiveIntent::DeleteCollection(doomed.clone()), window, cx)
                            },
                            window,
                            cx,
                        );
                    }
                })
                .danger(),
            ],
            colors,
        );
        let body = if cards.is_empty() {
            div()
                .text_sm()
                .text_color(colors.muted)
                .child(if collection.members.is_empty() {
                    tr!("library-collection-empty")
                } else {
                    tr!("library-no-match")
                })
                .into_any_element()
        } else {
            let row = h_flex()
                .w_full()
                .flex_wrap()
                .gap_4()
                .children(cards.iter().map(|item| {
                    let element = card(next, item, ctx);
                    next += 1;
                    element
                }));
            row.into_any_element()
        };
        sections.push(
            v_flex()
                .w_full()
                .gap_3()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_baseline()
                                .child(
                                    div()
                                        .text_color(colors.foreground)
                                        .child(collection.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.muted)
                                        .child(tr!("library-game-count", count = cards.len())),
                                ),
                        )
                        .child(menu),
                )
                .child(body)
                .into_any_element(),
        );
    }
    v_flex()
        .w_full()
        .gap_6()
        .children(sections)
        .into_any_element()
}

pub fn library(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let tab = ctx.state.library_tab.min(library_tabs().len() - 1);
    let needle = ctx.filter.to_lowercase();
    let loaders = present_loaders(&ctx.model.library);
    let sort = ctx
        .state
        .choice(LIBRARY_SORT, 0)
        .min(sort_labels().len() - 1);
    // The remembered loader, if the library still has one: its place among
    // the chips (0 is every loader).
    let code = ctx.state.choice(LIBRARY_LOADER, 0);
    let loader_choice = loaders
        .iter()
        .position(|loader| loader_code(*loader) == code)
        .map_or(0, |at| at + 1);
    let loader = loader_choice.checked_sub(1).map(|at| loaders[at]);
    let shown: Vec<&LibraryCard> = arranged(
        ctx.model
            .library
            .iter()
            .filter(|card| tab == 0 || card.favorite)
            .filter(|card| needle.is_empty() || card.name.to_lowercase().contains(&needle))
            .collect(),
        sort,
        loader,
    );

    let body = if tab == COLLECTIONS_TAB && !ctx.model.library.is_empty() {
        collections_body(ctx, &needle)
    } else if shown.is_empty() {
        let (title, text) = if !ctx.model.library_loaded {
            (tr!("library-loading"), "")
        } else if ctx.model.library.is_empty() {
            (tr!("library-empty"), tr!("library-empty-help"))
        } else if tab == 1 && needle.is_empty() {
            (
                tr!("library-favorites-empty"),
                tr!("library-favorites-empty-help"),
            )
        } else {
            (tr!("library-no-match"), tr!("library-no-match-help"))
        };
        kit::empty(title, text, colors).into_any_element()
    } else {
        h_flex()
            .w_full()
            .flex_wrap()
            .gap_4()
            .children(
                shown
                    .iter()
                    .enumerate()
                    .map(|(index, item)| card(index, item, ctx)),
            )
            .into_any_element()
    };

    let mut actions = kit::PageActions::new("live-library-actions");
    if tab == COLLECTIONS_TAB {
        // ia[library]: 合集：新建 | 合集标签 L2「新建合集」 | 弹窗取名；名称不重复
        actions = actions.secondary(
            kit::action(
                "live-new-collection",
                tr!("library-collection-new"),
                Some(UiIcon::Plus),
                false,
                send(&ctx.handler, LiveIntent::NewCollection),
            )
            .debug_selector(|| "live-new-collection".into()),
        );
    } else {
        // ia[library]: 导入整合包 | L2 次要 → 系统选文件（.mrpack，或 MultiMC/Prism/本启动器备份的 .zip） | 后台导入，进度在动态；完成 toast 并可点“打开”
        actions = actions.secondary(kit::action(
            "live-import",
            tr!("library-import-pack"),
            Some(UiIcon::Download),
            false,
            send(&ctx.handler, LiveIntent::ImportPack),
        ));
    }
    // ia[library]: 导入其他启动器的游戏 | L2 ⋯ 菜单 → 选文件夹（MultiMC/Prism 实例、.minecraft）→ 有多个时勾选 | 后台导入，复制玩家文件，原文件不动；CurseForge 格式不支持 | ADR 0016
    // ia[library]: 从备份恢复 | L2 ⋯ 菜单 → 选备份 .zip | 后台恢复成新游戏，不覆盖已有的 | ADR 0015
    // ia[library]: 打开游戏库文件夹 | L2 ⋯ 菜单 | 在访达中打开所有游戏所在的文件夹
    let actions = actions
        .more(kit::MenuEntry::new(
            tr!("library-import-game"),
            send(&ctx.handler, LiveIntent::ImportGame),
        ))
        .more(kit::MenuEntry::new(
            tr!("library-restore"),
            send(&ctx.handler, LiveIntent::RestoreBackup),
        ))
        .more(kit::MenuEntry::new(
            tr!("library-open-folder"),
            send(&ctx.handler, LiveIntent::OpenGamesFolder),
        ))
        // ia[library]: 新建游戏 | L2 主要 → 新建游戏弹窗 | 创建并（可选）立即安装；成功后打开新游戏页
        .primary(kit::action(
            "live-new",
            tr!("library-new-game"),
            Some(UiIcon::Plus),
            true,
            send(&ctx.handler, LiveIntent::NewInstance),
        ));

    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            tr!("route-library"),
            tr!("library-game-count", count = ctx.model.library.len()),
            actions.render(colors),
            colors,
        ))
        .child(kit::toolbar(
            Some(
                kit::tabs("live-library-tabs", library_tabs(), tab, {
                    let emit = ctx.emit.clone();
                    move |index, window, app| emit(ViewIntent::LibraryTab(index), window, app)
                })
                .into_any_element(),
            ),
            // ia[library]: 搜索 | L3 搜索框 | 按名称过滤；空结果“没有匹配的游戏”
            Some(kit::search_field(&ctx.controls.library_filter, colors)),
        ))
        .children(
            (tab != COLLECTIONS_TAB && ctx.model.library.len() > 1).then(|| {
                // ia[library]: 排序 / 按加载器筛选 | L4 两个下拉（排序、加载器），与发现页同一种控件 | 记在偏好设置里，下次打开还是这样；库里只有一种加载器时不显示加载器下拉
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(toolbar_select(
                        tr!("library-sort"),
                        &ctx.controls.library_sort,
                        130.,
                        colors,
                    ))
                    .children((loaders.len() > 1).then(|| {
                        toolbar_select(
                            tr!("library-loader"),
                            &ctx.controls.library_loader,
                            150.,
                            colors,
                        )
                    }))
            }),
        )
        .child(kit::entrance(body, ("live-library-body", tab)))
}
