use gpui::prelude::*;
use gpui::{ClickEvent, IntoElement, div, px};
use gpui_component::{Icon, Sizable as _, h_flex, v_flex};
use lumilio_core::ProjectKind;

use super::super::controls::{LiveCtx, send};
use super::{card, sidebar};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::live::{DiscoverChange, LiveIntent, PageItem, SearchStatus, page_items};
use crate::{kit, theme, tr};

/// The gap between the result list and the filter column.
pub(super) const LIST_GAP: f32 = 16.;

/// Whether the filter column fits beside the list: the window's content
/// column must hold it, its gap and a result row at its narrowest. Below
/// that the filters fold into a key above the list.
pub(super) fn filters_docked(window_width: f32) -> bool {
    let content = window_width.min(f32::from(theme::CONTENT_MAX_WIDTH))
        - 2. * f32::from(theme::CONTENT_PADDING_X);
    content - sidebar::SIDEBAR_WIDTH - LIST_GAP >= card::ROW_MIN_WIDTH
}

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
pub(in super::super) fn pager(ctx: &LiveCtx) -> impl IntoElement {
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
    let docked = filters_docked(ctx.window_width);

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
                .gap(px(LIST_GAP))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_4()
                        .child(
                            // In a narrow window the pager goes to its own line
                            // rather than under the sidebar.
                            h_flex()
                                .w_full()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .debug_selector(|| "live-discover-refinements".into())
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .gap_3()
                                        .items_center()
                                        .children((!docked).then(|| sidebar::filters_key(ctx)))
                                        // ia[discover]: 排序 / 显示数量 | L4 两个下拉：相关度、下载量、关注数、发布时间、更新时间（只列来源支持的）；每页 5 / 10 / 15 / 20 / 50 / 100 | 重新搜索；按发布时间排序时结果行显示发布时间，其余显示更新时间；按关注数排序时显示关注数，其余显示下载量
                                        .child(kit::labeled_select(
                                            tr!("library-sort"),
                                            &ctx.controls.sort,
                                            150.,
                                            colors,
                                        ))
                                        .child(kit::labeled_select(
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
                .children(docked.then(|| sidebar::sidebar(ctx))),
        )
}
