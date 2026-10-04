use super::*;

#[test]
fn names_are_explained_one_reason_at_a_time() {
    assert_eq!(name_problem("Steve"), None);
    assert_eq!(name_problem(""), Some("请输入名称".into()));
    assert!(
        name_problem("a_very_long_name_indeed")
            .unwrap()
            .contains("16")
    );
    assert!(name_problem("bad name").unwrap().contains("不能用"));
}

#[test]
fn a_profile_id_is_checked_only_when_typed() {
    assert_eq!(uuid_problem(""), None);
    assert_eq!(uuid_problem("   "), None);
    assert_eq!(uuid_problem("123e4567-e89b-12d3-a456-426614174000"), None);
    assert!(uuid_problem("nope").is_some());
}

#[test]
fn the_request_ignores_a_hidden_id_and_waits_for_a_valid_one() {
    // A leftover id is not sent while the advanced section is closed.
    let request = request_from(" Steve ", "garbage", false).unwrap();
    assert_eq!(request.name, "Steve");
    assert_eq!(request.uuid, None);
    // Open, it must be valid; blank still means "from the name".
    assert_eq!(request_from("Steve", "garbage", true), None);
    assert_eq!(request_from("Steve", " ", true).unwrap().uuid, None);
    let chosen = request_from("Steve", "123e4567e89b12d3a456426614174000", true).unwrap();
    assert_eq!(
        chosen.uuid.as_deref(),
        Some("123e4567e89b12d3a456426614174000")
    );
    assert_eq!(request_from("", "", false), None);
}

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

#[gpui::test]
fn the_dialog_checks_as_you_type_saves_once_and_keeps_the_draft_on_failure(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::Modifiers;
    use std::cell::RefCell;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen: Rc<RefCell<Vec<AccountRequest>>> = Rc::default();
    let sink = seen.clone();
    let form = cx.update(|window, cx| {
        cx.new(|cx| {
            AccountForm::new(
                Rc::new(move |request, _, _| sink.borrow_mut().push(request)),
                true,
                window,
                cx,
            )
        })
    });
    cx.update(|window, cx| AccountForm::open(form.clone(), window, cx));
    cx.run_until_parked();
    let name = form.read_with(cx, |form, _| form.name.clone());
    let type_name = |cx: &mut gpui::VisualTestContext, text: &str| {
        cx.update(|window, cx| {
            name.update(cx, |input, cx| input.set_value(text.to_owned(), window, cx))
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    };

    // Nothing typed: the button does nothing and nobody is scolded.
    let add = cx.debug_bounds("account-add").expect("dialog on screen");
    cx.simulate_click(add.center(), Modifiers::none());
    assert!(seen.borrow().is_empty());
    assert!(cx.debug_bounds("account-name-problem").is_none());

    // A bad name says why and cannot be saved.
    type_name(cx, "bad name");
    assert!(cx.debug_bounds("account-name-problem").is_some());
    let add = cx.debug_bounds("account-add").unwrap();
    cx.simulate_click(add.center(), Modifiers::none());
    assert!(seen.borrow().is_empty());

    // A good name saves once, however often the button is pressed.
    type_name(cx, "Steve");
    assert!(cx.debug_bounds("account-name-problem").is_none());
    let add = cx.debug_bounds("account-add").unwrap();
    cx.simulate_click(add.center(), Modifiers::none());
    cx.simulate_click(add.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().as_slice(),
        [AccountRequest {
            name: "Steve".into(),
            uuid: None
        }]
    );

    // A refusal stays in the dialog, with what was typed.
    form.update(cx, |form, cx| {
        form.saved(Err(("已经有叫“Steve”的账户了".into(), "dup".into())), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("account-error").is_some());
    assert!(cx.debug_bounds("dialog-layer").is_some(), "still open");
    assert_eq!(form.read_with(cx, |form, cx| form.typed(cx).0), "Steve");

    // Success closes it.
    form.update(cx, |form, cx| form.saved(Ok(()), cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}
