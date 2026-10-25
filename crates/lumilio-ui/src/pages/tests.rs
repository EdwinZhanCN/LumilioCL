use super::*;

#[test]
fn tabs_and_choices_are_remembered_separately() {
    let mut state = ViewState::default();
    assert_eq!(state.choice(1, 7), 7, "an unchosen group uses its default");
    state.apply(ViewIntent::Choose(1, 2));
    state.apply(ViewIntent::ActivityTab(1));
    assert_eq!(state.choice(1, 7), 2);
    assert_eq!(state.activity_tab, 1);
}

#[test]
fn a_task_detail_opens_and_closes_by_task() {
    let mut state = ViewState::default();
    state.apply(ViewIntent::TaskDetail(4));
    state.apply(ViewIntent::TaskDetail(7));
    state.apply(ViewIntent::TaskDetail(4));
    assert_eq!(state.activity_open, [7].into());
}
