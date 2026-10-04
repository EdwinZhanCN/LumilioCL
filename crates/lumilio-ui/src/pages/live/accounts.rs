use super::controls::{LiveCtx, send};
use crate::assets::UiIcon;
use crate::kit;
use crate::live::{AccountRow, LiveIntent};
use gpui::prelude::*;
use gpui::{App, IntoElement, Window, div, px};
use gpui_component::StyledExt as _;
use gpui_component::WindowExt as _;
use gpui_component::{h_flex, v_flex};

pub(super) fn account_row(index: usize, row: &AccountRow, ctx: &LiveCtx) -> gpui::Div {
    let colors = ctx.colors;
    // ia[accounts]: 选为当前 | 账户行左侧的单选圆点 | 之后的启动使用该身份
    let select = send(&ctx.handler, LiveIntent::SelectAccount(row.key.clone()));
    let dot = div()
        .flex_none()
        .size(px(16.))
        .rounded_full()
        .border_1()
        .border_color(if row.selected {
            colors.primary
        } else {
            colors.muted.opacity(0.6)
        })
        .flex()
        .items_center()
        .justify_center()
        .when(row.selected, |ring| {
            ring.child(div().size(px(8.)).rounded_full().bg(colors.primary))
        });
    let short = row.uuid.split('-').next().unwrap_or_default().to_owned();
    let detail = format!(
        "{} · {short}{}{}",
        row.kind_label(),
        if row.custom_id {
            " · 自定义 UUID"
        } else {
            ""
        },
        row.skin_text()
            .map_or_else(String::new, |skin| format!(" · {skin}")),
    );
    // ia[accounts]: 复制 UUID | 账户行 ⋯ 菜单 | 复制到剪贴板，toast“已复制 UUID”
    let copy = {
        let handler = ctx.handler.clone();
        let text = row.uuid.clone();
        move |window: &mut Window, cx: &mut App| {
            handler(
                LiveIntent::CopyText {
                    text: text.clone(),
                    notice: "已复制 UUID".to_owned(),
                },
                window,
                cx,
            )
        }
    };
    // ia[accounts]: 移除 | 账户行 ⋯ 菜单 → 警告弹窗 | 只删身份（Microsoft 账户同时删凭据库里的登录信息），不删游戏和存档；移除当前账户后改用剩下的第一个
    let remove = {
        let handler = ctx.handler.clone();
        let (key, name) = (row.key.clone(), row.name.clone());
        let selected = row.selected;
        let microsoft = row.signed_in();
        move |window: &mut Window, cx: &mut App| {
            let handler = handler.clone();
            let target = key.clone();
            let title = format!("移除账户“{name}”？");
            let description = match (microsoft, selected) {
                (true, true) => {
                    "会忘记这个身份并从系统凭据库删除它的登录信息（第三方账户还会通知服务器作废令牌），不会删除任何游戏或存档。它是当前账户，移除后会改用剩下的第一个。"
                }
                (true, false) => {
                    "会忘记这个身份并从系统凭据库删除它的登录信息，不会删除任何游戏或存档。"
                }
                (false, true) => {
                    "只会忘记这个身份，不会删除任何游戏或存档。它是当前账户，移除后会改用剩下的第一个。"
                }
                (false, false) => "只会忘记这个身份，不会删除任何游戏或存档。",
            };
            window.open_alert_dialog(cx, move |alert, _, _| {
                let handler = handler.clone();
                let target = target.clone();
                alert
                    .title(title.clone())
                    .description(description)
                    .ok_text("移除")
                    .ok_variant(gpui_component::button::ButtonVariant::Danger)
                    .cancel_text("取消")
                    .show_cancel(true)
                    .on_ok(move |_, window, cx| {
                        handler(LiveIntent::RemoveAccount(target.clone()), window, cx);
                        true
                    })
            });
        }
    };
    let mut entries = vec![kit::MenuEntry::new("复制 UUID", copy)];
    if !row.microsoft && !row.third_party {
        // ia[accounts]: 设置皮肤 | 离线账户行 ⋯ 菜单「皮肤…」→ 弹窗 | 选默认、本地文件、LittleSkin 或自定义皮肤站；游戏里按所选显示（启动时在本机起一个皮肤服务器，需要 authlib-injector）；加载不到时游戏照常启动并在日志里说明 | ADR 0024
        entries.push(kit::MenuEntry::new(
            "皮肤…",
            send(&ctx.handler, LiveIntent::EditSkin(row.key.clone())),
        ));
    }
    if row.signed_in() {
        // ia[accounts]: 刷新登录 | 已登录账户行 ⋯ 菜单「刷新登录」 | 失效的登录显示“需要重新登录”，刷新后恢复
        entries.push(kit::MenuEntry::new(
            "刷新登录",
            send(&ctx.handler, LiveIntent::RefreshAccount(row.key.clone())),
        ));
    }
    entries.push(kit::MenuEntry::new("移除…", remove).danger());
    let menu = kit::more_menu(("account-more", index), entries, colors);
    h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .py(px(10.))
        .child(
            h_flex()
                .id(("account-choose", index))
                .debug_selector(move || format!("account-choose-{index}"))
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .on_click(move |_, window, cx| select(window, cx))
                .child(dot)
                .child(kit::avatar(&row.name, 32.))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(colors.foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(row.name.clone()),
                        )
                        .child(div().text_xs().text_color(colors.muted).child(detail)),
                ),
        )
        .children(
            row.needs_sign_in
                .then(|| kit::chip("需要重新登录", Some(colors.danger), colors)),
        )
        .children(
            row.selected
                .then(|| kit::chip("当前", Some(colors.primary), colors)),
        )
        .child(menu)
}

pub fn accounts(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let model = ctx.model;
    let subtitle = match model.selected_account() {
        Some(row) => format!("当前：{}（{}）", row.name, row.kind_label()),
        None if model.accounts_loaded => "还没有账户，添加一个才能进游戏".to_owned(),
        None => String::new(),
    };
    // ia[accounts]: 添加离线账户 | L2 次要「添加离线账户」→ 弹窗（可自定义 UUID） | 列表新增；UUID 只在添加时可设
    let add = kit::action(
        "account-new",
        "添加离线账户",
        Some(UiIcon::Plus),
        false,
        send(&ctx.handler, LiveIntent::NewAccount),
    );
    // ia[accounts]: Microsoft 登录 | L2 主要「登录 Microsoft」→ 设备码弹窗 | 登录后出现在列表和导航芯片里，启动用真实的玩家名和令牌 | ADR 0020；真实登录要 Mojang 批准应用注册
    let microsoft = kit::action(
        "account-microsoft",
        "登录 Microsoft",
        None,
        true,
        send(&ctx.handler, LiveIntent::MicrosoftSignIn),
    );
    // ia[accounts]: 第三方登录 | 页头 ⋯ 菜单「第三方登录…」→ 弹窗 | 选认证服务器（内置 LittleSkin）、输入账号密码；多个角色时选一个；登录后出现在列表里，启动时自动加载 authlib-injector | ADR 0024
    // ia[accounts]: 管理认证服务器 | 页头 ⋯ 菜单「认证服务器…」→ 弹窗 | 添加（输入地址，先看到名称，http 有警告）、移除（会一并移除该服务器上的账户）；LittleSkin 内置
    let body = if model.accounts.is_empty() {
        v_flex()
            .items_center()
            .child(kit::empty(
                "还没有账户",
                "用 Microsoft 登录可以进入正版服务器；LittleSkin 等第三方认证服务器在右上角 ⋯ 里登录；离线账户不需要登录，名称就是你在游戏里的名字。",
                colors,
            ))
            .into_any_element()
    } else {
        kit::list(
            model
                .accounts
                .iter()
                .enumerate()
                .map(|(index, row)| account_row(index, row, ctx))
                .collect(),
            colors,
        )
        .into_any_element()
    };
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "账户",
            subtitle,
            kit::PageActions::new("accounts-actions")
                .secondary(add)
                .primary(microsoft)
                .more(kit::MenuEntry::new(
                    "第三方登录…",
                    send(&ctx.handler, LiveIntent::ThirdPartySignIn),
                ))
                .more(kit::MenuEntry::new(
                    "认证服务器…",
                    send(&ctx.handler, LiveIntent::ManageAuthServers),
                ))
                .render(colors),
            colors,
        ))
        .child(body)
}
