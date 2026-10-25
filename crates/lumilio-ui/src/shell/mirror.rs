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

    /// Whether core says a launch is on its way or a game runs.
    pub(crate) fn game_active(&self) -> bool {
        self.context
            .session
            .as_ref()
            .is_some_and(lumilio_core::Session::is_active)
    }

    /// Whether a game can be started now: no session is active, and Home
    /// has not begun a launch core has yet to publish.
    pub(crate) fn can_start(&self) -> bool {
        !self.game_active() && self.home.can_start()
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
