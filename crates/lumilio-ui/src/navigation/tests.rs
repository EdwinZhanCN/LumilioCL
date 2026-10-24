use std::cell::Cell;

use gpui::{Modifiers, Render, TestAppContext, point};

use super::*;
use crate::route::Route;

fn no_history(title: &str) -> Leading {
    Leading {
        title: title.to_owned().into(),
        can_back: false,
        can_forward: false,
        on_back: Rc::new(|_, _| {}),
        on_forward: Rc::new(|_, _| {}),
        update: None,
    }
}

struct Page {
    clicks: Rc<Cell<u32>>,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
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
                ShellColors::current(cx),
                Rc::new(|_, _, _| {}),
                no_history("首页"),
                Trailing {
                    account: None,
                    instance: None,
                },
                f32::from(window.viewport_size().width),
            ))
    }
}

/// The bar alone: an account and a game with a long name, or neither.
struct Bar {
    empty: bool,
}

const LONG_NAME: &str = "Ed's Vanilla 26.2 with Friends and Far Too Many Words";

impl Render for Bar {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let game = InstanceChoice {
            id: "a".into(),
            name: LONG_NAME.into(),
            version: "26.2".into(),
            seed: 1,
            loader: crate::cover::Loader::Vanilla,
            world: WorldHint::Overworld,
        };
        let player = AccountChoice {
            key: "a".into(),
            name: "Steve".into(),
            detail: "Offline".into(),
            selected: true,
            face: None,
        };
        let (games, players) = if self.empty {
            (Vec::new(), Vec::new())
        } else {
            (vec![game], vec![player])
        };
        let trailing = Trailing {
            account: Some(CurrentAccount {
                choices: players,
                on_choose: Rc::new(|_, _, _| {}),
                on_manage: Rc::new(|_, _| {}),
                on_add: Rc::new(|_, _| {}),
            }),
            instance: Some(CurrentInstance {
                current: games.first().cloned(),
                choices: games,
                on_choose: Rc::new(|_, _, _| {}),
                on_empty: Rc::new(|_, _| {}),
            }),
        };
        div().size_full().relative().child(render(
            Route::Library,
            0,
            ShellColors::current(cx),
            Rc::new(|_, _, _| {}),
            // On an instance's page the location is the instance's name.
            no_history(LONG_NAME),
            trailing,
            f32::from(window.viewport_size().width),
        ))
    }
}

#[gpui::test]
fn the_zones_keep_their_gaps_and_a_narrow_window_shows_only_pictures(cx: &mut TestAppContext) {
    use gpui::size;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    for empty in [false, true] {
        let (_view, cx) = cx.add_window_view(|_, _| Bar { empty });
        for (width, full) in [
            (720., false),
            (FULL_FROM_WIDTH - 1., false),
            (FULL_FROM_WIDTH, true),
            (1440., true),
        ] {
            cx.simulate_resize(size(px(width), px(480.)));
            cx.run_until_parked();
            let case = format!("{width} px wide, empty: {empty}");
            let leading = cx.debug_bounds("navigation-leading").expect("leading");
            let capsule = cx.debug_bounds("navigation-capsule").expect("capsule");
            let account = cx.debug_bounds("navigation-account").expect("account");
            let instance = cx.debug_bounds("navigation-instance").expect("instance");
            // Each zone stays in its own place: the full gap either side
            // of the capsule, nothing pushed past the window's edge.
            assert!(
                capsule.left() - leading.right() >= ZONE_GAP
                    && account.left() - capsule.right() >= ZONE_GAP
                    && account.right() <= instance.left()
                    && instance.right() <= px(width),
                "{case}: {leading:?} {capsule:?} {account:?} {instance:?}"
            );
            let centre = f32::from(capsule.center().x);
            assert!(
                (centre - width / 2.).abs() < 1.,
                "{case}: the capsule stays centred: {capsule:?}"
            );
            if !empty {
                assert_eq!(
                    cx.debug_bounds("navigation-instance-name").is_some(),
                    full,
                    "{case}: the name shows only when the window is wide enough"
                );
            }
        }
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
