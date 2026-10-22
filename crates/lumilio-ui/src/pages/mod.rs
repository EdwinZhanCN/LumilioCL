//! Page state shared by the live pages: which tab each page is on and the
//! small choices (sort, filter, settings tab) the person made.

use std::collections::{BTreeSet, HashMap};

use crate::kit::ViewIntent;

pub mod live;
pub mod settings;

/// Which tab each global page is on, and the small choices made on them.
#[derive(Clone, Debug, Default)]
pub struct ViewState {
    pub library_tab: usize,
    pub activity_tab: usize,
    /// Running tasks whose downloads Activity shows, by task id.
    pub activity_open: BTreeSet<u64>,
    choices: HashMap<u8, usize>,
}

impl ViewState {
    pub fn choice(&self, group: u8, default: usize) -> usize {
        self.choices.get(&group).copied().unwrap_or(default)
    }

    /// Applies an intent to the view state.
    pub fn apply(&mut self, intent: ViewIntent) {
        match intent {
            ViewIntent::LibraryTab(index) => self.library_tab = index,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_and_choices_are_remembered_separately() {
        let mut state = ViewState::default();
        assert_eq!(state.choice(1, 7), 7, "an unchosen group uses its default");
        state.apply(ViewIntent::Choose(1, 2));
        state.apply(ViewIntent::LibraryTab(3));
        assert_eq!(state.choice(1, 7), 2);
        assert_eq!((state.library_tab, state.activity_tab), (3, 0));
    }

    #[test]
    fn a_task_detail_opens_and_closes_by_task() {
        let mut state = ViewState::default();
        state.apply(ViewIntent::TaskDetail(4));
        state.apply(ViewIntent::TaskDetail(7));
        state.apply(ViewIntent::TaskDetail(4));
        assert_eq!(state.activity_open, [7].into());
    }
}
