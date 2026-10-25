use gpui::prelude::*;
use gpui::{App, ClickEvent, IntoElement, Window, div, px};
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};

use super::controls::LibraryControls;
use super::order::{arranged, loader_code, present_loaders, sort_labels};
use super::page::{COLLECTIONS_TAB, LibraryInputs, LibraryView};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit::{Emit, ViewIntent};
use crate::live::{LibraryCard, LiveHandler, LiveIntent, cover_loader};
use crate::theme::ShellColors;
use crate::{kit, tr, tr_all};

/// What the page is drawn from.
pub(super) struct LibraryCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a LibraryInputs,
    pub view: LibraryView,
    pub emit: Emit,
    pub handler: LiveHandler,
    pub controls: &'a LibraryControls,
    /// What the search box holds.
    pub filter: String,
}

pub(super) fn send(
    handler: &LiveHandler,
    intent: LiveIntent,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}

#[must_use]
pub fn library_tabs() -> &'static [&'static str] {
    tr_all![
        "library-tab-all",
        "library-tab-favorites",
        "library-tab-collections",
        "library-tab-presets"
    ]
}

/// The card's ⋯ menu (IA `library.md`): everything about a game that is not
/// worth a button of its own.
fn card_menu(card: &LibraryCard, ctx: &LibraryCtx) -> Vec<kit::MenuEntry> {
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
    // ia[library]: 设为当前游戏 | 卡片 ⋯ 菜单 | 当前游戏换成它：右下角芯片、首页「继续」和发现页的安装目标跟着换
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

fn card(index: usize, card: &LibraryCard, ctx: &LibraryCtx) -> impl IntoElement {
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
        // The loader is the label under the name; the bottom line is the
        // version and when it was last played.
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .gap(px(8.))
                .child(kit::tag(
                    card.game_version.clone(),
                    kit::TagKind::Plain,
                    colors,
                ))
                .child(kit::reading(
                    card.ago
                        .clone()
                        .unwrap_or_else(|| tr!("library-never-played").to_owned()),
                    colors,
                )),
        )
        .child(
            h_flex()
                .gap_2()
                // ia[library]: 直接启动 | 卡片按键行里的「启动」 | 先设为当前游戏（芯片和「继续」跟着换），再由首页启动时刻接管；不打开游戏页 | global-context D4
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
fn collections_body(ctx: &LibraryCtx, needle: &str) -> gpui::AnyElement {
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

/// The Library's page actions with the tab's own secondary key; every tab
/// shares the primary and the More menu.
pub(super) fn page_actions(secondary: Key, ctx: &LibraryCtx) -> kit::PageActions {
    // ia[library]: 导入其他启动器的游戏 | L2 ⋯ 菜单 → 选文件夹（MultiMC/Prism 实例、.minecraft）→ 有多个时勾选 | 后台导入，复制玩家文件，原文件不动；CurseForge 格式不支持 | ADR 0016
    // ia[library]: 从备份恢复 | L2 ⋯ 菜单 → 选备份 .zip | 后台恢复成新游戏，不覆盖已有的 | ADR 0015
    // ia[library]: 打开游戏库文件夹 | L2 ⋯ 菜单 | 在访达中打开所有游戏所在的文件夹
    kit::PageActions::new("live-library-actions")
        .secondary(secondary)
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
        ))
}

/// The page's tabs, with what the search box searches beside them.
pub(super) fn toolbar(tab: usize, ctx: &LibraryCtx) -> impl IntoElement {
    let emit = ctx.emit.clone();
    kit::toolbar(
        Some(
            kit::tabs(
                "live-library-tabs",
                library_tabs(),
                tab,
                move |index, window, cx| emit(ViewIntent::LibraryTab(index), window, cx),
            )
            .into_any_element(),
        ),
        // ia[library]: 搜索 | L3 搜索框 | 按名称过滤；空结果“没有匹配的游戏”
        // ia[library]: 预设：搜索选项 | 预设 tab 的 L3 搜索框 | 按名称或值过滤右侧的选项；没有匹配时说一句
        Some(kit::search_field(&ctx.controls.filter, ctx.colors)),
    )
}

pub(super) fn library(ctx: &LibraryCtx) -> gpui::AnyElement {
    let colors = ctx.colors;
    let tab = ctx.view.tab.min(library_tabs().len() - 1);
    let needle = ctx.filter.to_lowercase();
    let loaders = present_loaders(&ctx.model.library);
    let sort = ctx.view.sort.min(sort_labels().len() - 1);
    // The remembered loader, if the library still has one: its place among
    // the chips (0 is every loader).
    let loader_choice = loaders
        .iter()
        .position(|loader| loader_code(*loader) == ctx.view.loader)
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

    let secondary = if tab == COLLECTIONS_TAB {
        // ia[library]: 合集：新建 | 合集标签 L2「新建合集」 | 弹窗取名；名称不重复
        kit::action(
            "live-new-collection",
            tr!("library-collection-new"),
            Some(UiIcon::Plus),
            false,
            send(&ctx.handler, LiveIntent::NewCollection),
        )
        .debug_selector(|| "live-new-collection".into())
    } else {
        // ia[library]: 导入整合包 | L2 次要 → 系统选文件（.mrpack、CurseForge 整合包 .zip，或 MultiMC/Prism/本启动器备份的 .zip） | 后台导入，进度在动态；完成后 toast，并打开新游戏的页面、设为当前游戏（加载器筛选不会把它藏起来）；CurseForge 整合包的文件由 CurseForge 来源解析，作者只允许在网站下载的先按 SHA-1 到 Modrinth 找，找不到的列在游戏的内容页 | CurseForge 整合包需要开着 CurseForge 来源
        kit::action(
            "live-import",
            tr!("library-import-pack"),
            Some(UiIcon::Download),
            false,
            send(&ctx.handler, LiveIntent::ImportPack),
        )
    };
    let actions = page_actions(secondary, ctx);

    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            tr!("route-library"),
            tr!("library-game-count", count = ctx.model.library.len()),
            actions.render(colors),
            colors,
        ))
        .child(toolbar(tab, ctx))
        .children(
            (tab != COLLECTIONS_TAB && ctx.model.library.len() > 1).then(|| {
                // ia[library]: 排序 / 按加载器筛选 | L4 两个下拉（排序、加载器），与发现页同一种控件 | 记在偏好设置里，下次打开还是这样；库里只有一种加载器时不显示加载器下拉
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(kit::labeled_select(
                        tr!("library-sort"),
                        &ctx.controls.sort,
                        130.,
                        colors,
                    ))
                    .children((loaders.len() > 1).then(|| {
                        kit::labeled_select(
                            tr!("library-loader"),
                            &ctx.controls.loader,
                            150.,
                            colors,
                        )
                    }))
            }),
        )
        .child(kit::entrance(body, ("live-library-body", tab)))
        .into_any_element()
}
