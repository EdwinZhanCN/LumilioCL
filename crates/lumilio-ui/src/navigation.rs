//! The floating navigation bar (design language §6): history and the current
//! location on the leading side, the landmark capsule in the centre, the
//! current instance on the trailing side.

use std::rc::Rc;
use std::sync::Arc;

use crate::key::Key;
use crate::tr;
use gpui::{
    Anchor, AnyElement, App, ClickEvent, IntoElement, RenderImage, SharedString, Window, div,
    prelude::*, px,
};
use gpui_component::popover::Popover;
use gpui_component::{Icon, Selectable as _, Sizable as _, StyledExt as _, h_flex, v_flex};

use crate::{
    assets::{LandmarkIcon, UiIcon},
    home::WorldHint,
    route::Route,
    theme::{self, ShellColors},
};

pub type RouteHandler = Rc<dyn Fn(&Route, &mut Window, &mut App)>;
pub type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;
pub type ChooseHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The leading zone: browser-style history over locations, and where you are.
pub struct Leading {
    pub title: SharedString,
    pub can_back: bool,
    pub can_forward: bool,
    pub on_back: CloseHandler,
    pub on_forward: CloseHandler,
}

/// One instance the trailing zone can switch to.
#[derive(Clone)]
pub struct InstanceChoice {
    pub id: String,
    pub name: SharedString,
    pub meta: SharedString,
    pub seed: u32,
    pub loader: crate::cover::Loader,
    pub world: WorldHint,
}

/// The trailing zone: the instance play and installs target, and the others
/// to switch to.
pub struct CurrentInstance {
    pub current: Option<InstanceChoice>,
    pub choices: Vec<InstanceChoice>,
    pub on_choose: ChooseHandler,
    /// With no instances the chip leads to Library instead.
    pub on_empty: CloseHandler,
}

/// One account the right-hand chip can switch to.
#[derive(Clone)]
pub struct AccountChoice {
    /// What choosing this account sends (its key, not its display name).
    pub key: String,
    pub name: SharedString,
    pub detail: SharedString,
    pub selected: bool,
    /// The account's skin face, once it has been loaded.
    pub face: Option<Arc<RenderImage>>,
}

/// The trailing zone's account chip: who plays, and the others to switch to.
pub struct CurrentAccount {
    pub choices: Vec<AccountChoice>,
    pub on_choose: ChooseHandler,
    /// "管理账户…" at the foot of the list.
    pub on_manage: CloseHandler,
    /// With no account the chip offers to add one.
    pub on_add: CloseHandler,
}

const ZONE_RADIUS: gpui::Pixels = px(8.);

/// One floating surface of the bar. It takes the pointer, or clicks and hover
/// would reach the page underneath; the gaps between zones do not.
fn zone(colors: ShellColors) -> gpui::Div {
    h_flex()
        .occlude()
        .min_h(theme::NAV_HEIGHT)
        .items_center()
        .gap(px(4.))
        .p(px(6.))
        .rounded(ZONE_RADIUS)
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface)
        .shadow_lg()
}

pub fn render(
    active: Route,
    activity_count: u32,
    colors: ShellColors,
    on_route: RouteHandler,
    leading: Leading,
    current: Option<CurrentInstance>,
    account: Option<CurrentAccount>,
) -> impl IntoElement {
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(theme::NAV_OFFSET)
        .child(
            theme::content_column().mx_auto().child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .justify_start()
                            .child(leading_zone(leading, colors)),
                    )
                    .child(capsule(active, activity_count, colors, on_route))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .justify_end()
                            .gap_2()
                            .children(account.map(|account| account_zone(account, colors)))
                            .children(current.map(|current| trailing_zone(current, colors))),
                    ),
            ),
        )
}

// ia[navigation]: 点地标 | 中段胶囊按钮 | 打开该页并清空“前进”
// ia[activity]: 导航角标 | 中段「动态」地标上的数字 | 运行中的任务数（超过 99 显示 99+）
fn capsule(
    active: Route,
    activity_count: u32,
    colors: ShellColors,
    on_route: RouteHandler,
) -> impl IntoElement {
    let items = Route::ORDER.into_iter().map(|route| {
        let callback = on_route.clone();
        let is_active = route == active;
        let body = colors.body;
        let button = Key::new(("nav", route.index()))
            .icon(Icon::new(icon_for(route)))
            .tooltip(format!("{}  {}", route.label(), route.shortcut()))
            .white()
            .selected(is_active)
            .on_click(move |_: &ClickEvent, window, cx| callback(&route, window, cx));
        // A landmark is a key with an LED under it; the open one sits pressed.
        let landmark = v_flex()
            .relative()
            .items_center()
            .gap(px(4.))
            .child(button)
            .child(crate::controls::led(is_active, body));

        if route == Route::Activity && activity_count > 0 {
            landmark
                .child(
                    h_flex()
                        .absolute()
                        .top(px(-4.))
                        .right(px(-6.))
                        .items_center()
                        .gap(px(3.))
                        .px(px(4.))
                        .h(px(14.))
                        .rounded(px(2.))
                        .bg(body.panel)
                        .border_1()
                        .border_color(body.hairline)
                        .child(crate::controls::led(true, body))
                        .child(
                            div()
                                .font_family(theme::MONO_FONT)
                                .text_size(px(10.))
                                .line_height(px(10.))
                                .text_color(colors.foreground)
                                .child(ActivityBadge::text(activity_count)),
                        ),
                )
                .into_any_element()
        } else {
            landmark.into_any_element()
        }
    });

    zone(colors)
        .id("global-navigation-capsule")
        .debug_selector(|| "navigation-capsule".into())
        .flex_none()
        .children(items)
}

fn history_button(
    id: &'static str,
    icon: UiIcon,
    tooltip: String,
    enabled: bool,
    on_click: CloseHandler,
) -> impl IntoElement {
    // A direction with nowhere to go stays in place, muted, so the name
    // beside it never jumps (§6).
    Key::new(id)
        .icon(Icon::new(icon))
        .ghost()
        .small()
        .tooltip(tooltip)
        .disabled(!enabled)
        .debug_selector(move || id.into())
        .on_click(move |_: &ClickEvent, window, cx| on_click(window, cx))
}

fn leading_zone(leading: Leading, colors: ShellColors) -> impl IntoElement {
    let title = leading.title.clone();
    zone(colors)
        .id("navigation-leading")
        .debug_selector(|| "navigation-leading".into())
        .max_w_full()
        // ia[navigation]: 后退 / 前进 | 左段的两个图标按钮 | 回到上/下一个位置；位置 = 地标页、游戏页、项目详情；标签/筛选/滚动不进历史
        .child(history_button(
            "navigation-back",
            UiIcon::Back,
            back_tooltip(),
            leading.can_back,
            leading.on_back,
        ))
        .child(history_button(
            "navigation-forward",
            UiIcon::Next,
            forward_tooltip(),
            leading.can_forward,
            leading.on_forward,
        ))
        .child(
            div()
                .id("navigation-title")
                .debug_selector(|| "navigation-title".into())
                .min_w_0()
                .max_w(px(180.))
                .pl(px(4.))
                .pr(px(10.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_sm()
                .font_medium()
                .text_color(colors.foreground)
                .tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(title.clone()).build(window, cx)
                })
                .child(leading.title),
        )
}

fn back_tooltip() -> String {
    if cfg!(target_os = "macos") {
        format!("{}  ⌘[", tr!("nav-back"))
    } else {
        format!("{}  Ctrl+[", tr!("nav-back"))
    }
}

fn forward_tooltip() -> String {
    if cfg!(target_os = "macos") {
        format!("{}  ⌘]", tr!("nav-forward"))
    } else {
        format!("{}  Ctrl+]", tr!("nav-forward"))
    }
}

fn thumb(choice: &InstanceChoice, size: f32, colors: ShellColors) -> impl IntoElement {
    // The same display window a faceplate's cover sits in (design language
    // §12): 3 px corners, no dissolve, on the display colour.
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded(px(3.))
        .overflow_hidden()
        .bg(colors.body.display)
        .child(crate::kit::faceplate_cover(
            choice.seed,
            choice.loader,
            choice.world,
            colors,
        ))
}

fn account_zone(account: CurrentAccount, colors: ShellColors) -> AnyElement {
    let Some(chosen) = account
        .choices
        .iter()
        .find(|choice| choice.selected)
        .cloned()
    else {
        let add = account.on_add;
        return zone(colors)
            .id("navigation-account")
            .debug_selector(|| "navigation-account".into())
            .child(
                // ia[navigation]: 没有账户时 | 芯片显示“添加账户” | 打开账户页的添加离线账户弹窗
                Key::new("navigation-account-empty")
                    .label(tr!("nav-add-account"))
                    .ghost()
                    .small()
                    .tooltip(tr!("nav-add-account-help"))
                    .on_click(move |_: &ClickEvent, window, cx| add(window, cx)),
            )
            .into_any_element();
    };
    let trigger = Key::new("navigation-account-trigger")
        .white()
        .large()
        .tooltip(tr!("nav-current-account", name = chosen.name.to_string()))
        .child(crate::kit::avatar(&chosen.name, chosen.face.clone(), 30.));
    let choices = Rc::new(account.choices);
    let choose = account.on_choose;
    let manage = account.on_manage;
    zone(colors)
        .id("navigation-account")
        .debug_selector(|| "navigation-account".into())
        .child(
            // ia[navigation]: 切换账户 | 右段 Popover 列表：头像、名字、类型；单选 | 之后的启动使用该身份
            // ia[navigation]: 查看当前账户 | 账户下拉底部「账户详情」 | 打开当前账户详情与外观；不沿用账户页上次查看的其他账户
            Popover::new("navigation-account-popover")
                .anchor(Anchor::BottomRight)
                .trigger(trigger)
                .content(move |_, _, cx| {
                    let popover = cx.entity();
                    let manage = manage.clone();
                    let manage_popover = popover.clone();
                    v_flex()
                        .w(px(260.))
                        .max_h(px(360.))
                        .gap(px(2.))
                        .child(
                            div()
                                .px_2()
                                .pb_1()
                                .text_xs()
                                .font_semibold()
                                .text_color(colors.muted)
                                .child(tr!("nav-switch-account")),
                        )
                        .children(choices.iter().enumerate().map(|(index, choice)| {
                            let choose = choose.clone();
                            let popover = popover.clone();
                            let key = choice.key.clone();
                            h_flex()
                                .id(("navigation-account-choice", index))
                                .debug_selector(move || {
                                    format!("navigation-account-choice-{index}")
                                })
                                .gap_3()
                                .items_center()
                                .px_2()
                                .py(px(6.))
                                .rounded(px(8.))
                                .cursor_pointer()
                                .when(choice.selected, |row| row.bg(colors.surface_subtle))
                                .hover(|row| row.bg(colors.surface_subtle))
                                .on_click(move |_, window, cx| {
                                    choose(&key, window, cx);
                                    popover.update(cx, |state, cx| state.dismiss(window, cx));
                                })
                                .child(crate::kit::avatar(&choice.name, choice.face.clone(), 28.))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_medium()
                                                .text_color(colors.foreground)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .child(choice.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(colors.muted)
                                                .child(choice.detail.clone()),
                                        ),
                                )
                                .when(choice.selected, |row| {
                                    row.child(div().size(px(6.)).rounded_full().bg(colors.primary))
                                })
                        }))
                        .child(
                            div()
                                .id("navigation-account-manage")
                                .debug_selector(|| "navigation-account-manage".into())
                                .mt_1()
                                .px_2()
                                .py(px(8.))
                                .rounded(px(8.))
                                .border_t_1()
                                .border_color(colors.border.opacity(0.6))
                                .text_sm()
                                .text_color(colors.muted)
                                .cursor_pointer()
                                .hover(|row| row.bg(colors.surface_subtle))
                                .on_click(move |_, window, cx| {
                                    manage(window, cx);
                                    manage_popover
                                        .update(cx, |state, cx| state.dismiss(window, cx));
                                })
                                .child(tr!("nav-manage-accounts")),
                        )
                }),
        )
        .into_any_element()
}

fn trailing_zone(current: CurrentInstance, colors: ShellColors) -> AnyElement {
    let Some(chosen) = current.current.clone() else {
        let open = current.on_empty;
        return zone(colors)
            .id("navigation-instance")
            .debug_selector(|| "navigation-instance".into())
            .child(
                // ia[navigation]: 没有游戏时点芯片 | 右段文字按钮“还没有游戏” | 打开游戏库
                Key::new("navigation-instance-empty")
                    .label(tr!("nav-no-game"))
                    .ghost()
                    .small()
                    .tooltip(tr!("nav-no-game-help"))
                    .on_click(move |_: &ClickEvent, window, cx| open(window, cx)),
            )
            .into_any_element();
    };

    let trigger = {
        Key::new("navigation-instance-trigger")
            .white()
            .large()
            .tooltip(tr!("nav-current-game"))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(thumb(&chosen, 28., colors))
                    .child(
                        div()
                            .max_w(px(140.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_sm()
                            .font_medium()
                            .child(chosen.name.clone()),
                    )
                    .child(
                        Icon::new(UiIcon::Switch)
                            .size(px(14.))
                            .text_color(colors.body.on_white.opacity(0.6)),
                    ),
            )
    };

    let choices = Rc::new(current.choices);
    let choose = current.on_choose;
    let chosen_id = chosen.id.clone();
    zone(colors)
        .id("navigation-instance")
        .debug_selector(|| "navigation-instance".into())
        .child(
            // ia[navigation]: 切换当前游戏 | 右段 Popover 列表：封面、名称、版本 · 加载器；当前项高亮；游戏多时顶部出现搜索 | 之后的“开始游戏”和发现页安装都指向它；已开始的操作不受影响；保存在 settings.json，重启后保持，被删除时回落到剩余第一个
            Popover::new("navigation-instance-popover")
                .anchor(Anchor::BottomRight)
                .trigger(trigger)
                .content(move |_, _, cx| {
                    let popover = cx.entity();
                    v_flex()
                        .w(px(280.))
                        .max_h(px(360.))
                        .gap(px(2.))
                        .child(
                            div()
                                .px_2()
                                .pb_1()
                                .text_xs()
                                .font_semibold()
                                .text_color(colors.muted)
                                .child(tr!("nav-switch-game")),
                        )
                        .children(choices.iter().enumerate().map(|(index, choice)| {
                            let selected = choice.id == chosen_id;
                            let choose = choose.clone();
                            let popover = popover.clone();
                            let id = choice.id.clone();
                            h_flex()
                                .id(("navigation-instance-choice", index))
                                .debug_selector(move || {
                                    format!("navigation-instance-choice-{index}")
                                })
                                .gap_3()
                                .items_center()
                                .px_2()
                                .py(px(6.))
                                .rounded(px(8.))
                                .cursor_pointer()
                                .when(selected, |row| row.bg(colors.surface_subtle))
                                .hover(|row| row.bg(colors.surface_subtle))
                                .on_click(move |_, window, cx| {
                                    choose(&id, window, cx);
                                    popover.update(cx, |state, cx| state.dismiss(window, cx));
                                })
                                .child(thumb(choice, 32., colors))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_medium()
                                                .text_color(colors.foreground)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .child(choice.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(colors.muted)
                                                .child(choice.meta.clone()),
                                        ),
                                )
                                .when(selected, |row| {
                                    row.child(div().size(px(6.)).rounded_full().bg(colors.primary))
                                })
                        }))
                }),
        )
        .into_any_element()
}

fn icon_for(route: Route) -> LandmarkIcon {
    match route {
        Route::Home => LandmarkIcon::Home,
        Route::Library => LandmarkIcon::Library,
        Route::Discover => LandmarkIcon::Discover,
        Route::Activity => LandmarkIcon::Activity,
        Route::Accounts => LandmarkIcon::Accounts,
        Route::Settings => LandmarkIcon::Settings,
    }
}

struct ActivityBadge;

impl ActivityBadge {
    fn text(count: u32) -> String {
        match count {
            0 => String::new(),
            1..=99 => count.to_string(),
            _ => "99+".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::Route;
    use gpui::{Modifiers, Render, TestAppContext, point};
    use std::cell::Cell;

    struct Page {
        clicks: Rc<Cell<u32>>,
    }

    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            div()
                .size_full()
                .relative()
                .child(
                    div()
                        .id("under")
                        .size_full()
                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
                )
                .child(render(
                    Route::Home,
                    0,
                    ShellColors::from_theme(&gpui_component::Theme::default()),
                    Rc::new(|_, _, _| {}),
                    Leading {
                        title: "首页".into(),
                        can_back: false,
                        can_forward: false,
                        on_back: Rc::new(|_, _| {}),
                        on_forward: Rc::new(|_, _| {}),
                    },
                    None,
                    None,
                ))
        }
    }

    #[gpui::test]
    fn the_capsule_does_not_pass_clicks_to_the_page_underneath(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let clicks: Rc<Cell<u32>> = Rc::default();
        let seen = clicks.clone();
        let (_view, cx) = cx.add_window_view(|_, _| Page { clicks: seen });
        cx.run_until_parked();
        let capsule = cx.debug_bounds("navigation-capsule").expect("capsule");
        // On the capsule's own padding, between its edge and the first button.
        cx.simulate_click(
            capsule.origin + point(px(3.), capsule.size.height / 2.),
            Modifiers::none(),
        );
        assert_eq!(clicks.get(), 0, "the page underneath was clicked");
        cx.simulate_click(point(px(5.), px(5.)), Modifiers::none());
        assert_eq!(clicks.get(), 1, "the test page itself is clickable");
    }

    #[test]
    fn activity_badge_is_bounded() {
        assert_eq!(ActivityBadge::text(0), "");
        assert_eq!(ActivityBadge::text(7), "7");
        assert_eq!(ActivityBadge::text(99), "99");
        assert_eq!(ActivityBadge::text(100), "99+");
    }
}
