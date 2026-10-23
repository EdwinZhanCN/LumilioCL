use super::*;
use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext, size};
use std::cell::Cell;

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn window(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx
}

fn redraw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn before_first_use_only_agreeing_or_quitting_closes_the_terms(cx: &mut TestAppContext) {
    let cx = window(cx);
    let (agreed, quit) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let occasion = {
        let (agreed, quit) = (agreed.clone(), quit.clone());
        Occasion::Agree {
            agree: Rc::new(move |_, _| agreed.set(agreed.get() + 1)),
            quit: Rc::new(move |_, _| quit.set(quit.get() + 1)),
        }
    };
    cx.update(|window, cx| show(occasion, window, cx));
    redraw(cx);

    // Everything to decide with is on screen in the smallest window; the
    // body scrolls and the keys stay inside it.
    let window = size(px(720.), px(480.));
    for selector in ["terms-body", "terms-agree-note", "terms-eula", "terms-quit"] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector}");
    }
    let agree = cx.debug_bounds("terms-agree").expect("agree key");
    assert!(
        agree.bottom() <= window.height && agree.right() <= window.width,
        "{agree:?}"
    );

    // Esc does not stand for an answer.
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(cx.debug_bounds("dialog-layer").is_some(), "still open");

    let quit_key = cx.debug_bounds("terms-quit").unwrap();
    cx.simulate_click(quit_key.center(), Modifiers::none());
    redraw(cx);
    assert_eq!((agreed.get(), quit.get()), (0, 1));

    let agree = cx.debug_bounds("terms-agree").unwrap();
    cx.simulate_click(agree.center(), Modifiers::none());
    redraw(cx);
    assert_eq!(agreed.get(), 1);
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}

#[gpui::test]
fn read_again_the_terms_ask_nothing_and_close_as_any_dialog(cx: &mut TestAppContext) {
    let cx = window(cx);
    cx.update(|window, cx| show(Occasion::Read, window, cx));
    redraw(cx);
    assert!(cx.debug_bounds("terms-body").is_some());
    assert!(cx.debug_bounds("terms-eula").is_some());
    for asked in ["terms-agree-note", "terms-agree", "terms-quit"] {
        assert!(cx.debug_bounds(asked).is_none(), "{asked}");
    }
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(cx.debug_bounds("dialog-layer").is_none(), "Esc closes it");

    cx.update(|window, cx| show(Occasion::Read, window, cx));
    redraw(cx);
    let done = cx.debug_bounds("terms-done").unwrap();
    cx.simulate_click(done.center(), Modifiers::none());
    redraw(cx);
    assert!(
        cx.debug_bounds("dialog-layer").is_none(),
        "its key closes it"
    );
}
