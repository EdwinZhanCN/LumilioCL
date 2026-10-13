//! The Discover page: the browse page of Modrinth App, drawn in this
//! launcher's own language (ADR 0022), over whichever content source is
//! chosen. Inside a game's Content tab it browses *for that game*: an install
//! header names it, its version and loader are provided to the search
//! (locked until released), and tabs that cannot apply are not offered.

mod card;
mod sidebar;

pub use self::card::project_icon;

use super::controls::{LiveCtx, send};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::live::{DiscoverChange, LiveIntent, PageItem, SearchStatus, page_items};
use crate::theme::ShellColors;
use crate::tr;
use crate::{kit, theme};
use gpui::prelude::*;
use gpui::{ClickEvent, Entity, IntoElement, div, px};
use gpui_component::Sizable as _;
use gpui_component::select::{SearchableVec, Select, SelectState};
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::ProjectKind;

/// The tab label of a kind.
pub fn kind_label(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Modpack => tr!("discover-kind-modpack"),
        ProjectKind::Mod => "Mod",
        ProjectKind::ResourcePack => tr!("discover-kind-resource-pack"),
        ProjectKind::Shader => tr!("discover-kind-shader"),
    }
}

// ia[discover]: 分页 | 列表上方的页码键 | 翻页并回到列表顶部的第一行
pub(super) fn pager(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let pages = ctx.model.pages();
    let current = ctx.model.query.page;
    let number = |page: u32, active: bool| {
        let change = ctx.change.clone();
        let mut button = Key::new(("live-page", page as usize))
            .label((page + 1).to_string())
            .small();
        button = if active {
            button.primary()
        } else {
            button.ghost()
        };
        theme::clickable(button, !active).on_click(move |_: &ClickEvent, window, cx| {
            change(DiscoverChange::Page(page), window, cx)
        })
    };
    h_flex()
        .gap_1()
        .items_center()
        .children(page_items(current, pages).into_iter().map(|item| {
            match item {
                PageItem::Page(page) => number(page, page == current).into_any_element(),
                PageItem::Gap => div()
                    .px_1()
                    .text_color(colors.muted)
                    .child("…")
                    .into_any_element(),
            }
        }))
        .children((pages > 1 && current + 1 < pages).then(|| {
            let change = ctx.change.clone();
            let next = current + 1;
            theme::clickable(
                Key::new("live-page-next")
                    .icon(Icon::new(UiIcon::Next))
                    .ghost()
                    .small(),
                true,
            )
            .on_click(move |_: &ClickEvent, window, cx| {
                change(DiscoverChange::Page(next), window, cx)
            })
        }))
}

pub fn discover(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let query = &ctx.model.query;
    let kinds = ctx.model.discover_kinds();
    let tab = kinds
        .iter()
        .position(|kind| *kind == query.kind)
        .unwrap_or(0);
    let labels: Vec<&'static str> = kinds.iter().map(|kind| kind_label(*kind)).collect();

    let body = match &ctx.model.search {
        SearchStatus::Idle | SearchStatus::Searching if ctx.model.results.is_empty() => {
            kit::empty(tr!("discover-searching"), "", colors).into_any_element()
        }
        // ia[discover]: 没有内容源 | 搜索结果区整页提示 | 说明没有可用的内容源，指向设置里的插件页；打开内容源后重新搜索
        SearchStatus::NoSource => kit::empty(
            tr!("discover-no-source"),
            tr!("discover-no-source-help"),
            colors,
        )
        .into_any_element(),
        // ia[discover]: 连不上 | 搜索失败的整页提示 | 说明连不上正在浏览的来源，附技术详情；改任何条件会再试一次
        SearchStatus::Failed(message) => v_flex()
            .items_center()
            .child(kit::empty(
                tr!("discover-offline", source = ctx.model.browsed_source_name()),
                tr!("discover-offline-help"),
                colors,
            ))
            .child(kit::technical("live-search-technical", message.clone()))
            .into_any_element(),
        SearchStatus::Done { .. } if ctx.model.results.is_empty() => kit::empty(
            tr!("discover-no-results"),
            tr!("discover-no-results-help"),
            colors,
        )
        .into_any_element(),
        _ => v_flex()
            .w_full()
            .border_t_1()
            .border_color(colors.foreground)
            .children(
                ctx.model
                    .results
                    .iter()
                    .enumerate()
                    .map(|(index, row)| card::result_row(index, row, ctx)),
            )
            .into_any_element(),
    };

    let mut actions = kit::PageActions::new("live-discover-actions");
    if let Some(url) = ctx.model.filters.browse_url(query.kind) {
        let url = url.to_owned();
        // ia[discover]: 在来源网站浏览 | L2 ⋯ 菜单「在（来源）中浏览」 | 浏览器打开这个来源当前类型的列表页
        actions = actions.more(kit::MenuEntry::new(
            tr!(
                "discover-browse-source",
                source = ctx.model.browsed_source_name()
            ),
            move |_, cx| cx.open_url(&url),
        ));
    }
    let actions = actions.more(kit::MenuEntry::new(
        tr!("discover-search-again"),
        send(&ctx.handler, LiveIntent::Search(query.clone())),
    ));

    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            tr!("route-discover"),
            tr!("discover-subtitle"),
            actions.render(colors),
            colors,
        ))
        .child(kit::toolbar(
            Some(
                // ia[discover]: 分类 | L3 标签：整合包 / Mod / 资源包 / 光影（游戏页进入时去掉整合包，原版游戏再去掉 Mod） | 切换搜索的项目类型并重新搜索；类型一换，排序回到相关度、搜索词清空
                kit::tabs("live-discover-tabs", &labels, tab, {
                    let change = ctx.change.clone();
                    move |index, window, app| {
                        if let Some(kind) = kinds.get(index) {
                            change(DiscoverChange::Kind(*kind), window, app);
                        }
                    }
                })
                .into_any_element(),
            ),
            Some(
                // ia[discover]: 搜索 | L3 搜索框（回车确认，有清除键） | 按关键词搜索正在浏览的来源
                div()
                    .debug_selector(|| "live-discover-search".into())
                    .child(kit::search_field(&ctx.controls.discover_search, colors))
                    .into_any_element(),
            ),
        ))
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap_4()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_4()
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(
                                    h_flex()
                                        .gap_3()
                                        .items_center()
                                        .children(source_segments(ctx))
                                        // ia[discover]: 排序 / 显示数量 | L4 两个下拉：相关度、下载量、关注数、发布时间、更新时间（只列来源支持的）；每页 5 / 10 / 15 / 20 / 50 / 100 | 重新搜索；按发布时间排序时卡片显示发布时间，其余显示更新时间
                                        .child(toolbar_select(
                                            tr!("library-sort"),
                                            &ctx.controls.sort,
                                            150.,
                                            colors,
                                        ))
                                        .child(toolbar_select(
                                            tr!("discover-page-size"),
                                            &ctx.controls.page_size,
                                            90.,
                                            colors,
                                        )),
                                )
                                .child(pager(ctx)),
                        )
                        .child(kit::entrance(
                            body,
                            ("live-discover-body", query.page as usize),
                        )),
                )
                .child(sidebar::sidebar(ctx)),
        )
}

/// The content sources to browse, when more than one is on.
fn source_segments(ctx: &LiveCtx) -> Option<impl IntoElement> {
    let sources = &ctx.model.sources;
    if sources.len() < 2 {
        return None;
    }
    let names: Vec<String> = sources.iter().map(|source| source.name.clone()).collect();
    let ids: Vec<String> = sources.iter().map(|source| source.id.clone()).collect();
    let active = ids
        .iter()
        .position(|id| *id == ctx.model.query.source)
        .unwrap_or(0);
    let change = ctx.change.clone();
    // ia[discover]: 切换来源 | L4 分段键：Modrinth / CurseForge（只开了一个来源时不显示） | 在这个来源重新搜索：搜索词、类型和游戏版本保留，加载器、分类和排序重新开始；筛选项换成这个来源的；记住下次打开的来源
    Some(
        div()
            .debug_selector(|| "live-discover-sources".into())
            .child(crate::controls::Segments::new(
                "live-discover-source",
                &names,
                active,
                move |index, window, cx| {
                    if let Some(id) = ids.get(index) {
                        change(DiscoverChange::Source(id.clone()), window, cx);
                    }
                },
            )),
    )
}

pub(super) fn toolbar_select(
    label: &'static str,
    state: &Entity<SelectState<SearchableVec<String>>>,
    width: f32,
    colors: ShellColors,
) -> impl IntoElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(div().text_sm().text_color(colors.muted).child(label))
        .child(div().w(px(width)).child(Select::new(state).small()))
}
