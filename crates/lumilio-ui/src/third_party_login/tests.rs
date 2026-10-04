use super::*;
use gpui::Modifiers;
use std::cell::RefCell;

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn server(url: &str, name: &str, non_email: bool) -> AuthServer {
    AuthServer {
        url: url.into(),
        name: Some(name.into()),
        non_email_login: non_email,
        links: Default::default(),
    }
}

type Seen = Rc<RefCell<Vec<ThirdPartyIntent>>>;

fn open(
    cx: &mut gpui::TestAppContext,
    servers: Vec<AuthServer>,
) -> (Entity<ThirdPartyDialog>, &mut gpui::VisualTestContext, Seen) {
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
            ThirdPartyDialog::new(
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
                servers,
                window,
                cx,
            )
        })
    });
    cx.update(|window, cx| ThirdPartyDialog::open(dialog.clone(), window, cx));
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

fn type_into(
    cx: &mut gpui::VisualTestContext,
    dialog: &Entity<ThirdPartyDialog>,
    login: &str,
    password: &str,
) {
    let (login_box, password_box) = dialog.read_with(cx, |dialog, _| {
        (dialog.login.clone(), dialog.password.clone())
    });
    cx.update(|window, cx| {
        login_box.update(cx, |input, cx| input.set_value(login, window, cx));
        password_box.update(cx, |input, cx| input.set_value(password, window, cx));
    });
    cx.run_until_parked();
}

const LITTLE: &str = "https://littleskin.cn/api/yggdrasil/";
const OTHER: &str = "https://auth.example/api/";

#[gpui::test]
fn signing_in_needs_both_fields_and_asks_the_chosen_server_once(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, seen) = open(
        cx,
        vec![
            server(LITTLE, "LittleSkin", false),
            server(OTHER, "Example", true),
        ],
    );
    assert_eq!(dialog.read_with(cx, |d, _| d.login_label()), "邮箱");

    // Nothing to send while a field is empty.
    type_into(cx, &dialog, "me@example.com", "");
    assert!(dialog.read_with(cx, |d, cx| d.request(cx).is_none()));
    type_into(cx, &dialog, "  me@example.com ", "pw");
    click(cx, "tp-go");
    click(cx, "tp-go");
    assert_eq!(
        seen.borrow().as_slice(),
        [ThirdPartyIntent::SignIn {
            server: LITTLE.into(),
            login: "me@example.com".into(),
            password: "pw".into(),
        }],
        "pressing twice asks once, and the name is trimmed"
    );

    // A refusal returns to the form with what was typed and a reason.
    dialog.update(cx, |d, cx| {
        d.failed(("用户名或密码错误".into(), "InvalidCredentials".into()), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("tp-error").is_some());
    assert_eq!(
        dialog.read_with(cx, |d, cx| d.login.read(cx).value().to_string()),
        "  me@example.com "
    );

    // The other server: its own address and its own name for the first box.
    click(cx, "tp-server-1");
    assert_eq!(dialog.read_with(cx, |d, _| d.login_label()), "用户名");
    click(cx, "tp-go");
    let Some(ThirdPartyIntent::SignIn { server, .. }) = seen.borrow().last().cloned() else {
        panic!("expected a sign-in");
    };
    assert_eq!(server, OTHER);
}

#[gpui::test]
fn an_http_server_warns_and_adding_a_server_leaves_for_the_list(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, seen) = open(
        cx,
        vec![
            server(LITTLE, "LittleSkin", false),
            server("http://plain.example/api/", "Plain", true),
        ],
    );
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("tp-http-warning").is_none());
    click(cx, "tp-server-1");
    assert!(cx.debug_bounds("tp-http-warning").is_some());
    click(cx, "tp-manage");
    assert_eq!(seen.borrow().as_slice(), [ThirdPartyIntent::ManageServers]);
    let _ = dialog;
}

#[gpui::test]
fn a_choice_of_characters_is_made_once_and_leaving_abandons_it(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, seen) = open(cx, vec![server(LITTLE, "LittleSkin", false)]);
    type_into(cx, &dialog, "me", "pw");
    click(cx, "tp-go");
    let first = ProfileId::parse("123e4567e89b12d3a456426614174000").unwrap();
    let second = ProfileId::parse("223e4567e89b12d3a456426614174000").unwrap();
    // An answer nobody waits for is ignored.
    dialog.update(cx, |d, cx| d.choose(7, vec![], cx));
    dialog.update(cx, |d, cx| {
        d.choose(
            7,
            vec![
                CharacterProfile {
                    id: first,
                    name: "A".into(),
                },
                CharacterProfile {
                    id: second,
                    name: "B".into(),
                },
            ],
            cx,
        )
    });
    click(cx, "tp-character-1");
    click(cx, "tp-go");
    assert_eq!(
        seen.borrow().last(),
        Some(&ThirdPartyIntent::Choose {
            pending: 7,
            character: second
        })
    );
}

#[gpui::test]
fn cancelling_a_pending_choice_abandons_it(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, seen) = open(cx, vec![server(LITTLE, "LittleSkin", false)]);
    type_into(cx, &dialog, "me", "pw");
    click(cx, "tp-go");
    let id = ProfileId::parse("123e4567e89b12d3a456426614174000").unwrap();
    dialog.update(cx, |d, cx| {
        d.choose(
            9,
            vec![
                CharacterProfile {
                    id,
                    name: "A".into(),
                },
                CharacterProfile {
                    id,
                    name: "B".into(),
                },
            ],
            cx,
        )
    });
    click(cx, "tp-cancel");
    assert_eq!(seen.borrow().last(), Some(&ThirdPartyIntent::Abandon(9)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}

#[gpui::test]
fn a_finished_sign_in_closes_the_dialog(cx: &mut gpui::TestAppContext) {
    let (dialog, cx, _) = open(cx, vec![server(LITTLE, "LittleSkin", false)]);
    dialog.update(cx, |d, cx| d.signed_in(cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}
