use super::{Look, SkinViewer};
use gpui::{Entity, TestAppContext, VisualTestContext};
use lumilio_skin_render::Arms;

fn open(cx: &mut TestAppContext) -> (Entity<SkinViewer>, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    cx.add_window_view(|_, cx| SkinViewer::new(cx))
}

#[gpui::test]
fn a_look_is_drawn_and_a_new_look_replaces_the_picture(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx);
    cx.run_until_parked();
    viewer.read_with(cx, |viewer, _| {
        assert!(viewer.image.is_none(), "nothing to draw yet")
    });

    viewer.update(cx, |viewer, cx| viewer.set_look(Ok(Look::default()), cx));
    cx.run_until_parked();
    let first = viewer.read_with(cx, |viewer, _| viewer.image.clone().expect("drawn"));

    viewer.update(cx, |viewer, cx| {
        viewer.set_look(
            Ok(Look {
                arms: Arms::Slim,
                ..Look::default()
            }),
            cx,
        )
    });
    cx.run_until_parked();
    viewer.read_with(cx, |viewer, _| {
        let second = viewer.image.clone().expect("drawn again");
        assert!(!std::sync::Arc::ptr_eq(&first, &second));
        assert!(viewer.drawing.is_none());
    });
}

#[gpui::test]
fn a_failed_look_shows_why_and_draws_nothing(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx);
    viewer.update(cx, |viewer, cx| {
        viewer.set_look(Err(("没能读到皮肤".into(), "io".into())), cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("skin-view-error").is_some());
    viewer.read_with(cx, |viewer, _| assert!(viewer.image.is_none()));
}

#[test]
fn a_core_look_keeps_its_pictures_and_arm_model() {
    let look = lumilio_core::AccountLook {
        model: lumilio_core::SkinModel::Slim,
        skin: Some(lumilio_core::SkinPixels {
            width: 64,
            height: 64,
            rgba: vec![7; 64 * 64 * 4],
        }),
        cape: None,
    };
    let look = Look::from_core(&look);
    assert_eq!(look.arms, Arms::Slim);
    assert_eq!(look.skin.map(|skin| skin.width), Some(64));
    assert!(look.cape.is_none());
}
