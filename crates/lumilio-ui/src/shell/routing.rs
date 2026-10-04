use super::locations::Location;
use super::{LauncherShell, ShellIntent};
use crate::route::Route;
use gpui::{Context, KeyDownEvent, Window};

impl LauncherShell {
    /// Opens a page directly, for design review and tests.
    pub fn show(&mut self, route: Route) {
        self.route = route;
    }

    /// Opens a route's tab directly, for design review.
    pub fn show_tab(&mut self, route: Route, tab: usize) {
        match route {
            Route::Library => self.view.library_tab = tab,
            Route::Activity => self.view.activity_tab = tab,
            Route::Discover | Route::Home | Route::Accounts | Route::Settings => {}
        }
    }

    pub fn route(&self) -> Route {
        self.route
    }

    /// Goes to a page, as choosing it in the navigation does.
    pub fn go_to(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.select_route(route, window, cx);
    }

    pub(super) fn select_route(
        &mut self,
        route: Route,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.location().same(&Location::Page(route)) {
            self.visit();
        }
        self.route = route;
        self.detail = None;
        self.live_instance = None;
        self.emit(ShellIntent::Navigate(route), window, cx);
        cx.notify();
    }

    // ia[navigation]: 后退 / 前进快捷键 | ⌘[ / ⌘]；游戏页、项目详情里 Esc | 同后退 / 前进
    pub(super) fn handle_shortcut(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let secondary = event.keystroke.modifiers.secondary();
        let detail = !matches!(self.location(), Location::Page(_));
        let moved = match event.keystroke.key.as_str() {
            "escape" if detail => {
                self.go_back(Some(window), cx) || {
                    // A detail opened directly (no history) still leaves.
                    self.restore(Location::Page(self.route), Some(window), cx);
                    true
                }
            }
            "[" if secondary => self.go_back(Some(window), cx),
            "]" if secondary => self.go_forward(Some(window), cx),
            _ => false,
        };
        if moved {
            cx.stop_propagation();
            return;
        }
        let route =
            Route::from_shortcut(&event.keystroke.key, event.keystroke.modifiers.secondary());
        if let Some(route) = route {
            self.select_route(route, window, cx);
            cx.stop_propagation();
        }
    }
}
