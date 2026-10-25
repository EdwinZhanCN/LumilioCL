use super::super::{PLUGIN_GROUP, SettingsChoices, TAB_GROUP};
use crate::kit::ViewIntent;

#[test]
fn the_tab_and_the_plugin_are_chosen_by_their_groups() {
    let mut choices = SettingsChoices::default();
    choices.apply(ViewIntent::Choose(TAB_GROUP, 6));
    choices.apply(ViewIntent::Choose(PLUGIN_GROUP, 3));
    // Another page's choice is not this page's.
    choices.apply(ViewIntent::Choose(1, 2));
    choices.apply(ViewIntent::ActivityTab(1));
    assert_eq!(choices, SettingsChoices { tab: 6, plugin: 3 });
}
