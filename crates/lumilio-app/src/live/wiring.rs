use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::{App, AppContext as _, Context, Entity, WeakEntity, Window};
use lumilio_core::{CancellationToken, Preferences, ProjectKind};
use lumilio_ui::LauncherShell;
use lumilio_ui::features::discover::DiscoverEvent;
use lumilio_ui::home::HomePresentation;
use lumilio_ui::instance_detail::TAB_WORLDS;
use lumilio_ui::live::{LiveHandler, LiveIntent};
use lumilio_ui::route::Route;
use lumilio_ui::shell::{IntentHandler, ShellIntent};
use lumilio_ui::toast::Toast;

use super::dispatch::{on_live_intent, on_shell_intent};
use super::instance::open_instance;
use super::jobs::reload;
use super::settings::apply_saved_preferences;
use super::{context, instance_plugins, jobs, launch, library, terms, update};
use crate::backend::Backend;

/// How many finished tasks the Activity page lists.
pub(super) const ACTIVITY_LIMIT: usize = 30;
/// How often running installs refresh the Activity page.
pub(super) const POLL: Duration = Duration::from_millis(600);

#[derive(Default)]
pub(super) struct State {
    /// An older list snapshot cannot replace a newer request.
    pub(super) reload_seq: u64,
    /// The background checks of Home's games now running.
    pub(super) home_diagnostics: Option<tokio::task::AbortHandle>,
    /// Each game's last checks, for Home's 需要留意.
    pub(super) home_checks: Option<context::Checked>,
    /// Choices of the current game, saved in order.
    pub(super) choices: Option<tokio::sync::mpsc::UnboundedSender<library::Choice>>,
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
    /// Stops the Microsoft sign-in that is waiting for the browser.
    pub(super) sign_in_cancel: Option<CancellationToken>,
    /// What the running game has printed: the instance and its newest lines.
    pub(super) game_log: Option<(String, std::collections::VecDeque<String>)>,
    /// Commands for the background update service.
    pub(super) update_commands: Option<tokio::sync::mpsc::UnboundedSender<update::UpdateCommand>>,
    /// The open schematic studio windows, one per game and folder (plan
    /// schematic-studio, D22). A closed window's entry is dropped on the
    /// next look.
    pub(super) studio_windows: Vec<instance_plugins::StudioWindowEntry>,
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
            let preferences = {
                let mut state = wiring.state.borrow_mut();
                state.preferences.discover = discover.clone();
                state.preferences.clone()
            };
            // Quiet, like the Library's order: not worth a message when it fails.
            let service = wiring.backend.service.clone();
            drop(
                wiring
                    .backend
                    .spawn(async move { service.set_preferences(preferences).await }),
            );
        }
        DiscoverEvent::ProjectOpened(_) => {}
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
    // `LUMILIO_ACTIVITY=java|graalvm` downloads Java 21 from that source, as
    // 重试 would, so Activity shows a real download; `LUMILIO_ACTIVITY_DETAIL=1`
    // opens its files and `LUMILIO_ACTIVITY_PAUSE=<seconds>` pauses it then.
    if let Ok(which) = std::env::var("LUMILIO_ACTIVITY") {
        review_download(wiring, shell, &which, cx);
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
pub(super) fn review_download(
    wiring: &Wiring,
    shell: &Entity<LauncherShell>,
    which: &str,
    cx: &mut App,
) {
    use lumilio_core::{JavaSource, RetryAction};
    let source = match which {
        "graalvm" => JavaSource::OracleGraalVm,
        _ => JavaSource::Mojang,
    };
    jobs::retry_task(
        wiring,
        RetryAction::InstallJava {
            major: Some(21),
            source,
            build: lumilio_core::JavaBuild::default(),
        },
        cx,
    );
    // The first task the service starts has id 1.
    if std::env::var("LUMILIO_ACTIVITY_DETAIL").is_ok() {
        shell.update(cx, |shell, cx| {
            shell.apply_view_intent(lumilio_ui::kit::ViewIntent::TaskDetail(1), cx);
        });
    }
    if let Some(seconds) = std::env::var("LUMILIO_ACTIVITY_PAUSE")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        let wiring = wiring.clone();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(seconds))
                .await;
            wiring.backend.service.pause_task(1);
            cx.update(|cx| reload(&wiring, Reload::Lists, cx));
        })
        .detach();
    }
}
