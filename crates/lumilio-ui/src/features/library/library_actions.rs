use std::future::Future;

use gpui::{App, Context, EventEmitter};
use tokio::task::JoinError;

use super::controls::LibraryControls;
use super::page::{LibraryInputs, LibraryView};
use crate::live::LiveHandler;
use crate::runtime::{self, Worker};
use crate::toast::{self, Toast};
use crate::tr;

/// What the Library tells the application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibraryEvent {
    /// Work started here changed the library or the tasks Activity lists.
    ListsChanged,
    /// How the page is ordered and filtered changed; the application saves
    /// it with the other preferences (see `Preferences::library_sort`).
    Remember { sort: u8, loader: u8 },
}

/// The Library page: its tabs and choices, the collections, favorites and
/// presets it changes, and games brought in from elsewhere.
pub struct Library {
    pub(super) worker: Option<Worker>,
    /// What the page shows of the rest of the launcher.
    pub(super) inputs: LibraryInputs,
    pub(super) view: LibraryView,
    pub(super) controls: Option<LibraryControls>,
    /// Where the page sends what other features answer.
    pub(super) handler: Option<LiveHandler>,
}

impl EventEmitter<LibraryEvent> for Library {}

impl Library {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            inputs: LibraryInputs::default(),
            view: LibraryView::default(),
            controls: None,
            handler: None,
        }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// The page's controls, once it has drawn.
    #[must_use]
    pub fn controls(&self) -> Option<&LibraryControls> {
        self.controls.as_ref()
    }

    /// Stars or unstars a game. The star shows by itself: only a failure is
    /// told.
    pub fn toggle_favorite(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(service) = self.service() else {
            return;
        };
        self.change(
            cx,
            async move { service.toggle_favorite(&id).await },
            |result, cx| {
                if let Err(error) = result {
                    toast::post(
                        Toast::error(tr!("library-favorite-failed")).technical(error.to_string()),
                        cx,
                    );
                }
            },
        );
    }

    /// Shows a game's folder in the file manager.
    pub fn reveal_game(&self, id: &str, cx: &App) {
        if let Some(service) = self.service() {
            cx.reveal_path(&service.game_dir(id));
        }
    }

    /// Shows the folder the launcher keeps its games in.
    pub fn open_games_folder(&self, cx: &App) {
        if let Some(service) = self.service() {
            cx.reveal_path(&service.layout().profiles());
        }
    }

    pub(super) fn service(&self) -> Option<std::sync::Arc<runtime::Service>> {
        self.worker.as_ref().map(|worker| worker.service().clone())
    }

    /// A quick change to the library: `done` says how it went, then the
    /// lists are read again.
    pub(super) fn change<T, E>(
        &mut self,
        cx: &mut Context<Self>,
        work: impl Future<Output = Result<T, E>> + Send + 'static,
        done: impl FnOnce(Result<T, E>, &mut App) + 'static,
    ) where
        T: Send + 'static,
        E: From<JoinError> + Send + 'static,
    {
        let Some(worker) = &self.worker else {
            return;
        };
        runtime::job(
            worker,
            cx,
            None::<LibraryEvent>,
            None,
            work,
            move |_, result, cx| {
                done(result, cx);
                cx.emit(LibraryEvent::ListsChanged);
            },
        );
    }
}
