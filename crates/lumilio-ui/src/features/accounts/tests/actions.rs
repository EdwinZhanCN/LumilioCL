//! Accounts' actions with a real launcher behind them, on its own runtime.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, TestAppContext, VisualTestContext, Window, div};

use super::super::{Accounts, AccountsEvent, AccountsInputs};
use super::account;
use crate::testing::{Launcher, launcher, toasts, until};

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

/// Accounts in a window, with what it told the application so far.
fn accounts<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (
    Entity<Accounts>,
    Rc<RefCell<Vec<AccountsEvent>>>,
    &'a mut VisualTestContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|_, _| Host);
    let worker = launcher.worker.clone();
    let accounts = cx.new(|_| Accounts::new(Some(worker)));
    let events: Rc<RefCell<Vec<AccountsEvent>>> = Rc::default();
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&accounts, move |_, event: &AccountsEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (accounts, events, cx)
}

fn add(launcher: &Launcher, name: &'static str) {
    let service = launcher.worker.service().clone();
    launcher
        .runtime
        .block_on(async move { service.add_account(name, None).await })
        .unwrap();
}

#[gpui::test]
fn removing_an_account_forgets_it_and_says_so(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    add(&launcher, "Steve");
    let (accounts, _, cx) = accounts(&launcher, cx);
    accounts.update(cx, |accounts, cx| accounts.remove("Steve".into(), cx));
    until(cx, "the removal to be told", |cx| !toasts(cx).is_empty());

    let told = toasts(cx);
    assert!(
        told[0].starts_with("已移除账户") && told[0].contains("Steve"),
        "{told:?}"
    );
    let service = launcher.worker.service().clone();
    let left = launcher
        .runtime
        .block_on(async move { service.settings().await.accounts.len() });
    assert_eq!(left, 0);
}

#[gpui::test]
fn every_account_is_asked_once_for_its_face_and_a_gone_one_forgotten(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    add(&launcher, "Steve");
    add(&launcher, "Alex");
    let (accounts, _, cx) = accounts(&launcher, cx);
    let faces = |accounts: &Entity<Accounts>, cx: &mut VisualTestContext| {
        accounts.read_with(cx, |accounts, _| {
            let mut keys: Vec<String> = accounts.faces.keys().cloned().collect();
            keys.sort();
            keys
        })
    };

    // Not read yet: nothing is asked.
    accounts.update(cx, |accounts, cx| {
        let inputs = AccountsInputs {
            accounts: vec![account("Steve", true)],
            loaded: false,
        };
        accounts.set_inputs(inputs, cx);
    });
    assert!(faces(&accounts, cx).is_empty());

    accounts.update(cx, |accounts, cx| {
        let inputs = AccountsInputs {
            accounts: vec![account("Steve", true), account("Alex", false)],
            loaded: true,
        };
        accounts.set_inputs(inputs, cx);
    });
    assert_eq!(faces(&accounts, cx), ["Alex", "Steve"], "asked at once");
    accounts.update(cx, |accounts, cx| {
        let inputs = AccountsInputs {
            accounts: vec![account("Alex", true)],
            loaded: true,
        };
        accounts.set_inputs(inputs, cx);
    });
    assert_eq!(faces(&accounts, cx), ["Alex"], "Steve is gone");
}
