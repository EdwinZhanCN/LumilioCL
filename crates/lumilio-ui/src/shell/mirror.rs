//! The interface's copy of what core publishes: the current game and the
//! current account (plan global-context, D1). Pages read it; only the app's
//! subscription writes it, through [`LauncherShell::follow_context`].
//! Choosing a game only sends an intent, and the chip changes when core
//! says so.

use gpui::Context;
use lumilio_core::Context as CoreContext;

use super::LauncherShell;

impl LauncherShell {
    /// What core last published.
    pub fn context(&self) -> &CoreContext {
        &self.context
    }

    /// The game play and installs go into: the current game, once the
    /// library shows it (decision D3).
    pub fn install_target(&self) -> Option<&str> {
        let id = self.context.current_instance.as_deref()?;
        self.live
            .as_ref()?
            .library
            .iter()
            .any(|card| card.id == id)
            .then_some(id)
    }

    /// Whether core says any launch is on its way or any game runs.
    pub(crate) fn game_active(&self) -> bool {
        self.context.any_active()
    }

    /// Whether game `id` can be started now, beside any other game (plan
    /// concurrent-sessions): core has no active session of it, no launch
    /// moment is kept for it, and Home has not begun a launch of it that
    /// core has yet to publish.
    pub(crate) fn can_start(&self, id: &str) -> bool {
        let home_busy = self.home_game().as_deref() == Some(id) && !self.home.can_start();
        !self.context.is_active(id) && !self.moments.contains_key(id) && !home_busy
    }

    /// Whether the game Home is about can be started now.
    pub(crate) fn can_start_home(&self) -> bool {
        self.home_game()
            .or_else(|| self.context.current_instance.clone())
            .is_none_or(|id| self.can_start(&id))
    }

    /// Takes what core published. Another current game is the one Discover
    /// installs into; Discover ends browsing for a different one (D3).
    pub(crate) fn follow_context(&mut self, context: CoreContext, cx: &mut Context<Self>) {
        let moved = self.context.current_instance != context.current_instance;
        self.context = context;
        if !moved {
            return;
        }
        self.sync_discover(cx);
        cx.notify();
    }
}
