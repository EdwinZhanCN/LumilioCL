use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, TestAppContext};
use lumilio_core::{LauncherSettings, PackPolicy, ServerEntry, ServerStatus};

use super::super::intent::{InstanceIntent, Section};
use super::super::panels::{Arrived, Confirm, ServerState};
use super::super::{InstanceDetailView, TAB_WORLDS};
use super::{click, record, rooted};

fn server(name: &str, address: &str) -> ServerEntry {
    ServerEntry {
        name: name.into(),
        address: address.into(),
        packs: PackPolicy::Ask,
        icon: None,
    }
}

fn status() -> ServerStatus {
    ServerStatus {
        motd: lumilio_core::Motd::from_legacy("§aWelcome\nsecond line"),
        online: Some(3),
        max: Some(20),
        version: Some("Paper 1.21".into()),
        latency_ms: Some(42),
        favicon: None,
    }
}

fn open(
    seen: Rc<RefCell<Vec<InstanceIntent>>>,
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<InstanceDetailView>,
    &mut gpui::VisualTestContext,
) {
    let (view, cx) = rooted(cx, seen);
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.worlds_sub = 1;
        view.arrived(
            Arrived::Servers(Ok(vec![
                server("Home", "a.example"),
                server("Work", "b.example:1"),
            ])),
            cx,
        );
    });
    cx.run_until_parked();
    (view, cx)
}

fn writes(seen: &Rc<RefCell<Vec<InstanceIntent>>>) -> Vec<InstanceIntent> {
    seen.borrow()
        .iter()
        .filter(|intent| {
            !matches!(
                intent,
                InstanceIntent::Load(_) | InstanceIntent::PingServer(_)
            )
        })
        .cloned()
        .collect()
}

#[gpui::test]
fn a_row_shows_how_the_server_is_in_the_form_each_fact_takes(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen, cx);
    let drawn = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
        cx.debug_bounds(selector).is_some()
    };
    assert!(drawn(cx, "server-checking-0") && drawn(cx, "server-checking-1"));

    view.update(cx, |view, cx| {
        view.server_status_arrived("a.example".into(), Ok(status()), cx);
        view.server_status_arrived("b.example:1".into(), Err("timeout".into()), cx);
    });
    cx.run_until_parked();
    for shown in [
        "server-players-0",
        "server-signal-0",
        "server-motd-0",
        "server-offline-1",
    ] {
        assert!(drawn(cx, shown), "{shown} is drawn");
    }
    for hidden in ["server-checking-0", "server-signal-1", "server-motd-1"] {
        assert!(!drawn(cx, hidden), "{hidden} is not drawn");
    }

    // An answer with nothing to count still says the server is there; an
    // empty message has no window.
    let bare = ServerStatus {
        motd: lumilio_core::Motd::default(),
        online: None,
        max: None,
        latency_ms: None,
        ..status()
    };
    view.update(cx, |view, cx| {
        view.server_status_arrived("b.example:1".into(), Ok(bare), cx);
    });
    cx.run_until_parked();
    assert!(drawn(cx, "server-online-1"));
    assert!(!drawn(cx, "server-motd-1") && !drawn(cx, "server-players-1"));
}

#[gpui::test]
fn arriving_servers_are_each_asked_about_once_and_answers_show(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    let pings: Vec<_> = seen
        .borrow()
        .iter()
        .filter_map(|intent| match intent {
            InstanceIntent::PingServer(address) => Some(address.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(pings, ["a.example", "b.example:1"]);
    view.update(cx, |view, cx| {
        view.server_status_arrived("a.example".into(), Ok(status()), cx);
        view.server_status_arrived("b.example:1".into(), Err("timeout".into()), cx);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.server_status["a.example"],
            ServerState::Online(status())
        );
        assert_eq!(view.server_status["b.example:1"], ServerState::Offline);
    });
}

#[gpui::test]
fn a_server_is_entered_added_and_deleted_with_a_question(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    click(cx, "server-play-1");
    assert_eq!(
        writes(&seen),
        [InstanceIntent::PlayServer("b.example:1".into())]
    );

    // Deleting asks first and names the server.
    view.update(cx, |view, cx| {
        view.busy = false;
        cx.notify();
    });
    let sent = writes(&seen).len();
    let ask = cx.debug_bounds("server-delete-0").expect("delete button");
    cx.simulate_click(ask.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(writes(&seen).len(), sent, "the click only asks");
    let confirm = Confirm::DeleteServer {
        index: 0,
        entry: server("Home", "a.example"),
    };
    view.read_with(cx, |view, _| {
        assert_eq!(view.confirm, Some(confirm.clone()))
    });
    assert!(confirm.words(&[], &[]).0.contains("Home"));
    cx.update(|window, cx| view.update(cx, |view, cx| view.confirmed(&confirm, window, cx)));
    assert_eq!(
        writes(&seen).last(),
        Some(&InstanceIntent::DeleteServer {
            index: 0,
            expected: server("Home", "a.example"),
        })
    );

    // Adding: the dialog's fields become the entry.
    view.update(cx, |view, cx| {
        view.busy = false;
        cx.notify();
    });
    click(cx, "server-add");
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let fields = view.fields.as_ref().unwrap();
            fields
                .server_name
                .update(cx, |f, cx| f.set_value("New", window, cx));
            fields
                .server_address
                .update(cx, |f, cx| f.set_value(" c.example ", window, cx));
            view.submit_server(window, cx);
        })
    });
    assert_eq!(
        writes(&seen).last(),
        Some(&InstanceIntent::SaveServer {
            index: None,
            expected: None,
            entry: server("New", "c.example"),
        })
    );
}

#[gpui::test]
fn a_running_game_owns_the_list_and_an_old_version_cannot_connect_directly(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    let before = writes(&seen).len();
    let mut old = record();
    old.game_version = "1.16.5".into();
    view.update(cx, |view, cx| {
        view.loaded(Ok((old, LauncherSettings::default())), cx);
    });
    cx.run_until_parked();
    click(cx, "server-play-0");
    assert_eq!(writes(&seen).len(), before, "too old to connect directly");

    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.game_output(vec![], true, cx);
    });
    cx.run_until_parked();
    click(cx, "server-add");
    click(cx, "server-play-0");
    assert_eq!(writes(&seen).len(), before, "no edits while the game runs");
    let _ = Section::Servers;
}
