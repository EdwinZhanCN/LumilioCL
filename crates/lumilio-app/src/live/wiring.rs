use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::{App, AppContext as _, Context, Entity, WeakEntity, Window};
use lumilio_core::{Preferences, ProjectKind};
use lumilio_ui::LauncherShell;
use lumilio_ui::features::discover::DiscoverEvent;
use lumilio_ui::features::game::GameWindows;
use lumilio_ui::features::library::LibraryEvent;
use lumilio_ui::features::settings::SettingsEvent;
use lumilio_ui::home::HomePresentation;
use lumilio_ui::instance_detail::TAB_WORLDS;
use lumilio_ui::live::{LiveHandler, LiveIntent};
use lumilio_ui::route::Route;
use lumilio_ui::toast::Toast;

use super::dispatch::on_live_intent;
use super::open_game::open_instance;
use super::settings::{apply_preferences, apply_saved_preferences};
use super::{recovery_review, terms, update};
use crate::backend::Backend;

#[derive(Default)]
pub(super) struct State {
    /// The saved preferences, which a page's remembered choices are saved with.
    pub(super) preferences: Preferences,
    /// Commands for the background update service.
    pub(super) update_commands: Option<tokio::sync::mpsc::UnboundedSender<update::UpdateCommand>>,
}

#[derive(Clone)]
pub(super) struct Wiring {
    pub(super) backend: Rc<Backend>,
    pub(super) shell: WeakEntity<LauncherShell>,
    pub(super) state: Rc<RefCell<State>>,
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
            .with_live(on_live)
            .with_worker(backend.worker(), cx)
    });
    let wiring = Wiring {
        backend,
        shell: shell.downgrade(),
        state,
    };
    follow_discover(&wiring, &shell, cx);
    follow_library(&wiring, &shell, cx);
    follow_settings(&wiring, &shell, cx);
    set_game_windows(&shell, cx);
    update::start(&wiring, cx);
    shell.update(cx, |shell, cx| shell.follow_launcher(cx));
    apply_saved_preferences(&wiring, window.window_handle(), cx);
    terms::ask(&wiring, window.window_handle(), cx);
    open_requested_page(&wiring, &shell, window, cx);
    // What start-up recovery did is told once, in words; details are in stderr.
    if let Some(text) = lumilio_ui::live::recovery_message(wiring.backend.service.startup_notes()) {
        shell.update(cx, |shell, cx| shell.toast(Toast::info(text).sticky(), cx));
    }
    shell
}

/// Saves what Discover remembers between visits. The shell answers the rest.
fn follow_discover(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let discover = shell.read(cx).discover().clone();
    let wiring = wiring.clone();
    cx.subscribe(&discover, move |_, event: &DiscoverEvent, _| match event {
        DiscoverEvent::TasksChanged | DiscoverEvent::ProjectOpened(_) => {}
        DiscoverEvent::Remember(discover) => {
            remember(&wiring, |preferences| {
                preferences.discover = discover.clone();
            });
        }
    })
    .detach();
}

/// Passes on what Settings tells the application: saved preferences to
/// apply, and the updater's switch and check.
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
        // The shell reads the lists again and tells the game pages.
        SettingsEvent::TasksChanged | SettingsEvent::PluginsRead(_) => {}
    })
    .detach();
}

/// Saves how the Library is ordered and filtered. The shell answers the rest.
fn follow_library(wiring: &Wiring, shell: &Entity<LauncherShell>, cx: &mut App) {
    let library = shell.read(cx).library().clone();
    let wiring = wiring.clone();
    cx.subscribe(&library, move |_, event: &LibraryEvent, _| match *event {
        LibraryEvent::ListsChanged => {}
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

/// Where the game pages open their own windows.
fn set_game_windows(shell: &Entity<LauncherShell>, cx: &mut App) {
    let game = shell.read(cx).game().clone();
    game.update(cx, |game, _| {
        game.set_windows(GameWindows {
            app_id: Some(crate::APP_ID.to_owned()),
            studio_size: crate::review_size_hint(),
        });
    });
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
    // the current game going wrong (`recovery_review::review_recovery`).
    if let Ok(which) = std::env::var("LUMILIO_HOME_RECOVERY") {
        recovery_review::review_recovery(wiring, which, window.window_handle(), cx);
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
