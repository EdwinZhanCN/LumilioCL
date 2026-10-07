//! Switching the language is process-wide, so it is tested in its own test
//! binary, away from the unit tests that read Chinese.

use lumilio_ui::i18n::{Locale, locale, set_locale};
use lumilio_ui::pages::settings::{list_text, memory_text, tabs};

#[test]
fn switching_language_changes_the_launcher_and_component_words() {
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
