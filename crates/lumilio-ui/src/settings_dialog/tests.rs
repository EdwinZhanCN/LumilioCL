use super::*;
use crate::live::LiveIntent;
use crate::settings_forms as forms;
use std::cell::RefCell;

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

impl<I: Clone + 'static> SettingsDialog<I> {
    fn set_field(&mut self, index: usize, text: &str, window: &mut Window, cx: &mut App) {
        match &self.inputs[index] {
            Field::Line(state) => {
                state.update(cx, |state, cx| state.set_value(text.to_owned(), window, cx));
            }
            Field::Lines(state) => {
                state.update(cx, |state, cx| state.set_value(text.to_owned(), window, cx));
            }
            Field::Choice(_) => {}
        }
    }
}

fn memory_spec() -> DialogSpec<LiveIntent> {
    DialogSpec {
        title: "默认内存",
        intro: None,
        fields: ["最小", "最大"]
            .into_iter()
            .map(|label| FieldSpec {
                label,
                help: None,
                placeholder: "",
                value: String::new(),
                kind: FieldKind::Line,
            })
            .collect(),
        parse: Rc::new(|values| forms::memory(&values[0], &values[1])),
        reset: Some(LiveIntent::SetMemory {
            min_mb: None,
            max_mb: None,
        }),
    }
}

#[gpui::test]
fn a_refused_value_stays_in_the_dialog_and_a_good_one_is_sent_once_and_closes(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::Modifiers;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let handler: Handler<LiveIntent> = Rc::new(move |intent, _, _| sink.borrow_mut().push(intent));
    let form = cx.update(|window, cx| {
        cx.new(|cx| SettingsDialog::<LiveIntent>::new(memory_spec(), handler.clone(), window, cx))
    });
    cx.update(|window, cx| {
        SettingsDialog::show(
            form.clone(),
            "默认内存",
            memory_spec().reset,
            handler.clone(),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    let set = |cx: &mut gpui::VisualTestContext, index: usize, text: &str| {
        cx.update(|window, cx| form.update(cx, |form, cx| form.set_field(index, text, window, cx)));
    };

    // Minimum above maximum: told why, nothing sent, still open.
    set(cx, 0, "8192");
    set(cx, 1, "4096");
    let save = cx.debug_bounds("settings-save").expect("dialog on screen");
    cx.simulate_click(save.center(), Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(seen.borrow().is_empty());
    assert!(cx.debug_bounds("settings-error").is_some());
    assert!(cx.debug_bounds("dialog-layer").is_some(), "still open");

    // Corrected: sent once, dialog closes.
    set(cx, 0, "");
    let save = cx.debug_bounds("settings-save").unwrap();
    cx.simulate_click(save.center(), Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SetMemory {
            min_mb: None,
            max_mb: Some(4096)
        }]
    );
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}

#[gpui::test]
fn restore_defaults_sends_the_reset_and_closes(cx: &mut gpui::TestAppContext) {
    use gpui::Modifiers;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let handler: Handler<LiveIntent> = Rc::new(move |intent, _, _| sink.borrow_mut().push(intent));
    cx.update(|window, cx| SettingsDialog::open(memory_spec(), handler, window, cx));
    cx.run_until_parked();
    let reset = cx.debug_bounds("settings-reset").expect("restore button");
    cx.simulate_click(reset.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SetMemory {
            min_mb: None,
            max_mb: None
        }]
    );
}
