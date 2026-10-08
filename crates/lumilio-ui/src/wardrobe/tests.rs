use super::*;
use gpui::{InputEvent as _, Modifiers, TestAppContext};
use std::{cell::RefCell, rc::Rc};

fn profile() -> MojangProfile {
    MojangProfile {
        id: "id".into(),
        name: "Player".into(),
        skins: vec![lumilio_core::MojangSkin {
            id: "skin".into(),
            state: "ACTIVE".into(),
            url: "old".into(),
            model: SkinModel::Wide,
        }],
        capes: vec![lumilio_core::MojangCape {
            id: "owned".into(),
            state: "ACTIVE".into(),
            alias: "Owned".into(),
            url: "cape".into(),
        }],
    }
}

#[gpui::test]
fn library_wear_is_bound_to_the_detail_and_duplicate_clicks_wait_for_confirmation(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        Wardrobe::new(
            "account".into(),
            7,
            true,
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.listed(
            Ok(vec![LibrarySkin {
                id: "skin".into(),
                name: "Skin".into(),
                model: SkinModel::Wide,
                source: SkinSource::LocalFile("chosen.png".into()),
            }]),
            Some(Ok(profile())),
            cx,
        )
    });
    cx.run_until_parked();
    let bounds = cx.debug_bounds("wardrobe-wear-skin").unwrap();
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::Wardrobe {
            key: "account".into(),
            revision: 7,
            action: WardrobeAction::Wear("skin".into())
        }]
    );
    view.update(cx, |view, cx| {
        view.finished(WardrobeAction::Wear("skin".into()), Ok(()), cx)
    });
    cx.run_until_parked();
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().len(),
        1,
        "accepted upload must not be sent again while confirming"
    );
    view.update(cx, |view, cx| view.confirmed(Ok(profile()), None, cx));
    view.read_with(cx, |view, _| {
        assert!(view.pending.is_some(), "old profile stays pending")
    });
    let mut changed = profile();
    changed.skins[0].url = "new".into();
    view.update(cx, |view, cx| view.confirmed(Ok(changed), Some(true), cx));
    view.read_with(cx, |view, _| assert!(view.pending.is_none()));
}

#[gpui::test]
fn a_failed_write_keeps_the_profile_and_allows_an_explicit_retry(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        Wardrobe::new(
            "account".into(),
            8,
            true,
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.listed(Ok(vec![]), Some(Ok(profile())), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.request(WardrobeAction::Default, window, cx)
        })
    });
    view.update(cx, |view, cx| {
        view.finished(
            WardrobeAction::Default,
            Err(("Failed".into(), "detail".into())),
            cx,
        )
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.profile, Some(profile()));
        assert!(view.pending.is_none());
        assert!(view.error.is_some());
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.request(WardrobeAction::Default, window, cx)
        })
    });
    assert_eq!(seen.borrow().len(), 2);
}

#[gpui::test]
fn dropping_pngs_imports_into_the_initiating_account_library(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (_, cx) = cx.add_window_view(|_, _| {
        Wardrobe::new(
            "offline".into(),
            19,
            false,
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    cx.run_until_parked();
    let at = cx.debug_bounds("wardrobe").unwrap().center();
    let paths = vec![PathBuf::from("first.png"), PathBuf::from("second.png")];
    cx.update(|window, cx| {
        window.dispatch_event(
            gpui::FileDropEvent::Entered {
                position: at,
                paths: gpui::ExternalPaths(paths.clone().into_iter().collect()),
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            gpui::FileDropEvent::Submit { position: at }.to_platform_input(),
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::Wardrobe {
            key: "offline".into(),
            revision: 19,
            action: WardrobeAction::Import(paths)
        }]
    );
}
