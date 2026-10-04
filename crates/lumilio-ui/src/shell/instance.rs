use super::LauncherShell;
use super::locations::Location;
use crate::instance_detail::{InstanceDetailView, InstanceHandler};
use crate::route::Route;
use gpui::prelude::*;
use gpui::{App, Context, Entity};

impl LauncherShell {
    /// Opens a fresh, stable-ID view. Async results update this entity only.
    pub fn open_live_instance(
        &mut self,
        id: String,
        handler: impl FnOnce(gpui::WeakEntity<InstanceDetailView>) -> InstanceHandler,
        cx: &mut Context<Self>,
    ) -> Entity<InstanceDetailView> {
        let view = cx.new(|cx| InstanceDetailView::new(id, handler(cx.weak_entity())));
        if let Some(settings) = self.live.as_ref().and_then(|model| model.settings.as_ref()) {
            let enabled = enabled_plugins(&settings.plugins);
            view.update(cx, |view, cx| view.plugins_changed(enabled, cx));
        }
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        self.visit();
        self.live_instance = Some(view.clone());
        self.route = Route::Library;
        self.detail = None;
        cx.notify();
        view
    }

    /// The game's output so far, for the open page of that game (if any).
    pub fn game_output(
        &mut self,
        id: &str,
        lines: Vec<String>,
        running: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = &self.live_instance
            && view.read(cx).id() == id
        {
            view.update(cx, |view, cx| view.game_output(lines, running, cx));
        }
    }

    pub fn live_instance(&self) -> Option<&Entity<InstanceDetailView>> {
        self.live_instance.as_ref()
    }

    /// Update retained pages as well as the visible page, so Back cannot
    /// resurrect contributions from a plugin that was just turned off.
    pub fn plugins_changed(
        &mut self,
        plugins: &[lumilio_core::PluginInfo],
        cx: &mut Context<Self>,
    ) {
        let enabled = enabled_plugins(plugins);
        let views = self
            .history
            .iter()
            .filter_map(|location| match location {
                Location::Instance(view) => Some(view.clone()),
                _ => None,
            })
            .chain(self.live_instance.clone())
            .collect::<Vec<_>>();
        for view in views {
            view.update(cx, |view, cx| view.plugins_changed(enabled.clone(), cx));
        }
    }

    /// Leaves the open instance: back where it came from, or Library.
    pub fn close_live_instance(&mut self, cx: &mut Context<Self>) {
        if self.live_instance.is_some() && !self.go_back(None, cx) {
            self.live_instance = None;
            self.route = Route::Library;
            cx.notify();
        }
    }

    /// The instance was deleted: no direction may lead to it any more, and
    /// if it is showing, the window goes back.
    pub fn forget_instance(&mut self, id: &str, cx: &mut Context<Self>) {
        let showing = self
            .live_instance
            .as_ref()
            .is_some_and(|view| view.read(cx).id() == id);
        if showing && !self.go_back(None, cx) {
            self.restore(Location::Page(Route::Library), None, cx);
        }
        let app: &App = cx;
        self.history.forget(|location| match location {
            Location::Instance(view) => view.read(app).id() == id,
            _ => false,
        });
        cx.notify();
    }
}

fn enabled_plugins(plugins: &[lumilio_core::PluginInfo]) -> Vec<String> {
    plugins
        .iter()
        .filter(|info| info.status == lumilio_core::PluginStatus::Enabled)
        .map(|info| info.manifest.id.clone())
        .collect()
}
