use std::cell::RefCell;

use super::*;

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

#[gpui::test]
fn the_dialog_walks_from_intro_to_code_to_success_and_cancels_what_it_started(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::Modifiers;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen: Rc<RefCell<Vec<SignInIntent>>> = Rc::default();
    let sink = seen.clone();
    let form = cx.update(|_, cx| {
        cx.new(|_| SignInDialog::new(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent))))
    });
    cx.update(|window, cx| SignInDialog::open(form.clone(), window, cx));
    cx.run_until_parked();
    let click = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is not on screen"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    };

    // Nothing is asked until the person says so; pressing twice asks once.
    assert!(seen.borrow().is_empty());
    click(cx, "signin-start");
    click(cx, "signin-start");
    assert_eq!(seen.borrow().as_slice(), [SignInIntent::Start]);

    // The code shows with its helpers, and the dialog can only be left by cancelling.
    cx.update(|_, cx| crate::toast::copy_text("previous clipboard text", cx));
    assert!(form.update(cx, |form, cx| {
        form.code_arrived("AB12CD", "https://www.microsoft.com/link", cx)
    }));
    assert_eq!(
        cx.read(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("AB12CD".to_owned()),
    );
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    for selector in ["signin-code", "signin-copy", "signin-open"] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector}");
    }

    // A failure says why and offers another try.
    form.update(cx, |form, cx| {
        form.failed(
            ("代码已经过期，请重新开始登录".into(), "expired".into()),
            cx,
        )
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("signin-error").is_some());
    click(cx, "signin-start");
    assert_eq!(
        seen.borrow().as_slice(),
        [SignInIntent::Start, SignInIntent::Start]
    );

    // Cancelling while it waits tells the application and closes the dialog.
    click(cx, "signin-cancel");
    assert_eq!(seen.borrow().last(), Some(&SignInIntent::Cancel));
    assert!(!form.update(cx, |form, cx| {
        form.code_arrived("TOO-LATE", "https://www.microsoft.com/link", cx)
    }));
    assert_eq!(
        cx.read(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("AB12CD".to_owned()),
    );
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}

#[gpui::test]
fn a_finished_sign_in_closes_the_dialog_and_a_late_code_after_a_failure_is_ignored(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let form = cx.update(|_, cx| cx.new(|_| SignInDialog::new(Rc::new(|_, _, _| {}))));
    cx.update(|window, cx| SignInDialog::open(form.clone(), window, cx));
    cx.run_until_parked();
    cx.update(|_, cx| crate::toast::copy_text("keep this", cx));
    form.update(cx, |form, cx| form.failed(("x".into(), "y".into()), cx));
    assert!(!form.update(cx, |form, cx| {
        form.code_arrived("LATE", "https://x", cx)
    }));
    assert_eq!(
        cx.read(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("keep this".to_owned()),
    );
    form.read_with(cx, |form, _| {
        assert!(
            matches!(form.phase, Phase::Failed(_)),
            "a stale code changes nothing"
        );
    });
    form.update(cx, |form, cx| form.signed_in(cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}
