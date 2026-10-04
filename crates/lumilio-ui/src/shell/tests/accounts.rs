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
    assert!(cx.debug_bounds("account-choose-0").is_none());

    // With accounts: the page lists them, a click on a row selects it.
    seen.borrow_mut().clear();
    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| model.accounts = vec![account("Steve", true), account("Alex", false)],
            cx,
        );
    });
    cx.run_until_parked();
    let alex = cx.debug_bounds("account-choose-1").expect("Alex's row");
    cx.simulate_click(alex.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SelectAccount("Alex".into())]
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
}
