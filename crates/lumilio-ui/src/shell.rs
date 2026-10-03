//! Root shell: route state, Home snapshot, and presentation-only intents.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, Render, SharedString,
    Task, Window, div, prelude::*, px,
};
use gpui_component::input::InputEvent;
use gpui_component::select::SelectEvent;
use gpui_component::{ActiveTheme as _, TITLE_BAR_HEIGHT, Theme, TitleBar, v_flex};
use lumilio_core::{LaunchSignal, ProjectKind};

use crate::{
    hero::HeroCarousel,
    history::History,
    home::{self, HomeIntent, HomePresentation, ShellHomeColors},
    instance_detail::{InstanceDetailView, InstanceHandler},
    kit::{self, Emit, ViewIntent},
    live::{
        DiscoverChange, LiveHandler, LiveIntent, LiveModel, PAGE_SIZES, SORTS, SearchStatus,
        sort_label,
    },
    navigation::{self, AccountChoice, CurrentAccount, CurrentInstance, InstanceChoice, Leading},
    pages::{
        self, ViewState,
        live::{ALL_VERSIONS, LiveControls},
    },
    placeholders,
    project_detail::{DetailHandler, DetailIntent, DetailState, InstallTarget, ProjectDetailView},
    route::Route,
    theme::{self, ShellColors},
    toast::{self, Toast},
};

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
    /// The project open on Discover, in place of the list.
    detail: Option<DetailSlot>,
    live_instance: Option<Entity<InstanceDetailView>>,
    /// Where back and forward lead (design language §6).
    history: History<Location>,
    /// Messages waiting for the next render to float up (§11).
    toasts: Vec<Toast>,
}

/// A place the navigation can return to. Detail views stay alive in history,
/// so going back is instant and keeps their tab and scroll.
#[derive(Clone)]
enum Location {
    Page(Route),
    Instance(Entity<InstanceDetailView>),
    Project(DetailSlot),
}

impl Location {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Page(a), Self::Page(b)) => a == b,
            (Self::Instance(a), Self::Instance(b)) => a.entity_id() == b.entity_id(),
            (Self::Project(a), Self::Project(b)) => a.view.entity_id() == b.view.entity_id(),
            _ => false,
        }
    }
}

/// How many locations back remembers.
const HISTORY_LIMIT: usize = 50;

#[derive(Clone)]
struct DetailSlot {
    view: Entity<ProjectDetailView>,
    kind: ProjectKind,
    slug: String,
}

/// How often the play-time readout refreshes while the game runs.
const PLAY_CLOCK_INTERVAL: Duration = Duration::from_secs(20);

impl LauncherShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
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
            live_controls: None,
            detail: None,
            live_instance: None,
            history: History::new(HISTORY_LIMIT),
            toasts: Vec::new(),
        }
    }

    /// Where the window is now, as history records it.
    fn location(&self) -> Location {
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
    fn visit(&mut self) {
        let here = self.location();
        self.history.visit(here);
    }

    /// Shows a remembered location. A page is told it was navigated to (so
    /// it refreshes) when a window is at hand.
    fn restore(&mut self, to: Location, window: Option<&mut Window>, cx: &mut Context<Self>) {
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

    /// Chooses the instance play and installs target, everywhere at once.
    pub fn set_install_target(&mut self, id: String, cx: &mut Context<Self>) {
        let target = self.live.as_ref().and_then(|model| {
            let card = model.library.iter().find(|card| card.id == id)?;
            Some(InstallTarget {
                name: card.name.clone(),
                game_version: card.game_version.clone(),
                loader: card.loader,
            })
        });
        if let (Some(slot), Some(target)) = (&self.detail, target) {
            slot.view
                .update(cx, |view, cx| view.set_target(Some(target), cx));
        }
        self.update_live(
            |model| {
                model.install_target = Some(id);
            },
            cx,
        );
    }

    /// Opens a project's detail in place of the Discover list. It starts as
    /// "loading"; [`Self::set_detail_state`] fills it in.
    pub fn open_detail(&mut self, kind: ProjectKind, slug: &str, cx: &mut Context<Self>) {
        let Some(live) = self.live_handler.clone() else {
            return;
        };
        let project = slug.to_owned();
        let handler: DetailHandler = Rc::new(move |intent, window, cx| match intent {
            DetailIntent::SaveAs {
                version_id,
                file_name,
                title,
            } => live(
                LiveIntent::SaveVersionAs {
                    project: project.clone(),
                    version_id,
                    file_name,
                    title,
                },
                window,
                cx,
            ),
            DetailIntent::Update {
                file_name,
                version_id,
                title,
            } => live(
                LiveIntent::UpdateInstalled {
                    kind,
                    project: project.clone(),
                    title,
                    file_name,
                    version_id,
                },
                window,
                cx,
            ),
            DetailIntent::Install { version_id, title } => live(
                LiveIntent::Install {
                    kind,
                    slug: project.clone(),
                    title,
                    version: version_id,
                },
                window,
                cx,
            ),
        });
        let url = lumilio_core::project_page_url(kind, slug);
        let target = self.live.as_ref().and_then(|model| {
            let card = model
                .library
                .iter()
                .find(|card| Some(&card.id) == model.install_target.as_ref())?;
            Some(InstallTarget {
                name: card.name.clone(),
                game_version: card.game_version.clone(),
                loader: card.loader,
            })
        });
        let view = cx.new(|_| {
            let mut view = ProjectDetailView::new(slug.to_owned(), url, handler);
            view.set_target_quiet(target);
            view
        });
        self.visit();
        self.live_instance = None;
        self.detail = Some(DetailSlot {
            view,
            kind,
            slug: slug.to_owned(),
        });
        self.route = Route::Discover;
        cx.notify();
    }

    /// Fills the open detail in, if it is still the one asked for.
    pub fn set_detail_state(&mut self, slug: &str, state: DetailState, cx: &mut Context<Self>) {
        if let Some(slot) = self.detail.as_ref().filter(|slot| slot.slug == slug) {
            slot.view.update(cx, |view, cx| view.set_state(state, cx));
        }
    }

    pub fn select_detail_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        if let Some(slot) = &self.detail {
            slot.view.update(cx, |view, cx| view.select_tab(tab, cx));
        }
    }

    /// Leaves the open project: back where it came from, or the list.
    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        if self.detail.is_some() && !self.go_back(None, cx) {
            self.detail = None;
            cx.notify();
        }
    }

    /// The project open on Discover, if any.
    pub fn detail_project(&self) -> Option<(ProjectKind, &str)> {
        self.detail
            .as_ref()
            .map(|slot| (slot.kind, slot.slug.as_str()))
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

    /// Opens a fresh, stable-ID view. Async results update this entity only.
    pub fn open_live_instance(
        &mut self,
        id: String,
        handler: impl FnOnce(gpui::WeakEntity<InstanceDetailView>) -> InstanceHandler,
        cx: &mut Context<Self>,
    ) -> Entity<InstanceDetailView> {
        let view = cx.new(|cx| InstanceDetailView::new(id, handler(cx.weak_entity())));
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        self.visit();
        self.live_instance = Some(view.clone());
        self.route = Route::Library;
        self.detail = None;
        cx.notify();
        view
    }

    /// The game's output so far, for the open page of that game (if any).
    pub fn game_output(
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

    /// Leaves the open instance: back where it came from, or Library.
    pub fn close_live_instance(&mut self, cx: &mut Context<Self>) {
        if self.live_instance.is_some() && !self.go_back(None, cx) {
            self.live_instance = None;
            self.route = Route::Library;
            cx.notify();
        }
    }

    /// The instance was deleted: no direction may lead to it any more, and
    /// if it is showing, the window goes back.
    pub fn forget_instance(&mut self, id: &str, cx: &mut Context<Self>) {
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

    /// Turns on the live Library, Discover and Activity pages. The handler
    /// receives every [`LiveIntent`].
    pub fn with_live(mut self, handler: LiveHandler) -> Self {
        self.live = Some(LiveModel::default());
        self.live_handler = Some(handler);
        self
    }

    pub fn live(&self) -> Option<&LiveModel> {
        self.live.as_ref()
    }

    /// Changes the live model and redraws. Ignored when live pages are off.
    pub fn update_live(&mut self, change: impl FnOnce(&mut LiveModel), cx: &mut Context<Self>) {
        if let Some(model) = &mut self.live {
            change(model);
            self.activity = ActivitySummary::new(model.active_tasks());
            if let Some(slot) = &self.detail {
                let installed = model.installed.clone();
                slot.view
                    .update(cx, |view, cx| view.set_installed(installed, cx));
            }
            cx.notify();
        }
    }

    /// Goes to Home and presses Continue, as if the person had.
    pub fn request_continue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.route = Route::Home;
        self.live_instance = None;
        self.emit(ShellIntent::Home(HomeIntent::Continue), window, cx);
        cx.notify();
    }

    /// Applies one change to the Discover query — with whatever is typed in
    /// the search box — and searches again.
    /// Opens Discover on one kind of content (from a game's Content tab).
    pub fn browse(&mut self, kind: ProjectKind, window: &mut Window, cx: &mut Context<Self>) {
        self.select_route(Route::Discover, window, cx);
        self.change_query(DiscoverChange::Kind(kind), window, cx);
    }

    pub fn change_query(
        &mut self,
        change: DiscoverChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(controls), Some(handler), Some(model)) =
            (&self.live_controls, self.live_handler.clone(), &self.live)
        else {
            return;
        };
        let mut query = model.query.clone();
        query.text = controls
            .read(cx)
            .discover_search
            .read(cx)
            .value()
            .to_string();
        let query = query.apply(change);
        self.update_live(
            |model| {
                model.query = query.clone();
                model.search = SearchStatus::Searching;
            },
            cx,
        );
        handler(LiveIntent::Search(query), window, cx);
    }

    /// Enter in the search box.
    fn submit_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.live.as_ref().map_or(0, |model| model.query.page);
        // Same filters, new words: back to the first page.
        let _ = page;
        self.change_query(DiscoverChange::Page(0), window, cx);
    }

    pub fn view(&self) -> &ViewState {
        &self.view
    }

    /// Applies a page intent to the view state only.
    /// The Library's ordering and loader filter outlive this run: tell the
    /// application when either changes.
    fn remember_library_view(
        &mut self,
        intent: ViewIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::pages::live::{LIBRARY_LOADER, LIBRARY_SORT};
        if !matches!(intent, ViewIntent::Choose(LIBRARY_SORT | LIBRARY_LOADER, _)) {
            return;
        }
        if let Some(handler) = self.live_handler.clone() {
            let (sort, loader) = (
                self.view.choice(LIBRARY_SORT, 0),
                self.view.choice(LIBRARY_LOADER, 0),
            );
            handler(
                LiveIntent::RememberLibraryView {
                    sort: u8::try_from(sort).unwrap_or(0),
                    loader: u8::try_from(loader).unwrap_or(0),
                },
                window,
                cx,
            );
        }
    }

    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.view.apply(intent);
        cx.notify();
    }

    pub fn home(&self) -> &HomePresentation {
        &self.home
    }

    /// Replaces the Home state and points the hero at it.
    pub fn set_home(&mut self, home: HomePresentation, cx: &mut Context<Self>) {
        let playing = matches!(home, HomePresentation::Playing { .. });
        let mode = home.hero_mode();
        self.home = home;
        self.hero.update(cx, |hero, cx| hero.set_mode(mode, cx));
        match (playing, self.play_clock.is_some()) {
            (true, false) => {
                self.play_clock = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(PLAY_CLOCK_INTERVAL).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }));
            }
            (false, true) => self.play_clock = None,
            _ => {}
        }
        cx.notify();
    }

    /// Feeds one launch signal into the Home launch moment.
    pub fn apply_launch_signal(&mut self, signal: LaunchSignal, cx: &mut Context<Self>) {
        let home = std::mem::take(&mut self.home).on_launch_signal(signal, Instant::now());
        self.set_home(home, cx);
    }

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

    /// Goes to a page, as choosing it in the navigation does.
    pub fn go_to(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.select_route(route, window, cx);
    }

    fn select_route(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        if !self.location().same(&Location::Page(route)) {
            self.visit();
        }
        self.route = route;
        self.detail = None;
        self.live_instance = None;
        self.emit(ShellIntent::Navigate(route), window, cx);
        cx.notify();
    }

    fn handle_shortcut(
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

impl LauncherShell {
    /// What the page below Home's world can open or do: the games with
    /// something wrong, and the recent cards.
    fn home_links(&self) -> home::HomeLinks {
        let Some(handler) = self.live_handler.clone() else {
            return home::HomeLinks::default();
        };
        let open = handler.clone();
        let act = handler;
        home::HomeLinks {
            attention: self
                .live
                .as_ref()
                .map(|model| model.attention.clone())
                .unwrap_or_default(),
            on_open: Some(Rc::new(move |id, window, cx| {
                open(LiveIntent::OpenInstance(id), window, cx)
            })),
            on_act: Some(Rc::new(move |id, action, window, cx| {
                act(LiveIntent::Resolve(id, action), window, cx)
            })),
        }
    }

    /// Home: the world runs full-bleed from the top of the window and
    /// dissolves into the page; everything else sits on the content column.
    fn render_home(
        &self,
        colors: ShellHomeColors,
        home_handler: Option<home::HomeIntentHandler>,
        window: &Window,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let hero = self.hero.clone();
        let on_continue_hover: home::HoverHandler = Rc::new(move |hovered, _, cx| {
            hero.update(cx, |hero, cx| hero.set_flare(hovered, cx));
        });
        let overlay =
            home::render_overlay(&self.home, home_handler.clone(), Some(on_continue_hover));
        // The world is as tall as its share of the window, so it is sized in
        // pixels: the whole page scrolls and the world scrolls away with it.
        let world_height = px(
            (f32::from(window.viewport_size().height) * theme::HERO_SHARE)
                .max(f32::from(theme::HERO_MIN_HEIGHT)),
        );
        let links = self.home_links();
        let has_attention = !links.attention.is_empty() && self.home.recent_is_instances();
        let body = (self.home.has_body() || has_attention)
            .then(|| home::render_body(&self.home, home_handler, &links, colors));

        div()
            .id("home-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(world_height)
                            .flex_none()
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .size_full()
                                    .child(self.hero.clone()),
                            )
                            .child(
                                div().absolute().inset_0().flex().justify_center().child(
                                    theme::content_column()
                                        .debug_selector(|| "home-overlay-column".into())
                                        .h_full()
                                        .justify_end()
                                        .pb(theme::HERO_FADE)
                                        .children(overlay.map(|overlay| {
                                            div()
                                                .debug_selector(|| "home-overlay".into())
                                                .child(overlay)
                                        })),
                                ),
                            ),
                    )
                    .children(body.map(|body| {
                        theme::content_column()
                            .debug_selector(|| "home-body".into())
                            .pt(px(4.))
                            .pb(theme::BOTTOM_SAFE_AREA)
                            .child(body)
                    })),
            )
            .into_any_element()
    }
}

impl Render for LauncherShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        toast::flush(&mut self.toasts, window, cx);
        let colors = ShellColors::from_theme(cx.theme());
        let home_colors = ShellHomeColors {
            foreground: colors.foreground,
            muted: colors.muted,
            border: colors.border,
            surface: colors.surface_subtle.opacity(0.5),
            primary: colors.primary,
            primary_foreground: colors.primary_foreground,
            focus: colors.focus,
            danger: colors.danger,
        };
        let home_handler = self.intent_handler.as_ref().map(|_| {
            let callback = cx.listener(|this, intent: &HomeIntent, window, cx| {
                this.emit(ShellIntent::Home(*intent), window, cx);
            });
            Rc::new(
                move |intent: HomeIntent, window: &mut Window, cx: &mut App| {
                    callback(&intent, window, cx)
                },
            ) as home::HomeIntentHandler
        });
        let route_callback = cx.listener(|this, route: &Route, window, cx| {
            this.select_route(*route, window, cx);
        });

        let view_callback = cx.listener(|this, intent: &ViewIntent, window, cx| {
            this.apply_view_intent(*intent, cx);
            this.remember_library_view(*intent, window, cx);
        });
        let emit_view: Emit = Rc::new(move |intent, window: &mut Window, app: &mut App| {
            view_callback(&intent, window, app)
        });

        let leading = {
            let back = cx.listener(|shell, _: &(), window, cx| {
                shell.go_back(Some(window), cx);
            });
            let forward = cx.listener(|shell, _: &(), window, cx| {
                shell.go_forward(Some(window), cx);
            });
            Leading {
                title: self.location_title(cx),
                can_back: self.can_go_back(),
                can_forward: self.can_go_forward(),
                on_back: Rc::new(move |window, cx| back(&(), window, cx)),
                on_forward: Rc::new(move |window, cx| forward(&(), window, cx)),
            }
        };
        let current_instance = self.current_instance(cx);
        let current_account = self.current_account(cx);

        if self.live.is_some() && self.live_controls.is_none() {
            let controls = cx.new(|cx| LiveControls::new(window, cx));
            let (search, filter) = {
                let read = controls.read(cx);
                (read.discover_search.clone(), read.library_filter.clone())
            };
            cx.subscribe_in(
                &search,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.submit_search(window, cx);
                    }
                },
            )
            .detach();
            cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            let (sort, size, version) = {
                let read = controls.read(cx);
                (
                    read.sort.clone(),
                    read.page_size.clone(),
                    read.version.clone(),
                )
            };
            cx.subscribe_in(
                &sort,
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
                &size,
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
            cx.subscribe_in(
                &version,
                window,
                |this, _, event: &SelectEvent<Labels>, window, cx| {
                    let SelectEvent::Confirm(choice) = event;
                    let version = choice.clone().filter(|text| text != ALL_VERSIONS);
                    this.change_query(DiscoverChange::Version(version), window, cx);
                },
            )
            .detach();
            let (library_sort, library_loader) = {
                let read = controls.read(cx);
                (read.library_sort.clone(), read.library_loader.clone())
            };
            cx.subscribe_in(
                &library_sort,
                window,
                |this, _, event: &SelectEvent<Labels>, window, cx| {
                    if let SelectEvent::Confirm(Some(label)) = event
                        && let Some(at) = pages::live::SORT_LABELS
                            .iter()
                            .position(|sort| *sort == label)
                    {
                        let intent = ViewIntent::Choose(pages::live::LIBRARY_SORT, at);
                        this.apply_view_intent(intent, cx);
                        this.remember_library_view(intent, window, cx);
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &library_loader,
                window,
                |this, _, event: &SelectEvent<Labels>, window, cx| {
                    let SelectEvent::Confirm(choice) = event;
                    let code = choice.as_ref().map_or(0, |label| {
                        this.live
                            .as_ref()
                            .map(|model| pages::live::present_loaders(&model.library))
                            .and_then(|loaders| {
                                loaders
                                    .into_iter()
                                    .find(|loader| crate::live::loader_label(*loader) == label)
                            })
                            .map_or(0, pages::live::loader_code)
                    });
                    let intent = ViewIntent::Choose(pages::live::LIBRARY_LOADER, code);
                    this.apply_view_intent(intent, cx);
                    this.remember_library_view(intent, window, cx);
                },
            )
            .detach();
            self.live_controls = Some(controls);
        }
        // The Library's two dropdowns follow what is remembered and what the
        // library holds.
        if let (Some(controls), Some(model)) = (&self.live_controls, &self.live) {
            use gpui_component::IndexPath;
            use pages::live::{ALL_LOADERS, LIBRARY_LOADER, LIBRARY_SORT, SORT_LABELS};
            let present = pages::live::present_loaders(&model.library);
            let codes: Vec<usize> = present
                .iter()
                .copied()
                .map(pages::live::loader_code)
                .collect();
            let want_sort = self.view.choice(LIBRARY_SORT, 0).min(SORT_LABELS.len() - 1);
            let code = self.view.choice(LIBRARY_LOADER, 0);
            let want_loader = codes.iter().position(|c| *c == code).map_or(0, |at| at + 1);
            let read = controls.read(cx);
            let row = |select: &Entity<gpui_component::select::SelectState<Labels>>, cx: &App| {
                select.read(cx).selected_index(cx).map(|ix| ix.row)
            };
            let stale_list = read.library_loaders != codes;
            let stale_sort = row(&read.library_sort, cx) != Some(want_sort);
            let stale_loader = stale_list || row(&read.library_loader, cx) != Some(want_loader);
            if stale_list || stale_sort || stale_loader {
                controls.update(cx, |controls, cx| {
                    if stale_list {
                        let mut items = vec![ALL_LOADERS.to_owned()];
                        items.extend(
                            present
                                .iter()
                                .map(|loader| crate::live::loader_label(*loader).to_owned()),
                        );
                        controls.library_loaders = codes;
                        controls.library_loader.update(cx, |select, cx| {
                            select.set_items(Labels::new(items), window, cx)
                        });
                    }
                    if stale_loader {
                        controls.library_loader.update(cx, |select, cx| {
                            select.set_selected_index(Some(IndexPath::new(want_loader)), window, cx)
                        });
                    }
                    if stale_sort {
                        controls.library_sort.update(cx, |select, cx| {
                            select.set_selected_index(Some(IndexPath::new(want_sort)), window, cx)
                        });
                    }
                });
            }
        }
        // The version list fills in when the filters arrive.
        if let (Some(controls), Some(model)) = (&self.live_controls, &self.live)
            && model.filters.loaded
            && controls.read(cx).versions_shown != model.filters.versions.len()
        {
            let mut items = vec![ALL_VERSIONS.to_owned()];
            items.extend(model.filters.versions.iter().cloned());
            let count = model.filters.versions.len();
            controls.update(cx, |controls, cx| {
                controls.versions_shown = count;
                controls.version.update(cx, |select, cx| {
                    select.set_items(
                        gpui_component::select::SearchableVec::new(items),
                        window,
                        cx,
                    );
                });
            });
        }
        let change_callback = cx.listener(|this, change: &DiscoverChange, window, cx| {
            this.change_query(change.clone(), window, cx);
        });
        let change_handler: pages::live::DiscoverChangeHandler =
            Rc::new(move |change, window: &mut Window, app: &mut App| {
                change_callback(&change, window, app)
            });
        let live_page = match (&self.live, &self.live_controls, &self.live_handler) {
            (Some(model), Some(controls), Some(handler)) if self.route != Route::Home => {
                let filter = controls
                    .read(cx)
                    .library_filter
                    .read(cx)
                    .value()
                    .to_string();
                let ctx = pages::live::LiveCtx {
                    colors,
                    model,
                    state: &self.view,
                    emit: emit_view.clone(),
                    handler: handler.clone(),
                    change: change_handler.clone(),
                    controls: controls.read(cx),
                    filter,
                };
                Some(match (self.route, &self.detail) {
                    (Route::Discover, Some(slot)) => slot.view.clone().into_any_element(),
                    (Route::Library, _) => {
                        let handler = ctx.handler.clone();
                        div()
                            .id("live-library-drop")
                            .size_full()
                            .on_drop::<gpui::ExternalPaths>(move |paths, window, cx| {
                                handler(LiveIntent::DropFiles(paths.paths().to_vec()), window, cx)
                            })
                            .child(kit::page("live-library", pages::live::library(&ctx)))
                            .into_any_element()
                    }
                    (Route::Settings, _) => {
                        kit::page("live-settings", pages::settings::render(&ctx)).into_any_element()
                    }
                    (Route::Accounts, _) => {
                        kit::page("live-accounts", pages::live::accounts(&ctx)).into_any_element()
                    }
                    (Route::Discover, None) => {
                        kit::page("live-discover", pages::live::discover(&ctx)).into_any_element()
                    }
                    _ => kit::page("live-activity", pages::live::activity(&ctx)).into_any_element(),
                })
            }
            _ => None,
        };

        let body = if let Some(view) = &self.live_instance {
            view.clone().into_any_element()
        } else if let Some(page) = live_page {
            page
        } else if self.route == Route::Home {
            self.render_home(home_colors, home_handler, window, cx)
        } else {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .pt(TITLE_BAR_HEIGHT)
                .pb(theme::BOTTOM_SAFE_AREA)
                .px_8()
                .child(placeholders::render(self.route, colors))
                .into_any_element()
        };

        v_flex()
            .id("lumilio-shell")
            .relative()
            .size_full()
            .key_context("lumilio-shell")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_shortcut))
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(body)
            // The title bar is transparent and floats over everything, so on
            // Home the world reaches the top edge under the traffic lights.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(TitleBar::new().bg(gpui::transparent_black()).border_b_0()),
            )
            // A veil in the page colour under the capsule: content scrolling
            // behind it fades out instead of colliding with the icons.
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(theme::NAV_VEIL)
                    .bg(gpui::linear_gradient(
                        180.,
                        gpui::linear_color_stop(colors.background.opacity(0.), 0.),
                        gpui::linear_color_stop(colors.background.opacity(0.94), 0.62),
                    )),
            )
            .child(navigation::render(
                self.route,
                self.activity.active_tasks,
                colors,
                Rc::new(route_callback),
                leading,
                current_instance,
                current_account,
            ))
    }
}

impl LauncherShell {
    /// The trailing zone: the live install target.
    fn current_instance(&self, cx: &mut Context<Self>) -> Option<CurrentInstance> {
        let empty = cx.listener(|shell, _: &(), window, cx| {
            shell.select_route(Route::Library, window, cx);
        });
        let on_empty: navigation::CloseHandler = Rc::new(move |window, cx| empty(&(), window, cx));
        let model = self.live.as_ref()?;
        let handler = self.live_handler.clone()?;
        let choices: Vec<InstanceChoice> = model
            .library
            .iter()
            .map(|card| InstanceChoice {
                id: card.id.clone(),
                name: card.name.clone().into(),
                meta: card.meta.clone().into(),
                seed: card.seed,
                loader: crate::live::cover_loader(card.loader),
                world: card.world,
            })
            .collect();
        let current = model
            .install_target
            .as_ref()
            .and_then(|id| choices.iter().find(|choice| &choice.id == id))
            .cloned();
        Some(CurrentInstance {
            current,
            choices,
            on_choose: Rc::new(move |id, window, cx| {
                handler(LiveIntent::InstallTarget(id.to_owned()), window, cx)
            }),
            on_empty,
        })
    }
}

impl LauncherShell {
    /// The trailing zone's account chip.
    fn current_account(&self, cx: &mut Context<Self>) -> Option<CurrentAccount> {
        let model = self.live.as_ref().filter(|model| model.accounts_loaded)?;
        let handler = self.live_handler.clone()?;
        let manage = cx.listener(|shell, _: &(), window, cx| {
            shell.select_route(Route::Accounts, window, cx);
        });
        let choices = model
            .accounts
            .iter()
            .map(|row| AccountChoice {
                key: row.key.clone(),
                name: row.name.clone().into(),
                detail: if row.needs_sign_in {
                    format!("{} · 需要重新登录", row.kind_label())
                } else {
                    format!(
                        "{} · {}",
                        row.kind_label(),
                        row.uuid.split('-').next().unwrap_or_default()
                    )
                }
                .into(),
                selected: row.selected,
            })
            .collect();
        let choose = handler.clone();
        Some(CurrentAccount {
            choices,
            on_choose: Rc::new(move |name, window, cx| {
                choose(LiveIntent::SelectAccount(name.to_owned()), window, cx)
            }),
            on_manage: Rc::new(move |window, cx| manage(&(), window, cx)),
            on_add: Rc::new(move |window, cx| handler(LiveIntent::NewAccount, window, cx)),
        })
    }

    /// A launch that cannot start (no account yet) leaves the Home launch
    /// moment as if cancelled, so the dialog that follows has the page.
    pub fn abort_launch(&mut self, cx: &mut Context<Self>) {
        let home = std::mem::take(&mut self.home).cancel_launch();
        self.set_home(home, cx);
    }
}

/// How the navigation names a project detail (§6).
const fn detail_title(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Modpack => "整合包详情",
        ProjectKind::Mod => "Mod 详情",
        ProjectKind::ResourcePack => "资源包详情",
        ProjectKind::Shader => "光影详情",
    }
}

/// Keeps the gpui-component theme in step with the system appearance, now
/// and whenever it changes.
pub fn follow_system_appearance(window: &mut Window, cx: &mut App) {
    sync_appearance(window, cx);
    window.observe_window_appearance(sync_appearance).detach();
}

pub(crate) fn sync_appearance(window: &mut Window, cx: &mut App) {
    // A look chosen in Settings wins over the system; the review variable
    // (for screenshots) wins over everything.
    let chosen = crate::platform::mode_of(crate::platform::chosen_appearance(cx));
    match std::env::var("LUMILIO_REVIEW_APPEARANCE").as_deref() {
        Ok("light") => Theme::change(gpui_component::ThemeMode::Light, Some(window), cx),
        Ok("dark") => Theme::change(gpui_component::ThemeMode::Dark, Some(window), cx),
        _ => match chosen {
            Some(mode) => Theme::change(mode, Some(window), cx),
            None => Theme::sync_system_appearance(Some(window), cx),
        },
    }
    theme::tune(cx);
}

/// Builds the window content. The caller mounts it under the framework `Root`
/// (`gpui_kit::open_window` does this) and opens the window with
/// [`window_titlebar`].
pub fn build_root(window: &mut Window, cx: &mut App) -> Entity<LauncherShell> {
    follow_system_appearance(window, cx);
    cx.new(LauncherShell::new)
}

/// Title bar options for the launcher window: transparent, so Home's world
/// can run under the traffic lights.
pub fn window_titlebar() -> gpui::TitlebarOptions {
    gpui::TitlebarOptions {
        title: Some("LumilioCL".into()),
        ..TitleBar::title_bar_options()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Modifiers, TestAppContext, point};
    use lumilio_core::{LaunchPhase, LaunchSignal};

    use super::{ActivitySummary, LauncherShell, Route, ShellIntent};
    use crate::home::{HomeIntent, HomePresentation, RecentEntry, Subject, WorldHint};

    fn continuing() -> HomePresentation {
        HomePresentation::Continue {
            subject: Subject {
                title: "生存".into(),
                metadata: "1.21.1 · Fabric".into(),
                world: WorldHint::Underground,
            },
            recent: vec![RecentEntry {
                id: None,
                title: "空岛".into(),
                metadata: "昨天".into(),
            }],
        }
    }

    /// Lets real-time entrance animations finish (gpui animations read the
    /// wall clock), then draws a fresh frame.
    fn settle(cx: &mut gpui::VisualTestContext) {
        std::thread::sleep(crate::theme::motion::SCENE + std::time::Duration::from_millis(120));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }

    /// Every quad painted exactly over an element's bounds. GPUI paints a
    /// filled, bordered element as a fill quad plus border-only quads.
    fn painted_at(cx: &mut gpui::VisualTestContext, selector: &'static str) -> Vec<gpui::Quad> {
        let Some(bounds) = cx.debug_bounds(selector) else {
            return Vec::new();
        };
        cx.update(|window, _| {
            let scale = window.scale_factor();
            let near = |a: f32, b: gpui::Pixels| (a / scale - f32::from(b)).abs() < 1.5;
            window
                .painted_quads()
                .into_iter()
                .filter(|quad| {
                    near(quad.bounds.origin.x.0, bounds.origin.x)
                        && near(quad.bounds.origin.y.0, bounds.origin.y)
                        && near(quad.bounds.size.width.0, bounds.size.width)
                        && near(quad.bounds.size.height.0, bounds.size.height)
                })
                .collect()
        })
    }

    fn fill(quads: &[gpui::Quad]) -> gpui::Hsla {
        quads
            .iter()
            .filter_map(|quad| quad.background.as_solid())
            .find(|color| color.a > 0.)
            .expect("the element paints a fill")
    }

    #[gpui::test]
    fn keys_on_the_world_keep_their_colours_and_shift_on_hover(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx)
                .with_home(continuing(), cx)
                .with_intent_handler(Rc::new(|_, _, _| {}))
        });
        settle(cx);

        let resume_bg = fill(&painted_at(cx, "home-continue"));
        assert!(
            [
                crate::theme::Body::of(false).orange,
                crate::theme::Body::of(true).orange
            ]
            .contains(&resume_bg),
            "the primary on art is the orange key at rest, got {resume_bg:?}"
        );

        shell.update(cx, |shell, cx| {
            shell.set_home(continuing().begin_launch(), cx);
            shell.apply_launch_signal(LaunchSignal::Running, cx);
        });
        settle(cx);

        let rest = painted_at(cx, "home-stop-game");
        let rest_bg = fill(&rest);
        assert!(
            [
                crate::theme::Body::of(false).key_black,
                crate::theme::Body::of(true).key_black
            ]
            .contains(&rest_bg),
            "the secondary on art is the black key, got {rest_bg:?}"
        );

        let bounds = cx.debug_bounds("home-stop-game").unwrap();
        cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
        cx.run_until_parked();
        let hovered = fill(&painted_at(cx, "home-stop-game"));
        assert_ne!(hovered, rest_bg, "hovering shifts the key's face");
    }

    #[gpui::test]
    fn pressing_continue_plays_the_launch_moment_through_every_state(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let intents = Rc::new(RefCell::new(Vec::new()));
        let seen = intents.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx)
                .with_home(continuing(), cx)
                .with_intent_handler(Rc::new(move |intent, _, _| {
                    seen.borrow_mut().push(intent);
                }))
        });
        cx.run_until_parked();

        let button = cx
            .debug_bounds("home-continue")
            .expect("Continue is drawn on the hero");
        let column = cx
            .debug_bounds("home-overlay-column")
            .expect("the content column is drawn");
        assert!(
            button.size.width < column.size.width / 3.,
            "Continue keeps its own width instead of stretching ({button:?} in {column:?})"
        );
        cx.simulate_click(button.center(), Modifiers::none());
        assert_eq!(
            *intents.borrow(),
            vec![ShellIntent::Home(HomeIntent::Continue)]
        );
        let home =
            |cx: &mut gpui::VisualTestContext| shell.read_with(cx, |shell, _| shell.home().clone());
        assert!(matches!(home(cx), HomePresentation::Launching { .. }));

        let signals = [
            LaunchSignal::Phase(LaunchPhase::Libraries),
            LaunchSignal::Progress {
                done: 40,
                total: 86,
            },
            LaunchSignal::Phase(LaunchPhase::Assets),
            LaunchSignal::Progress {
                done: 900,
                total: 3412,
            },
            LaunchSignal::Running,
        ];
        for signal in signals {
            shell.update(cx, |shell, cx| shell.apply_launch_signal(signal, cx));
            cx.run_until_parked();
            assert!(cx.debug_bounds("home-overlay").is_some());
        }
        assert!(matches!(home(cx), HomePresentation::Playing { .. }));

        shell.update(cx, |shell, cx| {
            shell.apply_launch_signal(LaunchSignal::Exited { code: Some(3) }, cx)
        });
        cx.run_until_parked();
        assert!(matches!(home(cx), HomePresentation::Recovery { .. }));
        assert!(cx.debug_bounds("home-overlay").is_some());

        shell.update(cx, |shell, cx| {
            shell.set_home(HomePresentation::FirstUse, cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("home-overlay").is_none());
    }

    /// The whole Home page scrolls, world included.
    #[gpui::test]
    fn home_scrolls_under_the_wheel(cx: &mut TestAppContext) {
        use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};

        cx.update(gpui_component::init);
        let (_shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx)
                .with_home(continuing(), cx)
                .with_intent_handler(Rc::new(|_, _, _| {}))
        });
        cx.simulate_resize(gpui::size(px(1080.), px(500.)));
        cx.run_until_parked();

        let scroll = |cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>| {
            cx.simulate_event(ScrollWheelEvent {
                position: at,
                delta: ScrollDelta::Pixels(point(px(0.), px(-160.))),
                modifiers: Modifiers::none(),
                touch_phase: TouchPhase::Moved,
            });
            cx.run_until_parked();
        };

        let before = cx.debug_bounds("home-body").expect("Home body is drawn");
        // Wheel over the world, not only over the page below it.
        scroll(cx, point(px(300.), px(120.)));
        let after = cx.debug_bounds("home-body").expect("Home body is drawn");
        assert!(
            after.origin.y < before.origin.y - px(10.),
            "Home scrolls when the wheel is over the world ({before:?} → {after:?})"
        );
    }

    #[test]
    fn activity_summary_formats_a_bounded_badge() {
        assert_eq!(ActivitySummary::new(0).badge_label(), None);
        assert_eq!(ActivitySummary::new(4).badge_label(), Some("4".into()));
        assert_eq!(ActivitySummary::new(100).badge_label(), Some("99+".into()));
    }

    #[gpui::test]
    fn live_library_cards_manage_and_explicit_play_is_independent(cx: &mut TestAppContext) {
        use crate::live::{LiveIntent, library_card};
        use lumilio_core::{InstanceRecord, InstanceSettings, Loader};

        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        let record = InstanceRecord {
            id: "survival".to_owned(),
            name: "生存".to_owned(),
            game_version: "1.21.1".to_owned(),
            loader: Loader::Fabric,
            loader_version: None,
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
        };
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            shell.update_live(
                |model| model.set_library(vec![library_card(&record, 10)], None),
                cx,
            );
        });
        cx.run_until_parked();

        let star = cx
            .debug_bounds("live-favorite-0")
            .expect("the star is drawn");
        cx.simulate_click(star.center(), Modifiers::none());
        assert_eq!(
            *seen.borrow(),
            vec![LiveIntent::ToggleFavorite("survival".to_owned())],
            "starring must not also start the game"
        );

        let card = cx.debug_bounds("live-card-0").expect("the card is drawn");
        let body = gpui::point(card.center().x, card.bottom() - gpui::px(12.));
        cx.simulate_click(body, Modifiers::none());
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::OpenInstance("survival".to_owned()))
        );
        let before = seen.borrow().len();
        let play = cx
            .debug_bounds("live-play-0")
            .expect("explicit play button");
        cx.simulate_click(play.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().len(),
            before + 1,
            "play must not also open details"
        );
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::Play("survival".to_owned()))
        );
    }

    #[gpui::test]
    fn the_collections_tab_shows_each_collection_and_the_card_menu_does_not_open_the_game(
        cx: &mut TestAppContext,
    ) {
        use crate::live::{CollectionRow, LiveIntent, library_card};
        use lumilio_core::{InstanceRecord, InstanceSettings, Loader};

        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        let record = |id: &str| InstanceRecord {
            id: id.to_owned(),
            name: id.to_owned(),
            game_version: "1.21.1".to_owned(),
            loader: Loader::Fabric,
            loader_version: None,
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
        };
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            shell.show_tab(Route::Library, crate::pages::live::COLLECTIONS_TAB);
            shell.update_live(
                |model| {
                    model.set_library(
                        vec![
                            library_card(&record("a"), 10),
                            library_card(&record("b"), 10),
                        ],
                        None,
                    );
                    model.collections = vec![CollectionRow {
                        name: "生存".into(),
                        members: vec!["b".into()],
                    }];
                },
                cx,
            );
        });
        cx.run_until_parked();

        // Only the collection's own game is drawn, and its button asks for that game.
        assert!(cx.debug_bounds("live-card-0").is_some());
        assert!(cx.debug_bounds("live-card-1").is_none());
        let more = cx
            .debug_bounds("live-card-more-0")
            .expect("the card's more menu");
        cx.simulate_click(more.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(
            seen.borrow().is_empty(),
            "opening the card's menu must not also open the game: {:?}",
            seen.borrow()
        );

        let new = cx
            .debug_bounds("live-new-collection")
            .expect("new collection");
        cx.simulate_click(new.center(), Modifiers::none());
        assert_eq!(seen.borrow().last(), Some(&LiveIntent::NewCollection));
    }

    #[gpui::test]
    fn a_failed_task_offers_retry_and_open_and_a_game_that_is_gone_offers_no_open(
        cx: &mut TestAppContext,
    ) {
        use crate::live::{ActivityRow, ActivityState, LiveIntent, library_card};
        use lumilio_core::{InstanceRecord, InstanceSettings, Loader, RetryAction, TaskCategory};

        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        let record = InstanceRecord {
            id: "here".to_owned(),
            name: "在".to_owned(),
            game_version: "1.21.1".to_owned(),
            loader: Loader::Fabric,
            loader_version: None,
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
        };
        let failed = |instance: &str| ActivityRow {
            task: None,
            amount: None,
            unit: lumilio_core::ProgressUnit::Items,
            rate: None,
            instance: Some(instance.to_owned()),
            retry: Some(RetryAction::RepairInstance {
                instance: instance.to_owned(),
            }),
            category: TaskCategory::Repair,
            title: "修复".into(),
            detail: String::new(),
            fraction: None,
            state: ActivityState::Failed("no route".into()),
            cancel: None,
        };
        shell.update(cx, |shell, cx| {
            shell.show(Route::Activity);
            shell.update_live(
                |model| {
                    model.set_library(vec![library_card(&record, 10)], None);
                    model.set_activity(vec![failed("here"), failed("gone")], 1_000);
                },
                cx,
            );
        });
        cx.run_until_parked();

        let retry = cx.debug_bounds("live-retry-0").expect("retry");
        cx.simulate_click(retry.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::RetryTask(RetryAction::RepairInstance {
                instance: "here".into()
            }))
        );
        let open = cx.debug_bounds("live-open-0").expect("open");
        cx.simulate_click(open.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::OpenInstance("here".into()))
        );
        assert!(cx.debug_bounds("live-retry-1").is_some());
        assert!(
            cx.debug_bounds("live-open-1").is_none(),
            "a deleted game cannot be opened"
        );
    }

    #[gpui::test]
    fn home_lists_what_needs_attention_with_one_remedy_and_recent_games_open(
        cx: &mut TestAppContext,
    ) {
        use crate::home::AttentionRow;
        use crate::instance_detail::ProblemAction;
        use crate::live::LiveIntent;

        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        shell.update(cx, |shell, cx| {
            shell.set_home(
                HomePresentation::Continue {
                    subject: Subject {
                        title: "生存".into(),
                        metadata: "1.21.1 · Fabric".into(),
                        world: WorldHint::Underground,
                    },
                    recent: vec![RecentEntry {
                        id: Some("sky".into()),
                        title: "空岛".into(),
                        metadata: "昨天".into(),
                    }],
                },
                cx,
            );
            shell.update_live(
                |model| {
                    model.attention = vec![AttentionRow {
                        instance: "sky".into(),
                        name: "空岛".into(),
                        title: "有游戏文件缺失或损坏".into(),
                        detail: "共 2 个".into(),
                        action: Some((ProblemAction::Repair, "修复")),
                        more: 1,
                    }];
                },
                cx,
            );
        });
        settle(cx);

        let act = cx.debug_bounds("home-attention-act-0").expect("remedy");
        cx.simulate_click(act.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::Resolve("sky".into(), ProblemAction::Repair))
        );
        let card = cx.debug_bounds("home-recent-0").expect("recent card");
        cx.simulate_click(card.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::OpenInstance("sky".into()))
        );
    }

    #[gpui::test]
    fn discover_leaves_naming_the_target_game_to_the_corner_chip(cx: &mut TestAppContext) {
        use crate::live::{LiveIntent, library_card};
        use lumilio_core::{InstanceRecord, InstanceSettings, Loader};

        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        let record = |id: &str| InstanceRecord {
            id: id.to_owned(),
            name: id.to_owned(),
            game_version: "1.21.1".to_owned(),
            loader: Loader::Fabric,
            loader_version: None,
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
        };
        shell.update(cx, |shell, cx| {
            shell.show(Route::Discover);
            shell.update_live(
                |model| {
                    model.set_library(
                        vec![
                            library_card(&record("main"), 1),
                            library_card(&record("side"), 1),
                        ],
                        Some("main".into()),
                    );
                    model.query = crate::live::DiscoverQuery::new(lumilio_core::ProjectKind::Mod);
                },
                cx,
            );
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("live-discover-target").is_none(),
            "the corner already names the game; Discover adds no banner"
        );

        shell.update(cx, |shell, cx| shell.set_install_target("side".into(), cx));
        cx.run_until_parked();
        shell.update(cx, |shell, _| {
            assert_eq!(
                shell.live().unwrap().install_target.as_deref(),
                Some("side")
            );
        });
    }

    #[gpui::test]
    fn a_result_the_target_already_has_shows_installed_or_offers_the_update(
        cx: &mut TestAppContext,
    ) {
        use crate::live::{LiveIntent, SearchRow, SearchStatus, library_card};
        use lumilio_core::{InstalledProject, InstanceRecord, InstanceSettings, Loader};

        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        let record = InstanceRecord {
            id: "main".to_owned(),
            name: "main".to_owned(),
            game_version: "1.21.1".to_owned(),
            loader: Loader::Fabric,
            loader_version: None,
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
        };
        shell.update(cx, |shell, cx| {
            shell.show(Route::Discover);
            shell.update_live(
                |model| {
                    model.set_library(vec![library_card(&record, 1)], Some("main".into()));
                    model.query = crate::live::DiscoverQuery::new(lumilio_core::ProjectKind::Mod);
                    model.results = (0..3)
                        .map(|n| SearchRow {
                            project_id: format!("P{n}"),
                            kind: lumilio_core::ProjectKind::Mod,
                            slug: format!("mod-{n}"),
                            title: format!("Mod {n}"),
                            author: "a".into(),
                            summary: String::new(),
                            environment: None,
                            categories: Vec::new(),
                            loaders: Vec::new(),
                            downloads: "1".into(),
                            follows: "1".into(),
                            updated: String::new(),
                            icon_url: None,
                            seed: n,
                        })
                        .collect();
                    model.search = SearchStatus::Done { total: 3 };
                    model.installed.insert(
                        "P0".into(),
                        InstalledProject {
                            file_name: "mod0.jar".into(),
                            version_id: "v1".into(),
                            update: None,
                        },
                    );
                    model.installed.insert(
                        "P1".into(),
                        InstalledProject {
                            file_name: "mod1.jar".into(),
                            version_id: "v1".into(),
                            update: Some("v2".into()),
                        },
                    );
                },
                cx,
            );
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("live-installed-0").is_some(), "has it");
        let update = cx.debug_bounds("live-update-1").expect("newer version");
        cx.simulate_click(update.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::UpdateInstalled {
                kind: lumilio_core::ProjectKind::Mod,
                project: "mod-1".into(),
                title: "Mod 1".into(),
                file_name: "mod1.jar".into(),
                version_id: "v2".into(),
            })
        );
        assert!(cx.debug_bounds("live-installed-2").is_none());
        assert!(cx.debug_bounds("live-update-2").is_none());
    }

    #[gpui::test]
    fn changing_the_library_order_or_loader_is_reported_so_it_can_be_remembered(
        cx: &mut TestAppContext,
    ) {
        use crate::kit::ViewIntent;
        use crate::live::LiveIntent;
        use crate::pages::live::{LIBRARY_LOADER, LIBRARY_SORT};

        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        let choose = |intent: ViewIntent, cx: &mut gpui::VisualTestContext| {
            cx.update(|window, app| {
                shell.update(app, |shell, cx| {
                    shell.apply_view_intent(intent, cx);
                    shell.remember_library_view(intent, window, cx);
                })
            });
        };
        choose(ViewIntent::Choose(LIBRARY_SORT, 1), cx);
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::RememberLibraryView { sort: 1, loader: 0 })
        );
        choose(ViewIntent::Choose(LIBRARY_LOADER, 2), cx);
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::RememberLibraryView { sort: 1, loader: 2 })
        );
        // Other choices are not the library's business.
        let before = seen.borrow().len();
        choose(ViewIntent::Choose(7, 1), cx);
        assert_eq!(seen.borrow().len(), before);
    }

    #[gpui::test]
    fn the_library_sorts_and_filters_from_dropdowns_that_follow_what_is_remembered(
        cx: &mut TestAppContext,
    ) {
        use crate::kit::ViewIntent;
        use crate::live::{LiveIntent, library_card};
        use crate::pages::live::{LIBRARY_LOADER, LIBRARY_SORT, loader_code};
        use gpui_component::select::SelectEvent;
        use lumilio_core::{InstanceRecord, InstanceSettings, Loader};

        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
                sink.borrow_mut().push(intent);
            }))
        });
        let record = |id: &str, loader: Loader| InstanceRecord {
            id: id.to_owned(),
            name: id.to_owned(),
            game_version: "1.21.1".to_owned(),
            loader,
            loader_version: None,
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
        };
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            shell.update_live(
                |model| {
                    model.set_library(
                        vec![
                            library_card(&record("a", Loader::Fabric), 1),
                            library_card(&record("b", Loader::Vanilla), 1),
                        ],
                        None,
                    );
                },
                cx,
            );
        });
        cx.run_until_parked();
        let selected = |cx: &mut gpui::VisualTestContext, loader: bool| {
            shell.read_with(cx, |shell, cx| {
                let controls = shell.live_controls.as_ref().unwrap().read(cx);
                let select = if loader {
                    &controls.library_loader
                } else {
                    &controls.library_sort
                };
                select.read(cx).selected_index(cx).map(|ix| ix.row)
            })
        };
        assert_eq!(selected(cx, true), Some(0), "every loader to begin with");

        // What the application restores shows up in the dropdowns.
        shell.update(cx, |shell, cx| {
            shell.apply_view_intent(
                ViewIntent::Choose(LIBRARY_LOADER, loader_code(Loader::Fabric)),
                cx,
            );
            shell.apply_view_intent(ViewIntent::Choose(LIBRARY_SORT, 2), cx);
        });
        cx.run_until_parked();
        assert_eq!(selected(cx, true), Some(2), "全部, 原版, Fabric");
        assert_eq!(selected(cx, false), Some(2));
        assert!(
            seen.borrow().is_empty(),
            "restoring is not a new choice to save"
        );

        // A choice made in a dropdown is applied and reported.
        let sort = shell.read_with(cx, |shell, cx| {
            shell
                .live_controls
                .as_ref()
                .unwrap()
                .read(cx)
                .library_sort
                .clone()
        });
        sort.update(cx, |_, cx| {
            cx.emit(SelectEvent::Confirm(Some("名称".to_owned())))
        });
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::RememberLibraryView {
                sort: 1,
                loader: loader_code(Loader::Fabric) as u8
            })
        );
        assert_eq!(selected(cx, false), Some(1));
    }

    #[gpui::test]
    fn live_instance_back_keeps_library_filters_and_late_data_is_isolated(cx: &mut TestAppContext) {
        use lumilio_core::{InstanceRecord, InstanceSettings, LauncherSettings, Loader};
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let (shell, cx) =
            cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            shell.update(cx, |shell, cx| {
                shell
                    .live_controls
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .library_filter
                    .clone()
                    .update(cx, |input, cx| input.set_value("生存", window, cx));
            })
        });
        let first = shell.update(cx, |shell, cx| {
            shell.open_live_instance("first".into(), |_| Rc::new(|_, _, _| {}), cx)
        });
        let second = shell.update(cx, |shell, cx| {
            shell.open_live_instance("second".into(), |_| Rc::new(|_, _, _| {}), cx)
        });
        first.update(cx, |view, cx| {
            view.loaded(
                Ok((
                    InstanceRecord {
                        id: "first".into(),
                        name: "旧响应".into(),
                        game_version: "1.21.1".into(),
                        loader: Loader::Vanilla,
                        loader_version: None,
                        favorite: false,
                        created_at: 1,
                        last_played: None,
                        play_seconds: 0,
                        installed: false,
                        settings: InstanceSettings::default(),
                    },
                    LauncherSettings::default(),
                )),
                cx,
            )
        });
        cx.run_until_parked();
        assert_eq!(
            second.read_with(cx, |view, _| view.title().to_owned()),
            "游戏详情"
        );
        assert!(cx.debug_bounds("live-card-0").is_none());
        let title = |cx: &mut gpui::VisualTestContext| {
            shell.read_with(cx, |shell, cx| shell.location_title(cx).to_string())
        };
        assert_eq!(title(cx), "游戏详情");
        let back = cx.debug_bounds("navigation-back").expect("back button");
        // Back walks the instances in the order they were opened, then Library.
        cx.simulate_click(back.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(title(cx), "旧响应", "back to the first instance");
        cx.simulate_click(back.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(title(cx), "游戏库");
        assert!(shell.read_with(cx, |shell, _| shell.live_instance().is_none()));
        assert!(!shell.read_with(cx, |shell, _| shell.can_go_back()));
        shell.read_with(cx, |shell, cx| {
            assert_eq!(
                shell
                    .live_controls
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .library_filter
                    .read(cx)
                    .value(),
                "生存"
            )
        });
    }

    #[gpui::test]
    fn messages_float_as_toasts_and_never_take_room_in_the_page(cx: &mut TestAppContext) {
        use crate::toast::Toast;
        use gpui::{AppContext as _, Entity};
        use gpui_component::WindowExt as _;
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let slot: Rc<RefCell<Option<Entity<LauncherShell>>>> = Rc::default();
        let keep = slot.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            let shell = cx.new(|cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
            *keep.borrow_mut() = Some(shell.clone());
            gpui_component::Root::new(shell, window, cx)
        });
        let shell = slot.borrow().clone().expect("shell");
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            cx.notify();
        });
        cx.run_until_parked();
        let header = cx.debug_bounds("live-library-actions").map(|b| b.origin);
        shell.update(cx, |shell, cx| {
            shell.toast(Toast::error("没有装上 Sodium").technical("disk full"), cx)
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.read_with(cx, |shell, _| assert!(shell.pending_toasts().is_empty()));
        assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1);
        assert_eq!(
            cx.debug_bounds("live-library-actions").map(|b| b.origin),
            header,
            "the page did not move to make room"
        );
    }

    #[gpui::test]
    fn a_deleted_instance_drops_out_of_history_and_the_chip_retargets(cx: &mut TestAppContext) {
        use crate::live::{LiveIntent, library_card};
        use lumilio_core::{InstanceRecord, InstanceSettings, Loader};
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx)
                .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
        });
        let record = |id: &str| InstanceRecord {
            id: id.into(),
            name: id.into(),
            game_version: "1.21.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
            favorite: false,
            created_at: 1,
            last_played: None,
            play_seconds: 0,
            installed: true,
            settings: InstanceSettings::default(),
        };
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            let cards = vec![library_card(&record("a"), 1), library_card(&record("b"), 1)];
            shell.update_live(|model| model.set_library(cards, Some("a".into())), cx);
        });
        cx.run_until_parked();
        for id in ["a", "b"] {
            shell.update(cx, |shell, cx| {
                shell.open_live_instance(id.into(), |_| Rc::new(|_, _, _| {}), cx);
            });
        }
        // Back to "a", so "b" is the way forward; then "a" is deleted.
        shell.update(cx, |shell, cx| assert!(shell.go_back(None, cx)));
        shell.update(cx, |shell, cx| shell.forget_instance("a", cx));
        cx.run_until_parked();
        shell.read_with(cx, |shell, cx| {
            assert!(shell.live_instance().is_none(), "the deleted one left");
            assert_eq!(shell.location_title(cx), "游戏库");
            assert!(!shell.can_go_back());
        });
        shell.update(cx, |shell, cx| assert!(shell.go_forward(None, cx)));
        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.live_instance().unwrap().read(cx).id(), "b");
            assert!(!shell.can_go_forward(), "forward never reaches \"a\"");
        });

        // The trailing chip lists every instance and retargets on choice.
        cx.run_until_parked();
        let chip = cx
            .debug_bounds("navigation-instance")
            .expect("current instance");
        cx.simulate_click(chip.center(), Modifiers::none());
        cx.run_until_parked();
        let second = cx
            .debug_bounds("navigation-instance-choice-1")
            .expect("the list opened");
        cx.simulate_click(second.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().last(),
            Some(&LiveIntent::InstallTarget("b".into()))
        );
    }

    #[gpui::test]
    fn a_project_opens_in_place_and_back_returns_to_the_same_list(cx: &mut TestAppContext) {
        use crate::live::{DiscoverChange, LiveIntent, SearchStatus};
        use crate::project_detail::DetailState;
        use lumilio_core::ProjectKind;

        cx.update(gpui_component::init);
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(|_: LiveIntent, _, _| {}))
        });
        shell.update(cx, |shell, cx| {
            shell.show(Route::Discover);
            shell.update_live(
                |model| {
                    model.query = model
                        .query
                        .clone()
                        .apply(DiscoverChange::ToggleCategory("magic".to_owned()))
                        .apply(DiscoverChange::Page(3));
                    model.search = SearchStatus::Done { total: 500 };
                },
                cx,
            );
        });
        cx.run_until_parked();

        shell.update(cx, |shell, cx| {
            shell.open_detail(ProjectKind::Mod, "sodium", cx);
            shell.set_detail_state("sodium", DetailState::Failed("offline".to_owned()), cx);
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("live-detail-body").is_some(),
            "the detail is showing"
        );
        assert_eq!(
            shell.read_with(cx, |shell, cx| shell.location_title(cx).to_string()),
            "Mod 详情"
        );

        // A stale answer for another project changes nothing.
        shell.update(cx, |shell, cx| {
            shell.set_detail_state("other", DetailState::Loading, cx);
            assert_eq!(shell.detail_project(), Some((ProjectKind::Mod, "sodium")));
        });

        let back = cx.debug_bounds("navigation-back").unwrap();
        cx.simulate_click(back.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("live-detail-body").is_none(),
            "back leaves the detail"
        );
        shell.read_with(cx, |shell, _| {
            assert!(shell.detail_project().is_none());
            let query = &shell.live().unwrap().query;
            assert_eq!(query.page, 3, "the page is where it was");
            assert_eq!(query.categories, ["magic"], "the filters are still on");
        });

        // Forward returns to the same detail view, without asking again.
        let forward = cx.debug_bounds("navigation-forward").unwrap();
        cx.simulate_click(forward.center(), Modifiers::none());
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.detail_project(), Some((ProjectKind::Mod, "sodium")));
            assert!(!shell.can_go_forward());
        });
        shell.update(cx, |shell, cx| shell.close_detail(cx));

        // Choosing another landmark also closes an open detail.
        shell.update(cx, |shell, cx| {
            shell.open_detail(ProjectKind::Mod, "sodium", cx)
        });
        cx.run_until_parked();
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            cx.notify();
        });
        shell.update_in(cx, |shell, window, cx| {
            shell.select_route(Route::Library, window, cx)
        });
        shell.read_with(cx, |shell, _| assert!(shell.detail_project().is_none()));
    }

    fn account(name: &str, selected: bool) -> crate::live::AccountRow {
        crate::live::AccountRow {
            key: name.into(),
            name: name.into(),
            uuid: lumilio_core::ProfileId::offline(name).to_string(),
            selected,
            custom_id: false,
            microsoft: false,
            needs_sign_in: false,
        }
    }

    #[gpui::test]
    fn the_account_chip_and_page_choose_add_and_manage_accounts(cx: &mut TestAppContext) {
        use crate::live::LiveIntent;
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx)
                .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
        });
        shell.update(cx, |shell, cx| {
            shell.show(Route::Accounts);
            shell.update_live(|model| model.accounts_loaded = true, cx);
        });
        cx.run_until_parked();

        // No account yet: the chip offers to add one, and so does the page.
        let chip = cx.debug_bounds("navigation-account").expect("account chip");
        cx.simulate_click(chip.center(), Modifiers::none());
        assert_eq!(seen.borrow().as_slice(), [LiveIntent::NewAccount]);
        // The page offers both kinds: offline on the left, Microsoft on the right.
        let actions = cx
            .debug_bounds("accounts-actions")
            .expect("add on the page");
        let at = |share: f32| {
            point(
                actions.origin.x + actions.size.width * share,
                actions.center().y,
            )
        };
        cx.simulate_click(at(0.2), Modifiers::none());
        assert_eq!(seen.borrow().len(), 2);
        assert_eq!(seen.borrow()[1], LiveIntent::NewAccount);
        cx.simulate_click(at(0.85), Modifiers::none());
        assert_eq!(seen.borrow().last(), Some(&LiveIntent::MicrosoftSignIn));
        assert!(cx.debug_bounds("account-choose-0").is_none());

        // With accounts: the page lists them, a click on a row selects it.
        seen.borrow_mut().clear();
        shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| model.accounts = vec![account("Steve", true), account("Alex", false)],
                cx,
            );
        });
        cx.run_until_parked();
        let alex = cx.debug_bounds("account-choose-1").expect("Alex's row");
        cx.simulate_click(alex.center(), Modifiers::none());
        assert_eq!(
            seen.borrow().as_slice(),
            [LiveIntent::SelectAccount("Alex".into())]
        );

        // The chip lists them too and leads to the page from its foot.
        seen.borrow_mut().clear();
        let chip = cx.debug_bounds("navigation-account").unwrap();
        cx.simulate_click(chip.center(), Modifiers::none());
        cx.run_until_parked();
        let second = cx
            .debug_bounds("navigation-account-choice-1")
            .expect("the list opened");
        cx.simulate_click(second.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().as_slice(),
            [LiveIntent::SelectAccount("Alex".into())]
        );
        shell.update(cx, |shell, cx| {
            shell.show(Route::Library);
            cx.notify();
        });
        cx.run_until_parked();
        let chip = cx.debug_bounds("navigation-account").unwrap();
        cx.simulate_click(chip.center(), Modifiers::none());
        cx.run_until_parked();
        let manage = cx
            .debug_bounds("navigation-account-manage")
            .expect("manage entry");
        cx.simulate_click(manage.center(), Modifiers::none());
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| assert_eq!(shell.route(), Route::Accounts));
    }

    #[gpui::test]
    fn the_settings_page_shows_each_tab_and_edits_open_a_dialog(cx: &mut TestAppContext) {
        use crate::kit::ViewIntent;
        use crate::live::{LiveIntent, SettingsView};
        use crate::pages::settings::TAB_GROUP;
        use gpui::{AppContext as _, Entity};
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_reduce_motion(true);
        });
        let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
        let sink = seen.clone();
        let slot: Rc<RefCell<Option<Entity<LauncherShell>>>> = Rc::default();
        let keep = slot.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            let shell = cx.new(|cx| {
                LauncherShell::new(cx)
                    .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
            });
            *keep.borrow_mut() = Some(shell.clone());
            gpui_component::Root::new(shell, window, cx)
        });
        let shell = slot.borrow().clone().expect("shell");
        shell.update(cx, |shell, _| shell.show(Route::Settings));
        // Before the settings arrive the page says so instead of showing blanks.
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert!(cx.debug_bounds("settings-appearance").is_none());

        shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    model.settings = Some(SettingsView {
                        max_memory_mb: Some(4096),
                        total_memory_mb: Some(16_384),
                        data_dir: "/data".into(),
                        ..SettingsView::default()
                    })
                },
                cx,
            );
        });
        cx.run_until_parked();
        for selector in [
            "settings-appearance",
            "settings-after-launch",
            "settings-foreground",
            "settings-motion",
            "settings-language",
        ] {
            assert!(cx.debug_bounds(selector).is_some(), "{selector} on 通用");
        }

        let tabs = [
            (1, "settings-memory"),
            (2, "settings-java-roots"),
            (3, "settings-data-dir"),
            (4, "settings-version"),
        ];
        for (tab, selector) in tabs {
            shell.update(cx, |shell, cx| {
                shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, tab), cx)
            });
            cx.run_until_parked();
            assert!(
                cx.debug_bounds(selector).is_some(),
                "{selector} on tab {tab}"
            );
        }

        // An edit button opens its dialog; cancelling sends nothing.
        shell.update(cx, |shell, cx| {
            shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 1), cx)
        });
        cx.run_until_parked();
        let edit = cx
            .debug_bounds("settings-memory-edit")
            .expect("edit button");
        cx.simulate_click(edit.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("settings-save").is_some(),
            "the dialog opened"
        );
        assert!(seen.borrow().is_empty());
    }

    /// Not a check: prints how long the Discover list takes to lay out and
    /// paint, so performance work has a number to move. Run with
    /// `cargo test -p lumilio-ui frame_cost -- --ignored --nocapture`
    /// (add `--release` for an optimized figure).
    #[gpui::test]
    #[ignore = "a measurement, not a check"]
    fn frame_cost_of_a_full_discover_page(cx: &mut TestAppContext) {
        use crate::live::{LiveIntent, SearchRow, SearchStatus};
        use lumilio_core::ProjectKind;

        cx.update(gpui_component::init);
        let (shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx).with_live(Rc::new(|_: LiveIntent, _, _| {}))
        });
        let rows: Vec<SearchRow> = (0..20)
            .map(|n| SearchRow {
                project_id: format!("id{n}"),
                kind: ProjectKind::Modpack,
                slug: format!("p{n}"),
                title: format!("Project number {n}"),
                author: "someone".to_owned(),
                summary: "A long enough summary to wrap onto a second line when the window is narrow, describing the pack.".to_owned(),
                environment: Some(lumilio_core::Environment::ClientAndServer),
                categories: vec!["adventure".into(), "magic".into(), "technology".into(), "quests".into()],
                loaders: vec!["fabric".into()],
                downloads: "12.5 万".to_owned(),
                follows: "4886".to_owned(),
                updated: "3 天前".to_owned(),
                icon_url: Some(format!("https://cdn.example/{n}.png")),
                seed: n,
            })
            .collect();
        shell.update(cx, |shell, cx| {
            shell.show(Route::Discover);
            shell.update_live(
                |model| {
                    model.results = rows;
                    model.search = SearchStatus::Done { total: 500 };
                },
                cx,
            );
        });
        cx.run_until_parked();
        let frames = 60;
        let start = std::time::Instant::now();
        for _ in 0..frames {
            shell.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
        let each = start.elapsed() / frames;
        eprintln!("discover frame: {each:?} each over {frames} frames");
    }
}
