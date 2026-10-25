use std::collections::HashMap;

use crate::kit::ViewIntent;

/// The small choices made on the global pages. The Library, Activity and
/// Settings keep their own (`features::library::LibraryView`,
/// `features::activity::ActivityView`, `features::settings::SettingsChoices`).
#[derive(Clone, Debug, Default)]
pub struct ViewState {
    pub(super) choices: HashMap<u8, usize>,
}

impl ViewState {
    pub fn choice(&self, group: u8, default: usize) -> usize {
        self.choices.get(&group).copied().unwrap_or(default)
    }

    /// Applies an intent to the view state.
    pub fn apply(&mut self, intent: ViewIntent) {
        match intent {
            ViewIntent::LibraryTab(_) | ViewIntent::ActivityTab(_) | ViewIntent::TaskDetail(_) => {}
            ViewIntent::Choose(group, index) => {
                self.choices.insert(group, index);
            }
        }
    }
}
