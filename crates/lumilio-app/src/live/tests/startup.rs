//! What start-up says in words (the terms, the recovery toast) waits for the
//! saved language; before, the recovery toast was written in the system's.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::TestAppContext;
use lumilio_core::Language;
use lumilio_ui::i18n::{self, Locale};

use super::super::settings::apply_saved_preferences;
use super::global_context::{ask, open, until};

#[gpui_kit::test]
fn startup_words_wait_for_the_saved_language(cx: &mut TestAppContext) {
    let (_shell, wiring, cx) = open(cx);
    // Saved: the language the system does not speak, so words written
    // before the preferences are applied would be in the wrong one.
    cx.update(|_, cx| i18n::apply_language(Language::System, cx));
    let saved = match i18n::locale() {
        Locale::English => Language::SimplifiedChinese,
        Locale::SimplifiedChinese => Language::English,
    };
    ask(&wiring, move |service| async move {
        let mut preferences = service.settings().await.preferences;
        preferences.language = saved;
        service.set_preferences(preferences).await
    })
    .unwrap();

    let spoken: Rc<Cell<Option<Locale>>> = Rc::default();
    let heard = spoken.clone();
    cx.update(|window, cx| {
        let speak = move |_: &mut gpui_kit::App| heard.set(Some(i18n::locale()));
        apply_saved_preferences(&wiring, window.window_handle(), speak, cx);
    });
    until(cx, "start-up to speak", |_| spoken.get().is_some());
    assert_eq!(spoken.get(), Some(Locale::of(saved)));
}
