use gpui::{Context, Window};
use lumilio_core::{DiscoverPreferences, ProjectKind};

use super::{ActivitySummary, LauncherShell};
use crate::features::library::LibraryInputs;
use crate::kit::ViewIntent;
use crate::live::{LiveHandler, LiveModel};
use crate::pages::ViewState;
use crate::route::Route;

impl LauncherShell {
    /// Turns on the live Library, Discover and Activity pages. The handler
    /// receives every [`crate::live::LiveIntent`] no feature of the shell answers.
    pub fn with_live(mut self, handler: LiveHandler) -> Self {
        self.live = Some(LiveModel::default());
        self.live_handler = Some(super::features::route(&self, handler));
        self
    }

    pub fn live(&self) -> Option<&LiveModel> {
        self.live.as_ref()
    }

    /// Changes the live model and redraws. Ignored when live pages are off.
    pub fn update_live(&mut self, change: impl FnOnce(&mut LiveModel), cx: &mut Context<Self>) {
        if let Some(model) = &mut self.live {
            change(model);
            self.activity = ActivitySummary::new(model.active_tasks());
            let inputs = LibraryInputs {
                library: model.library.clone(),
                library_loaded: model.library_loaded,
                collections: model.collections.clone(),
                settings_presets: model.settings_presets.clone(),
                presets_error: model.presets_error.clone(),
                game_settings: model.game_settings.clone(),
            };
            self.sync_discover(cx);
            self.library
                .update(cx, |library, cx| library.set_inputs(inputs, cx));
            cx.notify();
        }
    }

    /// Gives Discover what it reads from the rest of the launcher: the
    /// library, the game installs go into and whether translation is on.
    pub(super) fn sync_discover(&mut self, cx: &mut Context<Self>) {
        let Some(model) = &self.live else {
            return;
        };
        let library = model.library.clone();
        let translation = self.translation;
        let target = self.install_target().map(str::to_owned);
        self.discover.update(cx, |discover, cx| {
            discover.set_inputs(library, target, translation, cx)
        });
    }

    /// Opens Discover for one game's Content tab: the page browses *for that
    /// game*, from a fresh search of that kind. The caller makes it the
    /// current game too (decision D3).
    pub fn browse_for(
        &mut self,
        game: String,
        kind: ProjectKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_route(Route::Discover, window, cx);
        self.discover.update(cx, |discover, cx| {
            discover.browse_for(game, kind, window, cx)
        });
    }

    /// Whether translation is on, as the saved preferences say; Discover
    /// follows it.
    pub fn set_translation(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.translation != on {
            self.translation = on;
            self.sync_discover(cx);
        }
    }

    /// Takes what was remembered of Discover into the page.
    pub fn apply_discover_preferences(
        &mut self,
        prefs: DiscoverPreferences,
        cx: &mut Context<Self>,
    ) {
        self.discover
            .update(cx, |discover, cx| discover.apply_preferences(prefs, cx));
    }

    pub fn view(&self) -> &ViewState {
        &self.view
    }

    /// Applies a page intent to the view state only.
    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.view.apply(intent);
        cx.notify();
    }
}
