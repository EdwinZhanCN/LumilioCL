use super::*;
use gpui::Modifiers;
use std::cell::RefCell;

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

type Seen = Rc<RefCell<Vec<SkinIntent>>>;

fn open(
    cx: &mut gpui::TestAppContext,
    current: Option<SkinChoice>,
) -> (Entity<SkinDialog>, &mut gpui::VisualTestContext, Seen) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen: Seen = Rc::default();
    let sink = seen.clone();
    let dialog = cx.update(|window, cx| {
        cx.new(|cx| {
            SkinDialog::new(
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
                "Steve".into(),
                "Steve".into(),
                current.as_ref(),
                window,
                cx,
            )
        })
    });
    cx.update(|window, cx| SkinDialog::open(dialog.clone(), window, cx));
    cx.run_until_parked();
    (dialog, cx, seen)
}

fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

fn type_into(cx: &mut gpui::VisualTestContext, field: &Entity<InputState>, text: &str) {
    cx.update(|window, cx| field.update(cx, |input, cx| input.set_value(text, window, cx)));
    cx.run_until_parked();
}

#[gpui::test]
fn the_default_skin_clears_the_choice_and_littleskin_needs_nothing_more(
    cx: &mut gpui::TestAppContext,
) {
    let (dialog, cx, seen) = open(cx, Some(SkinChoice::LittleSkin));
    // It opens on what the account has.
    assert_eq!(
        dialog.read_with(cx, |d, cx| d.choice(cx)),
        Ok(Some(SkinChoice::LittleSkin))
    );
    click(cx, "skin-kind-0");
    click(cx, "skin-save");
    click(cx, "skin-save");
    assert_eq!(
        seen.borrow().as_slice(),
        [SkinIntent {
            key: "Steve".into(),
            choice: None
        }],
        "saving twice asks once"
    );
    dialog.update(cx, |d, cx| {
        d.saved(Err(("没有保存成功".into(), "x".into())), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("skin-error").is_some());

    click(cx, "skin-kind-2");
    click(cx, "skin-save");
    assert_eq!(
        seen.borrow().last(),
        Some(&SkinIntent {
            key: "Steve".into(),
            choice: Some(SkinChoice::LittleSkin)
        })
    );
}

#[gpui::test]
fn local_files_need_a_picture_and_carry_the_model(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, seen) = open(cx, None);
    click(cx, "skin-kind-1");
    assert!(
        dialog.read_with(cx, |d, cx| d.choice(cx)).is_err(),
        "no picture chosen yet"
    );
    click(cx, "skin-save");
    assert!(
        seen.borrow().is_empty(),
        "nothing is sent while it is incomplete"
    );

    let (skin, cape) = dialog.read_with(cx, |d, _| (d.skin.clone(), d.cape.clone()));
    type_into(cx, &skin, " /tmp/me.png ");
    type_into(cx, &cape, "");
    click(cx, "skin-model-key-1");
    click(cx, "skin-save");
    assert_eq!(
        seen.borrow().last(),
        Some(&SkinIntent {
            key: "Steve".into(),
            choice: Some(SkinChoice::Local {
                model: SkinModel::Slim,
                skin: Some("/tmp/me.png".into()),
                cape: None,
            }),
        })
    );
}

#[gpui::test]
fn a_skin_site_needs_an_address_and_a_saved_dialog_closes(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, seen) = open(cx, None);
    click(cx, "skin-kind-3");
    click(cx, "skin-save");
    assert!(seen.borrow().is_empty());
    let api = dialog.read_with(cx, |d, _| d.api.clone());
    type_into(cx, &api, "skin.example/csl");
    click(cx, "skin-save");
    assert_eq!(
        seen.borrow()
            .last()
            .and_then(|intent| intent.choice.clone()),
        Some(SkinChoice::Csl {
            api: "skin.example/csl".into()
        })
    );
    dialog.update(cx, |d, cx| d.saved(Ok(()), cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}

#[gpui::test]
fn the_dialog_starts_from_the_choice_the_account_has(cx: &mut gpui::TestAppContext) {
    let local = SkinChoice::Local {
        model: SkinModel::Slim,
        skin: Some("/s.png".into()),
        cape: Some("/c.png".into()),
    };
    let (dialog, cx, _) = open(cx, Some(local.clone()));
    assert_eq!(dialog.read_with(cx, |d, cx| d.choice(cx)), Ok(Some(local)));
}
