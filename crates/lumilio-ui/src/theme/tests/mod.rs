//! Tests for the theme tokens and the look in force.

use gpui::TestAppContext;
use gpui_component::{Theme, ThemeMode};

use super::spec::Resolved;
use super::{
    Body, Catalog, Choice, Color, Entry, Origin, Palette, ShellColors, ThemeSpec, Tone, contrast,
    legible, parse_file, scan_dir, tune,
};

fn bodies() -> [(&'static str, Body); 2] {
    [("aluminium", Body::of(false)), ("night", Body::of(true))]
}

/// One colour that must stand out from another by a WCAG 2 contrast ratio,
/// on a light and on a dark body (design language §9). The website's theme
/// studio checks the same rules (`website.rs` exports them).
#[derive(serde::Serialize)]
pub(super) struct Rule {
    id: &'static str,
    fg: &'static str,
    bg: &'static str,
    light: f32,
    dark: f32,
}

const fn rule(id: &'static str, fg: &'static str, bg: &'static str, light: f32, dark: f32) -> Rule {
    Rule {
        id,
        fg,
        bg,
        light,
        dark,
    }
}

pub(super) const RULES: &[Rule] = &[
    // Text reaches 4.5:1 on what it sits on.
    rule("ink-page", "ink", "page", 4.5, 4.5),
    rule("ink-panel", "ink", "panel", 4.5, 4.5),
    rule("muted-page", "muted", "page", 4.5, 4.5),
    rule("muted-panel", "muted", "panel", 4.5, 4.5),
    rule("on-white", "on_white", "key_white", 4.5, 4.5),
    rule("on-black", "on_black", "key_black", 4.5, 4.5),
    // Plain tags and port tabs.
    rule("ink-key-grey", "ink", "key_grey", 4.5, 4.5),
    rule("orange-text-page", "orange_text", "page", 4.5, 4.5),
    rule("orange-text-panel", "orange_text", "panel", 4.5, 4.5),
    rule("danger-text-page", "danger_text", "page", 4.5, 4.5),
    rule("danger-text-panel", "danger_text", "panel", 4.5, 4.5),
    rule("on-danger", "on_orange", "danger", 4.5, 4.5),
    rule("display-ink", "display_ink", "display", 4.5, 4.5),
    rule("display-label", "display_label", "display", 4.5, 4.5),
    // An ink tag.
    rule("page-ink", "page", "ink", 4.5, 4.5),
    // White on orange is an accepted exception (§9).
    rule("on-orange", "on_orange", "orange", 3., 3.),
    // A lit LED is never the only sign of state, which is why orange on
    // aluminium may sit at 2.8:1 on the page; held to 3:1 everywhere else.
    // Keys are identified by their labels, which the text rules cover.
    rule("orange-page", "orange", "page", 2.7, 3.),
    rule("orange-panel", "orange", "panel", 3., 3.),
    // An unlit LED still shows.
    rule("led-off-page", "led_off", "page", 1.5, 1.5),
];

#[test]
fn the_builtin_bodies_meet_every_contrast_rule() {
    for (name, body) in bodies() {
        let palette = Palette::of(body);
        let colour = |key: &str| palette.get(key).expect("a palette colour").0;
        for rule in RULES {
            let ratio = contrast(colour(rule.fg), colour(rule.bg));
            let floor = if body.dark { rule.dark } else { rule.light };
            assert!(ratio >= floor, "{name}: {} is {ratio:.2}:1", rule.id);
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

/// Only the body lets the window show through; a key or a display given
/// opacity is drawn solid, and the settings can say how many were.
#[test]
fn only_the_body_may_be_translucent() {
    let theme = Resolved::new(&one(
        r##"{"themes":[{"name":"Glass","appearance":"light","colors":
            {"page":"#ffffff80","panel":"#eeeeeb99","orange":"#f0520f80","display":"#14141440"}}]}"##,
    ));
    assert!((theme.body.page.a - 0.5).abs() < 0.01);
    assert!((theme.body.panel.a - 0.6).abs() < 0.01);
    assert_eq!(theme.body.orange.a, 1.);
    assert_eq!(theme.body.display.a, 1.);
    assert_eq!(theme.solid, ["orange", "display"]);
}

/// Without blur, a blurred body is laid over the built-in page of its tone
/// and the window turns opaque; other themes are left as they are.
#[test]
fn a_blurred_theme_turns_opaque_where_the_system_cannot_blur() {
    use super::WindowBackground::{Blurred, Opaque, Transparent};
    let glass = |background: &str| {
        Resolved::new(&one(&format!(
            r##"{{"themes":[{{"name":"Glass","appearance":"dark","window_background":"{background}",
                "colors":{{"page":"#ffffff80"}}}}]}}"##
        )))
    };
    let flat = glass("blurred").without_blur();
    assert_eq!(flat.window_background, Opaque);
    assert_eq!(flat.body.page.a, 1.);
    // Half white over night's #111111.
    let page = gpui::Rgba::from(flat.body.page);
    assert!((page.r - (1. + 0x11 as f32 / 255.) / 2.).abs() < 0.01);
    let clear = glass("transparent").without_blur();
    assert_eq!(clear.window_background, Transparent);
    assert!((clear.body.page.a - 0.5).abs() < 0.01);
    assert_eq!(glass("blurred").window_background, Blurred);
}

#[gpui::test]
fn a_translucent_page_colour_and_the_window_material_reach_the_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let glass = Entry {
        spec: one(
            r##"{"themes":[{"name":"Glass","appearance":"dark","window_background":"blurred",
                "colors":{"page":"#10203080","panel":"#20304080"}}]}"##,
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
        let theme = Theme::global(cx);
        let page = theme.background;
        // What floats over the page is solid on any body: dialogs and sheets
        // (the token), the dialogs' raised surface, menus and toasts.
        for (name, color) in [
            ("tokens.background", theme.tokens.background.color),
            ("secondary", theme.secondary),
            ("popover", theme.popover),
            ("tokens.popover", theme.tokens.popover.color),
        ] {
            assert_eq!(color.a, 1., "{name} lets the window show through");
        }
        assert_eq!(ShellColors::current(cx).raised.a, 1.);
        if super::spec::can_blur() {
            assert_eq!(
                look.theme.window_background,
                super::WindowBackground::Blurred
            );
            assert!((page.a - 128. / 255.).abs() < 0.005, "alpha {}", page.a);
        } else {
            // Without blur the body is made opaque (plan translucent-themes, D3).
            assert_eq!(
                look.theme.window_background,
                super::WindowBackground::Opaque
            );
            assert_eq!(page.a, 1.);
        }
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
        // The accent is a key's colour, so its opacity is dropped.
        assert_eq!(Color(theme.primary).to_hex(), "#ff00ff");
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
fn a_colour_made_legible_reads_on_both_bodies_and_keeps_its_hue() {
    for (name, body) in bodies() {
        // The game's white, yellow, aqua, dark blue and black.
        for rgb in [0xff_ffff, 0xff_ff55, 0x55_ffff, 0x00_00aa, 0x00_0000] {
            let color: gpui::Hsla = gpui::rgb(rgb).into();
            let shown = legible(color, body.page, 4.5);
            assert!(
                contrast(shown, body.page) >= 4.5,
                "{name}: {rgb:06x} reads at {}",
                contrast(shown, body.page)
            );
            assert_eq!((shown.h, shown.s), (color.h, color.s), "{name}: {rgb:06x}");
        }
        assert_eq!(
            legible(body.ink, body.page, 4.5),
            body.ink,
            "{name}: a colour that already reads stays"
        );
    }
}

#[test]
fn the_bundled_themes_are_complete_attributed_and_pair_light_with_dark() {
    let catalog = Catalog::default();
    let bundled: Vec<_> = catalog
        .entries
        .iter()
        .filter(|entry| !super::OWN.contains(&entry.spec.name.as_str()))
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

mod website;

/// The framework's root paints the window with the page colour, which may be
/// translucent, not with the opaque token dialogs paint with (the local change
/// to `forks/gpui-component/src/root.rs`).
#[gpui::test]
fn the_root_paints_the_window_with_the_page_colour(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        let theme = Theme::global_mut(cx);
        theme.background = gpui::hsla(0.6, 0.2, 0.5, 0.5);
        theme.tokens.background = gpui::hsla(0.6, 0.2, 0.5, 1.).into();
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = gpui::AppContext::new(cx, |_| gpui::Empty);
        gpui_component::Root::new(host, window, cx)
    });
    cx.run_until_parked();
    let fills: Vec<gpui::Hsla> = cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .filter_map(|quad| quad.background.as_solid())
            .collect()
    });
    assert!(
        fills.contains(&gpui::hsla(0.6, 0.2, 0.5, 0.5)),
        "the root paints {fills:?}"
    );
}

/// The glass themes meet every contrast rule over a desktop of their own
/// tone: Glass over white, Tinted Glass over black (plan translucent-themes,
/// D6). Each translucent colour is judged as it is seen: the page over the
/// desktop, a panel over that page.
#[test]
fn glass_meets_every_contrast_rule_over_a_desktop_of_its_tone() {
    use super::spec::over;
    for (name, dark) in [(super::GLASS, false), (super::TINTED_GLASS, true)] {
        let theme = Catalog::default().resolve(Some(name), Tone::of(dark));
        assert_eq!(theme.name, name);
        assert_eq!(theme.window_background, super::WindowBackground::Blurred);
        assert!(theme.body.page.a < 1. && theme.body.panel.a < 1.);
        let desktop = if dark { gpui::black() } else { gpui::white() };
        let page = over(theme.body.page, desktop);
        let panel = over(theme.body.panel, page);
        let palette = Palette::of(theme.body);
        let colour = |key: &str| match key {
            "page" => page,
            "panel" => panel,
            _ => palette.get(key).expect("a palette colour").0,
        };
        for rule in RULES {
            let bg = colour(rule.bg);
            let fg = over(palette.get(rule.fg).expect("a palette colour").0, bg);
            let ratio = contrast(fg, bg);
            let floor = if dark { rule.dark } else { rule.light };
            assert!(ratio >= floor, "{name}: {} is {ratio:.2}:1", rule.id);
        }
    }
}

/// Birch and Deepslate give every colour and meet every contrast rule, as
/// the launcher's two bodies do (plan translucent-themes, T4).
#[test]
fn the_own_palettes_are_complete_and_meet_every_contrast_rule() {
    for (name, tone) in [(super::BIRCH, Tone::Light), (super::DEEPSLATE, Tone::Dark)] {
        let theme = Catalog::default().resolve(Some(name), tone);
        assert_eq!(theme.name, name);
        assert!(
            theme.missing.is_empty(),
            "{name} misses {:?}",
            theme.missing
        );
        let palette = Palette::of(theme.body);
        let colour = |key: &str| palette.get(key).expect("a palette colour").0;
        for rule in RULES {
            let ratio = contrast(colour(rule.fg), colour(rule.bg));
            let floor = if tone.is_dark() {
                rule.dark
            } else {
                rule.light
            };
            assert!(ratio >= floor, "{name}: {} is {ratio:.2}:1", rule.id);
        }
        // The signal stays the launcher's orange (D4).
        assert_eq!(theme.body.orange, super::Body::of(tone.is_dark()).orange);
    }
}
