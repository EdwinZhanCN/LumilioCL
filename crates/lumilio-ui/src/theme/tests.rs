//! Tests for the theme tokens and the look in force.

use gpui::{Hsla, Rgba, TestAppContext};
use gpui_component::{Theme, ThemeMode};

use super::spec::Resolved;
use super::{
    Body, Catalog, Choice, Color, Entry, Origin, Palette, ShellColors, ThemeSpec, Tone, parse_file,
    scan_dir, tune,
};

fn luminance(color: Hsla) -> f32 {
    let rgba = Rgba::from(color);
    let channel = |value: f32| {
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgba.r) + 0.7152 * channel(rgba.g) + 0.0722 * channel(rgba.b)
}

/// WCAG 2 contrast ratio.
fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn bodies() -> [(&'static str, Body); 2] {
    [("aluminium", Body::of(false)), ("night", Body::of(true))]
}

#[test]
fn text_tokens_reach_4_5_to_1_on_what_they_sit_on() {
    for (name, b) in bodies() {
        let pairs = [
            ("ink on page", b.ink, b.page),
            ("ink on panel", b.ink, b.panel),
            ("muted on page", b.muted, b.page),
            ("muted on panel", b.muted, b.panel),
            ("on white key", b.on_white, b.key_white),
            ("on black key", b.on_black, b.key_black),
            ("ink on key grey (plain tag, port tab)", b.ink, b.key_grey),
            ("orange text on page", b.orange_text, b.page),
            ("orange text on panel", b.orange_text, b.panel),
            ("danger text on page", b.danger_text, b.page),
            ("danger text on panel", b.danger_text, b.panel),
            ("white on danger key", b.on_orange, b.danger),
            ("display ink on display", b.display_ink, b.display),
            ("display label on display", b.display_label, b.display),
            ("page on ink tag", b.page, b.ink),
        ];
        for (what, fg, bg) in pairs {
            let ratio = contrast(fg, bg);
            assert!(ratio >= 4.5, "{name}: {what} is {ratio:.2}:1");
        }
    }
}

/// The accepted exceptions (design language §9), listed so that they can
/// neither widen nor sink further unnoticed.
#[test]
fn white_on_orange_stays_within_the_accepted_exception() {
    let (aluminium, night) = (Body::of(false), Body::of(true));
    let light = contrast(aluminium.on_orange, aluminium.orange);
    let dark = contrast(night.on_orange, night.orange);
    assert!((3.4..3.7).contains(&light), "aluminium: {light:.2}:1");
    assert!((3.0..3.3).contains(&dark), "night: {dark:.2}:1");
}

#[test]
fn marks_that_stand_alone_reach_3_to_1() {
    for (name, b) in bodies() {
        // A lit LED is never the only sign of state (§9), which is why
        // orange on aluminium may sit at 2.8:1 on the page; held to 3:1
        // everywhere else. Keys are identified by their labels, which the
        // text test covers.
        let led = contrast(b.orange, b.page);
        let floor = if b.dark { 3. } else { 2.7 };
        assert!(led >= floor, "{name}: lit LED on page is {led:.2}:1");
        let on_panel = contrast(b.orange, b.panel);
        assert!(on_panel >= 3., "{name}: orange on panel is {on_panel:.2}:1");
        let led_off = contrast(b.led_off, b.page);
        assert!(
            led_off >= 1.5,
            "{name}: an unlit LED still shows ({led_off:.2}:1)"
        );
    }
}

#[gpui::test]
fn tune_maps_the_body_onto_the_component_theme_in_both_appearances(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        cx.update(|cx| {
            Theme::change(mode, None, cx);
            tune(cx);
            let theme = Theme::global(cx);
            let body = Body::of(mode.is_dark());
            assert_eq!(theme.primary, body.orange, "{mode:?}");
            assert_eq!(theme.background, body.page, "{mode:?}");
            assert_eq!(theme.foreground, body.ink, "{mode:?}");
            assert_eq!(theme.border, body.hairline, "{mode:?}");
            assert_eq!(theme.font_family.as_ref(), super::SANS_FONT);
            assert_eq!(theme.mono_font_family.as_ref(), super::MONO_FONT);
            let colors = ShellColors::current(cx);
            assert_eq!(colors.primary, theme.primary);
            assert_eq!(colors.background, theme.background);
        });
    }
}

fn hexes(body: Body) -> String {
    serde_json::to_string(&Palette::of(body)).unwrap()
}

fn one(text: &str) -> ThemeSpec {
    parse_file(text).unwrap().remove(0).unwrap()
}

#[test]
fn colours_may_carry_opacity() {
    let solid = Color::parse("#ff5a1a").unwrap();
    assert_eq!(solid.to_hex(), "#ff5a1a");
    assert!((solid.0.a - 1.).abs() < f32::EPSILON);

    let half = Color::parse("#ff5a1a80").unwrap();
    assert!((half.0.a - 128. / 255.).abs() < 0.005);
    assert_eq!(half.to_hex(), "#ff5a1a80");

    for bad in ["orange", "#12345", "#ggg", "", "#ff5a1a8", "#é5a1a"] {
        assert!(Color::parse(bad).is_err(), "{bad:?} should not parse");
    }
}

#[test]
fn the_builtin_themes_travel_through_the_theme_file_unchanged() {
    for dark in [false, true] {
        let spec = ThemeSpec::of_body("Built-in", Body::of(dark));
        let file = serde_json::json!({ "themes": [spec] }).to_string();
        let back = Resolved::new(&one(&file));
        assert!(back.missing.is_empty(), "{:?}", back.missing);
        assert_eq!(hexes(back.body), hexes(Body::of(dark)));
        assert_eq!(back.tone.is_dark(), dark);
    }
}

#[test]
fn a_theme_without_some_keys_is_filled_from_the_builtin_body() {
    let spec =
        one(r##"{"themes":[{"name":"Sparse","appearance":"dark","colors":{"page":"#102030"}}]}"##);
    let resolved = Resolved::new(&spec);
    let night = Body::of(true);
    assert_eq!(Color(resolved.body.page).to_hex(), "#102030");
    assert_eq!(resolved.body.ink, night.ink);
    assert_eq!(resolved.body.orange, night.orange);
    assert!(resolved.missing.contains(&"ink"));
    assert!(!resolved.missing.contains(&"page"));
    assert_eq!(resolved.missing.len(), 20);
}

#[test]
fn one_bad_theme_does_not_hide_the_others_in_its_file() {
    let file = r##"{"themes":[
        {"name":"Good","appearance":"light"},
        {"name":"Broken","appearance":"light","colors":{"page":"nope"}},
        {"name":"No tone"}
    ]}"##;
    let themes = parse_file(file).unwrap();
    assert!(themes[0].is_ok());
    assert!(themes[1].as_ref().unwrap_err().starts_with("Broken: "));
    assert!(themes[2].as_ref().unwrap_err().starts_with("No tone: "));
    assert!(parse_file("[]").is_err());
    assert!(parse_file("not json").is_err());
}

#[test]
fn radii_stay_inside_what_the_keys_can_take() {
    let spec =
        one(r#"{"themes":[{"name":"Round","appearance":"light","radius":99,"radius_lg":-3}]}"#);
    let resolved = Resolved::new(&spec);
    assert_eq!(resolved.radius, gpui::px(super::spec::MAX_RADIUS));
    assert_eq!(resolved.radius_lg, gpui::px(0.));
}

#[test]
fn the_catalog_lists_by_tone_and_falls_back_to_the_builtin_theme() {
    let local = |name: &str, tone: &str| Entry {
        spec: one(&format!(
            r#"{{"themes":[{{"name":"{name}","appearance":"{tone}"}}]}}"#
        )),
        origin: Origin::Local("x.json".into()),
    };
    let catalog = Catalog::with_local(
        vec![
            local("Mine", "dark"),
            local(super::ALUMINIUM, "light"),
            local("Mine", "dark"),
        ],
        Vec::new(),
    );
    let names = |tone| {
        catalog
            .of_tone(tone)
            .map(|entry| entry.spec.name.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(Tone::Light)[0], super::ALUMINIUM);
    assert_eq!(
        names(Tone::Light)
            .iter()
            .filter(|name| **name == super::ALUMINIUM)
            .count(),
        1
    );
    assert_eq!(names(Tone::Dark)[0], super::NIGHT);
    assert_eq!(names(Tone::Dark).last(), Some(&"Mine"));
    assert_eq!(catalog.resolve(Some("Mine"), Tone::Dark).name, "Mine");
    // A dark theme cannot be chosen for the light slot, and a name that no
    // longer exists falls back to the built-in.
    assert_eq!(
        catalog.resolve(Some("Mine"), Tone::Light).name,
        super::ALUMINIUM
    );
    assert_eq!(catalog.resolve(Some("Gone"), Tone::Dark).name, super::NIGHT);
    assert_eq!(catalog.resolve(None, Tone::Dark).name, super::NIGHT);
}

#[test]
fn scanning_the_themes_directory_keeps_good_themes_and_says_why_a_file_failed() {
    let dir = std::env::temp_dir().join(format!("lumilio-themes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a-good.json"),
        r#"{"themes":[{"name":"Fine","appearance":"dark"}]}"#,
    )
    .unwrap();
    std::fs::write(dir.join("b-broken.json"), "{ nope").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();

    let (entries, rejected) = scan_dir(&dir);
    let missing = scan_dir(&dir.join("absent"));
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].spec.name, "Fine");
    assert_eq!(rejected.len(), 1);
    assert!(rejected[0].file.ends_with("b-broken.json"));
    assert!(!rejected[0].reason.is_empty());
    assert!(missing.0.is_empty() && missing.1.is_empty());
}

#[test]
fn a_theme_may_ask_for_a_transparent_or_blurred_window_and_defaults_to_opaque() {
    use super::WindowBackground::{Blurred, Opaque, Transparent};
    let themed = |value: &str| {
        let text = format!(
            r#"{{"themes":[{{"name":"Glass","appearance":"dark","window_background":"{value}"}}]}}"#
        );
        Resolved::new(&one(&text)).window_background
    };
    assert_eq!(themed("opaque"), Opaque);
    assert_eq!(themed("transparent"), Transparent);
    assert_eq!(themed("blurred"), Blurred);
    let plain = one(r#"{"themes":[{"name":"Plain","appearance":"light"}]}"#);
    assert_eq!(Resolved::new(&plain).window_background, Opaque);
    assert!(
        parse_file(r#"{"themes":[{"name":"X","appearance":"light","window_background":"foggy"}]}"#)
            .unwrap()[0]
            .is_err()
    );
    assert_eq!(Blurred.to_gpui(), gpui::WindowBackgroundAppearance::Blurred);
    assert_eq!(Opaque.to_gpui(), gpui::WindowBackgroundAppearance::Opaque);
}

#[gpui::test]
fn a_translucent_page_colour_and_the_window_material_reach_the_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let glass = Entry {
        spec: one(
            r##"{"themes":[{"name":"Glass","appearance":"dark","window_background":"blurred",
                "colors":{"page":"#10203080"}}]}"##,
        ),
        origin: Origin::Local("glass.json".into()),
    };
    cx.update(|cx| {
        cx.set_global(Catalog::with_local(vec![glass], Vec::new()));
        cx.set_global(Choice(lumilio_core::LookPreferences {
            dark_theme: Some("Glass".into()),
            ..Default::default()
        }));
    });
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, cx| {
        // The person's choice of dark wins over what the test window reports.
        crate::platform::apply_appearance(lumilio_core::Appearance::Dark, window, cx);
        let look = cx.global::<super::Look>();
        assert_eq!(
            look.theme.window_background,
            super::WindowBackground::Blurred
        );
        // The page colour keeps its opacity all the way to the component theme.
        let page = Theme::global(cx).background;
        assert!((page.a - 128. / 255.).abs() < 0.005, "alpha {}", page.a);
    });
}

#[test]
fn pixel_text_sizes_become_rems_so_the_scale_reaches_them() {
    assert_eq!(super::font_px(16.), gpui::rems(1.));
    assert_eq!(super::font_px(10.), gpui::rems(0.625));
}

#[gpui::test]
fn a_chinese_face_that_is_not_installed_is_reported_and_never_used(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(|cx| {
        cx.set_global(Choice(lumilio_core::LookPreferences {
            cjk_font: Some("No Such Chinese Family".into()),
            mono_font: Some("No Such Mono Family".into()),
            ..Default::default()
        }));
        Theme::change(ThemeMode::Light, None, cx);
        tune(cx);
        let look = cx.global::<super::Look>();
        assert!(look.cjk_missing);
        assert_eq!(look.cjk, None);
        assert!(Theme::global(cx).font_fallbacks.is_empty());
        assert_eq!(look.mono, super::MONO_FONT);
        assert_eq!(super::mono_font().as_ref(), super::MONO_FONT);
    });
}

#[gpui::test]
fn tune_applies_the_chosen_theme_per_appearance_and_the_scale(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let local = Entry {
        spec: one(
            r##"{"themes":[{"name":"Mine","appearance":"dark","radius":2,
                "colors":{"page":"#102030","orange":"#ff00ff80"}}]}"##,
        ),
        origin: Origin::Local("mine.json".into()),
    };
    cx.update(|cx| {
        cx.set_global(Catalog::with_local(vec![local], Vec::new()));
        cx.set_global(Choice(lumilio_core::LookPreferences {
            dark_theme: Some("Mine".into()),
            font_scale: Some(125),
            sans_font: Some("No Such Family".into()),
            ..Default::default()
        }));

        Theme::change(ThemeMode::Dark, None, cx);
        tune(cx);
        let theme = Theme::global(cx);
        assert_eq!(Color(theme.background).to_hex(), "#102030");
        assert_eq!(Color(theme.primary).to_hex(), "#ff00ff80");
        assert_eq!(theme.radius, gpui::px(2.));
        assert_eq!(theme.font_size, gpui::px(20.));
        assert_eq!(theme.mono_font_size, gpui::px(13. * 1.25));
        assert_eq!(theme.font_family.as_ref(), super::SANS_FONT);
        assert_eq!(Body::current(cx).page, theme.background);

        // The light slot still shows the built-in theme.
        Theme::change(ThemeMode::Light, None, cx);
        tune(cx);
        assert_eq!(Theme::global(cx).background, Body::of(false).page);

        cx.set_global(Choice(lumilio_core::LookPreferences {
            font_scale: Some(500),
            ..Default::default()
        }));
        tune(cx);
        assert_eq!(Theme::global(cx).font_size, gpui::px(16. * 1.3));
    });
}

#[test]
fn the_bundled_themes_are_complete_attributed_and_pair_light_with_dark() {
    let catalog = Catalog::default();
    let bundled: Vec<_> = catalog
        .entries
        .iter()
        .filter(|entry| ![super::ALUMINIUM, super::NIGHT].contains(&entry.spec.name.as_str()))
        .collect();
    assert!(bundled.len() >= 4, "{} bundled themes", bundled.len());
    for entry in &bundled {
        let resolved = Resolved::new(&entry.spec);
        let name = &entry.spec.name;
        assert!(
            resolved.missing.is_empty(),
            "{name} misses {:?}",
            resolved.missing
        );
        let source = entry.spec.source.as_deref().unwrap_or_default();
        assert!(source.contains("MIT License"), "{name} names its licence");
        assert_eq!(entry.origin, Origin::Builtin);
    }
    for tone in [Tone::Light, Tone::Dark] {
        assert!(bundled.iter().any(|entry| entry.spec.appearance == tone));
    }
}
