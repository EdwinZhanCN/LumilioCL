use gpui::{Context, Entity, Modifiers, Render, TestAppContext, Window, div, prelude::*, px};
use gpui_component::{
    IndexPath, Sizable as _,
    searchable_list::SearchableListItem,
    select::{SearchableVec, Select, SelectState},
};

#[derive(Clone)]
struct Choice(String);

impl SearchableListItem for Choice {
    type Value = String;
    fn title(&self) -> gpui::SharedString {
        self.0.clone().into()
    }
    fn value(&self) -> &String {
        &self.0
    }
    fn render(&self, _: &mut Window, _: &mut gpui::App) -> impl IntoElement {
        let name = self.0.clone();
        div()
            .debug_selector(move || format!("choice-{name}"))
            .child(self.0.clone())
    }
}

struct Harness(Entity<SelectState<SearchableVec<Choice>>>);
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(
            div()
                .w(px(240.))
                .debug_selector(|| "select-trigger".into())
                .child(Select::new(&self.0).small().placeholder("Nothing selected")),
        )
    }
}

fn open(
    multiple: bool,
    cx: &mut TestAppContext,
) -> (Entity<Harness>, &mut gpui::VisualTestContext) {
    cx.update(gpui_component::init);
    cx.add_window_view(|window, cx| {
        let state = cx.new(|cx| {
            let items = SearchableVec::new(
                ["Error", "Warn", "Info"]
                    .map(|label| Choice(label.into()))
                    .to_vec(),
            );
            if multiple {
                SelectState::new_multiple(items, vec![IndexPath::new(0)], window, cx)
            } else {
                SelectState::new(items, Some(IndexPath::new(0)), window, cx)
            }
        });
        Harness(state)
    })
}

fn click(cx: &mut gpui::VisualTestContext, name: &'static str) {
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(name)
        .unwrap_or_else(|| panic!("missing {name}"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn multi_select_toggles_independently_and_escape_preserves_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(true, cx);
    click(cx, "select-trigger");
    click(cx, "choice-Warn");
    view.read_with(cx, |view, cx| {
        assert_eq!(view.0.read(cx).selected_values(), vec!["Error", "Warn"]);
        assert!(view.0.read(cx).is_open());
    });
    click(cx, "choice-Error");
    view.read_with(cx, |view, cx| {
        assert_eq!(view.0.read(cx).selected_values(), vec!["Warn"])
    });
    cx.simulate_keystrokes("down down enter");
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        assert_eq!(view.0.read(cx).selected_values(), vec!["Warn", "Info"]);
        assert!(view.0.read(cx).is_open());
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        assert_eq!(view.0.read(cx).selected_values(), vec!["Warn", "Info"]);
        assert!(!view.0.read(cx).is_open());
    });
}

#[gpui::test]
fn single_select_still_replaces_and_closes(cx: &mut TestAppContext) {
    let (view, cx) = open(false, cx);
    click(cx, "select-trigger");
    click(cx, "choice-Warn");
    view.read_with(cx, |view, cx| {
        assert_eq!(view.0.read(cx).selected_values(), vec!["Warn"]);
        assert!(!view.0.read(cx).is_open());
    });
}

#[gpui::test]
fn empty_multi_selection_can_be_reselected_and_outside_click_preserves_it(cx: &mut TestAppContext) {
    let (view, cx) = open(true, cx);
    cx.update(|window, cx| {
        let state = view.read(cx).0.clone();
        state.update(cx, |state, cx| state.set_selected_values(&[], window, cx));
    });
    click(cx, "select-trigger");
    click(cx, "choice-Warn");
    click(cx, "choice-Warn");
    view.read_with(cx, |view, cx| {
        assert!(view.0.read(cx).selected_values().is_empty());
        assert!(view.0.read(cx).is_open());
    });
    click(cx, "choice-Info");
    cx.simulate_click(gpui::point(px(400.), px(300.)), Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        assert_eq!(view.0.read(cx).selected_values(), vec!["Info"]);
        assert!(!view.0.read(cx).is_open());
    });
}
