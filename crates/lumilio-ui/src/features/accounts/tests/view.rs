use super::super::{ACCOUNT_GROUP, AccountsView};
use super::account;
use crate::kit::ViewIntent;

#[test]
fn the_detail_shows_the_current_account_until_another_is_chosen() {
    let rows = [account("Steve", false), account("Alex", true)];
    let mut view = AccountsView::default();
    assert_eq!(view.shown_account(&rows).map(|(index, _)| index), Some(1));
    view.apply(ViewIntent::Choose(ACCOUNT_GROUP, 0));
    // Another page's choice is not this page's.
    view.apply(ViewIntent::Choose(1, 1));
    assert_eq!(view.shown_account(&rows).map(|(index, _)| index), Some(0));
    // A choice past the end of a shorter list shows the last account.
    view.apply(ViewIntent::Choose(ACCOUNT_GROUP, 5));
    assert_eq!(view.shown_account(&rows).map(|(index, _)| index), Some(1));
    assert_eq!(view.shown_account(&[]), None);
}
