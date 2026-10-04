use gpui::prelude::*;

use gpui::{IntoElement, Render, TestAppContext, Window, div};
use gpui_component::{ActiveTheme as _, Theme, ThemeMode};

use super::{TagKind, list, lit_bars, panel_list, tag};
use crate::theme::{self, ShellColors};

struct Sheet;

impl Render for Sheet {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
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
    }
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
