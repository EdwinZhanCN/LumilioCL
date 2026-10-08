//! Connects the interface to the launcher service (plan 0007).
//!
//! The shell only renders and reports intents. Here each intent becomes work on
//! the worker runtime, and each result is handed back to the shell on the
//! interface thread. The shell is never touched from inside one of its own
//! updates: results always arrive from a spawned task.

mod accounts;
mod discover;
mod dispatch;
mod instance;
mod instance_plugins;
mod instance_write;
mod jobs;
mod launch;
mod library;
mod logs;
mod new_game;
mod settings;
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
    /// Stops the Microsoft sign-in that is waiting for the browser.
    sign_in_cancel: Option<CancellationToken>,
    /// The target game and kind `installed` was last read for; `None` when it
    /// is out of date and should be read again.
    installed_key: Option<(String, ProjectKind)>,
    /// What the running game has printed: the instance and its newest lines.
    game_log: Option<(String, std::collections::VecDeque<String>)>,
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
    reload(&wiring, Reload::All, cx);
    apply_saved_preferences(&wiring, window.window_handle(), cx);
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
    // `LUMILIO_PROJECT=mod/sodium` also opens that project's detail window.
    if let Ok(path) = std::env::var("LUMILIO_PROJECT")
        && let Some((kind, slug)) = path.split_once('/')
        && let Some(kind) = ProjectKind::from_protocol(kind)
    {
        open_project(wiring, kind, slug.to_owned(), window, cx);
    }
    // Dev hook: `LUMILIO_INSTANCE=<id>` opens that game; `LUMILIO_INSTANCE_TAB`
    // and `LUMILIO_INSTANCE_GROUP` choose its tab and settings group.
    if let Ok(id) = std::env::var("LUMILIO_INSTANCE") {
        open_instance(wiring, id, None, window, cx);
        let tab = std::env::var("LUMILIO_INSTANCE_TAB")
            .ok()
            .and_then(|v| v.parse().ok());
        let group = std::env::var("LUMILIO_INSTANCE_GROUP")
            .ok()
            .and_then(|v| v.parse().ok());
        let shell = shell.clone();
        window.defer(cx, move |_, cx| {
            shell.update(cx, |shell, cx| {
                if let Some(view) = shell.live_instance().cloned() {
                    view.update(cx, |view, cx| {
                        if let Some(tab) = tab {
                            view.select_tab(tab, cx);
                        }
                        if let Some(group) = group {
                            view.select_settings_group(group, cx);
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
