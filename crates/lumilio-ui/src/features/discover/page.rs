use std::collections::HashMap;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Context, Entity, EntityId, EventEmitter, WeakEntity, Window};
use gpui_component::input::InputEvent;
use gpui_component::select::SelectEvent;
use lumilio_core::{DiscoverPreferences, ProjectKind};

use super::controls::{DiscoverControls, Labels};
use super::detail::{InstallTarget, ProjectDetailView};
use super::model::{DiscoverChange, DiscoverQuery, PAGE_SIZES, SORTS, sort_label};
use super::state::DiscoverState;
use super::view::{self, DiscoverCtx};
use crate::kit::{self, Emit, ViewIntent};
use crate::live::LibraryCard;
use crate::runtime::Worker;

/// What Discover tells the windows and the application around it.
#[derive(Clone)]
pub enum DiscoverEvent {
    /// A project's detail was opened; the shell shows it and keeps it in
    /// its history.
    ProjectOpened(OpenedProject),
    /// Work Discover started changed the tasks Activity lists.
    TasksChanged,
    /// What Discover remembers between visits changed; the application
    /// saves it with the other preferences.
    Remember(DiscoverPreferences),
}

/// A project detail Discover opened.
#[derive(Clone)]
pub struct OpenedProject {
    pub view: Entity<ProjectDetailView>,
    pub kind: ProjectKind,
    /// The content source (plugin ID) the project is from.
    pub source: String,
    pub slug: String,
}

/// The small choices made on the page (which sidebar sections are open), by
/// group.
#[derive(Clone, Debug, Default)]
pub struct Choices(HashMap<u8, usize>);

impl Choices {
    pub fn choice(&self, group: u8, default: usize) -> usize {
        self.0.get(&group).copied().unwrap_or(default)
    }

    pub(super) fn apply(&mut self, intent: ViewIntent) {
        if let ViewIntent::Choose(group, index) = intent {
            self.0.insert(group, index);
        }
    }
}

/// The Discover page: its search, the details it opens and the installs it
/// starts. It reads the library and the current game from the shell and
/// asks the launcher through its [`Worker`].
pub struct Discover {
    pub(super) worker: Option<Worker>,
    pub(super) state: DiscoverState,
    pub(super) controls: Option<DiscoverControls>,
    pub(super) choices: Choices,
    /// The details this page opened that are still alive (open, or in the
    /// shell's history), with the project each shows.
    pub(super) details: Vec<(WeakEntity<ProjectDetailView>, String)>,
    /// The detail on screen, as the shell says.
    pub(super) shown_detail: Option<EntityId>,
    /// Numbers searches so a slow answer cannot replace a newer one.
    pub(super) search_seq: u64,
    /// The target game, kind and source `installed` was last read for; `None`
    /// when it is out of date and should be read again.
    pub(super) installed_key: Option<(String, ProjectKind, String)>,
    /// What the page's rows and menus asked for (tests read it).
    #[cfg(test)]
    pub(super) acted: Vec<super::view::DiscoverIntent>,
}

impl EventEmitter<DiscoverEvent> for Discover {}

impl Discover {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            state: DiscoverState::default(),
            controls: None,
            choices: Choices::default(),
            details: Vec::new(),
            shown_detail: None,
            search_seq: 0,
            installed_key: None,
            #[cfg(test)]
            acted: Vec::new(),
        }
    }

    /// Gives the page the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    #[must_use]
    pub fn state(&self) -> &DiscoverState {
        &self.state
    }

    #[must_use]
    pub fn choices(&self) -> &Choices {
        &self.choices
    }

    /// Changes the page's state directly (tests, and dev hooks that put the
    /// page in a known place).
    pub fn edit_state(&mut self, edit: impl FnOnce(&mut DiscoverState), cx: &mut Context<Self>) {
        edit(&mut self.state);
        self.state_changed(cx);
    }

    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.choices.apply(intent);
        cx.notify();
    }

    /// The library, the game installs go into and whether translation is on,
    /// as the shell knows them. Another target ends browsing for a different
    /// game, so its version and loader no longer apply, and the list is
    /// searched again without them (D3).
    pub fn set_inputs(
        &mut self,
        library: Vec<LibraryCard>,
        target: Option<String>,
        translation: bool,
        cx: &mut Context<Self>,
    ) {
        let library_changed = self.state.library != library;
        let target_changed = self.state.target != target;
        let translation_changed = self.state.translation != translation;
        if !(library_changed || target_changed || translation_changed) {
            return;
        }
        self.state.library = library;
        self.state.target = target;
        self.state.translation = translation;
        if translation_changed && !translation {
            // Turning translation off shows the originals at once. Other
            // inputs changing leave what was translated alone.
            for row in &mut self.state.results {
                row.translated_summary = None;
            }
        }
        let ended = target_changed
            && self
                .state
                .browsing_for
                .as_ref()
                .is_some_and(|game| Some(game) != self.state.target.as_ref());
        if ended {
            self.state.browsing_for = None;
            let query = self.state.query.clone();
            self.search(&query, cx);
        }
        if target_changed {
            self.retarget_details(cx);
            self.refresh_installed(cx);
        }
        if translation_changed && translation {
            // What is shown gets its summaries now.
            self.request_shown_summaries(cx);
            self.request_detail_summaries(cx);
        }
        self.state_changed(cx);
    }

    /// The detail the shell shows now, if any.
    pub fn set_shown_detail(&mut self, detail: Option<EntityId>) {
        self.shown_detail = detail;
    }

    /// The page was navigated to: the first visit loads the filter choices and
    /// lists what is popular. A moment later, so a search the same update
    /// starts (browsing for a game) goes first.
    pub fn shown(&mut self, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = page.update(cx, |page, cx| page.first_search(cx));
        });
    }

    /// Redraws, and passes what the open details show along to them.
    pub(super) fn state_changed(&mut self, cx: &mut Context<Self>) {
        self.details.retain(|(view, _)| view.upgrade().is_some());
        let installed = self.state.installed.clone();
        let tags = self.state.filters.game_tags().to_vec();
        let translation = self.state.translation;
        for (view, slug) in &self.details {
            let Some(view) = view.upgrade() else {
                continue;
            };
            let installing = self.state.installing.contains(slug);
            let (installed, tags) = (installed.clone(), tags.clone());
            view.update(cx, |view, cx| {
                view.set_installed(installed, cx);
                view.set_installing(installing, cx);
                view.set_game_tags(tags, cx);
                view.set_translation_enabled(translation, cx);
            });
        }
        cx.notify();
    }

    /// The game installs go into, as a project detail shows it.
    pub(super) fn detail_target(&self) -> Option<InstallTarget> {
        let card = self.state.target_game()?;
        Some(InstallTarget {
            name: card.name.clone(),
            game_version: card.game_version.clone(),
            loader: card.loader,
            from_game: self.state.browsing_for.as_deref() == Some(card.id.as_str()),
        })
    }

    /// The open details' target follows the current game.
    fn retarget_details(&mut self, cx: &mut Context<Self>) {
        let target = self.detail_target();
        for view in self.details.iter().filter_map(|(view, _)| view.upgrade()) {
            let target = target.clone();
            view.update(cx, |view, cx| view.set_target(target, cx));
        }
    }

    /// Makes the controls the first time the page draws, and listens to them.
    fn ensure_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(controls) = &mut self.controls {
            if controls.language != crate::i18n::generation() {
                controls.relabel(window, cx);
            }
            // The search box and the sort menu follow the source browsed.
            let name = self.state.browsed_source_name().to_owned();
            let sorts = self.state.filters.sorts(self.state.query.kind);
            let chosen = self.state.query.sort;
            controls.search_source(&name, window, cx);
            controls.offer_sorts(sorts, chosen, window, cx);
            return;
        }
        let controls = DiscoverControls::new(window, cx);
        cx.subscribe_in(
            &controls.search,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.submit_search(window, cx),
                // The clear button empties the box: search again.
                InputEvent::Change if this.text_was_cleared(cx) => this.submit_search(window, cx),
                _ => {}
            },
        )
        .detach();
        cx.subscribe_in(
            &controls.sort,
            window,
            |this, _, event: &SelectEvent<Labels>, window, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some(sort) = SORTS.iter().find(|sort| sort_label(**sort) == label)
                {
                    this.change_query(DiscoverChange::Sort(*sort), window, cx);
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &controls.page_size,
            window,
            |this, _, event: &SelectEvent<Labels>, window, cx| {
                if let SelectEvent::Confirm(Some(text)) = event
                    && let Some(size) = PAGE_SIZES.iter().find(|size| size.to_string() == *text)
                {
                    this.change_query(DiscoverChange::PageSize(*size), window, cx);
                }
            },
        )
        .detach();
        cx.subscribe(&controls.version_search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        self.controls = Some(controls);
        self.ensure_controls(window, cx);
    }

    /// Whether the search box is empty while the list still shows a search for
    /// words (the clear button was pressed).
    fn text_was_cleared(&self, cx: &App) -> bool {
        let Some(controls) = &self.controls else {
            return false;
        };
        !self.state.query.text.is_empty() && controls.search.read(cx).value().is_empty()
    }

    /// Enter in the search box: same filters, new words, the first page.
    fn submit_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_query(DiscoverChange::Page(0), window, cx);
    }

    /// What is typed in the search box now.
    pub(super) fn typed_text(&self, cx: &App) -> Option<String> {
        self.controls
            .as_ref()
            .map(|controls| controls.search.read(cx).value().to_string())
    }

    /// Searches again with the query as it stands (the page's ⋯ menu).
    pub(super) fn search_again(&mut self, cx: &mut Context<Self>) {
        let query: DiscoverQuery = self.state.query.clone();
        self.search(&query, cx);
    }
}

impl Render for Discover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_controls(window, cx);
        let this = cx.entity().downgrade();
        let emit: Emit = {
            let this = this.clone();
            Rc::new(move |intent, _, cx| {
                let _ = this.update(cx, |page, cx| page.apply_view_intent(intent, cx));
            })
        };
        let change: view::ChangeHandler = {
            let this = this.clone();
            Rc::new(move |change, window, cx| {
                let _ = this.update(cx, |page, cx| page.change_query(change, window, cx));
            })
        };
        let handler: view::DiscoverHandler = Rc::new(move |intent, window, cx| {
            let _ = this.update(cx, |page, cx| page.act(intent, window, cx));
        });
        let controls = self
            .controls
            .as_ref()
            .expect("controls are made before the page draws");
        let version_filter = controls.version_search.read(cx).value().to_string();
        let ctx = DiscoverCtx {
            colors: crate::theme::ShellColors::current(cx),
            model: &self.state,
            state: &self.choices,
            emit,
            handler,
            change,
            controls,
            version_filter,
            window_width: f32::from(window.viewport_size().width),
            window_height: f32::from(window.viewport_size().height),
        };
        let page = view::discover(&ctx).into_any_element();
        kit::page("live-discover", page)
    }
}
