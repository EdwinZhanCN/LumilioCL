use gpui::prelude::*;

use gpui::{IntoElement, Render, TestAppContext, Window, div};
use gpui_component::{Theme, ThemeMode};

use super::motd::motd_color;
use super::{TagKind, list, lit_bars, motd_lines, panel_list, signal, signal_bars, tag};
use crate::theme::{self, ShellColors};

struct Sheet;

impl Render for Sheet {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        div()
            .w(gpui::px(300.))
            .child(
                div()
                    .debug_selector(|| "table".into())
                    .child(list(vec![div().child("内存"), div().child("窗口")], colors)),
            )
            .child(
                div()
                    .debug_selector(|| "panel".into())
                    .child(panel_list(vec![div().child("a"), div().child("b")], colors)),
            )
            .child(div().debug_selector(|| "tag".into()).child(tag(
                "FABRIC",
                TagKind::Orange,
                colors,
            )))
            .child(
                div()
                    .debug_selector(|| "signal".into())
                    .child(signal("signal", 120, colors)),
            )
    }
}

fn quads(cx: &mut gpui::VisualTestContext, selector: &'static str) -> Vec<gpui::Quad> {
    let bounds = cx.debug_bounds(selector).expect("drawn");
    cx.update(|window, _| {
        let scale = window.scale_factor();
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| {
                let x = quad.bounds.origin.x.0 / scale;
                let y = quad.bounds.origin.y.0 / scale;
                x >= f32::from(bounds.origin.x) - 1.5
                    && y >= f32::from(bounds.origin.y) - 1.5
                    && y <= f32::from(bounds.origin.y + bounds.size.height)
            })
            .collect()
    })
}

#[gpui::test]
fn a_hairline_table_has_an_ink_rule_on_top_and_hairlines_beneath(cx: &mut TestAppContext) {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(mode, None, cx);
            theme::tune(cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Sheet);
        cx.run_until_parked();
        let body = theme::Body::of(mode.is_dark());
        let edges: Vec<_> = quads(cx, "table")
            .into_iter()
            .filter(|quad| quad.border_widths.top.0 > 0. || quad.border_widths.bottom.0 > 0.)
            .collect();
        assert!(
            edges
                .iter()
                .any(|quad| quad.border_widths.top.0 > 0. && quad.border_color == body.ink),
            "{mode:?}: the top rule is ink"
        );
        assert!(
            edges
                .iter()
                .any(|quad| quad.border_widths.bottom.0 > 0. && quad.border_color == body.hairline),
            "{mode:?}: rows end in hairlines"
        );
        let panel = quads(cx, "panel");
        assert!(
            panel
                .iter()
                .any(|quad| quad.background.as_solid() == Some(body.panel)),
            "{mode:?}: a collection sits on a panel"
        );
        let orange = quads(cx, "tag");
        assert!(
            orange
                .iter()
                .any(|quad| quad.background.as_solid() == Some(body.orange)),
            "{mode:?}: the orange tag fills with the signal orange"
        );
        let lit = quads(cx, "signal")
            .iter()
            .filter(|quad| quad.background.as_solid() == Some(body.ok))
            .count();
        assert_eq!(
            lit, 5,
            "{mode:?}: a quick round trip lights five green bars"
        );
    }
}

#[test]
fn a_message_keeps_the_games_colours_where_the_page_can_read_them() {
    use lumilio_core::Motd;
    for dark in [false, true] {
        let body = theme::Body::of(dark);
        let colors = ShellColors::of(body);
        let spans = Motd::from_legacy("plain §fwhite §cred").spans;
        let [plain, white, red] = spans.as_slice() else {
            panic!("three runs: {spans:?}")
        };
        assert_eq!(motd_color(plain, colors), body.muted, "uncoloured is muted");
        for span in [white, red] {
            let shown = motd_color(span, colors);
            assert!(theme::contrast(shown, body.page) >= 4.5, "{span:?} reads");
        }
        let red_hue = gpui::Hsla::from(gpui::rgb(0xff_5555)).h;
        assert_eq!(motd_color(red, colors).h, red_hue, "red stays red");
    }
}

#[test]
fn signal_bars_follow_the_games_round_trip_steps() {
    let steps: Vec<_> = [0, 149, 150, 299, 300, 599, 600, 999, 1000, 60_000]
        .into_iter()
        .map(signal_bars)
        .collect();
    assert_eq!(steps, [5, 5, 4, 4, 3, 3, 2, 2, 1, 1]);
}

#[test]
fn a_message_shows_two_lines_without_the_space_around_them() {
    use lumilio_core::Motd;
    let text = |motd: &str| -> Vec<String> {
        motd_lines(&Motd::from_legacy(motd))
            .iter()
            .map(|line| line.iter().map(|span| span.text.as_str()).collect())
            .collect()
    };
    assert_eq!(
        text("   §a Centered §bpart  \n second\nthird"),
        ["Centered part", "second"]
    );
    assert_eq!(text("\n  only below  "), ["only below"]);
    assert!(text(" \n  ").is_empty());
}

#[test]
fn a_faceplate_cover_does_not_dissolve_into_the_page() {
    assert_eq!(super::FACEPLATE_FADE, gpui::px(0.));
}

#[test]
fn the_meter_lights_whole_bars_and_never_steps_back() {
    let mut last = 0;
    for step in 0..=1000 {
        let lit = lit_bars(step as f32 / 1000., 24);
        assert!(lit >= last, "bars only increase");
        last = lit;
    }
    assert_eq!(lit_bars(0., 24), 0);
    assert_eq!(lit_bars(1., 24), 24);
    assert_eq!(lit_bars(0.5, 24), 12);
    assert_eq!(lit_bars(-1., 24), 0, "below range clamps");
    assert_eq!(lit_bars(7., 24), 24, "above range clamps");
    assert_eq!(lit_bars(f32::NAN, 24), 0);
}

#[test]
fn a_reading_pads_to_its_display_and_never_spills_over() {
    use super::reading_digits;
    assert_eq!(reading_digits(7, 3), "007");
    assert_eq!(reading_digits(128, 4), "0128");
    assert_eq!(reading_digits(999, 3), "999");
    assert_eq!(
        reading_digits(1000, 3),
        "888",
        "too large lights every segment"
    );
}

#[test]
fn a_line_break_in_html_becomes_a_paragraph_break() {
    use super::rich::paragraph_breaks;
    assert_eq!(
        paragraph_breaks("<p>One<br>Two<BR/>Three<br />Four</p>"),
        "<p>One</p><p>Two</p><p>Three</p><p>Four</p>"
    );
    assert_eq!(
        paragraph_breaks("<p>a < b <brief> <b>x</b></p>"),
        "<p>a < b <brief> <b>x</b></p>",
        "other tags and a bare < stay"
    );
}
