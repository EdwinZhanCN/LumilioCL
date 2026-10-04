use gpui::{Hsla, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::{Theme, ThemeMode};

use super::{Key, KeyKind, face};
use crate::theme::{Body, KEY_TRAVEL};

struct Board {
    disabled: bool,
}

impl gpui::Render for Board {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{InteractiveElement as _, ParentElement as _, Styled as _};
        gpui::div()
            .flex()
            .gap_4()
            .p_4()
            .child(
                Key::new("white")
                    .label("导入")
                    .debug_selector(|| "white".into()),
            )
            .child(
                Key::new("black")
                    .label("更多")
                    .black()
                    .debug_selector(|| "black".into()),
            )
            .child(
                Key::new("orange")
                    .label("新建")
                    .primary()
                    .debug_selector(|| "orange".into()),
            )
            .child(
                Key::new("ghost")
                    .label("编辑")
                    .ghost()
                    .debug_selector(|| "ghost".into()),
            )
            .child(
                Key::new("off")
                    .label("已在运行")
                    .primary()
                    .disabled(self.disabled)
                    .debug_selector(|| "off".into()),
            )
    }
}

fn board(cx: &mut TestAppContext, mode: ThemeMode) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(mode, None, cx);
        crate::theme::tune(cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| Board { disabled: true });
    cx.run_until_parked();
    cx
}

/// The painted quads over a key's bounds, plus the bounds.
fn quads(cx: &mut VisualTestContext, selector: &'static str) -> Vec<gpui::Quad> {
    let bounds = cx.debug_bounds(selector).expect("the key is drawn");
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let near = |a: f32, b: gpui::Pixels| (a / scale - f32::from(b)).abs() < 1.5;
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| {
                near(quad.bounds.origin.x.0, bounds.origin.x)
                    && near(quad.bounds.size.width.0, bounds.size.width)
                    && near(quad.bounds.size.height.0, bounds.size.height)
            })
            .collect()
    })
}

fn face_of(cx: &mut VisualTestContext, selector: &'static str) -> Hsla {
    quads(cx, selector)
        .iter()
        .filter_map(|quad| quad.background.as_solid())
        .find(|color| color.a > 0.)
        .expect("the key paints a face")
}

fn top_of(cx: &mut VisualTestContext, selector: &'static str) -> f32 {
    f32::from(cx.debug_bounds(selector).unwrap().origin.y)
}

#[gpui::test]
fn each_kind_paints_its_face_in_both_bodies(cx: &mut TestAppContext) {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        let body = Body::of(mode.is_dark());
        let cx = board(cx, mode);
        assert_eq!(face_of(cx, "white"), body.key_white, "{mode:?} white");
        assert_eq!(face_of(cx, "black"), body.key_black, "{mode:?} black");
        assert_eq!(face_of(cx, "orange"), body.orange, "{mode:?} orange");
        assert_eq!(face_of(cx, "off"), body.key_grey, "{mode:?} disabled");
    }
}

#[gpui::test]
fn pressing_a_raised_key_moves_it_down_by_the_travel(cx: &mut TestAppContext) {
    let cx = board(cx, ThemeMode::Light);
    let rest = top_of(cx, "orange");
    let center = cx.debug_bounds("orange").unwrap().center();
    cx.simulate_mouse_move(center, None, Modifiers::none());
    cx.simulate_event(gpui::MouseDownEvent {
        position: center,
        modifiers: Modifiers::none(),
        button: gpui::MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    let pressed = top_of(cx, "orange");
    assert!(
        (pressed - rest - f32::from(KEY_TRAVEL)).abs() < 0.01,
        "pressed key travels {} px, moved {}",
        f32::from(KEY_TRAVEL),
        pressed - rest
    );
    cx.simulate_event(gpui::MouseUpEvent {
        position: center,
        modifiers: Modifiers::none(),
        button: gpui::MouseButton::Left,
        click_count: 1,
    });
}

#[gpui::test]
fn a_disabled_key_has_no_shadow_hover_or_travel(cx: &mut TestAppContext) {
    let cx = board(cx, ThemeMode::Light);
    let body = Body::of(false);
    let quads = quads(cx, "off");
    assert!(
        quads
            .iter()
            .all(|quad| quad.background.as_solid() != Some(body.orange)),
        "a disabled primary is not orange"
    );
    let rest = top_of(cx, "off");
    let center = cx.debug_bounds("off").unwrap().center();
    cx.simulate_mouse_move(center, None, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(face_of(cx, "off"), body.key_grey, "no hover shade");
    assert_eq!(top_of(cx, "off"), rest);
    assert!(!face(KeyKind::Orange, true, body).raised);
}

#[gpui::test]
fn hovering_a_key_shifts_its_face_and_a_ghost_takes_key_grey(cx: &mut TestAppContext) {
    let cx = board(cx, ThemeMode::Light);
    let body = Body::of(false);
    let rest = face_of(cx, "white");
    let center = cx.debug_bounds("white").unwrap().center();
    cx.simulate_mouse_move(center, None, Modifiers::none());
    cx.run_until_parked();
    assert_ne!(face_of(cx, "white"), rest, "hover shifts the face");

    let center = cx.debug_bounds("ghost").unwrap().center();
    cx.simulate_mouse_move(center, None, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(face_of(cx, "ghost"), body.key_grey);
}
