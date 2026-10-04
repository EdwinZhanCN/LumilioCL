//! The filter column of Discover: hide-installed, then the sections of the
//! kind, each an accordion whose open state is part of the view state.
//!
//! Sections, their order, what opens by default, the locked-by-game boxes with
//! their unlock and sync buttons, and the advanced exclusions follow Modrinth
//! App's `browse-tab/sidebar.vue` and `SearchSidebarFilter.vue` (GPL-3.0-only;
//! ADR 0022).

use super::super::controls::LiveCtx;
use crate::assets::UiIcon;
use crate::controls::Checkbox;
use crate::kit::{self, Stand, ViewIntent};
use crate::live::{
    DiscoverChange, Lock, PickGroup, Section, Side, advanced_options, default_loaders,
    section_title, sections, tag_label,
};
use gpui::prelude::*;
use gpui::{App, Window, div, px};
use gpui_component::input::Input;
use gpui_component::{Icon, h_flex, v_flex};
use gpui_component::{Sizable as _, StyledExt as _};
use lumilio_core::Stance;

/// View-state group for a section's open state (0 default, 1 open, 2 closed).
const SECTION_GROUP: u8 = 100;
/// View-state group for a loader list's "show more" (0 fewer, 1 more).
const MORE_GROUP: u8 = 120;

const fn slot(section: &Section) -> u8 {
    match section {
        Section::Version => 0,
        Section::Loader => 1,
        Section::Environment => 2,
        Section::License => 3,
        Section::Advanced => 4,
        Section::Category(_) => 5,
    }
}

fn group_of(section: &Section) -> u8 {
    let extra = match section {
        Section::Category(header) => match header.as_str() {
            "categories" => 0,
            "features" => 1,
            "performance impact" => 2,
            "resolutions" => 3,
            _ => 4,
        },
        _ => 0,
    };
    SECTION_GROUP + slot(section) * 5 + extra
}

/// Whether a section starts open: the app opens categories, environment and
/// license; a filter the game provides; and advanced exclusions when the
/// person left them open.
fn open_by_default(section: &Section, ctx: &LiveCtx) -> bool {
    let provided = ctx.model.provided();
    match section {
        Section::Category(_) | Section::Environment | Section::License => true,
        Section::Version => provided.version.is_some(),
        Section::Loader => provided.loader.is_some(),
        Section::Advanced => ctx.model.discover_prefs.advanced_open,
    }
}

fn is_open(section: &Section, ctx: &LiveCtx) -> bool {
    match ctx.state.choice(group_of(section), 0) {
        1 => true,
        2 => false,
        _ => open_by_default(section, ctx),
    }
}

fn stand_of(picks: &[lumilio_core::Pick], name: &str) -> Stand {
    match picks.iter().find(|pick| pick.name == name) {
        Some(pick) if pick.stance == Stance::Exclude => Stand::Exclude,
        Some(_) => Stand::Include,
        None => Stand::Off,
    }
}

fn pick_row(
    id: (&'static str, usize),
    ctx: &LiveCtx,
    group: PickGroup,
    name: &str,
    picks: &[lumilio_core::Pick],
) -> impl IntoElement {
    let (include, exclude) = (name.to_owned(), name.to_owned());
    let (on_include, on_exclude) = (ctx.change.clone(), ctx.change.clone());
    let selector = format!(
        "live-pick-{}-{name}",
        match group {
            PickGroup::Loader => "loader",
            PickGroup::Category => "category",
        }
    );
    kit::filter_row(
        id,
        tag_label(name),
        stand_of(picks, name),
        true,
        ctx.colors,
        move |window, cx| on_include(DiscoverChange::Include(group, include.clone()), window, cx),
        move |window, cx| on_exclude(DiscoverChange::Exclude(group, exclude.clone()), window, cx),
    )
    .debug_selector(move || selector)
}

/// What a filter the game provides looks like: a dashed box that says whose
/// choice it is, warns about unlocking, and has the unlock button.
fn locked_box(ctx: &LiveCtx, lock: Lock, what: &'static str, value: String) -> impl IntoElement {
    let colors = ctx.colors;
    let change = ctx.change.clone();
    v_flex()
        .gap_2()
        .p_3()
        .border_1()
        .border_dashed()
        .border_color(colors.border)
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(colors.foreground)
                .child(format!("{what}由游戏提供：{value}")),
        )
        .child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child("解锁后能看到不适合这个游戏的内容，安装时仍只装适合它的版本。"),
        )
        .child(
            kit::ghost(
                match lock {
                    Lock::Version => "live-unlock-version",
                    Lock::Loader => "live-unlock-loader",
                },
                "解锁筛选",
                move |window, cx| change(DiscoverChange::Unlock(lock), window, cx),
            )
            .debug_selector(move || {
                match lock {
                    Lock::Version => "live-unlock-version",
                    Lock::Loader => "live-unlock-loader",
                }
                .into()
            }),
        )
}

/// "Sync with the game": take the game's value back and forget the person's.
fn sync_button(ctx: &LiveCtx, lock: Lock) -> impl IntoElement {
    let change = ctx.change.clone();
    kit::ghost(
        match lock {
            Lock::Version => "live-sync-version",
            Lock::Loader => "live-sync-loader",
        },
        "与游戏同步",
        move |window, cx| change(DiscoverChange::Sync(lock), window, cx),
    )
    .debug_selector(move || {
        match lock {
            Lock::Version => "live-sync-version",
            Lock::Loader => "live-sync-loader",
        }
        .into()
    })
}

fn version_body(ctx: &LiveCtx) -> gpui::AnyElement {
    let query = &ctx.model.query;
    let provided = ctx.model.provided();
    if let (true, Some(version)) = (query.version_locked(&provided), &provided.version) {
        return locked_box(ctx, Lock::Version, "游戏版本", version.clone()).into_any_element();
    }
    let colors = ctx.colors;
    let needle = ctx.version_filter.to_lowercase();
    let mut shown: Vec<&str> = ctx
        .model
        .filters
        .versions(query.show_all_versions)
        .into_iter()
        .filter(|version| needle.is_empty() || version.to_lowercase().contains(&needle))
        .collect();
    // A chosen version stays listed even when the filters hide it.
    for chosen in &query.versions {
        if !shown.contains(&chosen.as_str()) {
            shown.insert(0, chosen);
        }
    }
    let change = ctx.change.clone();
    let all = query.show_all_versions;
    v_flex()
        .gap_2()
        .child(
            Input::new(&ctx.controls.version_search)
                .small()
                .cleanable(true)
                .prefix(
                    Icon::new(UiIcon::Search)
                        .size(px(14.))
                        .text_color(colors.muted),
                ),
        )
        .child(
            kit::keep_wheel(
                v_flex()
                    .id("live-version-list")
                    .max_h(px(256.))
                    .overflow_y_scroll()
                    .track_scroll(&ctx.controls.version_scroll)
                    .border_t_1()
                    .border_color(colors.foreground),
                &ctx.controls.version_scroll,
            )
            .children(shown.into_iter().enumerate().map(|(index, version)| {
                let on = query.versions.iter().any(|chosen| chosen == version);
                let change = ctx.change.clone();
                let version = version.to_owned();
                kit::led_option(
                    ("live-version", index),
                    version.clone(),
                    on,
                    colors,
                    move |window, cx| {
                        change(DiscoverChange::ToggleVersion(version.clone()), window, cx)
                    },
                )
            })),
        )
        .child(
            Checkbox::new("live-all-versions")
                .checked(all)
                .label("显示全部版本")
                .on_click(move |_, window, cx| {
                    change(DiscoverChange::ShowAllVersions(!all), window, cx)
                }),
        )
        .children(
            (provided.version.is_some() && query.is_unlocked(Lock::Version))
                .then(|| sync_button(ctx, Lock::Version)),
        )
        .into_any_element()
}

fn loader_body(ctx: &LiveCtx) -> gpui::AnyElement {
    let query = &ctx.model.query;
    let provided = ctx.model.provided();
    if let (true, Some(loader)) = (query.loader_locked(&provided), &provided.loader) {
        return locked_box(ctx, Lock::Loader, "加载器", tag_label(loader)).into_any_element();
    }
    let colors = ctx.colors;
    let options = ctx.model.filters.loader_options(query.kind);
    let usual = default_loaders(query.kind);
    let expandable = !usual.is_empty();
    let more = ctx.state.choice(MORE_GROUP, 0) == 1;
    let emit = ctx.emit.clone();
    // A picked loader stays visible even while the list is folded.
    let visible: Vec<&String> = options
        .iter()
        .filter(|name| {
            !expandable
                || more
                || usual.contains(&name.as_str())
                || query.loaders.iter().any(|pick| &pick.name == *name)
        })
        .collect();
    v_flex()
        .gap_2()
        .child(
            v_flex()
                .border_t_1()
                .border_color(colors.foreground)
                .children(visible.into_iter().enumerate().map(|(index, name)| {
                    pick_row(
                        ("live-loader", index),
                        ctx,
                        PickGroup::Loader,
                        name,
                        &query.loaders,
                    )
                })),
        )
        .children(expandable.then(|| {
            kit::ghost(
                "live-loader-more",
                if more { "收起" } else { "显示更多" },
                move |window, cx| {
                    emit(
                        ViewIntent::Choose(MORE_GROUP, usize::from(!more)),
                        window,
                        cx,
                    )
                },
            )
        }))
        .children(
            (provided.loader.is_some() && query.is_unlocked(Lock::Loader))
                .then(|| sync_button(ctx, Lock::Loader)),
        )
        .into_any_element()
}

fn category_body(ctx: &LiveCtx, header: &str) -> gpui::AnyElement {
    let query = &ctx.model.query;
    let groups = ctx.model.filters.category_groups(query.kind);
    let names: Vec<&str> = groups
        .iter()
        .find(|(name, _)| name == header)
        .map(|(_, names)| names.clone())
        .unwrap_or_default();
    v_flex()
        .border_t_1()
        .border_color(ctx.colors.foreground)
        .children(names.into_iter().enumerate().map(|(index, name)| {
            // Rows are keyed by the header too, so two groups never share an id.
            let key = (
                "live-category",
                index + group_of(&Section::Category(header.to_owned())) as usize * 100,
            );
            pick_row(key, ctx, PickGroup::Category, name, &query.categories)
        }))
        .into_any_element()
}

fn side_row(ctx: &LiveCtx, side: Side, label: &'static str, on: bool) -> impl IntoElement {
    let change = ctx.change.clone();
    kit::led_option(
        match side {
            Side::Client => "live-env-client",
            Side::Server => "live-env-server",
        },
        label,
        on,
        ctx.colors,
        move |window, cx| change(DiscoverChange::ToggleSide(side), window, cx),
    )
}

fn environment_body(ctx: &LiveCtx) -> gpui::AnyElement {
    let query = &ctx.model.query;
    v_flex()
        .border_t_1()
        .border_color(ctx.colors.foreground)
        .child(side_row(ctx, Side::Client, "客户端", query.client))
        .child(side_row(ctx, Side::Server, "服务端", query.server))
        .into_any_element()
}

fn license_body(ctx: &LiveCtx) -> gpui::AnyElement {
    let colors = ctx.colors;
    let stand = match ctx.model.query.license {
        Some(Stance::Include) => Stand::Include,
        Some(Stance::Exclude) => Stand::Exclude,
        None => Stand::Off,
    };
    let (include, exclude) = (ctx.change.clone(), ctx.change.clone());
    v_flex()
        .border_t_1()
        .border_color(colors.foreground)
        .child(kit::filter_row(
            "live-license-open",
            "开源",
            stand,
            true,
            colors,
            move |window, cx| include(DiscoverChange::License(Stance::Include), window, cx),
            move |window, cx| exclude(DiscoverChange::License(Stance::Exclude), window, cx),
        ))
        .into_any_element()
}

/// The advanced exclusions: only "leave out" makes sense, so a row is lit
/// when its exclusion is on. A note says they are remembered.
fn advanced_body(ctx: &LiveCtx) -> gpui::AnyElement {
    let colors = ctx.colors;
    let query = &ctx.model.query;
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child("这里的选择会被记住，下次发现时仍然生效。"),
        )
        .child(
            v_flex()
                .border_t_1()
                .border_color(colors.foreground)
                .children(advanced_options(query.kind).into_iter().enumerate().map(
                    |(index, option)| {
                        let on = query.advanced.iter().any(|id| id == option.id);
                        let change = ctx.change.clone();
                        let id = option.id.to_owned();
                        let row = kit::filter_row(
                            ("live-advanced", index),
                            option.label,
                            if on { Stand::Exclude } else { Stand::Off },
                            false,
                            colors,
                            move |window, cx| {
                                change(DiscoverChange::Advanced(id.clone()), window, cx)
                            },
                            |_, _| {},
                        );
                        // A finer choice sits under its parent.
                        div()
                            .when(option.parent.is_some(), |row| row.pl(px(14.)))
                            .child(row)
                    },
                )),
        )
        .into_any_element()
}

fn section_view(index: usize, section: &Section, ctx: &LiveCtx) -> gpui::AnyElement {
    let colors = ctx.colors;
    let open = is_open(section, ctx);
    let group = group_of(section);
    let emit = ctx.emit.clone();
    let change = ctx.change.clone();
    let advanced = *section == Section::Advanced;
    let toggle = move |window: &mut Window, cx: &mut App| {
        if advanced {
            change(DiscoverChange::AdvancedOpen(!open), window, cx);
        }
        emit(
            ViewIntent::Choose(group, if open { 2 } else { 1 }),
            window,
            cx,
        );
    };
    let head = h_flex()
        .id(("live-section", index))
        .debug_selector(move || format!("live-section-{index}"))
        .w_full()
        .gap_2()
        .items_center()
        .cursor_pointer()
        .on_click(move |_, window, cx| toggle(window, cx))
        .child(kit::section_head(index + 1, section_title(section), colors))
        .child(div().flex_1())
        .child(
            Icon::new(if open {
                UiIcon::Collapse
            } else {
                UiIcon::Expand
            })
            .size(px(14.))
            .text_color(colors.muted),
        );
    let body = open.then(|| match section {
        Section::Version => version_body(ctx),
        Section::Loader => loader_body(ctx),
        Section::Category(header) => category_body(ctx, header),
        Section::Environment => environment_body(ctx),
        Section::License => license_body(ctx),
        Section::Advanced => advanced_body(ctx),
    });
    v_flex()
        .w_full()
        .gap_3()
        .child(head)
        .children(body)
        .into_any_element()
}

// ia[discover]: 筛选 | 右侧丝印编号筛选栏，按类型分区、可折叠：游戏版本（可搜索，可显示全部版本）、加载器（常用在前，「显示更多」）、类别、运行环境、许可证、高级排除；每个选项点一下为「要」，点 ⊘ 为「不要」 | 重新搜索；游戏提供的版本和加载器带锁
// ia[discover]: 高级排除 | 筛选栏「高级排除」：光敏性触发、AI、广告、遥测等 | 选择会被记住；第一次选「光敏性触发」弹出提示，可选不再提示
// ia[discover]: 隐藏已安装 | 筛选栏顶部开关（整合包页，或从游戏页进入时） | 搜索时排除已有的整合包 / 目标游戏已装的项目；整合包页的选择会被记住
pub(super) fn sidebar(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let query = &ctx.model.query;
    let hide = ctx.model.can_hide_installed().then(|| {
        let change = ctx.change.clone();
        let on = ctx.model.hiding_installed();
        h_flex()
            .gap_3()
            .items_center()
            .child(kit::switch(
                "live-hide-installed",
                on,
                "隐藏已安装",
                move |window, cx| change(DiscoverChange::HideInstalled(!on), window, cx),
            ))
            .child(
                div()
                    .debug_selector(|| "live-hide-installed-label".into())
                    .text_sm()
                    .text_color(colors.foreground)
                    .child("隐藏已安装"),
            )
    });
    let list = sections(query.kind, &ctx.model.filters);
    v_flex()
        .w(px(240.))
        .flex_none()
        .gap_5()
        .children(hide)
        .children(
            list.iter()
                .enumerate()
                .map(|(index, section)| section_view(index, section, ctx)),
        )
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
            let change = ctx.change.clone();
            kit::ghost("live-clear-filters", "清除筛选", move |window, cx| {
                change(DiscoverChange::ClearFilters, window, cx)
            })
        }))
}
