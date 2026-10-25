use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Context, Entity, FocusHandle, Subscription, Task, WeakEntity, Window};

use super::current::CurrentChoices;
use super::detail::DetailSlot;
use super::locations::{HISTORY_LIMIT, Location};
use crate::features::accounts::{Accounts, AccountsEvent};
use crate::features::activity::Activity;
use crate::features::discover::{Discover, DiscoverEvent};
use crate::features::library::Library;
use crate::features::new_game::NewGame;
use crate::features::settings::{Settings, SettingsEvent};
use crate::hero::HeroCarousel;
use crate::history::History;
use crate::home::{HomeIntent, HomePresentation};
use crate::instance_detail::InstanceDetailView;
use crate::live::{LiveHandler, LiveModel};
use crate::pages::ViewState;
use crate::route::Route;
use crate::toast::Toast;
use crate::{home, toast};

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
    pub(super) route: Route,
    pub(super) home: HomePresentation,
    pub(super) hero: Entity<HeroCarousel>,
    pub(super) activity: ActivitySummary,
    pub(super) focus_handle: FocusHandle,
    pub(super) intent_handler: Option<IntentHandler>,
    pub(super) last_intent: Option<ShellIntent>,
    /// Refreshes the play-time readout while the game runs; nothing else ticks then.
    pub(super) play_clock: Option<Task<()>>,
    pub(super) view: ViewState,
    /// Live data pages (plan 0007); `None` keeps the truthful placeholders.
    pub(super) live: Option<LiveModel>,
    pub(super) live_handler: Option<LiveHandler>,
    /// The Discover page; its project details show here in place of it.
    pub(super) discover: Entity<Discover>,
    /// Makes new games, from the dialog or from packs.
    pub(super) new_game: Entity<NewGame>,
    /// The Library's actions.
    pub(super) library: Entity<Library>,
    /// The Activity page and what it does to tasks.
    pub(super) activity_page: Entity<Activity>,
    /// The Settings page and the saving of each setting.
    pub(super) settings_page: Entity<Settings>,
    /// Tells the game pages which plugins Settings found on.
    pub(super) _settings_events: Subscription,
    /// The Accounts page and what it does to accounts.
    pub(super) accounts: Entity<Accounts>,
    /// Saves which game is current, in the order chosen.
    pub(super) current_choices: CurrentChoices,
    /// This shell, for what its features route back to it.
    pub(super) this: WeakEntity<Self>,
    /// Whether translation is on, from the saved preferences; known from
    /// startup, before the Settings page has loaded anything.
    pub(super) translation: bool,
    /// The project open on Discover, in place of the list.
    pub(super) detail: Option<DetailSlot>,
    pub(super) live_instance: Option<Entity<InstanceDetailView>>,
    /// Where back and forward lead (design language §6).
    pub(super) history: History<Location>,
    /// Draws again when a view posts a toast, so it floats up (§11).
    pub(super) _posted: Subscription,
    /// Shows the project details Discover opens.
    pub(super) _discover_events: Subscription,
    /// Draws the corner chip again when an account's face arrives.
    pub(super) _accounts_events: Subscription,
    /// What core last published (see `mirror`).
    pub(super) context: lumilio_core::Context,
}

impl LauncherShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let discover = cx.new(|_| Discover::new(None));
        let new_game = cx.new(|_| NewGame::new(None));
        let library = cx.new(|_| Library::new(None));
        let activity_page = cx.new(|_| Activity::new(None));
        let settings_page = cx.new(|_| Settings::new(None));
        let settings_events =
            cx.subscribe(&settings_page, |shell, _, event: &SettingsEvent, cx| {
                if let SettingsEvent::PluginsRead(plugins) = event {
                    shell.plugins_changed(plugins, cx);
                }
            });
        let accounts = cx.new(|_| Accounts::new(None));
        let accounts_events = cx.subscribe(&accounts, |_, _, event: &AccountsEvent, cx| {
            if *event == AccountsEvent::FacesChanged {
                cx.notify();
            }
        });
        let discover_events = cx.subscribe(&discover, |shell, _, event: &DiscoverEvent, cx| {
            if let DiscoverEvent::ProjectOpened(opened) = event {
                shell.show_project(opened.clone(), cx);
            }
        });
        Self {
            route: Route::Home,
            home: home::initial_presentation(),
            hero: cx.new(HeroCarousel::new),
            activity: ActivitySummary::new(0),
            focus_handle: cx.focus_handle(),
            intent_handler: None,
            last_intent: None,
            play_clock: None,
            view: ViewState::default(),
            live: None,
            live_handler: None,
            discover,
            new_game,
            library,
            activity_page,
            settings_page,
            _settings_events: settings_events,
            accounts,
            current_choices: CurrentChoices::default(),
            this: cx.weak_entity(),
            translation: false,
            detail: None,
            live_instance: None,
            history: History::new(HISTORY_LIMIT),
            _posted: cx.observe_global::<toast::Posted>(|_, cx| cx.notify()),
            _discover_events: discover_events,
            _accounts_events: accounts_events,
            context: lumilio_core::Context::default(),
        }
    }

    /// Tells what just happened, as a toast over whatever page is showing.
    pub fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        toast::post(toast, cx);
    }

    /// Toasts not shown yet (a window without the framework `Root` keeps them).
    pub fn pending_toasts<'a>(&self, cx: &'a App) -> &'a [Toast] {
        toast::posted(cx)
    }

    pub fn with_home(mut self, home: HomePresentation, cx: &mut Context<Self>) -> Self {
        self.set_home(home, cx);
        self
    }

    /// Lets the pages that ask the launcher themselves do so.
    pub fn with_worker(mut self, worker: crate::runtime::Worker, cx: &mut Context<Self>) -> Self {
        self.discover
            .update(cx, |discover, _| discover.set_worker(worker.clone()));
        self.new_game
            .update(cx, |new_game, _| new_game.set_worker(worker.clone()));
        self.library
            .update(cx, |library, _| library.set_worker(worker.clone()));
        self.activity_page
            .update(cx, |activity, _| activity.set_worker(worker.clone()));
        self.settings_page
            .update(cx, |settings, _| settings.set_worker(worker.clone()));
        self.accounts
            .update(cx, |accounts, _| accounts.set_worker(worker.clone()));
        self.current_choices.worker = Some(worker);
        self
    }

    /// The Discover page, for the application to follow its events.
    pub fn discover(&self) -> &Entity<Discover> {
        &self.discover
    }

    /// What makes new games, for the application to follow its events.
    pub fn new_game(&self) -> &Entity<NewGame> {
        &self.new_game
    }

    /// The Library's actions, for the application to follow its events.
    pub fn library(&self) -> &Entity<Library> {
        &self.library
    }

    /// The Activity page, for the application to follow its events.
    pub fn activity_page(&self) -> &Entity<Activity> {
        &self.activity_page
    }

    /// The Settings page, for the application to follow its events.
    pub fn settings_page(&self) -> &Entity<Settings> {
        &self.settings_page
    }

    /// The Accounts page, for the application to follow its events.
    pub fn accounts(&self) -> &Entity<Accounts> {
        &self.accounts
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

    pub(super) fn emit(
        &mut self,
        intent: ShellIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        if intent == ShellIntent::Home(HomeIntent::Create) {
            self.new_game
                .update(cx, |new_game, cx| new_game.open(window, cx));
        }
        self.last_intent = Some(intent);
        if let Some(handler) = &self.intent_handler {
            handler(intent, window, cx);
        }
    }
}
