//! One result row: icon, name, tags, install control and numbers.
//!
//! Which tags show and in what order follows Modrinth App's project card
//! (`live::card_tags`); the date is the publishing date when the list is
//! sorted by newest, else the last update.

use super::super::controls::{LiveCtx, send};
use crate::assets::UiIcon;
use crate::cover::Loader;
use crate::home::WorldHint;
use crate::live::{DateKind, LiveIntent, SearchRow, environment_label, tag_label};
use crate::theme::ShellColors;
use crate::tr;
use crate::{kit, theme};
use gpui::StyledImage as _;
use gpui::prelude::*;
use gpui::{App, ObjectFit, Window, div, img, px};
use gpui_component::Sizable as _;
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::{h_flex, v_flex};

/// The placeholder cover: shown until the real icon arrives, and instead of
/// it when a project has none or the picture cannot be loaded.
fn placeholder(seed: u32, colors: ShellColors) -> gpui::AnyElement {
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
fn spec(legend: &'static str, value: String, colors: ShellColors) -> impl IntoElement {
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

/// The result row's action: install, "installing" while it runs, or — when
/// the target game already has it — a chip saying so, or an update button
/// when a newer version fits.
pub(super) fn install_control(
    index: usize,
    row: &SearchRow,
    ctx: &LiveCtx,
    install: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui::AnyElement {
    let colors = ctx.colors;
    if ctx.model.installing.contains(&row.slug) {
        return kit::action(
            ("live-installing", index),
            tr!("discover-installing"),
            Some(UiIcon::Download),
            false,
            |_, _| {},
        )
        .small()
        .disabled(true)
        .into_any_element();
    }
    let Some(have) = ctx.model.installed.get(&row.project_id) else {
        // ia[discover]: 安装（最新兼容版本） | 结果行「安装」 | 后台任务，toast“开始安装”，完成 toast，期间按钮显示「安装中…」；Mod 先过依赖提示
        return kit::action(
            ("live-install", index),
            tr!("discover-install"),
            Some(UiIcon::Download),
            false,
            install,
        )
        .small()
        .into_any_element();
    };
    // ia[discover]: 已安装状态 | 结果行「已安装」/「更新」 | 目标游戏已有的（按 Modrinth 识别）显示「已安装」；有更新显示「更新」，点了用新版本替换旧文件；详情页主按钮同样变化 | 只认 Modrinth 认得的文件
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
                tr!("discover-update"),
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
            .child(kit::chip(
                tr!("discover-installed"),
                Some(kit::tone_ok()),
                colors,
            ))
            .into_any_element(),
    }
}

/// The tags of a row: the environment, then categories and loaders as
/// `card_tags` chose, then how many more there are.
fn tag_line(row: &SearchRow, colors: ShellColors) -> impl IntoElement {
    let plain = |text: String| kit::tag(text, kit::TagKind::Plain, colors);
    h_flex()
        .flex_wrap()
        .gap(px(6.))
        .children(
            row.environment
                .map(|environment| plain(environment_label(environment).to_string())),
        )
        .children(row.tags.shown.iter().map(|tag| {
            let kind = if tag.loader {
                kit::TagKind::Ink
            } else {
                kit::TagKind::Plain
            };
            kit::tag(tag_label(&tag.name), kind, colors)
        }))
        .children(
            (!row.tags.overflow.is_empty()).then(|| plain(format!("+{}", row.tags.overflow.len()))),
        )
}

// ia[discover]: 打开项目详情 | 点结果行 | 详情页（进历史）
// ia[discover]: 结果行右键菜单 | 在 Modrinth 中打开 / 复制链接 | 浏览器打开项目页，或把项目页地址放进剪贴板并提示
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
    let body = colors.body;
    let (date_legend, date) = match row.date_kind {
        DateKind::Published => ("PUB", row.date.clone()),
        DateKind::Updated => ("UPD", row.date.clone()),
    };
    let (menu_url, copy_url) = (row.page_url.clone(), row.page_url.clone());
    let copy = ctx.handler.clone();

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
                .child(div().pt(px(2.)).child(tag_line(row, colors))),
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
                        .children((!date.is_empty()).then(|| spec(date_legend, date, colors))),
                ),
        )
        .context_menu(move |menu, _, _| {
            let (open_url, copy_url) = (menu_url.clone(), copy_url.clone());
            let copy = copy.clone();
            menu.item(PopupMenuItem::new(tr!("discover-open-modrinth")).on_click(
                move |_, _, cx| {
                    cx.open_url(&open_url);
                },
            ))
            .item(
                PopupMenuItem::new(tr!("discover-copy-link")).on_click(move |_, window, cx| {
                    copy(
                        LiveIntent::CopyText {
                            text: copy_url.clone(),
                            notice: tr!("discover-link-copied").to_owned(),
                        },
                        window,
                        cx,
                    );
                }),
            )
        })
}
