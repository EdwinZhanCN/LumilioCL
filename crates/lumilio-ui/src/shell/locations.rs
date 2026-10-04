use super::detail::{DetailSlot, detail_title};
use super::{LauncherShell, ShellIntent};
use crate::instance_detail::InstanceDetailView;
use crate::route::Route;
use gpui::{App, Context, Entity, SharedString, Window};

/// A place the navigation can return to. Detail views stay alive in history,
/// so going back is instant and keeps their tab and scroll.
#[derive(Clone)]
pub(super) enum Location {
    Page(Route),
    Instance(Entity<InstanceDetailView>),
    Project(DetailSlot),
}

impl Location {
    pub(super) fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Page(a), Self::Page(b)) => a == b,
            (Self::Instance(a), Self::Instance(b)) => a.entity_id() == b.entity_id(),
            (Self::Project(a), Self::Project(b)) => a.view.entity_id() == b.view.entity_id(),
            _ => false,
        }
    }
}

/// How many locations back remembers.
pub(super) const HISTORY_LIMIT: usize = 50;

impl LauncherShell {
    /// Where the window is now, as history records it.
    pub(super) fn location(&self) -> Location {
        if let Some(view) = &self.live_instance {
            Location::Instance(view.clone())
        } else if let Some(slot) = self
            .detail
            .as_ref()
            .filter(|_| self.route == Route::Discover)
        {
            Location::Project(slot.clone())
        } else {
            Location::Page(self.route)
        }
    }

    /// Records the current location before moving somewhere else.
    pub(super) fn visit(&mut self) {
        let here = self.location();
        self.history.visit(here);
    }

    /// Shows a remembered location. A page is told it was navigated to (so
    /// it refreshes) when a window is at hand.
    pub(super) fn restore(
        &mut self,
        to: Location,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.detail = None;
        self.live_instance = None;
        match to {
            Location::Page(route) => {
                self.route = route;
                if let Some(window) = window {
                    self.emit(ShellIntent::Navigate(route), window, cx);
                }
            }
            Location::Instance(view) => {
                self.route = Route::Library;
                self.live_instance = Some(view);
            }
            Location::Project(slot) => {
                self.route = Route::Discover;
                self.detail = Some(slot);
            }
        }
        cx.notify();
    }

    /// Goes back one location; `false` when there is nowhere to go.
    pub fn go_back(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) -> bool {
        let here = self.location();
        match self.history.back(here) {
            Some(to) => {
                self.restore(to, window, cx);
                true
            }
            None => false,
        }
    }

    pub fn go_forward(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) -> bool {
        let here = self.location();
        match self.history.forward(here) {
            Some(to) => {
                self.restore(to, window, cx);
                true
            }
            None => false,
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history.can_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.history.can_forward()
    }

    /// The current location's name for the navigation bar (§6).
    pub fn location_title(&self, cx: &App) -> SharedString {
        match self.location() {
            Location::Page(route) => route.label().into(),
            Location::Instance(view) => view.read(cx).title().to_owned().into(),
            Location::Project(slot) => detail_title(slot.kind).into(),
        }
    }
}
