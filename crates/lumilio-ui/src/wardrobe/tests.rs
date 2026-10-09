use super::*;
use gpui::{Entity, InputEvent as _, Modifiers, TestAppContext, VisualTestContext};
use lumilio_core::SkinSource;
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

fn entry(cape: PairedCape) -> LibrarySkin {
    LibrarySkin {
        id: "skin".into(),
        name: "Skin".into(),
        model: SkinModel::Slim,
        source: SkinSource::LocalFile("chosen.png".into()),
        cape,
    }
}

fn pixels(width: u32, height: u32) -> lumilio_core::SkinPixels {
    lumilio_core::SkinPixels {
        width,
        height,
        rgba: vec![200; (width * height * 4) as usize],
    }
}

fn look() -> AccountLook {
    AccountLook {
        model: SkinModel::Slim,
        skin: Some(pixels(64, 64)),
        cape: None,
    }
}

fn colored(byte: u8, width: u32, height: u32) -> lumilio_core::SkinPixels {
    lumilio_core::SkinPixels {
        width,
        height,
        rgba: vec![byte; (width * height * 4) as usize],
    }
}

fn colored_look(byte: u8) -> AccountLook {
    AccountLook {
        model: SkinModel::Slim,
        skin: Some(colored(byte, 64, 64)),
        cape: None,
    }
}

/// The detail's look area, as the Accounts page shows it: the wardrobe lays
/// out the preview it holds beside the skins.
struct Host {
    viewer: Entity<SkinViewer>,
    wardrobe: Entity<Wardrobe>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(900.)).h(px(700.)).child(self.wardrobe.clone())
    }
}

type Seen = Rc<RefCell<Vec<LiveIntent>>>;

fn open(
    cx: &mut TestAppContext,
    revision: u64,
    microsoft: bool,
) -> (Entity<Host>, Seen, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Seen = Rc::default();
    let sink = seen.clone();
    let (host, cx) = cx.add_window_view(|_, cx| {
        let viewer = cx.new(SkinViewer::new);
        let weak = viewer.downgrade();
        let wardrobe = cx.new(|cx| {
            Wardrobe::new(
                "account".into(),
                revision,
                microsoft,
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
                weak,
                cx,
            )
        });
        Host { viewer, wardrobe }
    });
    (host, seen, cx)
}

fn wardrobe(host: &Entity<Host>, cx: &mut VisualTestContext) -> Entity<Wardrobe> {
    host.read_with(cx, |host, _| host.wardrobe.clone())
}

fn viewer(host: &Entity<Host>, cx: &mut VisualTestContext) -> Entity<SkinViewer> {
    host.read_with(cx, |host, _| host.viewer.clone())
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is shown"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn editing_current_appearance_is_a_cancelable_draft_with_one_apply_intent(cx: &mut TestAppContext) {
    let (host, seen, cx) = open(cx, 21, true);
    let view = wardrobe(&host, cx);
    let preview = viewer(&host, cx);
    preview.update(cx, |preview, cx| {
        preview.set_look(Ok(crate::skin_view::Look::from_core(&look())), cx)
    });
    view.update(cx, |view, cx| {
        view.listed(Ok(vec![]), Some(Ok(profile())), None, cx)
    });
    cx.run_until_parked();
    click(cx, "wardrobe-edit");
    assert!(seen.borrow().is_empty());
    view.update(cx, |view, cx| {
        view.editor.as_mut().unwrap().edited.model = SkinModel::Slim;
        view.preview_editor(cx);
    });
    assert!(seen.borrow().is_empty());
    click(cx, "wardrobe-editor-cancel");
    assert!(!preview.read_with(cx, |preview, _| preview.is_trying_on()));
    click(cx, "wardrobe-edit");
    view.update(cx, |view, cx| {
        view.editor.as_mut().unwrap().edited.model = SkinModel::Slim;
        view.preview_editor(cx);
    });
    click(cx, "wardrobe-editor-apply");
    assert!(matches!(seen.borrow().as_slice(), [LiveIntent::Wardrobe {
        key, revision: 21,
        action: WardrobeAction::ApplyCurrentDraft { model: SkinModel::Slim, model_changed: true, cape_changed: false, .. },
    }] if key == "account"));
}

#[gpui::test]
fn escape_closes_the_edit_panel_and_returns_focus_to_its_key(cx: &mut TestAppContext) {
    let (host, seen, cx) = open(cx, 4, true);
    let view = wardrobe(&host, cx);
    viewer(&host, cx).update(cx, |preview, cx| {
        preview.set_look(Ok(crate::skin_view::Look::from_core(&look())), cx)
    });
    view.update(cx, |view, cx| {
        view.listed(Ok(vec![]), Some(Ok(profile())), None, cx)
    });
    cx.run_until_parked();
    click(cx, "wardrobe-edit");
    assert!(
        cx.update(|window, cx| view.read(cx).editor_focus.is_focused(window)),
        "the panel takes focus, so Escape reaches it"
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.editor.is_none()));
    assert!(cx.debug_bounds("wardrobe-editor").is_none());
    assert!(
        cx.update(|window, cx| view.read(cx).edit_focus.is_focused(window)),
        "focus goes back to「编辑外观…」"
    );
    assert!(seen.borrow().is_empty());
}

#[gpui::test]
fn a_skin_picture_is_a_key_enter_and_space_toggle_its_trial_once(cx: &mut TestAppContext) {
    let (host, _, cx) = open(cx, 5, true);
    let view = wardrobe(&host, cx);
    view.update(cx, |view, cx| {
        view.listed(
            Ok(vec![entry(PairedCape::Keep)]),
            Some(Ok(profile())),
            None,
            cx,
        );
        view.pictures(vec![("skin".into(), look())], vec![], cx);
    });
    cx.run_until_parked();
    // A click tries it on and leaves the picture focused.
    click(cx, "wardrobe-try-skin");
    let trying = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.trying.clone());
    assert_eq!(trying(cx).as_deref(), Some("skin"));
    // GPUI clicks a focused element when the key comes back up.
    let press = |cx: &mut VisualTestContext, key: &str| {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    };
    press(cx, "enter");
    assert_eq!(trying(cx), None, "Enter ends the trial, and only once");
    press(cx, "space");
    assert_eq!(
        trying(cx).as_deref(),
        Some("skin"),
        "Space tries it on again"
    );
}

#[gpui::test]
fn library_wear_is_bound_to_the_detail_and_duplicate_clicks_wait_for_confirmation(
    cx: &mut TestAppContext,
) {
    let (host, seen, cx) = open(cx, 7, true);
    let view = wardrobe(&host, cx);
    view.update(cx, |view, cx| {
        view.listed(
            Ok(vec![entry(PairedCape::Keep)]),
            Some(Ok(profile())),
            None,
            cx,
        );
        view.pictures(vec![("skin".into(), look())], vec![], cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("wardrobe-wear-skin").is_none(),
        "wearing starts from a trial"
    );
    click(cx, "wardrobe-try-skin");
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
        assert!(view.pending.is_some(), "old profile stays pending");
        assert!(
            view.trying.is_some(),
            "the trial stays until it is confirmed"
        );
    });
    let mut changed = profile();
    changed.skins[0].url = "new".into();
    view.update(cx, |view, cx| view.confirmed(Ok(changed), Some(true), cx));
    view.read_with(cx, |view, _| {
        assert!(view.pending.is_none());
        assert!(
            view.trying.is_none(),
            "the worn skin is the account's own now"
        );
    });
    viewer(&host, cx).read_with(cx, |viewer, _| assert!(!viewer.is_trying_on()));
}

#[gpui::test]
fn a_trial_shows_the_skin_and_its_paired_cape_and_cancel_restores(cx: &mut TestAppContext) {
    let (host, seen, cx) = open(cx, 3, true);
    let view = wardrobe(&host, cx);
    let shown = viewer(&host, cx);
    let account_cape = Arc::new(Texture::new(64, 32, vec![9; 64 * 32 * 4]).unwrap());
    shown.update(cx, |viewer, cx| {
        viewer.set_look(
            Ok(crate::skin_view::Look {
                cape: Some(account_cape),
                ..Default::default()
            }),
            cx,
        )
    });
    view.update(cx, |view, cx| {
        view.listed(
            Ok(vec![entry(PairedCape::Hidden)]),
            Some(Ok(profile())),
            None,
            cx,
        );
        view.pictures(vec![("skin".into(), look())], vec![], cx);
    });
    cx.run_until_parked();

    click(cx, "wardrobe-try-skin");
    assert!(
        cx.debug_bounds("skin-view-trial").is_some(),
        "badge on the preview"
    );
    shown.read_with(cx, |viewer, _| {
        let look = viewer.shown_look().expect("a look on trial");
        assert!(viewer.is_trying_on());
        assert_eq!(look.arms, Arms::Slim);
        assert!(look.cape.is_none(), "a hidden pairing takes the cape off");
    });
    assert!(seen.borrow().is_empty(), "a trial changes nothing yet");

    click(cx, "wardrobe-trial-cancel");
    assert!(cx.debug_bounds("skin-view-trial").is_none());
    shown.read_with(cx, |viewer, _| {
        assert!(!viewer.is_trying_on());
        assert!(viewer.shown_look().unwrap().cape.is_some(), "own cape back");
    });
}

#[gpui::test]
fn pictures_are_drawn_once_and_released_when_their_skin_leaves(cx: &mut TestAppContext) {
    let (host, _, cx) = open(cx, 5, true);
    let view = wardrobe(&host, cx);
    view.update(cx, |view, cx| {
        view.listed(
            Ok(vec![entry(PairedCape::Keep)]),
            Some(Ok(profile())),
            None,
            cx,
        );
        view.pictures(
            vec![("skin".into(), look())],
            vec![("owned".into(), pixels(64, 32))],
            cx,
        );
    });
    cx.run_until_parked();
    let first = view.read_with(cx, |view, _| {
        assert!(view.cape_pictures.contains_key("owned"));
        view.skin_pictures
            .get(&("skin".to_owned(), SkinModel::Slim))
            .cloned()
            .expect("the skin is drawn")
    });
    // Listing again keeps the drawn picture.
    view.update(cx, |view, cx| {
        view.pictures(vec![("skin".into(), look())], vec![], cx)
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let again = view
            .skin_pictures
            .get(&("skin".to_owned(), SkinModel::Slim))
            .unwrap();
        assert!(Arc::ptr_eq(&first, again));
        assert!(
            view.cape_pictures.is_empty(),
            "the gone cape's picture left"
        );
    });
    view.update(cx, |view, cx| view.pictures(vec![], vec![], cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(view.skin_pictures.is_empty());
        assert!(view.old_pictures.is_empty(), "released on the next render");
    });
}

#[gpui::test]
fn replacing_a_library_skin_texture_redraws_its_picture(cx: &mut TestAppContext) {
    let (host, _, cx) = open(cx, 5, true);
    let view = wardrobe(&host, cx);
    view.update(cx, |view, cx| {
        view.listed(
            Ok(vec![entry(PairedCape::Keep)]),
            Some(Ok(profile())),
            None,
            cx,
        );
        view.pictures(
            vec![("skin".into(), colored_look(10))],
            vec![("owned".into(), colored(1, 64, 32))],
            cx,
        );
    });
    cx.run_until_parked();
    let (first_skin, first_cape) = view.read_with(cx, |view, _| {
        (
            view.skin_pictures
                .get(&("skin".to_owned(), SkinModel::Slim))
                .cloned()
                .expect("the first skin picture"),
            view.cape_pictures
                .get("owned")
                .cloned()
                .expect("the first cape picture"),
        )
    });
    view.update(cx, |view, cx| {
        view.pictures(
            vec![("skin".into(), colored_look(200))],
            vec![("owned".into(), colored(9, 64, 32))],
            cx,
        )
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let skin = view
            .skin_pictures
            .get(&("skin".to_owned(), SkinModel::Slim))
            .expect("the replaced skin is drawn");
        let cape = view
            .cape_pictures
            .get("owned")
            .expect("the replaced cape is drawn");
        assert!(
            !Arc::ptr_eq(&first_skin, skin),
            "the same library id and arm model still showed the previous skin"
        );
        assert!(
            !Arc::ptr_eq(&first_cape, cape),
            "the same cape id still showed the previous texture"
        );
    });
}

#[gpui::test]
fn a_failed_write_keeps_the_profile_and_allows_an_explicit_retry(cx: &mut TestAppContext) {
    let (host, seen, cx) = open(cx, 8, true);
    let view = wardrobe(&host, cx);
    view.update(cx, |view, cx| {
        view.listed(Ok(vec![]), Some(Ok(profile())), None, cx)
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
    let (_, seen, cx) = open(cx, 19, false);
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
            key: "account".into(),
            revision: 19,
            action: WardrobeAction::Import(paths)
        }]
    );
}

#[gpui::test]
fn refresh_is_hidden_until_the_profile_read_fails(cx: &mut TestAppContext) {
    let (host, seen, cx) = open(cx, 4, true);
    let view = wardrobe(&host, cx);
    view.update(cx, |view, cx| {
        view.listed(Ok(vec![]), Some(Ok(profile())), None, cx)
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("wardrobe-reload").is_none(),
        "refresh stays in the current-appearance menu"
    );
    view.update(cx, |view, cx| {
        view.listed(
            Ok(vec![]),
            Some(Ok(profile())),
            Some(("stale".into(), "offline".into())),
            cx,
        )
    });
    cx.run_until_parked();
    click(cx, "wardrobe-reload");
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::Wardrobe {
            key: "account".into(),
            revision: 4,
            action: WardrobeAction::Reload,
        }]
    );
}
