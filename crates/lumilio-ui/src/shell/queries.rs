use super::{ActivitySummary, LauncherShell};
use crate::kit::ViewIntent;
use crate::live::{
    DiscoverChange, DiscoverQuery, EPILEPSY_TRIGGERS, LiveHandler, LiveIntent, LiveModel,
    SearchStatus, advanced_options,
};
use crate::pages::ViewState;
use crate::route::Route;
use gpui::{Context, Window};
use lumilio_core::{DiscoverPreferences, ProjectKind};

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
                let installing = model.installing.contains(&slot.slug);
                let tags = model.filters.game_tags().to_vec();
                slot.view.update(cx, |view, cx| {
                    view.set_installed(installed, cx);
                    view.set_installing(installing, cx);
                    view.set_game_tags(tags, cx);
                });
            }
            cx.notify();
        }
    }

    /// Opens Discover for one game's Content tab: the page browses *for that
    /// game*, from a fresh search of that kind.
    pub fn browse_for(
        &mut self,
        game: String,
        kind: ProjectKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_install_target(game.clone(), cx);
        self.select_route(Route::Discover, window, cx);
        self.update_live(
            |model| {
                model.browsing_for = Some(game);
                model.query = fresh_query(kind, &model.discover_prefs);
                model.results.clear();
                model.search = SearchStatus::Searching;
            },
            cx,
        );
        self.reset_discover_controls(window, cx);
        self.change_query(DiscoverChange::Page(0), window, cx);
    }

    /// Takes what was remembered of Discover (the advanced exclusions, the
    /// pack tab's hide-installed) into the page. A search that has not run
    /// yet starts from it.
    pub fn apply_discover_preferences(
        &mut self,
        prefs: DiscoverPreferences,
        cx: &mut Context<Self>,
    ) {
        self.update_live(
            |model| {
                if model.search == SearchStatus::Idle {
                    model.query = fresh_query(model.query.kind, &prefs);
                }
                model.discover_prefs = prefs;
            },
            cx,
        );
    }

    /// Drops the game Discover was browsing for: the page is plain browsing
    /// again, from a fresh search. A no-op when it was not browsing for one.
    pub(super) fn leave_browse_context(&mut self, cx: &mut Context<Self>) {
        if self
            .live
            .as_ref()
            .is_none_or(|model| model.browsing_for.is_none())
        {
            return;
        }
        self.update_live(
            |model| {
                model.browsing_for = None;
                model.query = fresh_query(ProjectKind::Modpack, &model.discover_prefs);
                model.results.clear();
                model.search = SearchStatus::Idle;
            },
            cx,
        );
    }

    /// The search box and the sort dropdown go back to their starting values
    /// (a new kind of content is a new search).
    fn reset_discover_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(controls) = self.live_controls.clone() else {
            return;
        };
        let (search, sort) = {
            let read = controls.read(cx);
            (read.discover_search.clone(), read.sort.clone())
        };
        search.update(cx, |input, cx| input.set_value("", window, cx));
        sort.update(cx, |select, cx| {
            select.set_selected_index(Some(gpui_component::IndexPath::default()), window, cx)
        });
    }

    /// Changes one thing Discover remembers and tells the application.
    fn remember_discover(
        &mut self,
        edit: impl FnOnce(&mut DiscoverPreferences),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(handler), Some(model)) = (self.live_handler.clone(), &self.live) else {
            return;
        };
        let mut prefs = model.discover_prefs.clone();
        edit(&mut prefs);
        if prefs == model.discover_prefs {
            return;
        }
        let saved = prefs.clone();
        self.update_live(|model| model.discover_prefs = prefs, cx);
        handler(LiveIntent::RememberDiscover(saved), window, cx);
    }

    /// Applies one change to the Discover query — with whatever is typed in
    /// the search box — and searches again.
    pub fn change_query(
        &mut self,
        change: DiscoverChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let DiscoverChange::AdvancedOpen(open) = &change {
            let open = *open;
            self.remember_discover(|prefs| prefs.advanced_open = open, window, cx);
            return;
        }
        let (Some(controls), Some(handler), Some(model)) =
            (&self.live_controls, self.live_handler.clone(), &self.live)
        else {
            return;
        };
        let before = model.query.clone();
        let saved = model.discover_prefs.clone();
        let mut query = before.clone();
        query.text = controls
            .read(cx)
            .discover_search
            .read(cx)
            .value()
            .to_string();
        // The pack tab's "hide already installed" is remembered, not session state.
        let mut prefs = saved.clone();
        if let DiscoverChange::HideInstalled(on) = &change
            && query.kind == ProjectKind::Modpack
        {
            prefs.hide_installed_modpacks = *on;
        }
        let new_kind = matches!(&change, DiscoverChange::Kind(kind) if *kind != query.kind);
        let query = query.apply(change);
        // Advanced exclusions are remembered for the next visit.
        prefs.advanced_exclusions =
            merge_advanced(&saved.advanced_exclusions, &query.advanced, query.kind);
        let warn = query.advanced.iter().any(|id| id == EPILEPSY_TRIGGERS)
            && !before.advanced.iter().any(|id| id == EPILEPSY_TRIGGERS)
            && !saved
                .advanced_exclusions
                .iter()
                .any(|id| id == EPILEPSY_TRIGGERS)
            && !saved.photosensitivity_warning_dismissed;
        let remembered = (prefs != saved).then(|| prefs.clone());
        self.update_live(
            |model| {
                model.query = query.clone();
                model.search = SearchStatus::Searching;
                model.discover_prefs = prefs;
            },
            cx,
        );
        if new_kind {
            // A new kind of content starts its own search (the query is
            // already fresh; the boxes follow).
            self.reset_discover_controls(window, cx);
        }
        if let Some(prefs) = remembered {
            handler(LiveIntent::RememberDiscover(prefs), window, cx);
        }
        let query = self
            .live
            .as_ref()
            .map_or(query, |model| model.query.clone());
        handler(LiveIntent::Search(query), window, cx);
        if warn {
            let shell = cx.entity().downgrade();
            crate::photosensitivity::warn(window, cx, move |window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.remember_discover(
                        |prefs| prefs.photosensitivity_warning_dismissed = true,
                        window,
                        cx,
                    )
                });
            });
        }
    }

    /// Whether the search box is empty while the list still shows a search for
    /// words (the clear button was pressed).
    pub(super) fn discover_text_was_cleared(&self, cx: &Context<Self>) -> bool {
        let (Some(controls), Some(model)) = (&self.live_controls, &self.live) else {
            return false;
        };
        !model.query.text.is_empty()
            && controls
                .read(cx)
                .discover_search
                .read(cx)
                .value()
                .is_empty()
    }

    /// Enter in the search box.
    pub(super) fn submit_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Same filters, new words: back to the first page.
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

/// A new search of `kind` that starts from what Discover remembers: the
/// advanced exclusions that apply to the kind.
pub(super) fn fresh_query(kind: ProjectKind, prefs: &DiscoverPreferences) -> DiscoverQuery {
    let mut query = DiscoverQuery::new(kind);
    query.advanced = prefs
        .advanced_exclusions
        .iter()
        .filter(|id| {
            advanced_options(kind)
                .iter()
                .any(|option| option.id == **id)
        })
        .cloned()
        .collect();
    query
}

/// What is remembered of the advanced exclusions after a change: what the
/// kind does not offer stays as it was, the rest follows the selection.
fn merge_advanced(saved: &[String], selected: &[String], kind: ProjectKind) -> Vec<String> {
    let offered = advanced_options(kind);
    let is_offered = |id: &String| offered.iter().any(|option| option.id == id);
    let mut next: Vec<String> = saved
        .iter()
        .filter(|id| !is_offered(id) || selected.contains(id))
        .cloned()
        .collect();
    for id in selected {
        if !next.contains(id) {
            next.push(id.clone());
        }
    }
    next.sort();
    next
}
