use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::{App, AppContext as _, Context, Entity, WeakEntity, Window};
use lumilio_core::{CancellationToken, Preferences, ProjectKind};
use lumilio_ui::LauncherShell;
use lumilio_ui::features::accounts::AccountsEvent;
use lumilio_ui::features::activity::ActivityEvent;
use lumilio_ui::features::discover::DiscoverEvent;
use lumilio_ui::features::game::{GameEvent, GameWindows};
use lumilio_ui::features::library::LibraryEvent;
use lumilio_ui::features::new_game::NewGameEvent;
use lumilio_ui::features::settings::SettingsEvent;
use lumilio_ui::home::HomePresentation;
use lumilio_ui::instance_detail::TAB_WORLDS;
use lumilio_ui::live::{LiveHandler, LiveIntent};
use lumilio_ui::route::Route;
use lumilio_ui::shell::{IntentHandler, ShellIntent};
use lumilio_ui::toast::Toast;

use super::dispatch::{on_live_intent, on_shell_intent};
use super::jobs::reload;
use super::open_game::open_instance;
use super::settings::{apply_preferences, apply_saved_preferences};
use super::{context, jobs, launch, terms, update};
use crate::backend::Backend;

/// How many finished tasks the Activity page lists.
pub(super) const ACTIVITY_LIMIT: usize = 30;

#[derive(Default)]
pub(super) struct State {
    /// An older list snapshot cannot replace a newer request.
    pub(super) reload_seq: u64,
    /// The background checks of Home's games now running.
    pub(super) home_diagnostics: Option<tokio::task::AbortHandle>,
    /// Each game's last checks, for Home's 需要留意.
    pub(super) home_checks: Option<context::Checked>,
    /// Stops the launch or the running game.
    pub(super) cancel: Option<CancellationToken>,
    /// The saved preferences, for what the window does around a game.
    pub(super) preferences: Preferences,
    /// A world the next launch goes straight into, once.
    pub(super) next_world: Option<String>,
    /// A server address the next launch goes straight onto, once.
    pub(super) next_server: Option<String>,
    /// The game whose Java conflict the person just answered: its next
    /// Continue starts without asking again.
    pub(super) java_answered: Option<String>,
    /// What the running game has printed: the instance and its newest lines.
    pub(super) game_log: Option<(String, std::collections::VecDeque<String>)>,
    /// Commands for the background update service.
    pub(super) update_commands: Option<tokio::sync::mpsc::UnboundedSender<update::UpdateCommand>>,
}

/// How many lines of the running game's output are kept.
pub(super) const GAME_LOG_LINES: usize = 2000;
/// The longest a new line waits before the open page shows it.
pub(super) const GAME_LOG_FLUSH: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub(super) struct Wiring {
    pub(super) backend: Rc<Backend>,
    pub(super) shell: WeakEntity<LauncherShell>,
    pub(super) state: Rc<RefCell<State>>,
}

/// What to reload once a job finishes. What follows the current game, the
/// library, accounts, Java and plugins is not chosen here: core publishes
/// those changes and [`context::follow`] works them out again.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Reload {
    Nothing,
    /// Activity, with the lists beside it.
    Lists,
}

/// Builds the production shell from stores opened before the UI loop starts.
pub fn build(backend: Backend, window: &mut Window, cx: &mut App) -> Entity<LauncherShell> {
    lumilio_ui::follow_system_appearance(window, cx);
    // Speak the system's language until the saved choice is read; it is
    // the default, so most people see no switch.
    lumilio_ui::i18n::apply_language(lumilio_core::Language::System, cx);
    let backend = Rc::new(backend);
    let state = Rc::new(RefCell::new(State::default()));
    let shell = cx.new(|cx: &mut Context<LauncherShell>| {
        let wiring = Wiring {
            backend: backend.clone(),
            shell: cx.weak_entity(),
            state: state.clone(),
        };
        let on_shell = {
            let wiring = wiring.clone();
            Rc::new(
                move |intent: ShellIntent, window: &mut Window, cx: &mut App| {
                    on_shell_intent(&wiring, intent, window, cx);
                },
            ) as IntentHandler
        };
        let on_live = {
            let wiring = wiring.clone();
            Rc::new(
                move |intent: LiveIntent, window: &mut Window, cx: &mut App| {
                    on_live_intent(&wiring, intent, window, cx);
                },
            ) as LiveHandler
        };
        LauncherShell::new(cx)
            .with_home(HomePresentation::Loading, cx)
            .with_intent_handler(on_shell)
            .with_live(on_live)
            .with_worker(backend.worker(), cx)
    });
    let wiring = Wiring {
        backend,
        shell: shell.downgrade(),
        state,
    };
    follow_discover(&wiring, &shell, cx);
    follow_new_game(&wiring, &shell, cx);
    follow_library(&wiring, &shell, cx);
    follow_activity(&wiring, &shell, cx);
    follow_settings(&wiring, &shell, cx);
    follow_accounts(&wiring, &shell, cx);
    follow_game(&wiring, &shell, cx);
    update::start(&wiring, cx);
    context::follow(&wiring, cx);
    reload(&wiring, Reload::Lists, cx);
    apply_saved_preferences(&wiring, window.window_handle(), cx);
    terms::ask(&wiring, window.window_handle(), cx);
    open_requested_page(&wiring, &shell, window, cx);
    // What start-up recovery did is told once, in words; details are in stderr.
    if let Some(text) = lumilio_ui::live::recovery_message(wiring.backend.service.startup_notes()) {
        shell.update(cx, |shell, cx| shell.toast(Toast::info(text).sticky(), cx));
    }
    shell
}

/// Passes on what Discover tells the application: its installs changed the
/// tasks Activity lists, or what it remembers between visits changed.
fn follow_discover(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let discover = shell.read(cx).discover().clone();
    let wiring = wiring.clone();
    cx.subscribe(&discover, move |_, event: &DiscoverEvent, cx| match event {
        DiscoverEvent::TasksChanged => reload(&wiring, Reload::Lists, cx),
        DiscoverEvent::Remember(discover) => {
            remember(&wiring, |preferences| {
                preferences.discover = discover.clone();
            });
        }
        DiscoverEvent::ProjectOpened(_) => {}
    })
    .detach();
}

/// Passes on what Activity tells the application: the tasks changed, or a
/// task ran again and may have changed what a game has.
fn follow_activity(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let activity = shell.read(cx).activity_page().clone();
    let wiring = wiring.clone();
    cx.subscribe(&activity, move |_, event: &ActivityEvent, cx| match event {
        ActivityEvent::ListsChanged => reload(&wiring, Reload::Lists, cx),
        ActivityEvent::Retried => jobs::installed_changed(&wiring, cx),
    })
    .detach();
}

/// Passes on what Settings tells the application: saved preferences to
/// apply, the updater's switch and check, and a Java download in Activity.
fn follow_settings(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let settings = shell.read(cx).settings_page().clone();
    let wiring = wiring.clone();
    cx.subscribe(&settings, move |_, event: &SettingsEvent, cx| match event {
        SettingsEvent::PreferencesSaved {
            preferences,
            window,
        } => {
            let (wiring, preferences) = (wiring.clone(), (**preferences).clone());
            let _ = cx.update_window(*window, move |_, window, cx| {
                apply_preferences(&wiring, &preferences, window, cx);
            });
        }
        SettingsEvent::AutoUpdate(enabled) => update::set_automatic(&wiring, *enabled),
        SettingsEvent::CheckUpdates => update::check_manually(&wiring),
        SettingsEvent::TasksChanged => reload(&wiring, Reload::Lists, cx),
        // The shell tells the game pages.
        SettingsEvent::PluginsRead(_) => {}
    })
    .detach();
}

/// Passes on what Accounts tells the application: an account's look changed
/// where the lists show it, or an account was added for a launch that waited
/// for one.
fn follow_accounts(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let accounts = shell.read(cx).accounts().clone();
    let wiring = wiring.clone();
    cx.subscribe(&accounts, move |_, event: &AccountsEvent, cx| match event {
        AccountsEvent::ListsChanged => reload(&wiring, Reload::Lists, cx),
        AccountsEvent::Continue(window) => {
            // The model learns of the account once core publishes it; give it a beat.
            let shell = wiring.shell.clone();
            let _ = cx.update_window(*window, move |_, window, cx| {
                window.defer(cx, move |window, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.request_continue(window, cx));
                });
            });
        }
        AccountsEvent::FacesChanged => {}
    })
    .detach();
}

/// Passes on what the Library tells the application: the lists changed, or
/// how the page is ordered and filtered.
fn follow_library(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let library = shell.read(cx).library().clone();
    let wiring = wiring.clone();
    cx.subscribe(&library, move |_, event: &LibraryEvent, cx| match *event {
        LibraryEvent::ListsChanged => reload(&wiring, Reload::Lists, cx),
        LibraryEvent::Remember { sort, loader } => {
            remember(&wiring, |preferences| {
                preferences.library_sort = sort;
                preferences.library_loader = loader;
            });
        }
    })
    .detach();
}

/// Saves something a page remembers between runs with the other
/// preferences. Quiet: a failure is not worth a message.
fn remember(wiring: &Wiring, change: impl FnOnce(&mut Preferences)) {
    let preferences = {
        let mut state = wiring.state.borrow_mut();
        change(&mut state.preferences);
        state.preferences.clone()
    };
    let service = wiring.backend.service.clone();
    drop(
        wiring
            .backend
            .spawn(async move { service.set_preferences(preferences).await }),
    );
}

/// Makes `id` the current game, through the shell. Deferred: this may run
/// inside one of the shell's own updates.
pub(super) fn choose_current(wiring: &Wiring, id: String, cx: &mut App) {
    let shell = wiring.shell.clone();
    cx.defer(move |cx| {
        let _ = shell.update(cx, |shell, cx| shell.choose_current(id, cx));
    });
}

/// Passes on what making games tells the application: the lists changed, a
/// new game is shown and made current, an open game page reads its download.
fn follow_new_game(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let new_game = shell.read(cx).new_game().clone();
    let wiring = wiring.clone();
    cx.subscribe(&new_game, move |_, event: &NewGameEvent, cx| match event {
        NewGameEvent::TasksChanged => reload(&wiring, Reload::Lists, cx),
        NewGameEvent::Created { id, window } => {
            let (wiring, id) = (wiring.clone(), id.clone());
            let _ = cx.update_window(*window, move |_, window, cx| {
                open_instance(&wiring, id.clone(), None, window, cx);
                choose_current(&wiring, id, cx);
            });
        }
        NewGameEvent::Installed(id) => {
            let open = wiring.shell.read_with(cx, |shell, cx| {
                let view = shell
                    .live_instance()
                    .filter(|view| view.read(cx).id() == id)
                    .cloned();
                view.map(|view| (shell.game().clone(), view))
            });
            if let Ok(Some((game, view))) = open {
                game.update(cx, |game, cx| game.reload(&view, cx));
            }
        }
    })
    .detach();
}

/// Passes on what a game's page tells the application: the lists changed, or
/// a page opened that should show what its running game printed so far.
fn follow_game(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let game = shell.read(cx).game().clone();
    game.update(cx, |game, _| {
        game.set_windows(GameWindows {
            app_id: Some(crate::APP_ID.to_owned()),
            studio_size: crate::review_size_hint(),
        });
    });
    let wiring = wiring.clone();
    cx.subscribe(&game, move |_, event: &GameEvent, cx| match event {
        GameEvent::ListsChanged => reload(&wiring, Reload::Lists, cx),
        GameEvent::Opened(id) => {
            // A game on its way or running has printed things the page should
            // show; which game that is, core's session says.
            let active = wiring
                .backend
                .service
                .context()
                .session
                .is_some_and(|session| session.is_active() && session.instance == *id);
            let running = wiring
                .state
                .borrow()
                .game_log
                .as_ref()
                .filter(|(running_id, _)| active && running_id == id)
                .map(|(_, lines)| lines.iter().cloned().collect::<Vec<_>>());
            if let Some(lines) = running {
                let _ = wiring
                    .shell
                    .update(cx, |shell, cx| shell.game_output(id, lines, true, cx));
            }
        }
        // The shell forgets it.
        GameEvent::Deleted(_) => {}
    })
    .detach();
}

/// Dev hook: `LUMILIO_PAGE=library|discover|activity|accounts` opens that page, so a
/// screen can be reviewed without clicking through to it.
pub(super) fn open_requested_page(
    wiring: &Wiring,
    shell: &Entity<LauncherShell>,
    window: &mut Window,
    cx: &mut App,
) {
    // `LUMILIO_HOME_RECOVERY=failed|details|crashed|crashed-details|away` shows a launch of
    // the current game going wrong (`launch::review_recovery`).
    if let Ok(which) = std::env::var("LUMILIO_HOME_RECOVERY") {
        launch::review_recovery(wiring, which, window.window_handle(), cx);
        return;
    }
    // `LUMILIO_PROJECT=mod/sodium` also opens that project's detail window;
    // `LUMILIO_PROJECT_SOURCE=lumilio.curseforge` takes it from that source.
    if let Ok(path) = std::env::var("LUMILIO_PROJECT")
        && let Some((kind, slug)) = path.split_once('/')
        && let Some(kind) = ProjectKind::from_protocol(kind)
    {
        let source = std::env::var("LUMILIO_PROJECT_SOURCE")
            .unwrap_or_else(|_| lumilio_core::DEFAULT_SOURCE.to_owned());
        let slug = slug.to_owned();
        shell.update(cx, |shell, cx| shell.open_project(kind, &source, &slug, cx));
    }
    // Dev hook: `LUMILIO_INSTANCE=<id>` opens that game; `LUMILIO_INSTANCE_TAB`
    // chooses its tab and `LUMILIO_INSTANCE_GROUP` the tab's group (a
    // settings group, or 1 for the Worlds tab's servers), and
    // `LUMILIO_STUDIO=schematics/<file>` opens its schematic studio on a file
    // (`LUMILIO_STUDIO_BLOCK=minecraft:<id>` chooses that block kind there,
    // `LUMILIO_STUDIO_INSPECT=<x>,<y>,<z>` inspects the block at that place,
    // `LUMILIO_STUDIO_DIALOG=replace|convert|incompatible` opens a dialog).
    if let Ok(id) = std::env::var("LUMILIO_INSTANCE") {
        if let Ok(file) = std::env::var("LUMILIO_STUDIO") {
            let inspect = std::env::var("LUMILIO_STUDIO_INSPECT").ok().and_then(|at| {
                let at: Vec<i32> = at
                    .split(',')
                    .filter_map(|v| v.trim().parse().ok())
                    .collect();
                <[i32; 3]>::try_from(at).ok()
            });
            let game = shell.read(cx).game().clone();
            game.update(cx, |game, cx| {
                game.open_studio(
                    id.clone(),
                    lumilio_plugin_litematica::ID.to_owned(),
                    "schematics".to_owned(),
                    Some(file),
                    cx,
                );
                if let Ok(block) = std::env::var("LUMILIO_STUDIO_BLOCK") {
                    game.studios_choose(&id, &block, cx);
                }
                if let Some(position) = inspect {
                    game.studios_inspect(&id, position, cx);
                }
                if let Ok(which) = std::env::var("LUMILIO_STUDIO_DIALOG") {
                    game.studios_dialog(&id, &which, cx);
                }
            });
        }
        open_instance(wiring, id, None, window, cx);
        let tab = std::env::var("LUMILIO_INSTANCE_TAB")
            .ok()
            .and_then(|v| v.parse().ok());
        let group = std::env::var("LUMILIO_INSTANCE_GROUP")
            .ok()
            .and_then(|v| v.parse().ok());
        // `LUMILIO_INSTANCE_PLUGIN=<plugin id>` opens that plugin's pane.
        let plugin = std::env::var("LUMILIO_INSTANCE_PLUGIN").ok();
        let shell = shell.clone();
        window.defer(cx, move |window, cx| {
            shell.update(cx, |shell, cx| {
                if let Some(view) = shell.live_instance().cloned() {
                    view.update(cx, |view, cx| {
                        // As a click on the tab: what it shows is loaded too.
                        if let Some(tab) = tab {
                            view.open_tab(tab, window, cx);
                        }
                        match group {
                            Some(group) if tab == Some(TAB_WORLDS) => {
                                view.open_worlds_sub(group, window, cx);
                            }
                            Some(group) => view.select_settings_group(group, cx),
                            None => {}
                        }
                        if let Some(plugin) = plugin {
                            view.open_plugin_pane(plugin, window, cx);
                        }
                    });
                }
            });
        });
        return;
    }
    // `LUMILIO_ACTIVITY=java|graalvm` downloads Java 21 from that source, as
    // 重试 would, so Activity shows a real download; `LUMILIO_ACTIVITY_DETAIL=1`
    // opens its files and `LUMILIO_ACTIVITY_PAUSE=<seconds>` pauses it then.
    if let Ok(which) = std::env::var("LUMILIO_ACTIVITY") {
        review_download(shell, &which, cx);
    }
    let route = match std::env::var("LUMILIO_PAGE").as_deref() {
        Ok("library") => Route::Library,
        Ok("discover") => Route::Discover,
        Ok("activity") => Route::Activity,
        Ok("accounts") => Route::Accounts,
        Ok("settings") => Route::Settings,
        _ => return,
    };
    // As choosing it in the navigation: the page is told it was shown.
    shell.update(cx, |shell, cx| shell.go_to(route, window, cx));
}

/// The dev hook `LUMILIO_ACTIVITY` (see [`open_requested_page`]).
pub(super) fn review_download(shell: &Entity<LauncherShell>, which: &str, cx: &mut App) {
    use lumilio_core::{JavaSource, RetryAction};
    let source = match which {
        "graalvm" => JavaSource::OracleGraalVm,
        _ => JavaSource::Mojang,
    };
    let activity = shell.read(cx).activity_page().clone();
    activity.update(cx, |activity, cx| {
        activity.retry(
            RetryAction::InstallJava {
                major: Some(21),
                source,
                build: lumilio_core::JavaBuild::default(),
            },
            cx,
        );
        // The first task the service starts has id 1.
        if std::env::var("LUMILIO_ACTIVITY_DETAIL").is_ok() {
            activity.apply_view_intent(lumilio_ui::kit::ViewIntent::TaskDetail(1), cx);
        }
    });
    if let Some(seconds) = std::env::var("LUMILIO_ACTIVITY_PAUSE")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        let activity = activity.downgrade();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(seconds))
                .await;
            let _ = activity.update(cx, |activity, cx| activity.pause(1, cx));
        })
        .detach();
    }
}
