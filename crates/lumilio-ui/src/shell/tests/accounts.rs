use super::super::{LauncherShell, Route};
use gpui::{Modifiers, TestAppContext, point};
use std::cell::RefCell;
use std::rc::Rc;

pub(super) fn account(name: &str, selected: bool) -> crate::live::AccountRow {
    crate::live::AccountRow {
        key: name.into(),
        name: name.into(),
        uuid: lumilio_core::ProfileId::offline(name).to_string(),
        selected,
        custom_id: false,
        microsoft: false,
        third_party: false,
        skin_site: None,
        kind_text: "离线账户".into(),
        skin: None,
        needs_sign_in: false,
    }
}

#[gpui::test]
fn the_account_chip_and_page_choose_add_and_manage_accounts(cx: &mut TestAppContext) {
    use crate::live::LiveIntent;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
    });
    shell.update(cx, |shell, cx| {
        shell.show(Route::Accounts);
        shell.update_live(|model| model.accounts_loaded = true, cx);
    });
    cx.run_until_parked();

    // No account yet: the chip offers to add one, and so does the page.
    let chip = cx.debug_bounds("navigation-account").expect("account chip");
    cx.simulate_click(chip.center(), Modifiers::none());
    assert_eq!(seen.borrow().as_slice(), [LiveIntent::NewAccount]);
    // The page offers both kinds: offline on the left, Microsoft on the right.
    let actions = cx
        .debug_bounds("accounts-actions")
        .expect("add on the page");
    let at = |share: f32| {
        point(
            actions.origin.x + actions.size.width * share,
            actions.center().y,
        )
    };
    cx.simulate_click(at(0.2), Modifiers::none());
    assert_eq!(seen.borrow().len(), 2);
    assert_eq!(seen.borrow()[1], LiveIntent::NewAccount);
    cx.simulate_click(at(0.85), Modifiers::none());
    assert_eq!(seen.borrow().last(), Some(&LiveIntent::MicrosoftSignIn));
    assert!(cx.debug_bounds("account-item-0").is_none());
    assert!(cx.debug_bounds("account-detail").is_none());

    // With accounts: the detail shows the current account and loads its look.
    seen.borrow_mut().clear();
    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| model.accounts = vec![account("Steve", true), account("Alex", false)],
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("account-detail").is_some());
    assert!(cx.debug_bounds("skin-view").is_some());
    assert!(
        cx.debug_bounds("account-make-current").is_none(),
        "Steve is current"
    );
    // The detail loads the shown account's look, and every account is asked
    // once for its avatar face.
    let asked = seen.borrow().clone();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert!(asked.contains(&LiveIntent::LoadAccountLook("Steve".into())));
    assert!(asked.contains(&LiveIntent::LoadAccountFace("Steve".into())));
    assert!(asked.contains(&LiveIntent::LoadAccountFace("Alex".into())));
    // A row shows that account without making it current; the detail does.
    seen.borrow_mut().clear();
    let alex = cx.debug_bounds("account-item-1").expect("Alex's row");
    cx.simulate_click(alex.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::LoadAccountLook("Alex".into())]
    );
    let make_current = cx
        .debug_bounds("account-make-current")
        .expect("Alex is not current");
    cx.simulate_click(make_current.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::SelectAccount("Alex".into()))
    );

    // The chip lists them too and leads to the page from its foot.
    seen.borrow_mut().clear();
    let chip = cx.debug_bounds("navigation-account").unwrap();
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let second = cx
        .debug_bounds("navigation-account-choice-1")
        .expect("the list opened");
    cx.simulate_click(second.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SelectAccount("Alex".into())]
    );
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        cx.notify();
    });
    cx.run_until_parked();
    let chip = cx.debug_bounds("navigation-account").unwrap();
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let manage = cx
        .debug_bounds("navigation-account-manage")
        .expect("manage entry");
    cx.simulate_click(manage.center(), Modifiers::none());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert_eq!(shell.route(), Route::Accounts));
    shell.read_with(cx, |shell, _| {
        assert!(
            shell.account_detail("Steve").is_some(),
            "the nav entry opens the current account, not Alex whom the page showed last"
        )
    });
}

#[gpui::test]
fn reopening_the_same_account_drops_old_look_and_wardrobe_results(cx: &mut TestAppContext) {
    use crate::live::LiveIntent;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(|_: LiveIntent, _, _| {}))
    });
    shell.update(cx, |shell, cx| {
        shell.show(Route::Accounts);
        shell.update_live(
            |model| {
                model.accounts_loaded = true;
                model.accounts = vec![account("Steve", true)];
            },
            cx,
        );
    });
    cx.run_until_parked();
    let (old, cancel) = shell.read_with(cx, |shell, _| shell.account_detail("Steve").unwrap());
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cancel.is_cancelled());
    shell.update(cx, |shell, cx| {
        shell.show(Route::Accounts);
        cx.notify();
    });
    cx.run_until_parked();
    let (new, _) = shell.read_with(cx, |shell, _| shell.account_detail("Steve").unwrap());
    assert_ne!(old, new);
    shell.update(cx, |shell, cx| {
        shell.account_look("Steve", old, Err("late old failure".into()), cx);
        shell.wardrobe_listed(
            "Steve",
            old,
            Err(("late".into(), "old".into())),
            None,
            None,
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("skin-view-error").is_none());
    shell.read_with(cx, |shell, cx| {
        let wardrobe = shell
            .account_viewer
            .as_ref()
            .unwrap()
            .wardrobe
            .as_ref()
            .unwrap();
        assert!(!wardrobe.read(cx).is_loaded());
    });
    shell.update(cx, |shell, cx| {
        shell.account_look("Steve", new, Err("current failure".into()), cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("skin-view-error").is_some());
}
