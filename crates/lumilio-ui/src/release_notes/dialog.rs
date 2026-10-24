//! The release-notes dialog: the summary, then each section's items, for one
//! release or for every release skipped since the notes were last shown.

use gpui::prelude::*;
use gpui::{App, Window, div, px};
use gpui_component::text::TextView;
use gpui_component::{WindowExt as _, h_flex, v_flex};

use super::text::{Notes, Section};
use super::{ReleaseNotes, text};
use crate::i18n::Locale;
use crate::key::Key;
use crate::kit::TagKind;
use crate::theme::ShellColors;
use crate::{i18n, kit, theme, tr};

const DIALOG_WIDTH: f32 = 520.;

/// Why the notes are shown, which decides the title.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Occasion {
    /// The first start after an update.
    Updated,
    /// Opened from Settings › About.
    Asked,
}

/// Opens the notes of `releases`, newest first. Nothing opens for none.
// ia[home]: 查看更新内容 | 更新后第一次打开时的弹窗 | 标题写明已更新到的版本，列出这次（以及跳过的版本）的新功能、改进和修复；「知道了」、Esc 或点外面关闭，之后不再自动出现；「全部更新日志」在浏览器打开网站的更新日志 | 设置 › 关于 可再看
pub fn show(
    releases: Vec<&'static ReleaseNotes>,
    occasion: Occasion,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(newest) = releases.first().map(|notes| notes.version) else {
        return;
    };
    let releases: Vec<(&'static str, Notes)> = releases
        .iter()
        .map(|notes| (notes.version, text::parse(notes.text())))
        .collect();
    window.open_dialog(cx, move |dialog, _, cx| {
        let colors = ShellColors::current(cx);
        let title = match occasion {
            Occasion::Updated => tr!("release-notes-updated", version = newest),
            Occasion::Asked => tr!("release-notes-title", version = newest),
        };
        let several = releases.len() > 1;
        let mut items = 0;
        let body = v_flex()
            .w_full()
            .gap_6()
            .pb_2()
            .debug_selector(|| "release-notes-body".into())
            .children(
                releases
                    .iter()
                    .enumerate()
                    .map(|(index, (version, notes))| {
                        let block = release(version, notes, several, &mut items, colors);
                        v_flex()
                            .w_full()
                            .gap_6()
                            .when(index > 0, |this| {
                                this.child(div().w_full().h(px(1.)).bg(colors.border))
                            })
                            .child(block)
                    }),
            );
        let changelog = match i18n::locale() {
            Locale::English => "https://launcher.lumilio.org/en/docs/changelog/",
            Locale::SimplifiedChinese => "https://launcher.lumilio.org/docs/changelog/",
        };
        let footer = h_flex()
            .w_full()
            .gap_2()
            .child(
                Key::new("release-notes-all")
                    .label(tr!("release-notes-all"))
                    .white()
                    .debug_selector(|| "release-notes-all".into())
                    .on_click(move |_, _, cx| cx.open_url(changelog)),
            )
            .child(div().flex_1())
            .child(
                Key::new("release-notes-done")
                    .label(tr!("release-notes-done"))
                    .primary()
                    .debug_selector(|| "release-notes-done".into())
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            );
        theme::dialog(dialog, cx)
            .title(title)
            .w(px(DIALOG_WIDTH))
            .child(body)
            .footer(footer)
    });
}

/// One release: its version when several are shown, the summary, and the
/// sections in the file's order.
fn release(
    version: &'static str,
    notes: &Notes,
    several: bool,
    items: &mut usize,
    colors: ShellColors,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_5()
        .debug_selector(move || format!("release-notes-{version}"))
        .when(several, |this| {
            this.child(h_flex().child(kit::tag(version, TagKind::Plain, colors)))
        })
        .when(!notes.summary.is_empty(), |this| {
            this.child(
                div()
                    .text_sm()
                    .text_color(colors.foreground)
                    .child(notes.summary.clone()),
            )
        })
        .children(
            notes
                .sections
                .iter()
                .map(|section| section_block(section, items, colors).into_any_element()),
        )
}

fn section_block(section: &Section, items: &mut usize, colors: ShellColors) -> impl IntoElement {
    let list = v_flex()
        .w_full()
        .gap_2()
        .children(section.items.iter().map(|item| {
            *items += 1;
            h_flex()
                .w_full()
                .items_start()
                .gap_3()
                .child(
                    // A quiet square marker, not an LED: it carries no state.
                    div().flex_none().mt(px(8.)).size(px(4.)).bg(colors.muted),
                )
                .child(
                    TextView::markdown(("release-notes-item", *items), item.clone())
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(colors.foreground),
                )
        }));
    if section.title.is_empty() {
        list.into_any_element()
    } else {
        kit::section(section.title.clone(), colors, list).into_any_element()
    }
}
