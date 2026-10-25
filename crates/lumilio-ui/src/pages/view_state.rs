use std::collections::{BTreeSet, HashMap};

use crate::kit::ViewIntent;

/// Which tab each global page is on, and the small choices made on them. The
/// Library keeps its own (`features::library::LibraryView`).
#[derive(Clone, Debug, Default)]
pub struct ViewState {
    pub activity_tab: usize,
    /// Running tasks whose downloads Activity shows, by task id.
    pub activity_open: BTreeSet<u64>,
    pub(super) choices: HashMap<u8, usize>,
}

impl ViewState {
    pub fn choice(&self, group: u8, default: usize) -> usize {
        self.choices.get(&group).copied().unwrap_or(default)
    }

    /// Applies an intent to the view state.
    pub fn apply(&mut self, intent: ViewIntent) {
        match intent {
            ViewIntent::LibraryTab(_) => {}
            ViewIntent::ActivityTab(index) => self.activity_tab = index,
            ViewIntent::TaskDetail(task) => {
                if !self.activity_open.remove(&task) {
                    self.activity_open.insert(task);
                }
            }
            ViewIntent::Choose(group, index) => {
                self.choices.insert(group, index);
            }
        }
    }
}
