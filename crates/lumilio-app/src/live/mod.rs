//! Connects the interface to the launcher service (plan 0007).
//!
//! The shell only renders and reports intents. Here each intent becomes work on
//! the worker runtime, and each result is handed back to the shell on the
//! interface thread. The shell is never touched from inside one of its own
//! updates: results always arrive from a spawned task.

mod accounts;
mod discover;
mod dispatch;
mod game_settings;
mod instance;
mod instance_content;
mod instance_plugins;
mod instance_write;
mod jobs;
mod launch;
mod library;
mod logs;
mod new_game;
mod settings;
mod translation;
mod update;
mod wardrobe;

#[cfg(test)]
mod tests;

use self::discover::open_project;
use self::dispatch::{on_live_intent, on_shell_intent};
use self::instance::open_instance;
use self::jobs::reload;
use self::settings::apply_saved_preferences;
use crate::backend::Backend;
use gpui_kit::AppContext as _;
use gpui_kit::{App, Context, Entity, WeakEntity, Window};
use lumilio_core::{CancellationToken, Preferences, ProjectKind};
use lumilio_ui::LauncherShell;
use lumilio_ui::home::HomePresentation;
use lumilio_ui::instance_detail::TAB_WORLDS;
use lumilio_ui::live::{LiveHandler, LiveIntent};
use lumilio_ui::route::Route;
use lumilio_ui::shell::{IntentHandler, ShellIntent};
use lumilio_ui::toast::Toast;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// How many finished tasks the Activity page lists.
const ACTIVITY_LIMIT: usize = 30;
/// How often running installs refresh the Activity page.
const POLL: Duration = Duration::from_millis(600);

#[derive(Default)]
struct State {
    /// Older snapshots and diagnostics cannot replace a newer request.
    reload_seq: u64,
    home_seq: u64,
    home_diagnostics: Option<tokio::task::AbortHandle>,
    selection_seq: u64,
    selection_save: Option<tokio::task::AbortHandle>,
    /// The instance Home's Continue launches.
    continue_id: Option<String>,
    /// Stops the launch or the running game.
    cancel: Option<CancellationToken>,
    /// Numbers searches so a slow answer cannot replace a newer one.
    search_seq: u64,
    /// The saved preferences, for what the window does around a game.
    preferences: Preferences,
    /// A world the next launch goes straight into, once.
    next_world: Option<String>,
    /// A server address the next launch goes straight onto, once.
    next_server: Option<String>,
    /// The game whose Java conflict the person just answered: its next
    /// Continue starts without asking again.
    java_answered: Option<String>,
    /// Stops the Microsoft sign-in that is waiting for the browser.
    sign_in_cancel: Option<CancellationToken>,
    /// The target game, kind and source `installed` was last read for; `None`
    /// when it is out of date and should be read again.
    installed_key: Option<(String, ProjectKind, String)>,
    /// What the running game has printed: the instance and its newest lines.
    game_log: Option<(String, std::collections::VecDeque<String>)>,
    /// Commands for the background update service.
    update_commands: Option<tokio::sync::mpsc::UnboundedSender<update::UpdateCommand>>,
    /// The open schematic studio windows, one per game and folder (plan
    /// schematic-studio, D22). A closed window's entry is dropped on the
    /// next look.
    studio_windows: Vec<instance_plugins::StudioWindowEntry>,
}

/// How many lines of the running game's output are kept.
const GAME_LOG_LINES: usize = 2000;
/// The longest a new line waits before the open page shows it.
const GAME_LOG_FLUSH: Duration = Duration::from_millis(150);

#[derive(Clone)]
struct Wiring {
    backend: Rc<Backend>,
    shell: WeakEntity<LauncherShell>,
    state: Rc<RefCell<State>>,
}

/// What to reload once a job finishes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reload {
    Nothing,
    /// Library and Activity.
    Lists,
    /// Current game and Home from saved data, without rerunning diagnostics.
    Selection,
    /// Everything, including Home.
    All,
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
    });
    let wiring = Wiring {
        backend,
        shell: shell.downgrade(),
        state,
    };
    update::start(&wiring, cx);
    reload(&wiring, Reload::All, cx);
    apply_saved_preferences(&wiring, window.window_handle(), cx);
    update::announce(&wiring, window.window_handle(), cx);
    open_requested_page(&wiring, &shell, window, cx);
    // What start-up recovery did is told once, in words; details are in stderr.
    if let Some(text) = lumilio_ui::live::recovery_message(wiring.backend.service.startup_notes()) {
        shell.update(cx, |shell, cx| shell.toast(Toast::info(text).sticky(), cx));
    }
    shell
}

/// Dev hook: `LUMILIO_PAGE=library|discover|activity|accounts` opens that page, so a
/// screen can be reviewed without clicking through to it.
fn open_requested_page(
    wiring: &Wiring,
    shell: &Entity<LauncherShell>,
    window: &mut Window,
    cx: &mut App,
) {
    // `LUMILIO_PROJECT=mod/sodium` also opens that project's detail window;
    // `LUMILIO_PROJECT_SOURCE=lumilio.curseforge` takes it from that source.
    if let Ok(path) = std::env::var("LUMILIO_PROJECT")
        && let Some((kind, slug)) = path.split_once('/')
        && let Some(kind) = ProjectKind::from_protocol(kind)
    {
        let source = std::env::var("LUMILIO_PROJECT_SOURCE")
            .unwrap_or_else(|_| lumilio_core::DEFAULT_SOURCE.to_owned());
        open_project(wiring, kind, source, slug.to_owned(), window, cx);
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
            instance_plugins::open_studio_window(
                wiring,
                id.clone(),
                lumilio_plugin_litematica::ID.to_owned(),
                "schematics".to_owned(),
                Some(file),
                cx,
            );
            if let Ok(block) = std::env::var("LUMILIO_STUDIO_BLOCK") {
                instance_plugins::studio_windows_choose(wiring, &id, &block, cx);
            }
            let inspect = std::env::var("LUMILIO_STUDIO_INSPECT").ok().and_then(|at| {
                let at: Vec<i32> = at
                    .split(',')
                    .filter_map(|v| v.trim().parse().ok())
                    .collect();
                <[i32; 3]>::try_from(at).ok()
            });
            if let Some(position) = inspect {
                instance_plugins::studio_windows_inspect(wiring, &id, position, cx);
            }
            if let Ok(which) = std::env::var("LUMILIO_STUDIO_DIALOG") {
                instance_plugins::studio_windows_dialog(wiring, &id, &which, cx);
            }
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
    let route = match std::env::var("LUMILIO_PAGE").as_deref() {
        Ok("library") => Route::Library,
        Ok("discover") => Route::Discover,
        Ok("activity") => Route::Activity,
        Ok("accounts") => Route::Accounts,
        Ok("settings") => Route::Settings,
        _ => return,
    };
    shell.update(cx, |shell, _| shell.show(route));
    on_shell_intent(wiring, ShellIntent::Navigate(route), window, cx);
}
