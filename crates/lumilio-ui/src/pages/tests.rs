use super::*;

#[test]
fn a_choice_is_remembered_by_its_group() {
    let mut state = ViewState::default();
    assert_eq!(state.choice(1, 7), 7, "an unchosen group uses its default");
    state.apply(ViewIntent::Choose(1, 2));
    state.apply(ViewIntent::ActivityTab(1));
    assert_eq!(state.choice(1, 7), 2);
    assert_eq!(state.choice(2, 7), 7, "another group keeps its default");
}
