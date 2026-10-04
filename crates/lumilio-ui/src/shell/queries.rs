use super::{ActivitySummary, LauncherShell};
use crate::kit::ViewIntent;
use crate::live::{DiscoverChange, LiveHandler, LiveIntent, LiveModel, SearchStatus};
use crate::pages::ViewState;
use crate::route::Route;
use gpui::{Context, Window};
use lumilio_core::ProjectKind;

impl LauncherShell {
    /// Turns on the live Library, Discover and Activity pages. The handler
    /// receives every [`LiveIntent`].
    pub fn with_live(mut self, handler: LiveHandler) -> Self {
        self.live = Some(LiveModel::default());
        self.live_handler = Some(handler);
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
            if let Some(slot) = &self.detail {
                let installed = model.installed.clone();
                slot.view
                    .update(cx, |view, cx| view.set_installed(installed, cx));
            }
            cx.notify();
        }
    }

    /// Applies one change to the Discover query — with whatever is typed in
    /// the search box — and searches again.
    /// Opens Discover on one kind of content (from a game's Content tab).
    pub fn browse(&mut self, kind: ProjectKind, window: &mut Window, cx: &mut Context<Self>) {
        self.select_route(Route::Discover, window, cx);
        self.change_query(DiscoverChange::Kind(kind), window, cx);
    }

    pub fn change_query(
        &mut self,
        change: DiscoverChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(controls), Some(handler), Some(model)) =
            (&self.live_controls, self.live_handler.clone(), &self.live)
        else {
            return;
        };
        let mut query = model.query.clone();
        query.text = controls
            .read(cx)
            .discover_search
            .read(cx)
            .value()
            .to_string();
        let query = query.apply(change);
        self.update_live(
            |model| {
                model.query = query.clone();
                model.search = SearchStatus::Searching;
            },
            cx,
        );
        handler(LiveIntent::Search(query), window, cx);
    }

    /// Enter in the search box.
    pub(super) fn submit_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.live.as_ref().map_or(0, |model| model.query.page);
        // Same filters, new words: back to the first page.
        let _ = page;
        self.change_query(DiscoverChange::Page(0), window, cx);
    }

    pub fn view(&self) -> &ViewState {
        &self.view
    }

    /// Applies a page intent to the view state only.
    /// The Library's ordering and loader filter outlive this run: tell the
    /// application when either changes.
    pub(super) fn remember_library_view(
        &mut self,
        intent: ViewIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::pages::live::{LIBRARY_LOADER, LIBRARY_SORT};
        if !matches!(intent, ViewIntent::Choose(LIBRARY_SORT | LIBRARY_LOADER, _)) {
            return;
        }
        if let Some(handler) = self.live_handler.clone() {
            let (sort, loader) = (
                self.view.choice(LIBRARY_SORT, 0),
                self.view.choice(LIBRARY_LOADER, 0),
            );
            handler(
                LiveIntent::RememberLibraryView {
                    sort: u8::try_from(sort).unwrap_or(0),
                    loader: u8::try_from(loader).unwrap_or(0),
                },
                window,
                cx,
            );
        }
    }

    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.view.apply(intent);
        cx.notify();
    }
}
