use super::*;
use std::cell::RefCell;
use std::task::{Poll, Waker};

struct Host;
impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

#[derive(Default)]
struct Reply {
    result: Option<Result<(), String>>,
    wake: Option<Waker>,
}

#[gpui::test]
fn save_failure_keeps_the_draft_and_repeat_submit_waits_for_success(cx: &mut gpui::TestAppContext) {
    use gpui::Modifiers;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen = Rc::new(RefCell::new(Vec::new()));
    let reply = Rc::new(RefCell::new(Reply::default()));
    let saved = Rc::new(RefCell::new(0));
    let form = cx.update(|window, cx| {
        let (sink, reply, saved) = (seen.clone(), reply.clone(), saved.clone());
        PluginSettingDialog::open(
            SettingField {
                key: "text".into(),
                label: lumilio_plugin_api::Words::new("名称", "Name"),
                help: lumilio_plugin_api::Words::default(),
                kind: SettingKind::Text {
                    default: String::new(),
                },
            },
            SettingValue::Text("draft".into()),
            Rc::new(move |value| {
                sink.borrow_mut().push(value);
                let reply = reply.clone();
                Box::pin(std::future::poll_fn(move |cx| {
                    let mut reply = reply.borrow_mut();
                    if let Some(result) = reply.result.take() {
                        Poll::Ready(result)
                    } else {
                        reply.wake = Some(cx.waker().clone());
                        Poll::Pending
                    }
                }))
            }),
            Rc::new(move |_| *saved.borrow_mut() += 1),
            window,
            cx,
        )
        .unwrap()
    });
    cx.run_until_parked();
    let save = cx.debug_bounds("plugin-setting-save").unwrap();
    cx.simulate_click(save.center(), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_click(save.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(seen.borrow().len(), 1, "no duplicate while saving");
    assert!(cx.debug_bounds("dialog-layer").is_some());
    {
        let mut pending = reply.borrow_mut();
        pending.result = Some(Err("保存失败".into()));
        pending.wake.take().unwrap().wake();
    }
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(form.read(cx).input.read(cx).value().as_ref(), "draft");
    });
    assert!(cx.debug_bounds("plugin-setting-error").is_some());
    let save = cx.debug_bounds("plugin-setting-save").unwrap();
    cx.simulate_click(save.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(seen.borrow().len(), 2);
    {
        let mut pending = reply.borrow_mut();
        pending.result = Some(Ok(()));
        pending.wake.take().unwrap().wake();
    }
    cx.run_until_parked();
    assert_eq!(*saved.borrow(), 1);
    assert!(cx.debug_bounds("dialog-layer").is_none());
}

#[test]
fn numbers_are_bounded_and_text_preserves_the_draft() {
    let kind = SettingKind::Number {
        min: 1,
        max: 10,
        default: 5,
    };
    for text in ["", "1.5", "11", "0", "9223372036854775808"] {
        assert!(parse(&kind, text).is_err());
    }
    assert_eq!(parse(&kind, " 10 "), Ok(SettingValue::Number(10)));
    assert_eq!(
        parse(
            &SettingKind::Text {
                default: String::new()
            },
            " 原文 "
        ),
        Ok(SettingValue::Text(" 原文 ".into()))
    );
}
