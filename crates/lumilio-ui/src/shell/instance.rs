use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Context, Entity, WeakEntity, Window};

use super::LauncherShell;
use super::features::play;
use super::locations::Location;
use crate::features::game::GameWork;
use crate::features::new_game::NewGameEvent;
use crate::instance_detail::{InstanceDetailView, InstanceHandler, InstanceIntent};
use crate::live::PlaceTarget;
use crate::route::Route;

impl LauncherShell {
    /// Opens game `id`'s page; `then` is something to ask of the page once
    /// it is up.
    pub fn open_game(
        &mut self,
        id: String,
        then: Option<InstanceIntent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InstanceDetailView> {
        let page = Page {
            shell: self.this.clone(),
            work: self.game.read(cx).work(),
            id: id.clone(),
        };
        let asked = page.clone();
        let view = self.open_live_instance(
            id,
            move |source| {
                Rc::new(move |intent, window, cx| {
                    asked.answer(source.clone(), intent, window, cx);
                })
            },
            cx,
        );
        self.game.update(cx, |game, cx| game.opened(&view, cx));
        // As if the page asked it: some questions are the shell's to answer.
        if let Some(intent) = then {
            page.answer(view.downgrade(), intent, window, cx);
        }
        view
    }

    /// Opens game `id`'s page on one of its tabs, as a click on the tab would.
    pub(crate) fn open_game_on(
        &mut self,
        id: String,
        tab: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = self.open_game(id, None, window, cx);
        view.update(cx, |view, cx| view.open_tab(tab, window, cx));
    }

    /// A new game is shown in the window it was made in and becomes the
    /// current game; an open page of a game whose download ended reads it
    /// again. Deferred: making games tells the shell inside its updates.
    pub(super) fn new_game_event(&mut self, event: &NewGameEvent, cx: &mut Context<Self>) {
        match event {
            NewGameEvent::TasksChanged => {}
            NewGameEvent::Created { id, window } => {
                let (this, id, window) = (self.this.clone(), id.clone(), *window);
                cx.defer(move |cx| {
                    let _ = cx.update_window(window, |_, window, cx| {
                        let _ = this.update(cx, |shell, cx| {
                            shell.open_game(id.clone(), None, window, cx);
                            shell.choose_current(id, cx);
                        });
                    });
                });
            }
            NewGameEvent::Installed(id) => {
                let open = self
                    .live_instance
                    .as_ref()
                    .filter(|view| view.read(cx).id() == id)
                    .cloned();
                if let Some(view) = open {
                    self.game.update(cx, |game, cx| game.reload(&view, cx));
                }
            }
        }
    }

    /// Opens a fresh, stable-ID view. Async results update this entity only.
    pub(crate) fn open_live_instance(
        &mut self,
        id: String,
        handler: impl FnOnce(gpui::WeakEntity<InstanceDetailView>) -> InstanceHandler,
        cx: &mut Context<Self>,
    ) -> Entity<InstanceDetailView> {
        let view = cx.new(|cx| InstanceDetailView::new(id, handler(cx.weak_entity())));
        let read = self.settings_page.read(cx).settings();
        if let Some(enabled) = read.map(|settings| enabled_plugins(&settings.plugins)) {
            view.update(cx, |view, cx| view.plugins_changed(enabled, cx));
        }
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        self.visit(cx);
        self.live_instance = Some(view.clone());
        self.route = Route::Library;
        self.detail = None;
        cx.notify();
        view
    }

    /// The game's output so far, for the open page of that game (if any).
    pub(crate) fn game_output(
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
    pub(crate) fn plugins_changed(
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
    #[cfg(test)]
    pub(crate) fn close_live_instance(&mut self, cx: &mut Context<Self>) {
        if self.live_instance.is_some() && !self.go_back(None, cx) {
            self.live_instance = None;
            self.route = Route::Library;
            cx.notify();
        }
    }

    /// The instance was deleted: no direction may lead to it any more, and
    /// if it is showing, the window goes back.
    pub(crate) fn forget_instance(&mut self, id: &str, cx: &mut Context<Self>) {
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

/// Who answers what one game's page asks: the shell what moves the window,
/// chooses the current game, plays and stops or asks the page a question,
/// the game feature the rest.
#[derive(Clone)]
struct Page {
    shell: WeakEntity<LauncherShell>,
    work: Option<GameWork>,
    id: String,
}

impl Page {
    fn answer(
        &self,
        view: WeakEntity<InstanceDetailView>,
        intent: InstanceIntent,
        window: &mut Window,
        cx: &mut App,
    ) {
        let id = self.id.clone();
        // Deferred: this may be running inside one of the shell's own updates.
        let shell = self.shell.clone();
        match intent {
            InstanceIntent::Play => play(&shell, id, None, window, cx),
            InstanceIntent::PlayWorld(world) => {
                play(&shell, id, Some(PlaceTarget::World(world)), window, cx)
            }
            InstanceIntent::PlayServer(address) => {
                play(&shell, id, Some(PlaceTarget::Server(address)), window, cx)
            }
            InstanceIntent::Stop => window.defer(cx, move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.launch.read(cx).stop_game(&id));
            }),
            InstanceIntent::OpenAccounts => window.defer(cx, move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.go_to(Route::Accounts, window, cx));
            }),
            InstanceIntent::SetCurrent => window.defer(cx, move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.choose_current(id, cx));
            }),
            InstanceIntent::OpenProject { kind, source, slug } => {
                window.defer(cx, move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| {
                        shell.open_project(kind, &source, &slug, cx);
                    });
                });
            }
            // ia[discover]: 锁定目标（从游戏页进入） | 游戏页内容标签「浏览 Mod / 资源包 / 光影」 | 发现页进入「为这个游戏浏览」：页头写明游戏，类型预选，版本和加载器筛选锁定；那个游戏成为当前游戏（芯片、继续与下次启动一致）；在芯片里换成别的游戏就退出锁定
            // ia[instance.content]: 浏览并安装 | L4a 主要 → 发现页（类型预选） | 安装完回到本页可见
            InstanceIntent::BrowseContent(kind) => window.defer(cx, move |window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    // The game browsed for is the game installs go into (D3).
                    shell.choose_current(id.clone(), cx);
                    shell.browse_for(id, kind, window, cx);
                });
            }),
            // The page asks it once it has read the game.
            question @ (InstanceIntent::AskCopy
            | InstanceIntent::AskDelete
            | InstanceIntent::Resolve(_)) => {
                let _ = view.update(cx, |view, cx| view.ask_later(question, cx));
            }
            other => {
                if let Some(work) = &self.work {
                    work.answer(&self.id, view, other, window, cx);
                }
            }
        }
    }
}

fn enabled_plugins(plugins: &[lumilio_core::PluginInfo]) -> Vec<String> {
    plugins
        .iter()
        .filter(|info| info.status == lumilio_core::PluginStatus::Enabled)
        .map(|info| info.manifest.id.clone())
        .collect()
}
