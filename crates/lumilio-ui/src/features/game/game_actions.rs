use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, AppContext, Context, Entity, EventEmitter, WeakEntity, Window};
use tokio::task::JoinHandle;

use super::answer::answer;
use super::load::{SectionLoads, load_instance};
use super::plugins::{StudioWindowEntry, open_studio_window, plugin_tabs};
use crate::instance_detail::{InstanceDetailView, InstanceIntent};
use crate::runtime::{Service, Worker};
use crate::schematic_studio::StudioView;

/// What the game page tells the shell and the application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameEvent {
    /// A game changed: the lists are read again.
    ListsChanged,
    /// A game's page opened: the application gives it what the game printed
    /// so far, if it is on its way or running.
    Opened(String),
    /// A game was deleted: no direction may lead to it any more.
    Deleted(String),
}

/// How the feature's own windows (the map and the studio) open.
#[derive(Clone, Debug, Default)]
pub struct GameWindows {
    /// The application's identity, which every window carries.
    pub app_id: Option<String>,
    /// The studio window's size, when the review hooks ask for one.
    pub studio_size: Option<(f32, f32)>,
}

/// The game page's work: everything a game's page asks of the launcher, and
/// the map and studio windows it opens. The page itself is the shell's
/// [`InstanceDetailView`]; the shell keeps for itself what moves the window
/// (Accounts, Discover, the current game) and passes playing on.
pub struct Game {
    worker: Option<Worker>,
    this: WeakEntity<Self>,
    windows: Rc<GameWindows>,
    /// The studio windows open now, one per game and folder.
    studios: Rc<RefCell<Vec<StudioWindowEntry>>>,
    loads: Rc<SectionLoads>,
}

impl EventEmitter<GameEvent> for Game {}

impl Game {
    #[must_use]
    pub fn new(worker: Option<Worker>, cx: &mut Context<Self>) -> Self {
        Self {
            worker,
            this: cx.weak_entity(),
            windows: Rc::default(),
            studios: Rc::default(),
            loads: Rc::default(),
        }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// How the feature's windows open (the application says, once).
    pub fn set_windows(&mut self, windows: GameWindows) {
        self.windows = Rc::new(windows);
    }

    /// What answers a game's page, once the feature can ask the launcher.
    #[must_use]
    pub fn work(&self) -> Option<GameWork> {
        Some(GameWork {
            worker: self.worker.clone()?,
            game: self.this.clone(),
            windows: self.windows.clone(),
            studios: self.studios.clone(),
            loads: self.loads.clone(),
        })
    }

    /// A game's page opened: it reads the game.
    pub fn opened(&mut self, view: &Entity<InstanceDetailView>, cx: &mut Context<Self>) {
        let Some(work) = self.work() else {
            return;
        };
        let id = view.read(cx).id().to_owned();
        cx.emit(GameEvent::Opened(id.clone()));
        load_instance(&work, id, view.downgrade(), cx);
    }

    /// Reads an open game's page again (its game changed elsewhere).
    pub fn reload(&self, view: &Entity<InstanceDetailView>, cx: &mut App) {
        if let Some(work) = self.work() {
            let id = view.read(cx).id().to_owned();
            load_instance(&work, id, view.downgrade(), cx);
        }
    }

    /// Asks again which plugin tabs an open game's page shows (which
    /// plugins are on may have changed).
    pub fn reload_plugin_tabs(&self, view: &Entity<InstanceDetailView>, cx: &mut App) {
        if let Some(work) = self.work() {
            let id = view.read(cx).id().to_owned();
            plugin_tabs(&work, id, view.downgrade(), cx);
        }
    }

    /// The game of `id` started (`running`) or ended: its open studio windows
    /// shut their writes, or list their folder again, as the page's studio does.
    pub fn game_changed(&self, id: &str, running: bool, cx: &mut App) {
        for studio in self.studios_of(id) {
            let _ = studio.update(cx, |studio, cx| {
                if running {
                    studio.write_access_changed(cx);
                } else {
                    studio.files_changed(cx);
                }
            });
        }
    }

    /// Opens the studio window for `plugin`'s `folder` on game `id`, on
    /// `file` if there is one, as the page's studio does.
    pub fn open_studio(
        &self,
        id: String,
        plugin: String,
        folder: String,
        file: Option<String>,
        cx: &mut App,
    ) {
        if let Some(work) = self.work() {
            open_studio_window(&work, id, plugin, folder, file, cx);
        }
    }

    /// Review hook: chooses block kind `block` in the open studio windows of
    /// game `id`, as a click on its tile does.
    pub fn studios_choose(&self, id: &str, block: &str, cx: &mut App) {
        for studio in self.studios_of(id) {
            let _ = studio.update(cx, |studio, cx| studio.choose_block(block, cx));
        }
    }

    /// The review hook's inspected block in `id`'s studio windows (T32).
    pub fn studios_inspect(&self, id: &str, position: [i32; 3], cx: &mut App) {
        for studio in self.studios_of(id) {
            let _ = studio.update(cx, |studio, cx| studio.inspect_block(position, cx));
        }
    }

    /// The review hook's dialog for the open studio windows of game `id`.
    pub fn studios_dialog(&self, id: &str, which: &str, cx: &mut App) {
        for studio in self.studios_of(id) {
            let _ = studio.update(cx, |studio, cx| studio.review_dialog(which, cx));
        }
    }

    fn studios_of(&self, id: &str) -> Vec<WeakEntity<StudioView>> {
        self.studios
            .borrow()
            .iter()
            .filter(|entry| entry.id == id)
            .map(|entry| entry.studio.clone())
            .collect()
    }
}

/// What every piece of the game page's work holds: the launcher to ask, the
/// feature to tell, and its windows.
#[derive(Clone)]
pub struct GameWork {
    worker: Worker,
    game: WeakEntity<Game>,
    windows: Rc<GameWindows>,
    pub(super) studios: Rc<RefCell<Vec<StudioWindowEntry>>>,
    loads: Rc<SectionLoads>,
}

impl GameWork {
    /// Answers what the page of game `id` asks.
    pub fn answer(
        &self,
        id: &str,
        view: WeakEntity<InstanceDetailView>,
        intent: InstanceIntent,
        window: &mut Window,
        cx: &mut App,
    ) {
        answer(self, id, view, intent, window, cx);
    }

    pub(super) fn service(&self) -> &Arc<Service> {
        self.worker.service()
    }

    pub(super) fn worker(&self) -> &Worker {
        &self.worker
    }

    pub(super) fn spawn<F>(&self, work: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.worker.spawn(work)
    }

    pub(super) fn windows(&self) -> &GameWindows {
        &self.windows
    }

    pub(super) fn loads(&self) -> &SectionLoads {
        &self.loads
    }

    /// Tells the shell and the application.
    pub(super) fn emit<C: AppContext>(&self, cx: &mut C, event: GameEvent) {
        let _ = self.game.update(cx, |_, cx| cx.emit(event));
    }
}
