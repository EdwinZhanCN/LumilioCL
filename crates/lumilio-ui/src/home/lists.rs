use super::buttons::{LocalActionIcon, page_button};
use super::render::{ShellHomeColors, body_label};
use super::{ActHandler, AttentionRow, HomeIntent, HomeIntentHandler, OpenHandler, RecentEntry};
use crate::key::Key;
use gpui::prelude::*;
use gpui::{AnyElement, ClickEvent, IntoElement, div, px};
use gpui_component::StyledExt as _;
use gpui_component::{h_flex, v_flex};

pub(super) fn render_first_use(
    intent_handler: Option<HomeIntentHandler>,
    colors: ShellHomeColors,
) -> impl IntoElement {
    // ia[home]: 空库：导入 | 首次使用的「把原来的游戏带过来」 | 同游戏库的导入其他启动器的游戏
    let primary = page_button(
        "home-import",
        "把原来的游戏带过来",
        LocalActionIcon::Import,
        HomeIntent::Import,
        true,
        intent_handler.clone(),
    );
    // ia[home]: 空库：新建 | 首次使用的「新建」 | 同游戏库的新建游戏
    let secondary = page_button(
        "home-create",
        "新建",
        LocalActionIcon::Create,
        HomeIntent::Create,
        false,
        intent_handler,
    );

    h_flex()
        .w_full()
        .flex_wrap()
        .justify_between()
        .items_center()
        .gap_6()
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child("第一次来到这里"),
                )
                .child(
                    div()
                        .text_size(px(20.))
                        .font_semibold()
                        .text_color(colors.foreground)
                        .child("先把熟悉的世界放在手边"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child("导入原来的游戏，其余设置之后再慢慢展开。"),
                ),
        )
        .child(h_flex().gap_3().child(primary).child(secondary))
}

/// Games with something wrong, one row each with its first remedy.
pub(super) fn render_attention(
    rows: &[AttentionRow],
    on_act: Option<ActHandler>,
    colors: ShellHomeColors,
) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    // ia[home]: 需要留意：解决 | 每个有问题的游戏一行（游戏名 · 最严重的问题 · 另有几个）和一个按钮（安装/修复/更换…/去设置/去添加/查看日志…） | 点后先打开游戏页再执行
    let list = rows.iter().enumerate().map(|(index, row)| {
        let button = row
            .action
            .zip(on_act.clone())
            .map(|((action, label), act)| {
                let id = row.instance.clone();
                Key::new(("home-attention-act", index))
                    .label(label)
                    .white()
                    .debug_selector(move || format!("home-attention-act-{index}"))
                    .on_click(move |_: &ClickEvent, window, cx| act(id.clone(), action, window, cx))
            });
        h_flex()
            .id(("home-attention", index))
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .py(px(10.))
            .border_b_1()
            .border_color(colors.border)
            .child(
                v_flex()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(colors.foreground)
                            .child(format!("{} · {}", row.name, row.title)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(if row.more > 0 {
                                format!("{}（另有 {} 个问题）", row.detail, row.more)
                            } else {
                                row.detail.clone()
                            }),
                    ),
            )
            .children(button)
    });
    Some(
        v_flex()
            .gap_2()
            .child(body_label("需要留意", colors.muted))
            .child(v_flex().w_full().children(list))
            .into_any_element(),
    )
}

pub(super) fn render_recent(
    recent: &[RecentEntry],
    on_open: Option<OpenHandler>,
    colors: ShellHomeColors,
) -> impl IntoElement {
    // Without the library (a Home drawn from a snapshot) the recent games are
    // plain cards; the live Home uses the Library's faceplates.
    let cards = recent.iter().enumerate().map(|(index, entry)| {
        let open = entry.id.clone().zip(on_open.clone());
        v_flex()
            .id(("home-recent", index))
            .w(px(232.))
            .gap(px(4.))
            .px(px(14.))
            .py(px(12.))
            .rounded(px(6.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface)
            // A card that opens a game says so (design language §9); one that
            // does not stays still.
            .when_some(open, |card, (id, open)| {
                card.cursor_pointer()
                    .hover(move |card| card.border_color(colors.foreground))
                    .debug_selector(move || format!("home-recent-{index}"))
                    .on_click(move |_, window, cx| open(id.clone(), window, cx))
            })
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(colors.foreground)
                    .child(entry.title.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .child(entry.metadata.clone()),
            )
    });

    v_flex()
        .gap_3()
        .child(body_label("最近", colors.muted))
        .child(h_flex().flex_wrap().gap_3().children(cards))
}
