mod accounts;
mod activity;
mod chrome;
mod discover;
mod home;
mod library;
mod settings;

use crate::home::{HomePresentation, RecentEntry, Subject, WorldHint};

fn continuing() -> HomePresentation {
    HomePresentation::Continue {
        subject: Subject {
            title: "生存".into(),
            metadata: "1.21.1 · Fabric".into(),
            world: WorldHint::Underground,
        },
        recent: vec![RecentEntry {
            id: None,
            title: "空岛".into(),
            metadata: "昨天".into(),
        }],
    }
}

/// Lets real-time entrance animations finish (gpui animations read the
/// wall clock), then draws a fresh frame.
fn settle(cx: &mut gpui::VisualTestContext) {
    std::thread::sleep(crate::theme::motion::SCENE + std::time::Duration::from_millis(120));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Every quad painted exactly over an element's bounds. GPUI paints a
/// filled, bordered element as a fill quad plus border-only quads.
fn painted_at(cx: &mut gpui::VisualTestContext, selector: &'static str) -> Vec<gpui::Quad> {
    let Some(bounds) = cx.debug_bounds(selector) else {
        return Vec::new();
    };
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let near = |a: f32, b: gpui::Pixels| (a / scale - f32::from(b)).abs() < 1.5;
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| {
                near(quad.bounds.origin.x.0, bounds.origin.x)
                    && near(quad.bounds.origin.y.0, bounds.origin.y)
                    && near(quad.bounds.size.width.0, bounds.size.width)
                    && near(quad.bounds.size.height.0, bounds.size.height)
            })
            .collect()
    })
}

fn fill(quads: &[gpui::Quad]) -> gpui::Hsla {
    quads
        .iter()
        .filter_map(|quad| quad.background.as_solid())
        .find(|color| color.a > 0.)
        .expect("the element paints a fill")
}
