//! Switching the language is process-wide, so it is tested in its own test
//! binary, away from the unit tests that read Chinese. The tests here take
//! turns through `LANGUAGE`.

use std::sync::{Mutex, MutexGuard, PoisonError};

use gpui::{Context, Entity, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use gpui_component::IndexPath;
use lumilio_ui::i18n::{Locale, generation, locale, set_locale};
use lumilio_ui::pages::live::LiveControls;
use lumilio_ui::pages::settings::{list_text, memory_text, tabs};

static LANGUAGE: Mutex<()> = Mutex::new(());

fn language_turn() -> MutexGuard<'static, ()> {
    LANGUAGE.lock().unwrap_or_else(PoisonError::into_inner)
}

#[test]
fn switching_language_changes_the_launcher_and_component_words() {
    let _turn = language_turn();
    set_locale(Locale::English);
    assert_eq!(locale(), Locale::English);
    assert_eq!(tabs()[0], "General");
    assert_eq!(memory_text(None), "Not set");
    assert_eq!(
        list_text(&["-Da".into(), "-Db".into(), "-Dc".into()]),
        "3 items: -Da, …"
    );
    assert_eq!(&*gpui_component::locale(), "en");

    set_locale(Locale::SimplifiedChinese);
    assert_eq!(tabs()[0], "通用");
    assert_eq!(memory_text(None), "未设置");
    assert_eq!(&*gpui_component::locale(), "zh-CN");
}

struct Host(Entity<LiveControls>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
fn a_language_switch_relabels_controls_and_keeps_the_choice(cx: &mut TestAppContext) {
    let _turn = language_turn();
    set_locale(Locale::SimplifiedChinese);
    cx.update(gpui_component::init);
    let (host, cx) =
        cx.add_window_view(|window, cx| Host(cx.new(|cx| LiveControls::new(window, cx))));
    host.update_in(cx, |host, window, cx| {
        host.0.update(cx, |controls, cx| {
            controls.library_sort.update(cx, |sort, cx| {
                sort.set_selected_index(Some(IndexPath::new(1)), window, cx)
            });
        });
    });

    set_locale(Locale::English);
    host.read_with(cx, |host, cx| {
        assert_ne!(
            host.0.read(cx).language,
            generation(),
            "the switch is noticed"
        );
    });
    host.update_in(cx, |host, window, cx| {
        host.0
            .update(cx, |controls, cx| controls.relabel(window, cx));
    });
    host.read_with(cx, |host, cx| {
        let controls = host.0.read(cx);
        assert_eq!(controls.language, generation());
        assert_eq!(
            controls
                .library_filter
                .read(cx)
                .presentation()
                .placeholder()
                .as_ref(),
            "Search games"
        );
        let sort = controls.library_sort.read(cx);
        assert_eq!(sort.selected_index(cx), Some(IndexPath::new(1)));
        assert_eq!(sort.selected_value().map(String::as_str), Some("Name"));
    });
    set_locale(Locale::SimplifiedChinese);
}
