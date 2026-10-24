use gpui::{Context, Window};
use lumilio_core::{DiscoverPreferences, ProjectKind};

use super::model::{
    DiscoverChange, DiscoverQuery, EPILEPSY_TRIGGERS, FilterModel, SearchStatus, advanced_options,
};
use super::page::{Discover, DiscoverEvent};

impl Discover {
    /// Opens the page for one game's Content tab: it browses *for that game*,
    /// from a fresh search of that kind. The caller makes it the current game
    /// too (decision D3).
    pub fn browse_for(
        &mut self,
        game: String,
        kind: ProjectKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.browsing_for = Some(game);
        self.state.query = fresh_query(kind, &self.state.discover_prefs);
        self.state.results.clear();
        self.state.search = SearchStatus::Searching;
        self.reset_controls(window, cx);
        self.change_query(DiscoverChange::Page(0), window, cx);
    }

    /// Takes what was remembered of Discover (the advanced exclusions, the
    /// pack tab's hide-installed) into the page. A search that has not run
    /// yet starts from it.
    pub fn apply_preferences(&mut self, prefs: DiscoverPreferences, cx: &mut Context<Self>) {
        if self.state.search == SearchStatus::Idle {
            self.state.query = fresh_query(self.state.query.kind, &prefs);
        }
        self.state.discover_prefs = prefs;
        self.state_changed(cx);
    }

    /// Drops the game the page was browsing for: plain browsing again, from a
    /// fresh search. A no-op when it was not browsing for one.
    pub fn leave_browse_context(&mut self, cx: &mut Context<Self>) {
        if self.state.browsing_for.is_none() {
            return;
        }
        self.state.browsing_for = None;
        self.state.query = fresh_query(ProjectKind::Modpack, &self.state.discover_prefs);
        self.state.results.clear();
        self.state.search = SearchStatus::Idle;
        self.state_changed(cx);
    }

    /// The search box and the sort menu go back to their starting values (a
    /// new kind of content is a new search).
    fn reset_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(controls) = &self.controls {
            controls.reset(window, cx);
        }
    }

    /// Changes one thing the page remembers and tells the application.
    fn remember(&mut self, edit: impl FnOnce(&mut DiscoverPreferences), cx: &mut Context<Self>) {
        let mut prefs = self.state.discover_prefs.clone();
        edit(&mut prefs);
        if prefs == self.state.discover_prefs {
            return;
        }
        self.state.discover_prefs = prefs.clone();
        self.state_changed(cx);
        cx.emit(DiscoverEvent::Remember(prefs));
    }

    /// Applies one change to the query — with whatever is typed in the search
    /// box — and searches again.
    pub fn change_query(
        &mut self,
        change: DiscoverChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let DiscoverChange::AdvancedOpen(open) = &change {
            let open = *open;
            self.remember(|prefs| prefs.advanced_open = open, cx);
            return;
        }
        let Some(text) = self.typed_text(cx) else {
            return;
        };
        let before = self.state.query.clone();
        let saved = self.state.discover_prefs.clone();
        let mut query = before.clone();
        query.text = text;
        // The pack tab's "hide already installed" is remembered, not session state.
        let mut prefs = saved.clone();
        if let DiscoverChange::HideInstalled(on) = &change
            && query.kind == ProjectKind::Modpack
        {
            prefs.hide_installed_modpacks = *on;
        }
        let new_kind = matches!(&change, DiscoverChange::Kind(kind) if *kind != query.kind);
        let new_source =
            matches!(&change, DiscoverChange::Source(source) if *source != query.source);
        if let DiscoverChange::Source(source) = &change {
            prefs.source = Some(source.clone());
        }
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
        self.state.query = query.clone();
        self.state.search = SearchStatus::Searching;
        self.state.discover_prefs = prefs;
        if new_source {
            // Another source's categories and loaders mean other things.
            self.state.filters = FilterModel::default();
            self.state.installed.clear();
        }
        self.state_changed(cx);
        if new_source {
            self.load_filters(cx);
        }
        if new_kind {
            // A new kind of content starts its own search (the query is
            // already fresh; the boxes follow).
            self.reset_controls(window, cx);
            self.refresh_installed(cx);
        }
        if let Some(prefs) = remembered {
            cx.emit(DiscoverEvent::Remember(prefs));
        }
        self.search(&query, cx);
        if warn {
            let page = cx.entity().downgrade();
            crate::photosensitivity::warn(window, cx, move |_, cx| {
                let _ = page.update(cx, |page, cx| {
                    page.remember(|prefs| prefs.photosensitivity_warning_dismissed = true, cx)
                });
            });
        }
    }
}

/// A new search of `kind` that starts from what Discover remembers: the
/// advanced exclusions that apply to the kind.
pub(super) fn fresh_query(kind: ProjectKind, prefs: &DiscoverPreferences) -> DiscoverQuery {
    let mut query = DiscoverQuery::new(kind);
    if let Some(source) = &prefs.source {
        query.source = source.clone();
    }
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
