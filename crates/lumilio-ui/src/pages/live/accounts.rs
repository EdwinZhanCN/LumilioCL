//! Accounts: the list on the leading side, the chosen account's detail on
//! the trailing side, each scrolling on its own (the master–detail of the
//! Settings plugins tab). The detail carries the account's look: a preview
//! and what can be worn, which depends on the kind of account.

use super::controls::{LiveCtx, send};
use crate::assets::UiIcon;
use crate::kit::{self, ViewIntent};
use crate::live::{AccountRow, LiveIntent, LiveModel};
use crate::pages::ViewState;
use crate::wardrobe::{PINNED_PREVIEW_WIDTH, PREVIEW_WIDTH};
use crate::{theme, tr};
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, Window, div, px};
use gpui_component::StyledExt as _;
use gpui_component::WindowExt as _;
use gpui_component::{h_flex, v_flex};

/// The view-state group that remembers which account the detail shows.
pub const ACCOUNT_GROUP: u8 = 220;

/// The account the detail shows: the one last chosen in the list, or the
/// current account.
#[must_use]
pub fn shown_account<'a>(
    model: &'a LiveModel,
    state: &ViewState,
) -> Option<(usize, &'a AccountRow)> {
    let current = model.accounts.iter().position(|row| row.selected);
    let index = state
        .choice(ACCOUNT_GROUP, current.unwrap_or(0))
        .min(model.accounts.len().checked_sub(1)?);
    model.accounts.get(index).map(|row| (index, row))
}

fn list_item(
    index: usize,
    row: &AccountRow,
    shown: bool,
    ctx: &LiveCtx,
) -> gpui::Stateful<gpui::Div> {
    let colors = ctx.colors;
    let emit = ctx.emit.clone();
    // ia[accounts]: 查看账户 | 左侧列表的一行 | 右侧显示该账户的详情与外观；不改变当前账户
    h_flex()
        .id(gpui::ElementId::Name(
            format!("account-item-{}", row.key).into(),
        ))
        .debug_selector(move || format!("account-item-{index}"))
        .w_full()
        .items_center()
        .gap_3()
        .px_3()
        .py(px(10.))
        .rounded_md()
        .cursor_pointer()
        .when(shown, |item| item.bg(colors.surface_subtle))
        .hover(|item| item.bg(colors.surface_subtle))
        .on_click(move |_, window, cx| emit(ViewIntent::Choose(ACCOUNT_GROUP, index), window, cx))
        .child(kit::avatar(&row.name, 28.))
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
                .child(
                    div()
                        .text_xs()
                        .text_color(if row.needs_sign_in {
                            colors.danger
                        } else {
                            colors.muted
                        })
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(if row.needs_sign_in {
                            tr!("account-needs-sign-in").to_owned()
                        } else {
                            row.kind_label().to_owned()
                        }),
                ),
        )
        .children(
            row.selected
                .then(|| kit::chip(tr!("account-chip-current"), Some(colors.primary), colors)),
        )
}

fn remove_action(row: &AccountRow, ctx: &LiveCtx) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = ctx.handler.clone();
    let (key, name) = (row.key.clone(), row.name.clone());
    let selected = row.selected;
    let signed_in = row.signed_in();
    move |window: &mut Window, cx: &mut App| {
        let handler = handler.clone();
        let target = key.clone();
        let title = tr!("account-remove-title", name = name.as_str());
        let description = match (signed_in, selected) {
            (true, true) => tr!("account-remove-body-signed-in-current"),
            (true, false) => tr!("account-remove-body-signed-in"),
            (false, true) => tr!("account-remove-body-offline-current"),
            (false, false) => tr!("account-remove-body-offline"),
        };
        window.open_alert_dialog(cx, move |alert, _, _| {
            let handler = handler.clone();
            let target = target.clone();
            alert
                .title(title.clone())
                .description(description)
                .ok_text(tr!("common-remove"))
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text(tr!("common-cancel"))
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    handler(LiveIntent::RemoveAccount(target.clone()), window, cx);
                    true
                })
        });
    }
}

/// The account's name, kind and id, with what can be done to it.
fn detail_head(row: &AccountRow, ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    // The UUID is copied from the ⋯ menu; reading it is rarely the task.
    let mut facts = vec![row.kind_label().to_owned()];
    if row.custom_id {
        facts.push(tr!("account-detail-custom-uuid").to_owned());
    }
    // ia[accounts]: 复制 UUID | 详情 ⋯ 菜单 | 复制到剪贴板，toast“已复制 UUID”
    let copy = send(
        &ctx.handler,
        LiveIntent::CopyText {
            text: row.uuid.clone(),
            notice: tr!("account-copied-uuid").to_owned(),
        },
    );
    let mut entries = vec![kit::MenuEntry::new(tr!("account-menu-copy-uuid"), copy)];
    if row.signed_in() {
        // ia[accounts]: 刷新登录 | 已登录账户详情 ⋯ 菜单「刷新登录」 | 失效的登录显示“需要重新登录”，刷新后恢复
        entries.push(kit::MenuEntry::new(
            tr!("account-menu-refresh"),
            send(&ctx.handler, LiveIntent::RefreshAccount(row.key.clone())),
        ));
    }
    // ia[accounts]: 移除 | 详情 ⋯ 菜单 → 警告弹窗 | 只删身份（已登录账户同时删凭据库里的登录信息），不删游戏和存档；移除当前账户后改用剩下的第一个
    entries.push(kit::MenuEntry::new(tr!("account-menu-remove"), remove_action(row, ctx)).danger());
    h_flex()
        .w_full()
        .items_center()
        .gap_4()
        .child(kit::avatar(&row.name, 56.))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_xl()
                                .font_semibold()
                                .text_color(colors.foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(row.name.clone()),
                        )
                        .children(row.needs_sign_in.then(|| {
                            kit::chip(tr!("account-needs-sign-in"), Some(colors.danger), colors)
                        }))
                        .children(row.selected.then(|| {
                            kit::chip(tr!("account-chip-current"), Some(colors.primary), colors)
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(facts.join(" · ")),
                ),
        )
        .children((!row.selected).then(|| {
            // ia[accounts]: 设为当前 | 详情页头「设为当前账户」 | 之后的启动使用该身份；导航芯片也跟着换
            kit::action(
                "account-make-current",
                tr!("account-make-current"),
                None,
                false,
                send(&ctx.handler, LiveIntent::SelectAccount(row.key.clone())),
            )
            .debug_selector(|| "account-make-current".into())
        }))
        .child(kit::more_menu("account-detail-more", entries, colors))
}

/// A third-party account's look: the preview, and where to change it.
fn third_party_look(row: &AccountRow, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let address = row.skin_site.clone();
    h_flex()
        .w_full()
        .flex_wrap()
        .items_start()
        .gap_8()
        .children(
            ctx.account_viewer
                .map(|viewer| div().w(px(PREVIEW_WIDTH)).flex_none().child(viewer.clone())),
        )
        // ia[accounts]: 在皮肤站修改 | 第三方账户人物旁的「在皮肤站修改」 | 打开服务器公布的主页（LittleSkin 打开其网站）；皮肤与披风只从会话档案预览
        .child(
            v_flex()
                .flex_1()
                .min_w(px(200.))
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(tr!("account-look-third-party")),
                )
                .children(address.map(|address| {
                    h_flex().child(kit::action(
                        "account-skin-site",
                        tr!("wardrobe-open-site"),
                        Some(UiIcon::External),
                        false,
                        send(&ctx.handler, LiveIntent::OpenSkinSite(address)),
                    ))
                })),
        )
        .into_any_element()
}

/// The detail fills the pane: the account's head over its look. With room,
/// the wardrobe keeps the figure put and scrolls its own grid; in a narrow
/// window everything stacks and scrolls together.
fn detail(row: &AccountRow, ctx: &LiveCtx) -> AnyElement {
    let look = match ctx.wardrobe {
        Some(wardrobe) => wardrobe.clone().into_any_element(),
        None => third_party_look(row, ctx),
    };
    if ctx.window_width < PINNED_PREVIEW_WIDTH || ctx.wardrobe.is_none() {
        return div()
            .id("account-detail")
            .debug_selector(|| "account-detail".into())
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .gap_6()
                    .pb(theme::BOTTOM_SAFE_AREA)
                    .child(detail_head(row, ctx))
                    .child(look),
            )
            .into_any_element();
    }
    v_flex()
        .id("account-detail")
        .debug_selector(|| "account-detail".into())
        .size_full()
        .min_h_0()
        .gap_6()
        .child(detail_head(row, ctx))
        .child(div().flex_1().min_h_0().child(look))
        .into_any_element()
}

pub fn accounts(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let model = ctx.model;
    let subtitle = match model.selected_account() {
        Some(row) => tr!(
            "account-subtitle-current",
            name = row.name.as_str(),
            kind = row.kind_label()
        ),
        None if model.accounts_loaded => tr!("account-subtitle-empty").to_owned(),
        None => String::new(),
    };
    // ia[accounts]: 添加离线账户 | L2 次要「添加离线账户」→ 弹窗（可自定义 UUID） | 列表新增；UUID 只在添加时可设
    let add = kit::action(
        "account-new",
        tr!("account-add-offline"),
        Some(UiIcon::Plus),
        false,
        send(&ctx.handler, LiveIntent::NewAccount),
    );
    // ia[accounts]: Microsoft 登录 | L2 主要「登录 Microsoft」→ 设备码弹窗 | 登录后出现在列表和导航芯片里，启动用真实的玩家名和令牌 | ADR 0020
    let microsoft = kit::action(
        "account-microsoft",
        tr!("account-sign-in-microsoft"),
        None,
        true,
        send(&ctx.handler, LiveIntent::MicrosoftSignIn),
    );
    // ia[accounts]: 第三方登录 | 页头 ⋯ 菜单「第三方登录…」→ 弹窗 | 选认证服务器（内置 LittleSkin）、输入账号密码；多个角色时选一个；登录后出现在列表里，启动时自动加载 authlib-injector | ADR 0024
    // ia[accounts]: 管理认证服务器 | 页头 ⋯ 菜单「认证服务器…」→ 弹窗 | 添加（输入地址，先看到名称，http 有警告）、移除（会一并移除该服务器上的账户）；LittleSkin 内置
    let head = kit::header(
        tr!("route-accounts"),
        subtitle,
        kit::PageActions::new("accounts-actions")
            .secondary(add)
            .primary(microsoft)
            .more(kit::MenuEntry::new(
                tr!("account-menu-third-party"),
                send(&ctx.handler, LiveIntent::ThirdPartySignIn),
            ))
            .more(kit::MenuEntry::new(
                tr!("account-menu-servers"),
                send(&ctx.handler, LiveIntent::ManageAuthServers),
            ))
            .render(colors),
        colors,
    );
    let body = match shown_account(model, ctx.state) {
        None => kit::scroll_body(
            "accounts-empty",
            v_flex().items_center().child(kit::empty(
                tr!("account-empty-title"),
                tr!("account-empty-help"),
                colors,
            )),
        )
        .into_any_element(),
        Some((shown, row)) => {
            let list = v_flex()
                .id("account-list")
                .w(px(260.))
                .flex_none()
                .h_full()
                .min_h_0()
                .overflow_y_scroll()
                .gap_1()
                .pb(theme::BOTTOM_SAFE_AREA)
                .children(
                    model
                        .accounts
                        .iter()
                        .enumerate()
                        .map(|(index, row)| list_item(index, row, index == shown, ctx)),
                );
            let pane = div()
                .flex_1()
                .min_w_0()
                .h_full()
                .min_h_0()
                .child(detail(row, ctx));
            kit::pane_body(
                h_flex()
                    .w_full()
                    .h_full()
                    .min_h_0()
                    .items_start()
                    .gap_6()
                    .child(list)
                    .child(pane),
            )
            .into_any_element()
        }
    };
    kit::fixed_page("live-accounts", head, body)
}
