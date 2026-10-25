use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, TestAppContext};

use super::super::{LauncherShell, Route};

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

    // No account yet: the page offers both kinds, offline and Microsoft, and
    // the chip offers to add one. Accounts opens their dialogs itself.
    assert!(cx.debug_bounds("accounts-actions").is_some());
    assert!(cx.debug_bounds("navigation-account").is_some());
    assert!(cx.debug_bounds("account-item-0").is_none());
    assert!(cx.debug_bounds("account-detail").is_none());

    // With accounts: the detail shows the current account and its look.
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
    let detail_of = |name: &str, cx: &mut gpui::VisualTestContext| {
        shell.read_with(cx, |shell, cx| {
            shell.accounts().read(cx).account_detail(name).is_some()
        })
    };
    assert!(detail_of("Steve", cx));
    // A row shows that account without making it current; the detail does,
    // through the shell, which keeps the choice.
    let alex = cx.debug_bounds("account-item-1").expect("Alex's row");
    cx.simulate_click(alex.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(detail_of("Alex", cx));
    let make_current = cx
        .debug_bounds("account-make-current")
        .expect("Alex is not current");
    cx.simulate_click(make_current.center(), Modifiers::none());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.current_choices.accounts_chosen, ["Alex"]);
    });

    // The chip lists them too and leads to the page from its foot.
    let chip = cx.debug_bounds("navigation-account").unwrap();
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let second = cx
        .debug_bounds("navigation-account-choice-1")
        .expect("the list opened");
    cx.simulate_click(second.center(), Modifiers::none());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.current_choices.accounts_chosen, ["Alex", "Alex"]);
    });
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
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
    assert!(
        detail_of("Steve", cx),
        "the nav entry opens the current account, not Alex whom the page showed last"
    );
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
    let accounts = shell.read_with(cx, |shell, _| shell.accounts().clone());
    let (old, cancel) =
        accounts.read_with(cx, |accounts, _| accounts.account_detail("Steve").unwrap());
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
    let (new, _) = accounts.read_with(cx, |accounts, _| accounts.account_detail("Steve").unwrap());
    assert_ne!(old, new);
    accounts.update(cx, |accounts, cx| {
        accounts.look_arrived("Steve", old, Err("late old failure".into()), cx);
        accounts.wardrobe_listed(
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
    accounts.read_with(cx, |accounts, cx| {
        let wardrobe = accounts.wardrobe_at("Steve", new).unwrap();
        assert!(!wardrobe.read(cx).is_loaded());
    });
    accounts.update(cx, |accounts, cx| {
        accounts.look_arrived("Steve", new, Err("current failure".into()), cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("skin-view-error").is_some());
}
