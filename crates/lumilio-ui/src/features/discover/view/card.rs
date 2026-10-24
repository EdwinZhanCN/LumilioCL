//! One result row: icon, name, tags, install control and numbers.
//!
//! Which tags show and in what order follows Modrinth App's project card
//! (`live::card_tags`); the date is the publishing date when the list is
//! sorted by newest, else the last update, and the count is followers when
//! the list is sorted by them, else downloads.

use gpui::prelude::*;
use gpui::{App, Window, div, px};
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::{Sizable as _, h_flex, v_flex};

use super::super::model::{CountKind, DateKind, SearchRow, environment_label, tag_label};
use super::context::{DiscoverCtx, DiscoverIntent, send};
use crate::assets::UiIcon;
use crate::theme::ShellColors;
use crate::{kit, theme, tr};

/// Edge of a result's icon: a list row's 48 px display window (§12).
const ICON_SIDE: f32 = 48.;
/// The readings column is at least this wide, so a short count and a long
/// date keep the keys after them in line.
const READINGS_WIDTH: f32 = 112.;
/// The key slot is as wide as the widest of its states (安装中…), so the
/// readings before it line up from row to row.
const KEY_WIDTH: f32 = 96.;
/// The narrowest list a result row reads well in: the index, the icon,
/// about 280 px for the name and summary, the readings and the key.
pub(super) const ROW_MIN_WIDTH: f32 = 624.;

/// One line of a row's spec column: a muted mono legend and its value.
fn spec(legend: &'static str, value: String, colors: ShellColors) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_baseline()
        .justify_end()
        .font_family(theme::mono_font())
        .child(
            div()
                .text_size(crate::theme::font_px(10.))
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
    ctx: &DiscoverCtx,
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
        .debug_selector(move || format!("live-install-{index}"))
        .into_any_element();
    };
    // ia[discover]: 已安装状态 | 结果行「已安装」/「更新」 | 目标游戏已有的（按正在浏览的来源识别）显示「已安装」；有更新显示「更新」，点了用新版本替换旧文件；详情页主按钮同样变化 | 只认这个来源认得的文件
    match &have.update {
        Some(version) => {
            let intent = DiscoverIntent::UpdateInstalled {
                kind: row.kind,
                source: row.source.clone(),
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

/// The tags of a row: whether the author allows downloads only on the
/// source's site, the environment, then categories and loaders as
/// `card_tags` chose, then how many more there are. They keep to one line:
/// a tag the line has no room for wraps out of sight whole rather than
/// being cut, and the count stays after the ones that show.
fn tag_line(row: &SearchRow, colors: ShellColors) -> impl IntoElement {
    let plain = |text: String| kit::tag(text, kit::TagKind::Plain, colors);
    let tags = h_flex()
        .min_w_0()
        .h(px(20.))
        .overflow_hidden()
        .flex_wrap()
        .gap(px(6.))
        .children(
            row.manual_download
                .then(|| kit::tag(tr!("discover-site-only"), kit::TagKind::Ink, colors)),
        )
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
        }));
    h_flex().min_w_0().gap(px(6.)).child(tags).children(
        (!row.tags.overflow.is_empty()).then(|| plain(format!("+{}", row.tags.overflow.len()))),
    )
}

// ia[discover]: 打开项目详情 | 点结果行 | 详情页（进历史）
// ia[discover]: 结果行右键菜单 | 在（来源）中打开 / 复制链接 | 浏览器打开项目页，或把项目页地址放进剪贴板并提示
pub(super) fn result_row(index: usize, row: &SearchRow, ctx: &DiscoverCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let open = send(
        &ctx.handler,
        DiscoverIntent::OpenProject {
            kind: row.kind,
            source: row.source.clone(),
            slug: row.slug.clone(),
        },
    );
    let install = {
        let handler = ctx.handler.clone();
        let intent = DiscoverIntent::Install {
            kind: row.kind,
            source: row.source.clone(),
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
    let open_label = tr!(
        "discover-open-in",
        source = ctx.model.source_name(&row.source)
    );
    let copy = ctx.handler.clone();
    let count_legend = match row.count_kind {
        CountKind::Downloads => "DL",
        CountKind::Follows => "FAV",
    };

    // A list row (§12): index, framed icon, then the name with its author,
    // a one-line summary and the tags; readings and the key on the trailing
    // edge. Everything is centred on the row.
    h_flex()
        .id(("live-result", index))
        .w_full()
        .gap_3()
        .py(px(10.))
        .px(px(4.))
        .items_center()
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
                .font_family(theme::mono_font())
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
                .child(kit::project_icon(
                    row.icon_url.as_deref(),
                    row.seed,
                    ICON_SIDE,
                    colors,
                )),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .debug_selector(move || format!("live-result-name-{index}"))
                .child(
                    // The author gives way first; the name is cut only
                    // when it alone is wider than the line.
                    h_flex()
                        .min_w_0()
                        .gap(px(10.))
                        .items_baseline()
                        .child(
                            div()
                                .flex_shrink_0()
                                .max_w_full()
                                .text_size(crate::theme::font_px(20.))
                                .line_height(px(26.))
                                .font_weight(gpui::FontWeight::LIGHT)
                                .text_color(colors.foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(row.title.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_family(theme::mono_font())
                                .text_size(crate::theme::font_px(11.))
                                .text_color(colors.muted)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(row.author.to_uppercase()),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(
                            row.translated_summary
                                .as_ref()
                                .unwrap_or(&row.summary)
                                .clone(),
                        ),
                )
                .child(div().pt(px(6.)).child(tag_line(row, colors))),
        )
        .child(
            v_flex()
                .flex_none()
                .min_w(px(READINGS_WIDTH))
                .debug_selector(move || format!("live-result-readings-{index}"))
                .items_end()
                .gap(px(3.))
                .child(spec(count_legend, row.count.clone(), colors))
                .children((!date.is_empty()).then(|| spec(date_legend, date, colors))),
        )
        .child(
            h_flex()
                .flex_none()
                .min_w(px(KEY_WIDTH))
                .justify_end()
                .child(install_control(index, row, ctx, install)),
        )
        .context_menu(move |menu, _, _| {
            let (open_url, copy_url) = (menu_url.clone(), copy_url.clone());
            let copy = copy.clone();
            menu.item(
                PopupMenuItem::new(open_label.clone()).on_click(move |_, _, cx| {
                    cx.open_url(&open_url);
                }),
            )
            .item(
                PopupMenuItem::new(tr!("discover-copy-link")).on_click(move |_, window, cx| {
                    copy(
                        DiscoverIntent::CopyText {
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
