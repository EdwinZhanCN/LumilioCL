use super::*;
use gpui::Modifiers;
use std::cell::RefCell;

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

type Seen = Rc<RefCell<Vec<ServerIntent>>>;

fn rows() -> Vec<ServerRow> {
    vec![
        ServerRow {
            url: "https://littleskin.cn/api/yggdrasil/".into(),
            name: "LittleSkin".into(),
            builtin: true,
            accounts: 1,
        },
        ServerRow {
            url: "https://auth.example/api/".into(),
            name: "Example".into(),
            builtin: false,
            accounts: 2,
        },
    ]
}

fn open(
    cx: &mut gpui::TestAppContext,
) -> (Entity<ServersDialog>, &mut gpui::VisualTestContext, Seen) {
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
            ServersDialog::new(
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
                rows(),
                window,
                cx,
            )
        })
    });
    cx.update(|window, cx| ServersDialog::open(dialog.clone(), window, cx));
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

fn type_address(cx: &mut gpui::VisualTestContext, dialog: &Entity<ServersDialog>, text: &str) {
    let address = dialog.read_with(cx, |d, _| d.address.clone());
    cx.update(|window, cx| address.update(cx, |input, cx| input.set_value(text, window, cx)));
    cx.run_until_parked();
}

fn found(url: &str, name: Option<&str>) -> AuthServer {
    AuthServer {
        url: url.into(),
        name: name.map(Into::into),
        non_email_login: false,
        links: Default::default(),
    }
}

#[gpui::test]
fn the_built_in_server_cannot_be_removed_and_another_asks_first(cx: &mut gpui::TestAppContext) {
    let (_, cx, seen) = open(cx);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("server-remove-0").is_none(),
        "LittleSkin stays"
    );
    assert!(cx.debug_bounds("server-remove-1").is_some());
    click(cx, "server-remove-1");
    assert!(seen.borrow().is_empty(), "removing only asks, in an alert");
}

#[gpui::test]
fn an_address_is_looked_up_and_shown_before_it_is_kept(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, seen) = open(cx);
    // Nothing to look up while the box is empty.
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click(cx, "server-find");
    assert!(seen.borrow().is_empty());

    type_address(cx, &dialog, "  skins.example/api ");
    click(cx, "server-find");
    assert_eq!(
        seen.borrow().as_slice(),
        [ServerIntent::Locate("skins.example/api".into())]
    );
    click(cx, "server-find");
    assert_eq!(seen.borrow().len(), 1, "asked once while waiting");

    // The server answers: its name shows, an http address warns, and only then can it be kept.
    dialog.update(cx, |d, cx| {
        d.located(
            Ok(found("http://skins.example/api/", Some("Plain Skins"))),
            cx,
        )
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("server-found").is_some());
    assert!(cx.debug_bounds("server-http-warning").is_some());
    click(cx, "server-add");
    assert_eq!(
        seen.borrow().last(),
        Some(&ServerIntent::Add(found(
            "http://skins.example/api/",
            Some("Plain Skins")
        )))
    );
}

#[gpui::test]
fn a_failed_lookup_says_why_and_a_new_list_clears_the_form(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, _) = open(cx);
    type_address(cx, &dialog, "nowhere.example");
    click(cx, "server-find");
    dialog.update(cx, |d, cx| {
        d.located(Err(("无法连接认证服务器".into(), "refused".into())), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("server-error").is_some());
    assert!(cx.debug_bounds("server-found").is_none());

    // A server already in the list is told so instead of offering to add it again.
    type_address(cx, &dialog, "auth.example/api");
    click(cx, "server-find");
    dialog.update(cx, |d, cx| {
        d.located(Ok(found("https://auth.example/api/", Some("Example"))), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("server-add").is_none());

    let mut more = rows();
    more.push(ServerRow {
        url: "https://new.example/".into(),
        name: "New".into(),
        builtin: false,
        accounts: 0,
    });
    cx.update(|window, cx| dialog.update(cx, |d, cx| d.listed(more, window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("server-remove-2").is_some());
    assert!(cx.debug_bounds("server-found").is_none());
    assert_eq!(dialog.read_with(cx, |d, cx| d.typed(cx)), "");
}
