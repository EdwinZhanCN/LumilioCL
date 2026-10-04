use gpui::{
    Context, Hsla, IntoElement, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div,
};
use gpui_component::{Theme, ThemeMode};

use super::{Checkbox, Fader, PortTabs, Segments};
use crate::theme::Body;

/// A board that owns the state each control toggles, as a page would.
struct Board {
    fader: bool,
    check: bool,
    segment: usize,
    tab: usize,
}

impl Render for Board {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let (a, b, c, d) = (entity.clone(), entity.clone(), entity.clone(), entity);
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .child(Fader::new("fader", self.fader, "示例", move |_, cx| {
                a.update(cx, |board, cx| {
                    board.fader = !board.fader;
                    cx.notify();
                })
            }))
            .child(
                Checkbox::new("check")
                    .checked(self.check)
                    .label("同意")
                    .on_click(move |_, _, cx| {
                        b.update(cx, |board, cx| {
                            board.check = !board.check;
                            cx.notify();
                        })
                    }),
            )
            .child(Segments::new(
                "seg",
                &["Mod", "资源包", "光影"],
                self.segment,
                move |index, _, cx| {
                    c.update(cx, |board, cx| {
                        board.segment = index;
                        cx.notify();
                    })
                },
            ))
            .child(PortTabs::new(
                "ports",
                &["概览", "内容", "世界"],
                self.tab,
                move |index, _, cx| {
                    d.update(cx, |board, cx| {
                        board.tab = index;
                        cx.notify();
                    })
                },
            ))
    }
}

fn board(
    cx: &mut TestAppContext,
    mode: ThemeMode,
) -> (gpui::Entity<Board>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(mode, None, cx);
        crate::theme::tune(cx);
    });
    let (board, cx) = cx.add_window_view(|_, _| Board {
        fader: false,
        check: false,
        segment: 0,
        tab: 0,
    });
    cx.run_until_parked();
    (board, cx)
}

fn fill_of(cx: &mut VisualTestContext, selector: &'static str) -> Hsla {
    let bounds = cx.debug_bounds(selector).expect("drawn");
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let near = |a: f32, b: gpui::Pixels| (a / scale - f32::from(b)).abs() < 1.5;
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| {
                near(quad.bounds.origin.x.0, bounds.origin.x)
                    && near(quad.bounds.origin.y.0, bounds.origin.y)
                    && near(quad.bounds.size.width.0, bounds.size.width)
                    && near(quad.bounds.size.height.0, bounds.size.height)
            })
            .filter_map(|quad| quad.background.as_solid())
            .find(|color| color.a > 0.)
            .expect("a fill is painted")
    })
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx.debug_bounds(selector).expect("drawn").center();
    cx.simulate_click(center, Modifiers::none());
    cx.run_until_parked();
}

fn left(cx: &mut VisualTestContext, selector: &'static str) -> f32 {
    f32::from(cx.debug_bounds(selector).unwrap().origin.x)
}

fn top(cx: &mut VisualTestContext, selector: &'static str) -> f32 {
    f32::from(cx.debug_bounds(selector).unwrap().origin.y)
}

#[gpui::test]
fn the_fader_lights_its_led_and_moves_its_cap_together(cx: &mut TestAppContext) {
    let body = Body::of(false);
    let (board, cx) = board(cx, ThemeMode::Light);
    let (led_off, cap_off) = (fill_of(cx, "fader-led"), left(cx, "fader-cap"));
    assert_eq!(led_off, body.led_off);

    click(cx, "fader-cap");
    assert!(board.read_with(cx, |board, _| board.fader));
    let (led_on, cap_on) = (fill_of(cx, "fader-led"), left(cx, "fader-cap"));
    assert_eq!(led_on, body.orange, "the LED lights");
    assert!(
        cap_on - cap_off > 20.,
        "the cap travels to the trailing end"
    );
}

#[gpui::test]
fn the_check_key_turns_orange_when_set(cx: &mut TestAppContext) {
    let body = Body::of(false);
    let (board, cx) = board(cx, ThemeMode::Light);
    assert_eq!(fill_of(cx, "check-box"), body.key_white);
    click(cx, "check-box");
    assert!(board.read_with(cx, |board, _| board.check));
    assert_eq!(fill_of(cx, "check-box"), body.orange);
}

#[gpui::test]
fn the_chosen_segment_lights_its_led_and_sits_pressed(cx: &mut TestAppContext) {
    let body = Body::of(false);
    let (board, cx) = board(cx, ThemeMode::Light);
    assert_eq!(fill_of(cx, "seg-led-0"), body.orange);
    assert_eq!(fill_of(cx, "seg-led-1"), body.led_off);
    let (pressed, resting) = (top(cx, "seg-key-0"), top(cx, "seg-key-1"));
    assert!(pressed > resting, "the chosen key sits lower");

    click(cx, "seg-key-1");
    assert_eq!(board.read_with(cx, |board, _| board.segment), 1);
    assert_eq!(fill_of(cx, "seg-led-0"), body.led_off);
    assert_eq!(fill_of(cx, "seg-led-1"), body.orange);
    assert!(top(cx, "seg-key-1") > top(cx, "seg-key-0"));
}

#[gpui::test]
fn the_open_port_tab_is_orange_the_rest_key_grey_and_one_pixel_apart(cx: &mut TestAppContext) {
    let body = Body::of(false);
    let (board, cx) = board(cx, ThemeMode::Light);
    assert_eq!(fill_of(cx, "ports-tab-0"), body.orange);
    assert_eq!(fill_of(cx, "ports-tab-1"), body.key_grey);
    let first = cx.debug_bounds("ports-tab-0").unwrap();
    let second = cx.debug_bounds("ports-tab-1").unwrap();
    assert!(
        (f32::from(second.origin.x) - f32::from(first.origin.x + first.size.width) - 1.).abs()
            < 0.01,
        "tabs are flush with 1 px between them"
    );

    click(cx, "ports-tab-2");
    assert_eq!(board.read_with(cx, |board, _| board.tab), 2);
    assert_eq!(fill_of(cx, "ports-tab-0"), body.key_grey);
    assert_eq!(fill_of(cx, "ports-tab-2"), body.orange);
}

#[gpui::test]
fn night_keeps_the_same_rules(cx: &mut TestAppContext) {
    let body = Body::of(true);
    let (_, cx) = board(cx, ThemeMode::Dark);
    assert_eq!(fill_of(cx, "ports-tab-0"), body.orange);
    assert_eq!(fill_of(cx, "ports-tab-1"), body.key_grey);
    assert_eq!(fill_of(cx, "seg-led-0"), body.orange);
}
