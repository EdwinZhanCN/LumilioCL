//! Acceptance of plan terms-consent (T2): the terms are agreed to before the
//! launcher is used, and the release notes of an update wait for them.

use super::super::terms::ask as ask_terms;
use super::global_context::{ask, open, until};
use gpui_kit::component::WindowExt as _;
use gpui_kit::{Modifiers, TestAppContext, VisualTestContext};

fn on_screen(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.debug_bounds(selector).is_some()
}

#[gpui_kit::test]
fn the_terms_are_asked_once_and_the_update_notes_wait_for_them(cx: &mut TestAppContext) {
    let (_shell, wiring, cx) = open(cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    // A start after an update: its notes must not cover the question.
    ask(&wiring, |service| async move {
        service.set_release_notes_seen("0.0.1").await
    })
    .unwrap();

    cx.update(|window, cx| ask_terms(&wiring, window.window_handle(), cx));
    until(cx, "the terms", |cx| on_screen(cx, "terms-agree"));
    assert!(!on_screen(cx, "release-notes-body"), "the notes wait");

    let agree = cx.debug_bounds("terms-agree").unwrap();
    cx.simulate_click(agree.center(), Modifiers::none());
    until(cx, "the update notes", |cx| {
        on_screen(cx, "release-notes-body")
    });
    assert!(!on_screen(cx, "terms-agree"));
    let accepted = ask(&wiring, |service| async move {
        service.settings().await.terms_accepted
    });
    assert_eq!(accepted, Some(lumilio_core::TERMS_VERSION));
    cx.update(|window, cx| window.close_dialog(cx));
    cx.run_until_parked();

    // Agreed: the next start asks nothing.
    cx.update(|window, cx| ask_terms(&wiring, window.window_handle(), cx));
    cx.run_until_parked();
    assert!(!on_screen(cx, "terms-agree"));
    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
}
