use std::rc::Rc;

use gpui::{App, Window};

/// Something a page asked the shell to do. Pages stay stateless: they render
/// from the shell's view state and report clicks as intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewIntent {
    LibraryTab(usize),
    ActivityTab(usize),
    /// Show or hide what this running task downloads.
    TaskDetail(u64),
    /// Choose one of a small set, keyed by (group, index).
    Choose(u8, usize),
}

pub type Emit = Rc<dyn Fn(ViewIntent, &mut Window, &mut App)>;
