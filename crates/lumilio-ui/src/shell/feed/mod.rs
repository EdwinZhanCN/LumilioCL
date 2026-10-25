//! Keeps the pages fed from the launcher: the lists (library, Activity,
//! accounts, game settings), read again when a feature says they changed,
//! and what follows core's context (Home, 需要留意, its worlds and servers,
//! the accounts and plugins).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::WeakEntity;

use super::LauncherShell;
use crate::runtime::Worker;

pub(super) use context::{check_again, follow};
pub(super) use lists::reload;

mod context;
mod hooks;
mod lists;

#[cfg(test)]
mod tests;

/// How many finished tasks the Activity page lists.
const ACTIVITY_LIMIT: usize = 30;

/// What feeding the pages needs: the shell to write into, the launcher to
/// ask, and what earlier answers left behind.
#[derive(Clone)]
pub(super) struct Feed {
    shell: WeakEntity<LauncherShell>,
    worker: Worker,
    state: Rc<RefCell<FeedState>>,
}

impl Feed {
    pub(super) fn new(shell: WeakEntity<LauncherShell>, worker: Worker) -> Self {
        Self {
            shell,
            worker,
            state: Rc::default(),
        }
    }
}

#[derive(Default)]
struct FeedState {
    /// An older list snapshot cannot replace a newer request.
    reload_seq: u64,
    /// The background checks of Home's games now running.
    home_diagnostics: Option<tokio::task::AbortHandle>,
    /// Each game's last checks, for Home's 需要留意.
    home_checks: Option<context::Checked>,
}
