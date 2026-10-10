//! Root shell: route state, Home snapshot, and presentation-only intents.

mod account_view;
mod chips;
mod chrome;
mod controls;
mod detail;
mod home_page;
mod instance;
mod locations;
mod queries;
mod render;
mod routing;

#[cfg(test)]
mod tests;

pub(crate) use self::chrome::sync_appearance;
pub use self::chrome::{build_root, follow_system_appearance, window_titlebar};

use self::account_view::AccountViewer;
use self::detail::DetailSlot;
use self::locations::{HISTORY_LIMIT, Location};
use crate::hero::HeroCarousel;
use crate::history::History;
use crate::home;
use crate::home::{HomeIntent, HomePresentation};
use crate::instance_detail::InstanceDetailView;
use crate::live::{LiveHandler, LiveModel};
use crate::pages::ViewState;
use crate::pages::live::LiveControls;
use crate::route::Route;
use crate::toast::Toast;
use gpui::prelude::*;
use gpui::{App, Context, Entity, FocusHandle, Task, Window};
use std::rc::Rc;

type Labels = gpui_component::select::SearchableVec<String>;

gpui::actions!(lumilio, [Quit]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivitySummary {
    pub active_tasks: u32,
}

impl ActivitySummary {
    pub const fn new(active_tasks: u32) -> Self {
        Self { active_tasks }
    }

    pub fn badge_label(self) -> Option<String> {
        match self.active_tasks {
            0 => None,
            1..=99 => Some(self.active_tasks.to_string()),
            _ => Some("99+".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellIntent {
    Navigate(Route),
    Home(HomeIntent),
}

pub type IntentHandler = Rc<dyn Fn(ShellIntent, &mut Window, &mut App)>;

pub struct LauncherShell {
    route: Route,
    home: HomePresentation,
    hero: Entity<HeroCarousel>,
    activity: ActivitySummary,
    game_running: bool,
    focus_handle: FocusHandle,
    intent_handler: Option<IntentHandler>,
    last_intent: Option<ShellIntent>,
    /// Refreshes the play-time readout while the game runs; nothing else ticks then.
    play_clock: Option<Task<()>>,
    view: ViewState,
    /// Live data pages (plan 0007); `None` keeps the truthful placeholders.
    live: Option<LiveModel>,
    live_handler: Option<LiveHandler>,
    live_controls: Option<Entity<LiveControls>>,
    /// The look preview of the account the Accounts detail shows.
    account_viewer: Option<AccountViewer>,
    account_revision: u64,
    /// The project open on Discover, in place of the list.
    detail: Option<DetailSlot>,
    live_instance: Option<Entity<InstanceDetailView>>,
    /// Where back and forward lead (design language §6).
    history: History<Location>,
    /// Messages waiting for the next render to float up (§11).
    toasts: Vec<Toast>,
}

impl LauncherShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            route: Route::Home,
            home: home::initial_presentation(),
            hero: cx.new(HeroCarousel::new),
            activity: ActivitySummary::new(0),
            game_running: false,
            focus_handle: cx.focus_handle(),
            intent_handler: None,
            last_intent: None,
            play_clock: None,
            view: ViewState::default(),
            live: None,
            live_handler: None,
            live_controls: None,
            account_viewer: None,
            account_revision: 0,
            detail: None,
            live_instance: None,
            history: History::new(HISTORY_LIMIT),
            toasts: Vec::new(),
        }
    }

    /// Tells what just happened, as a toast over whatever page is showing.
    pub fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toasts.push(toast);
        cx.notify();
    }

    /// Toasts not shown yet (a window without the framework `Root` keeps them).
    pub fn pending_toasts(&self) -> &[Toast] {
        &self.toasts
    }

    pub fn with_home(mut self, home: HomePresentation, cx: &mut Context<Self>) -> Self {
        self.set_home(home, cx);
        self
    }

    pub fn with_activity(mut self, activity: ActivitySummary) -> Self {
        self.activity = activity;
        self
    }

    /// The application receives every intent. It must answer
    /// `Home(Continue | Recover)` by feeding [`Self::apply_launch_signal`];
    /// the shell has already entered the launch moment optimistically.
    pub fn with_intent_handler(mut self, handler: IntentHandler) -> Self {
        self.intent_handler = Some(handler);
        self
    }

    fn emit(&mut self, intent: ShellIntent, window: &mut Window, cx: &mut Context<Self>) {
        // Home reacts before the application does, so the press is felt at once.
        let home = match intent {
            ShellIntent::Home(HomeIntent::Continue | HomeIntent::Recover) => {
                Some(std::mem::take(&mut self.home).begin_launch())
            }
            ShellIntent::Home(HomeIntent::CancelLaunch) => {
                Some(std::mem::take(&mut self.home).cancel_launch())
            }
            _ => None,
        };
        if let Some(home) = home {
            self.set_home(home, cx);
        }
        self.last_intent = Some(intent);
        if let Some(handler) = &self.intent_handler {
            handler(intent, window, cx);
        }
    }
}
