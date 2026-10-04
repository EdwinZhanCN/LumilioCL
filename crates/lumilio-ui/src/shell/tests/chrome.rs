use super::super::{LauncherShell, Route};
use gpui::{Modifiers, TestAppContext};
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn messages_float_as_toasts_and_never_take_room_in_the_page(cx: &mut TestAppContext) {
    use crate::toast::Toast;
    use gpui::{AppContext as _, Entity};
    use gpui_component::WindowExt as _;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let slot: Rc<RefCell<Option<Entity<LauncherShell>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
        *keep.borrow_mut() = Some(shell.clone());
        gpui_component::Root::new(shell, window, cx)
    });
    let shell = slot.borrow().clone().expect("shell");
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        cx.notify();
    });
    cx.run_until_parked();
    let header = cx.debug_bounds("live-library-actions").map(|b| b.origin);
    shell.update(cx, |shell, cx| {
        shell.toast(Toast::error("没有装上 Sodium").technical("disk full"), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    shell.read_with(cx, |shell, _| assert!(shell.pending_toasts().is_empty()));
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1);
    assert_eq!(
        cx.debug_bounds("live-library-actions").map(|b| b.origin),
        header,
        "the page did not move to make room"
    );
}

#[gpui::test]
fn a_deleted_instance_drops_out_of_history_and_the_chip_retargets(cx: &mut TestAppContext) {
    use crate::live::{LiveIntent, library_card};
    use lumilio_core::{InstanceRecord, InstanceSettings, Loader};
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
    let record = |id: &str| InstanceRecord {
        id: id.into(),
        name: id.into(),
        game_version: "1.21.1".into(),
        loader: Loader::Vanilla,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: true,
        settings: InstanceSettings::default(),
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        let cards = vec![library_card(&record("a"), 1), library_card(&record("b"), 1)];
        shell.update_live(|model| model.set_library(cards, Some("a".into())), cx);
    });
    cx.run_until_parked();
    for id in ["a", "b"] {
        shell.update(cx, |shell, cx| {
            shell.open_live_instance(id.into(), |_| Rc::new(|_, _, _| {}), cx);
        });
    }
    // Back to "a", so "b" is the way forward; then "a" is deleted.
    shell.update(cx, |shell, cx| assert!(shell.go_back(None, cx)));
    shell.update(cx, |shell, cx| shell.forget_instance("a", cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, cx| {
        assert!(shell.live_instance().is_none(), "the deleted one left");
        assert_eq!(shell.location_title(cx), "游戏库");
        assert!(!shell.can_go_back());
    });
    shell.update(cx, |shell, cx| assert!(shell.go_forward(None, cx)));
    shell.read_with(cx, |shell, cx| {
        assert_eq!(shell.live_instance().unwrap().read(cx).id(), "b");
        assert!(!shell.can_go_forward(), "forward never reaches \"a\"");
    });

    // The trailing chip lists every instance and retargets on choice.
    cx.run_until_parked();
    let chip = cx
        .debug_bounds("navigation-instance")
        .expect("current instance");
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let second = cx
        .debug_bounds("navigation-instance-choice-1")
        .expect("the list opened");
    cx.simulate_click(second.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::InstallTarget("b".into()))
    );
}
