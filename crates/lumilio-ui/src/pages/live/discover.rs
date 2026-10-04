use super::controls::{LiveCtx, send};
use crate::assets::UiIcon;
use crate::cover::Loader;
use crate::home::WorldHint;
use crate::key::Key;
use crate::live::{
    DiscoverChange, LOADER_CHOICES, LiveIntent, PageItem, SearchRow, SearchStatus,
    environment_label, page_items, tag_label,
};
use crate::theme::ShellColors;
use crate::{kit, theme};
use gpui::StyledImage as _;
use gpui::prelude::*;
use gpui::{App, ClickEvent, Entity, IntoElement, ObjectFit, Window, div, img, px};
use gpui_component::Sizable as _;
use gpui_component::select::{SearchableVec, Select, SelectState};
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::ProjectKind;

pub const DISCOVER_TABS: [&str; 4] = ["整合包", "Mod", "资源包", "光影"];
pub const DISCOVER_KINDS: [ProjectKind; 4] = [
    ProjectKind::Modpack,
    ProjectKind::Mod,
    ProjectKind::ResourcePack,
    ProjectKind::Shader,
];

/// The placeholder cover: shown until the real icon arrives, and instead of
/// it when a project has none or the picture cannot be loaded.
pub(super) fn placeholder(seed: u32, colors: ShellColors) -> gpui::AnyElement {
    const LOADERS: [Loader; 5] = [
        Loader::Fabric,
        Loader::Forge,
        Loader::NeoForge,
        Loader::Quilt,
        Loader::Vanilla,
    ];
    const WORLDS: [WorldHint; 4] = [
        WorldHint::Overworld,
        WorldHint::Underground,
        WorldHint::Redstone,
        WorldHint::Nether,
    ];
    div()
        .size_full()
        .child(crate::cover::element(
            seed,
            LOADERS[seed as usize % LOADERS.len()],
            WORLDS[seed as usize / 5 % WORLDS.len()],
            colors.surface,
            px(0.),
            px(0.),
        ))
        .into_any_element()
}

/// A project's icon at `side` pixels, fetched by address.
pub fn project_icon(
    url: Option<&str>,
    seed: u32,
    side: f32,
    colors: ShellColors,
) -> gpui::AnyElement {
    let frame = div()
        .flex_none()
        .size(px(side))
        .rounded(px(side / 6.))
        .overflow_hidden()
        .bg(colors.surface_subtle);
    match url {
        Some(url) => frame
            .child(
                img(url.to_owned())
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_loading(move || placeholder(seed, colors))
                    .with_fallback(move || placeholder(seed, colors)),
            )
            .into_any_element(),
        None => frame.child(placeholder(seed, colors)).into_any_element(),
    }
}

/// One line of a row's spec column: a muted mono legend and its value.
pub(super) fn spec(legend: &'static str, value: String, colors: ShellColors) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_baseline()
        .justify_end()
        .font_family(theme::MONO_FONT)
        .child(
            div()
                .text_size(px(10.))
                .text_color(colors.muted)
                .child(legend),
        )
        .child(
            div()
                .min_w(px(52.))
                .text_xs()
                .text_color(colors.foreground)
                .text_right()
                .child(value),
        )
}

/// How many category tags a row shows before folding the rest into `+N`.
pub(super) const SHOWN_TAGS: usize = 3;

/// The result row's action: install, or — when the target game already has
/// it — a chip saying so, or an update button when a newer version fits.
pub(super) fn install_control(
    index: usize,
    row: &SearchRow,
    ctx: &LiveCtx,
    install: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui::AnyElement {
    let colors = ctx.colors;
    let Some(have) = ctx.model.installed.get(&row.project_id) else {
        // ia[discover]: 安装（最新兼容版本） | 结果行「安装」 | 后台任务，toast“开始安装”，完成 toast；Mod 先过依赖提示 | H-DISC-04
        return kit::action(
            ("live-install", index),
            "安装",
            Some(UiIcon::Download),
            false,
            install,
        )
        .small()
        .into_any_element();
    };
    // ia[discover]: 已安装状态 | 结果行「已安装」/「更新」 | 目标游戏已有的（按 Modrinth 识别）显示「已安装」；有更新显示「更新」，点了用新版本替换旧文件；详情页主按钮同样变化 | — | 只认 Modrinth 认得的文件
    match &have.update {
        Some(version) => {
            let intent = LiveIntent::UpdateInstalled {
                kind: row.kind,
                project: row.slug.clone(),
                title: row.title.clone(),
                file_name: have.file_name.clone(),
                version_id: version.clone(),
            };
            let handler = ctx.handler.clone();
            kit::action(
                ("live-update", index),
                "更新",
                Some(UiIcon::Refresh),
                false,
                move |window, cx| {
                    cx.stop_propagation();
                    handler(intent.clone(), window, cx);
                },
            )
            .small()
            .debug_selector(move || format!("live-update-{index}"))
            .into_any_element()
        }
        None => div()
            .debug_selector(move || format!("live-installed-{index}"))
            .child(kit::chip("已安装", Some(kit::tone_ok()), colors))
            .into_any_element(),
    }
}

// ia[discover]: 打开项目详情 | 点结果行 | 详情页（进历史） | H-DISC-02
pub(super) fn result_row(index: usize, row: &SearchRow, ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let open = send(
        &ctx.handler,
        LiveIntent::OpenProject {
            kind: row.kind,
            slug: row.slug.clone(),
        },
    );
    let install = {
        let handler = ctx.handler.clone();
        let intent = LiveIntent::Install {
            kind: row.kind,
            slug: row.slug.clone(),
            title: row.title.clone(),
            version: None,
        };
        move |window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            handler(intent.clone(), window, cx);
        }
    };
    let hidden = row.categories.len().saturating_sub(SHOWN_TAGS);
    let plain = |text: String| kit::tag(text, kit::TagKind::Plain, colors);
    let tags = h_flex()
        .flex_wrap()
        .gap(px(6.))
        .children(
            row.loaders
                .iter()
                .map(|name| kit::tag(tag_label(name), kit::TagKind::Ink, colors)),
        )
        .children(
            row.environment
                .map(|environment| plain(environment_label(environment).to_string())),
        )
        .children(
            row.categories
                .iter()
                .take(SHOWN_TAGS)
                .map(|name| plain(tag_label(name).to_string())),
        )
        .children((hidden > 0).then(|| plain(format!("+{hidden}"))));
    let body = colors.body;

    // A catalogue line: index, framed icon, name and summary, spec column.
    h_flex()
        .id(("live-result", index))
        .w_full()
        .gap_4()
        .py(px(16.))
        .px(px(4.))
        .items_start()
        .border_b_1()
        .border_color(colors.border)
        .cursor_pointer()
        .hover(move |row| row.bg(body.panel))
        .debug_selector(|| format!("live-result-{index}"))
        .on_click(move |_, window, cx| open(window, cx))
        .child(
            div()
                .flex_none()
                .w(px(24.))
                .pt(px(4.))
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(body.orange_text)
                .child(format!("{:02}", index + 1)),
        )
        .child(
            div()
                .flex_none()
                .p(px(3.))
                .rounded(px(5.))
                .border_1()
                .border_color(colors.border)
                .bg(body.display)
                .child(project_icon(row.icon_url.as_deref(), row.seed, 80., colors)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(20.))
                        .line_height(px(26.))
                        .font_weight(gpui::FontWeight::LIGHT)
                        .text_color(colors.foreground)
                        .child(row.title.clone()),
                )
                .child(
                    div()
                        .font_family(theme::MONO_FONT)
                        .text_size(px(11.))
                        .text_color(colors.muted)
                        .child(row.author.to_uppercase()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .line_clamp(2)
                        .child(row.summary.clone()),
                )
                .child(div().pt(px(2.)).child(tags)),
        )
        .child(
            v_flex()
                .flex_none()
                .w(px(190.))
                .self_stretch()
                .items_end()
                .justify_between()
                .gap_2()
                .child(install_control(index, row, ctx, install))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(3.))
                        .border_t_1()
                        .border_color(colors.border)
                        .pt(px(6.))
                        .child(spec("DL", row.downloads.clone(), colors))
                        .child(spec("FAV", row.follows.clone(), colors))
                        .children(
                            (!row.updated.is_empty())
                                .then(|| spec("UPD", row.updated.clone(), colors)),
                        ),
                ),
        )
}

// ia[discover]: 分页 | 列表上方的页码键 | 翻页并回到列表顶部的第一行 | H-DISC-01
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

// ia[discover]: 筛选 | 右侧丝印编号筛选栏：游戏版本、加载器、类别；清除筛选 | 重新搜索；加载器与类别是带 LED 的选项列表 | H-DISC-01
pub(super) fn sidebar(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let query = &ctx.model.query;
    let change = |value: DiscoverChange| {
        let change = ctx.change.clone();
        move |window: &mut Window, cx: &mut App| change(value.clone(), window, cx)
    };
    // The filters are a numbered silkscreen column, not a card.
    let mut number = 0;
    let mut section = |title: &'static str, body: gpui::AnyElement| {
        number += 1;
        kit::section_at(number, title, colors, body).into_any_element()
    };

    let version = section(
        "游戏版本",
        Select::new(&ctx.controls.version)
            .small()
            .placeholder("全部版本")
            .into_any_element(),
    );

    let loaders = query.has_loaders().then(|| {
        section(
            "加载器",
            v_flex()
                .border_t_1()
                .border_color(colors.foreground)
                .children(LOADER_CHOICES.iter().map(|name| {
                    kit::led_option(
                        ("live-loader", name.len() + name.as_bytes()[0] as usize),
                        tag_label(name),
                        query.loaders.iter().any(|chosen| chosen == name),
                        colors,
                        change(DiscoverChange::ToggleLoader((*name).to_owned())),
                    )
                }))
                .into_any_element(),
        )
    });

    let categories = ctx.model.filters.categories(query.kind);
    let category_body = if categories.is_empty() {
        div()
            .text_xs()
            .text_color(colors.muted)
            .child(if ctx.model.filters.loaded {
                "这个分类下没有类别"
            } else {
                "正在读取…"
            })
            .into_any_element()
    } else {
        v_flex()
            .border_t_1()
            .border_color(colors.foreground)
            .children(categories.into_iter().enumerate().map(|(index, name)| {
                kit::led_option(
                    ("live-category", index),
                    tag_label(name),
                    query.categories.iter().any(|chosen| chosen == name),
                    colors,
                    change(DiscoverChange::ToggleCategory(name.to_owned())),
                )
            }))
            .into_any_element()
    };
    let categories = section("类别", category_body);

    v_flex()
        .w(px(240.))
        .flex_none()
        .gap_5()
        .child(version)
        .children(loaders)
        .child(categories)
        .children(ctx.model.filters.error.as_ref().map(|message| {
            h_flex()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.danger)
                        .child("筛选项没有加载"),
                )
                .child(kit::technical("live-filters-technical", message.clone()).xsmall())
        }))
        .children(query.filtered().then(|| {
            kit::ghost(
                "live-clear-filters",
                "清除筛选",
                change(DiscoverChange::ClearFilters),
            )
        }))
}

pub fn discover(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let query = &ctx.model.query;
    let tab = DISCOVER_KINDS
        .iter()
        .position(|kind| *kind == query.kind)
        .unwrap_or(0);

    let body = match &ctx.model.search {
        SearchStatus::Idle | SearchStatus::Searching if ctx.model.results.is_empty() => {
            kit::empty("正在搜索…", "", colors).into_any_element()
        }
        SearchStatus::Failed(message) => v_flex()
            .items_center()
            .child(kit::empty("连不上 Modrinth", "检查网络后再试一次", colors))
            .child(kit::technical("live-search-technical", message.clone()))
            .into_any_element(),
        SearchStatus::Done { .. } if ctx.model.results.is_empty() => {
            kit::empty("没有结果", "换个关键词或放宽筛选试试", colors).into_any_element()
        }
        _ => v_flex()
            .w_full()
            .border_t_1()
            .border_color(colors.foreground)
            .children(
                ctx.model
                    .results
                    .iter()
                    .enumerate()
                    .map(|(index, row)| result_row(index, row, ctx)),
            )
            .into_any_element(),
    };

    let actions = kit::PageActions::new("live-discover-actions")
        .more(kit::MenuEntry::new("在 Modrinth 中浏览", {
            let url = lumilio_core::browse_page_url(query.kind);
            move |_, cx| cx.open_url(&url)
        }))
        .more(kit::MenuEntry::new(
            "重新搜索",
            send(&ctx.handler, LiveIntent::Search(query.clone())),
        ));

    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "发现",
            "找到下一次想玩的东西",
            actions.render(colors),
            colors,
        ))
        .child(kit::toolbar(
            Some(
                // ia[discover]: 分类 | L3 标签：整合包 / Mod / 资源包 / 光影 | 切换搜索的项目类型并重新搜索 | H-DISC-01
                kit::tabs("live-discover-tabs", &DISCOVER_TABS, tab, {
                    let change = ctx.change.clone();
                    move |index, window, app| {
                        let kind = DISCOVER_KINDS[index.min(DISCOVER_KINDS.len() - 1)];
                        change(DiscoverChange::Kind(kind), window, app);
                    }
                })
                .into_any_element(),
            ),
            Some(
                // ia[discover]: 搜索 | L3 搜索框（回车确认） | 按关键词搜索 Modrinth | H-DISC-01
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
                                        // ia[discover]: 排序 / 显示数量 | L4 两个下拉 | 重新搜索 | H-DISC-01
                                        .child(toolbar_select(
                                            "排序方式",
                                            &ctx.controls.sort,
                                            150.,
                                            colors,
                                        ))
                                        .child(toolbar_select(
                                            "显示数量",
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
                .child(sidebar(ctx)),
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
