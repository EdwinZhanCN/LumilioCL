use std::future::Future;

use gpui::{App, Context, EventEmitter};
use tokio::task::JoinError;

use crate::live::CollectionRow;
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

/// The Library's actions. The page itself is drawn from the shell's live
/// model.
pub struct Library {
    pub(super) worker: Option<Worker>,
    /// The collections the page shows, as the shell knows them.
    pub(super) collections: Vec<CollectionRow>,
}

impl EventEmitter<LibraryEvent> for Library {}

impl Library {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            collections: Vec::new(),
        }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// The collections, as the shell knows them.
    pub fn set_collections(&mut self, collections: Vec<CollectionRow>) {
        self.collections = collections;
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

    /// The page's order or loader filter changed.
    pub fn remember_view(&mut self, sort: u8, loader: u8, cx: &mut Context<Self>) {
        cx.emit(LibraryEvent::Remember { sort, loader });
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
