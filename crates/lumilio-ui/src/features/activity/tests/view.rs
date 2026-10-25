use super::super::ActivityView;
use crate::kit::ViewIntent;

#[test]
fn a_tab_is_chosen_and_a_task_detail_opens_and_closes_by_task() {
    let mut view = ActivityView::default();
    view.apply(ViewIntent::ActivityTab(1));
    view.apply(ViewIntent::TaskDetail(4));
    view.apply(ViewIntent::TaskDetail(7));
    view.apply(ViewIntent::TaskDetail(4));
    // Another page's choice is not this page's.
    view.apply(ViewIntent::Choose(1, 2));
    assert_eq!(view.tab, 1);
    assert_eq!(view.open, [7].into());
}
