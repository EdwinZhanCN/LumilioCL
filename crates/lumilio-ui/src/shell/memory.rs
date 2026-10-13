//! What the launcher lets go of while a game runs.

use super::LauncherShell;
use super::locations::Location;
use gpui::Context;

impl LauncherShell {
    /// A game has started and has better use for the memory. The launcher
    /// keeps the page on screen and lets go of what it holds only in case it
    /// is needed again: game and project pages kept for Back (the landmark
    /// pages stay in history), and pictures and covers not on screen.
    pub(super) fn release_for_game(&mut self, cx: &mut Context<Self>) {
        let here = self.location();
        self.history
            .forget(|location| !matches!(location, Location::Page(_)));
        self.history.collapse(&here, Location::same);
        crate::pictures::release_unseen(cx);
        crate::cover::release(cx);
        cx.notify();
    }
}
