use gpui::prelude::*;
use gpui::{Context, Window};

use super::LauncherShell;
use crate::pages::live::LiveControls;

impl LauncherShell {
    /// Builds the controls the live pages own the first time they show.
    pub(super) fn ensure_live_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.live.is_some() && self.live_controls.is_none() {
            self.live_controls = Some(cx.new(|cx| LiveControls::new(window, cx)));
        }
    }
}
