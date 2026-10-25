use std::cell::{Cell, RefCell};

use gpui::prelude::*;
use gpui::{Modifiers, Render, ScrollDelta, ScrollWheelEvent, TestAppContext, div, point, px};

use super::bar::{ActivityBadge, FULL_FROM_WIDTH, ZONE_GAP};
use super::*;
use crate::home::WorldHint;
use crate::theme::ShellColors;

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
                window,
                cx,
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
            window,
            cx,
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

/// A library too long for the list, one game in it named apart, and a few
/// accounts.
struct Crowd {
    chosen: Rc<RefCell<Vec<String>>>,
}

const CROWD: usize = 30;
const SKY_BLOCK: usize = 13;

impl Render for Crowd {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let games: Vec<InstanceChoice> = (0..CROWD)
            .map(|index| InstanceChoice {
                id: format!("game-{index}"),
                name: if index == SKY_BLOCK {
                    "Sky Block".into()
                } else {
                    format!("Game {index}").into()
                },
                version: "1.21.1".into(),
                seed: index as u32,
                loader: crate::cover::Loader::Vanilla,
                world: WorldHint::Overworld,
            })
            .collect();
        let players = ["Steve", "Alex", "Notch"]
            .map(|name| AccountChoice {
                key: name.into(),
                name: name.into(),
                detail: "Offline".into(),
                selected: name == "Steve",
                face: None,
            })
            .to_vec();
        let chosen = self.chosen.clone();
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
                on_choose: Rc::new(move |id, _, _| chosen.borrow_mut().push(id.to_owned())),
                on_empty: Rc::new(|_, _| {}),
            }),
        };
        div().size_full().relative().child(render(
            Route::Home,
            0,
            ShellColors::current(cx),
            Rc::new(|_, _, _| {}),
            no_history("首页"),
            trailing,
            window,
            cx,
        ))
    }
}

#[gpui::test]
fn a_long_list_scrolls_in_its_popover_and_a_search_picks_from_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let chosen: Rc<RefCell<Vec<String>>> = Rc::default();
    let seen = chosen.clone();
    let (_view, cx) = cx.add_window_view(|_, _| Crowd { chosen: seen });
    cx.simulate_resize(gpui::size(px(1200.), px(800.)));
    cx.run_until_parked();

    // Three accounts fit: no search, and Escape closes the list.
    let account = cx.debug_bounds("navigation-account").expect("account chip");
    cx.simulate_click(account.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("navigation-account-choice-2").is_some());
    assert!(cx.debug_bounds("navigation-account-search").is_none());
    cx.simulate_keystrokes("escape");
    assert!(cx.debug_bounds("navigation-account-choice-0").is_none());

    // Thirty games do not: the list keeps its height and scrolls.
    let chip = cx.debug_bounds("navigation-instance").expect("game chip");
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let list = cx
        .debug_bounds("navigation-instance-choices")
        .expect("the list opened");
    assert!(
        list.size.height <= choices::LIST_MAX_HEIGHT && list.top() >= px(0.),
        "{list:?}"
    );
    assert!(cx.debug_bounds("navigation-instance-search").is_some());
    let last: &'static str = format!("navigation-instance-choice-{}", CROWD - 1).leak();
    let row = cx.debug_bounds(last).expect("the last game");
    assert!(
        row.top() >= list.bottom(),
        "the last game is below: {row:?}"
    );
    cx.simulate_event(ScrollWheelEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-10_000.))),
        ..ScrollWheelEvent::default()
    });
    cx.run_until_parked();
    let row = cx.debug_bounds(last).expect("the last game");
    assert!(
        row.top() >= list.top() && row.bottom() <= list.bottom() + px(1.),
        "scrolled to the last game: {row:?} in {list:?}"
    );

    // The field has the focus: typing filters, a space included (the
    // popover binds Space to confirm), and Enter takes the one left.
    cx.simulate_keystrokes("s k y space b");
    let sky_block: &'static str = format!("navigation-instance-choice-{SKY_BLOCK}").leak();
    assert!(cx.debug_bounds(sky_block).is_some(), "Sky Block is found");
    assert!(
        cx.debug_bounds("navigation-instance-choice-0").is_none(),
        "the others are not"
    );
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(chosen.borrow().as_slice(), [format!("game-{SKY_BLOCK}")]);
    assert!(
        cx.debug_bounds("navigation-instance-choices").is_none(),
        "choosing closes the list"
    );

    // Opened again, the search is empty and the list starts at the top.
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let first = cx
        .debug_bounds("navigation-instance-choice-0")
        .expect("every game again");
    assert_eq!(first.top(), list.top(), "back at the top");
    cx.simulate_keystrokes("x y z");
    assert!(cx.debug_bounds("navigation-instance-choice-0").is_none());
    assert!(
        cx.debug_bounds("navigation-instance-no-match").is_some(),
        "a search that keeps nothing says so"
    );
}

#[test]
fn a_search_keeps_the_choices_holding_every_word() {
    let words = [
        "Sky Block 1.21.1 Fabric",
        "Survival 26.2 原版",
        "SkyFactory 1.12.2 Forge",
    ]
    .map(String::from);
    assert_eq!(choices::matching("", &words), [0, 1, 2]);
    assert_eq!(choices::matching("sky", &words), [0, 2]);
    assert_eq!(choices::matching(" SKY  fabric ", &words), [0]);
    assert_eq!(choices::matching("1.12", &words), [2]);
    assert_eq!(choices::matching("原版", &words), [1]);
    assert!(choices::matching("quilt", &words).is_empty());
}

#[test]
fn activity_badge_is_bounded() {
    assert_eq!(ActivityBadge::text(0), "");
    assert_eq!(ActivityBadge::text(7), "7");
    assert_eq!(ActivityBadge::text(99), "99");
    assert_eq!(ActivityBadge::text(100), "99+");
}
