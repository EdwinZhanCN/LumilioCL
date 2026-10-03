//! Connects the interface to the launcher service (plan 0007).
//!
//! The shell only renders and reports intents. Here each intent becomes work on
//! the worker runtime, and each result is handed back to the shell on the
//! interface thread. The shell is never touched from inside one of its own
//! updates: results always arrive from a spawned task.

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::{App, AppContext as _, Context, Entity, WeakEntity, Window};
use lumilio_core::{
    AfterLaunch, CancellationToken, ContentEffect, ContentResult, LaunchFailure,
    LaunchServiceError, LaunchSignal, LaunchUpdate, Preferences, ProjectKind, ServiceError,
    SnapshotScope, unix_now,
};
use lumilio_ui::LauncherShell;
use lumilio_ui::accounts::{AccountForm, AccountRequest};
use lumilio_ui::collections::{CollectionPicker, NamePrompt};
use lumilio_ui::dependency_prompt::DependencyPrompt;
use lumilio_ui::export_form::ExportForm;
use lumilio_ui::game_picker::GamePicker;
use lumilio_ui::home::{HomeIntent, HomePresentation, Subject};
use lumilio_ui::instance_detail::{
    Arrived, InstanceDetailView, InstanceIntent, Operated, Section, export_notice,
};
use lumilio_ui::live::{
    CollectionRow, DiscoverQuery, FilterModel, LiveHandler, LiveIntent, SearchStatus,
    account_failure, account_rows, activity_rows, attention_rows, home_presentation, library_cards,
    search_rows, settings_view,
};
use lumilio_ui::microsoft_login::{SignInDialog, SignInIntent};
use lumilio_ui::new_game::{NewGameForm, NewGameIntent, NewGameRequest, RuntimeChange};
use lumilio_ui::platform;
use lumilio_ui::project_detail::DetailState;
use lumilio_ui::route::Route;
use lumilio_ui::shell::{IntentHandler, ShellIntent};
use lumilio_ui::toast::Toast;

use crate::backend::Backend;

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

fn on_shell_intent(wiring: &Wiring, intent: ShellIntent, window: &mut Window, cx: &mut App) {
    match intent {
        ShellIntent::Home(HomeIntent::Continue | HomeIntent::Recover) => {
            // Asked of the service, not the page: an account added a moment ago
            // may not have reached the page's model yet.
            let service = wiring.backend.service.clone();
            let work = wiring.backend.spawn(async move {
                account_rows(&service.settings().await)
                    .iter()
                    .any(|row| row.selected)
            });
            let wiring = wiring.clone();
            let window = window.window_handle();
            cx.spawn(async move |cx| {
                if work.await.unwrap_or(true) {
                    cx.update(|cx| start_launch(&wiring, window, cx));
                    return;
                }
                // No account: ask for one, then start where the person left off.
                let _ = wiring.shell.update(cx, |shell, cx| shell.abort_launch(cx));
                let _ = cx.update_window(window, |_, window, cx| {
                    open_add_account(&wiring, true, window, cx);
                });
            })
            .detach();
        }
        ShellIntent::Home(HomeIntent::CancelLaunch | HomeIntent::StopGame) => {
            if let Some(cancel) = wiring.state.borrow().cancel.clone() {
                cancel.cancel();
            }
        }
        ShellIntent::Home(HomeIntent::Create) => open_new_game(wiring, window, cx),
        ShellIntent::Navigate(Route::Settings) => load_settings(wiring, cx),
        ShellIntent::Navigate(Route::Library | Route::Activity | Route::Accounts) => {
            reload(wiring, Reload::Lists, cx)
        }
        ShellIntent::Navigate(Route::Discover) => first_search(wiring, cx),
        ShellIntent::Navigate(Route::Home)
        | ShellIntent::Home(HomeIntent::Import | HomeIntent::TechnicalDetails) => {}
    }
}

fn on_live_intent(wiring: &Wiring, intent: LiveIntent, window: &mut Window, cx: &mut App) {
    match intent {
        LiveIntent::NewInstance => open_new_game(wiring, window, cx),
        LiveIntent::ImportPack => import_pack(wiring, cx),
        LiveIntent::OpenInstance(id) => open_instance(wiring, id, None, window, cx),
        LiveIntent::SaveVersionAs {
            project,
            version_id,
            file_name,
            title,
        } => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let chosen = platform::pick_save_path(cx, &start, &file_name);
            let wiring = wiring.clone();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                cx.update(|cx| {
                    let service = wiring.backend.service.clone();
                    let target = path.clone();
                    job(
                        &wiring,
                        cx,
                        true,
                        Reload::Lists,
                        Some(Toast::info(format!("开始下载 {title}，进度在动态里"))),
                        async move {
                            service
                                .save_version_as(
                                    &project,
                                    &version_id,
                                    &target,
                                    CancellationToken::new(),
                                )
                                .await
                        },
                        move |shell, result, cx| {
                            shell.toast(
                                outcome(
                                    result,
                                    |_| format!("已保存到 {}", path.display()),
                                    format!("没有下载 {title}"),
                                ),
                                cx,
                            )
                        },
                    );
                });
            })
            .detach();
        }
        LiveIntent::UpdateInstalled {
            kind,
            project,
            title,
            file_name,
            version_id,
        } => {
            let Some((instance, instance_name)) = install_target(wiring, cx) else {
                return;
            };
            let service = wiring.backend.service.clone();
            let state = wiring.state.clone();
            job(
                wiring,
                cx,
                true,
                Reload::Lists,
                Some(Toast::info(format!("开始更新 {title}，进度在动态里"))),
                async move {
                    service
                        .switch_content_version(
                            &instance,
                            kind,
                            &file_name,
                            &project,
                            &version_id,
                            CancellationToken::new(),
                        )
                        .await
                },
                move |shell, result, cx| {
                    state.borrow_mut().installed_key = None;
                    shell.toast(
                        outcome(
                            result,
                            |file| format!("已更新为 {file}（{instance_name}）"),
                            format!("没有更新 {title}"),
                        ),
                        cx,
                    )
                },
            );
        }
        LiveIntent::Resolve(id, action) => open_instance(
            wiring,
            id,
            Some(InstanceIntent::Resolve(action)),
            window,
            cx,
        ),
        LiveIntent::CopyGameOf(id) => {
            open_instance(wiring, id, Some(InstanceIntent::AskCopy), window, cx)
        }
        LiveIntent::DeleteGameOf(id) => {
            open_instance(wiring, id, Some(InstanceIntent::AskDelete), window, cx)
        }
        LiveIntent::RevealGame(id) => {
            cx.reveal_path(&wiring.backend.service.game_dir(&id));
        }
        LiveIntent::DropFiles(paths) => {
            let packs: Vec<_> = paths
                .into_iter()
                .filter(|path| {
                    path.extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("mrpack") || ext.eq_ignore_ascii_case("zip")
                    })
                })
                .collect();
            if packs.is_empty() {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::info("这里只能放整合包（.mrpack 或 .zip）"), cx)
                });
            }
            for path in packs {
                import_pack_file(wiring, path, cx);
            }
        }
        LiveIntent::ExportPackOf(id) => {
            open_instance(wiring, id, Some(InstanceIntent::ExportPack), window, cx)
        }
        LiveIntent::Refresh => reload(wiring, Reload::All, cx),
        LiveIntent::ToggleFavorite(id) => {
            let service = wiring.backend.service.clone();
            job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.toggle_favorite(&id).await },
                |_, _, _| {},
            );
        }
        LiveIntent::CancelTask(id) => {
            // The task finishes on its own terms; the next poll shows the outcome.
            wiring.backend.service.cancel_task(id);
        }
        LiveIntent::RetryTask(action) => retry_task(wiring, action, cx),
        LiveIntent::ClearFinished => {
            let service = wiring.backend.service.clone();
            job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.clear_finished() },
                |_, _, _| {},
            );
        }
        LiveIntent::NewCollection => ask_collection_name(wiring, None, window, cx),
        LiveIntent::RenameCollection(name) => ask_collection_name(wiring, Some(name), window, cx),
        LiveIntent::DeleteCollection(name) => {
            let service = wiring.backend.service.clone();
            job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.delete_collection(&name).await.map(|()| name) },
                |shell, result, cx| {
                    shell.toast(
                        outcome(
                            result,
                            |name| format!("已删除合集“{name}”"),
                            "没能删除合集".to_owned(),
                        ),
                        cx,
                    )
                },
            );
        }
        LiveIntent::EditCollections(id) => ask_collections_of(wiring, id, window, cx),
        LiveIntent::InstallTarget(id) => choose_current(wiring, id, cx),
        LiveIntent::NewAccount => open_add_account(wiring, false, window, cx),
        LiveIntent::MicrosoftSignIn => open_microsoft_sign_in(wiring, window, cx),
        LiveIntent::RefreshAccount(key) => {
            let service = wiring.backend.service.clone();
            job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.refresh_account(&key).await },
                |shell, result, cx| {
                    shell.toast(
                        match result {
                            Ok(()) => Toast::success("登录已刷新"),
                            Err(error) => {
                                let (message, technical) = account_failure(&error);
                                Toast::error(message).technical(technical)
                            }
                        },
                        cx,
                    );
                },
            );
        }
        LiveIntent::SelectAccount(name) => {
            let service = wiring.backend.service.clone();
            job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.select_account(&name).await },
                |shell, result, cx| {
                    if let Err(error) = result {
                        shell.toast(
                            Toast::error("没能切换账户").technical(error.to_string()),
                            cx,
                        );
                    }
                },
            );
        }
        LiveIntent::RemoveAccount(name) => {
            let service = wiring.backend.service.clone();
            let removed = name.clone();
            job(
                wiring,
                cx,
                false,
                Reload::Lists,
                None,
                async move { service.remove_account(&name).await },
                move |shell, result, cx| {
                    shell.toast(
                        outcome(
                            result,
                            |()| format!("已移除账户 {removed}"),
                            "没能移除账户".to_owned(),
                        ),
                        cx,
                    );
                },
            );
        }
        LiveIntent::LoadSettings => load_settings(wiring, cx),
        LiveIntent::RememberLibraryView { sort, loader } => {
            let preferences = {
                let mut state = wiring.state.borrow_mut();
                state.preferences.library_sort = sort;
                state.preferences.library_loader = loader;
                state.preferences.clone()
            };
            // Quiet: a failure to remember a sort order is not worth a message.
            let service = wiring.backend.service.clone();
            drop(
                wiring
                    .backend
                    .spawn(async move { service.set_preferences(preferences).await }),
            );
        }
        LiveIntent::SetPreferences(preferences) => {
            let service = wiring.backend.service.clone();
            let saved = preferences.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_preferences(saved).await },
                "没能保存设置",
                None,
                move |window, cx| apply_preferences(&preferences, window, cx),
                cx,
            );
        }
        LiveIntent::SetLaunchDefaults(launch) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_launch_defaults(launch).await },
                "没能保存设置",
                Some("已保存，下次启动游戏时生效"),
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetMemory { min_mb, max_mb } => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_default_memory(min_mb, max_mb).await },
                "没能保存内存设置",
                Some("已保存，下次启动游戏时生效"),
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetConcurrency(count) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_download_concurrency(count).await },
                "没能保存设置",
                Some("已保存，下次下载时生效"),
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetMirrors { mirrors, prefer } => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_mirrors(mirrors, prefer).await },
                "没能保存镜像设置",
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetJavaRoots(roots) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_java_roots(roots).await },
                "没能保存搜索目录",
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetJavaDisabled { home, disabled } => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_java_disabled(&home, disabled).await },
                "没能保存 Java 设置",
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::AddJava => add_java(wiring, cx),
        LiveIntent::InstallJava(major) => install_java(wiring, major, cx),
        LiveIntent::OpenGamesFolder => {
            cx.reveal_path(&wiring.backend.service.layout().profiles());
        }
        LiveIntent::RestoreBackup => restore_backup(wiring, cx),
        LiveIntent::ImportGame => import_game(wiring, window, cx),
        LiveIntent::Reveal(path) => platform::reveal(&path, cx),
        LiveIntent::ClearCache => clear_cache(wiring, cx),
        LiveIntent::CheckReclaimable => check_reclaimable(wiring, window, cx),
        LiveIntent::Reclaim => reclaim(wiring, cx),
        LiveIntent::ExportDiagnostics => export_diagnostics(wiring, cx),
        LiveIntent::CopyText { text, notice } => {
            lumilio_ui::toast::copy_text(text, cx);
            let _ = wiring
                .shell
                .update(cx, |shell, cx| shell.toast(Toast::success(notice), cx));
        }
        LiveIntent::Play(id) => play(wiring, id, window, cx),
        LiveIntent::Search(query) => search(wiring, &query, cx),
        LiveIntent::LoadFilters => load_filters(wiring, cx),
        LiveIntent::OpenProject { kind, slug } => open_project(wiring, kind, slug, window, cx),
        LiveIntent::Install {
            kind,
            slug,
            title,
            version,
        } => install(wiring, kind, slug, title, version, window, cx),
    }
}

// ── jobs ────────────────────────────────────────────────────────────────

/// Runs `work` on the worker runtime, then hands the result to `done` on the
/// interface thread and reloads what changed. `started` floats up when it
/// begins; `poll` keeps Activity fresh meanwhile.
fn job<R: Send + 'static>(
    wiring: &Wiring,
    cx: &mut App,
    poll: bool,
    then: Reload,
    started: Option<Toast>,
    work: impl Future<Output = R> + Send + 'static,
    done: impl FnOnce(&mut LauncherShell, R, &mut Context<LauncherShell>) + 'static,
) {
    let handle = wiring.backend.spawn(work);
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        if let Some(toast) = started {
            let _ = wiring.shell.update(cx, |shell, cx| shell.toast(toast, cx));
        }
        while poll && !handle.is_finished() {
            cx.background_executor().timer(POLL).await;
            cx.update(|cx| reload(&wiring, Reload::Lists, cx));
        }
        let Ok(output) = handle.await else { return };
        let _ = wiring.shell.update(cx, |shell, cx| done(shell, output, cx));
        cx.update(|cx| {
            reload(&wiring, then, cx);
            refresh_installed(&wiring, cx);
        });
    })
    .detach();
}

/// Reads again what the target game already has of the searched kind, when
/// the game or the kind changed or something was installed. Failing to ask
/// just shows nothing as installed.
fn refresh_installed(wiring: &Wiring, cx: &mut App) {
    let wanted = wiring
        .shell
        .read_with(cx, |shell, _| {
            shell
                .live()
                .and_then(|model| Some((model.install_target.clone()?, model.query.kind)))
        })
        .ok()
        .flatten();
    let Some((target, kind)) = wanted else {
        return;
    };
    if kind == ProjectKind::Modpack {
        wiring.state.borrow_mut().installed_key = None;
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.installed.clear(), cx)
        });
        return;
    }
    let key = (target.clone(), kind);
    if wiring.state.borrow().installed_key.as_ref() == Some(&key) {
        return;
    }
    wiring.state.borrow_mut().installed_key = Some(key);
    let service = wiring.backend.service.clone();
    let asked = target.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.installed_projects(&asked, kind).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let found = handle.await.ok().and_then(Result::ok).unwrap_or_default();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    // Only if still about the same game and kind.
                    if model.install_target.as_ref() == Some(&target) && model.query.kind == kind {
                        model.installed = found;
                    }
                },
                cx,
            )
        });
    })
    .detach();
}

/// The result of an operation: success in words, failure as one plain
/// sentence with the raw cause behind 技术详情 (design language §8, §11).
fn outcome<T, E: std::fmt::Display>(
    result: Result<T, E>,
    ok: impl FnOnce(T) -> String,
    failed: String,
) -> Toast {
    match result {
        Ok(value) => Toast::success(ok(value)),
        Err(error) => Toast::error(failed).technical(error.to_string()),
    }
}

/// Reloads the library, Activity and (optionally) Home from the service.
fn reload(wiring: &Wiring, what: Reload, cx: &mut App) {
    if what == Reload::Nothing {
        return;
    }
    let full = what == Reload::All;
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        let (records, summary) = if full {
            let (records, summary) = service.home().await;
            (records, Some(summary))
        } else {
            (service.library().await.instances, None)
        };
        (
            records,
            summary,
            service.activity(ACTIVITY_LIMIT),
            service.settings().await,
            service.current_instance().await,
            service.library().await.collections,
        )
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok((records, summary, activity, settings, current, collections)) = handle.await else {
            return;
        };
        let now = unix_now();
        let cards = library_cards(&records, now);
        let rows = activity_rows(&activity, now);
        let attention = summary
            .as_ref()
            .map(|summary| attention_rows(&summary.needs_attention, &cards));
        let accounts = account_rows(&settings);
        let home =
            summary.map(|summary| home_presentation(&records, &summary, current.as_deref(), now));
        let hint = match &home {
            Some((_, id)) => {
                wiring.state.borrow_mut().continue_id.clone_from(id);
                id.clone()
            }
            None => {
                // The current instance is what Continue starts.
                if let Some(id) = &current {
                    wiring.state.borrow_mut().continue_id = Some(id.clone());
                }
                wiring.state.borrow().continue_id.clone()
            }
        };
        let hint = current.or(hint);
        let _ = wiring.shell.update(cx, |shell, cx| {
            // A launch in progress, or a failure being explained, keeps Home.
            if let Some((home, _)) = home
                && matches!(
                    shell.home(),
                    HomePresentation::Loading
                        | HomePresentation::Ambient
                        | HomePresentation::FirstUse
                        | HomePresentation::Continue { .. }
                )
            {
                shell.set_home(home, cx);
            }
            shell.update_live(
                |model| {
                    model.set_library(cards, hint);
                    if let Some(attention) = attention {
                        model.attention = attention;
                    }
                    model.collections = collections
                        .into_iter()
                        .map(|collection| CollectionRow {
                            name: collection.name,
                            members: collection.members,
                        })
                        .collect();
                    model.set_activity(rows, unix_millis());
                    model.accounts = accounts;
                    model.accounts_loaded = true;
                },
                cx,
            );
        });
    })
    .detach();
}

/// Makes `id` the current instance everywhere at once and remembers it for
/// the next start.
fn choose_current(wiring: &Wiring, id: String, cx: &mut App) {
    let _ = wiring
        .shell
        .update(cx, |shell, cx| shell.set_install_target(id.clone(), cx));
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::All,
        None,
        async move { service.set_current_instance(&id).await },
        |_, _, _| {},
    );
}

/// The names of the collections the library page shows.
fn collection_names(wiring: &Wiring, cx: &mut App) -> Vec<String> {
    wiring
        .shell
        .update(cx, |shell, _| {
            shell.live().map_or_else(Vec::new, |model| {
                model.collections.iter().map(|c| c.name.clone()).collect()
            })
        })
        .unwrap_or_default()
}

/// Asks for a collection's name: a new one, or another for `original`.
fn ask_collection_name(
    wiring: &Wiring,
    original: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let taken = collection_names(&wiring, cx);
        let renaming = original.is_some();
        let form = cx.new(|cx| {
            let (job_wiring, from) = (wiring.clone(), original.clone());
            NamePrompt::new(
                original,
                taken,
                Rc::new(move |name, _, cx| {
                    let service = job_wiring.backend.service.clone();
                    let from = from.clone();
                    job(
                        &job_wiring,
                        cx,
                        false,
                        Reload::Lists,
                        None,
                        async move {
                            match from {
                                Some(from) => service
                                    .rename_collection(&from, &name)
                                    .await
                                    .map(|()| format!("合集已改名为“{name}”")),
                                None => service
                                    .create_collection(&name)
                                    .await
                                    .map(|()| format!("已新建合集“{name}”")),
                            }
                        },
                        |shell, result, cx| {
                            shell.toast(outcome(result, |text| text, "没能保存合集".to_owned()), cx)
                        },
                    );
                }),
                window,
                cx,
            )
        });
        if renaming {
            NamePrompt::open(form, "合集改名", "改名", window, cx);
        } else {
            NamePrompt::open(form, "新建合集", "新建", window, cx);
        }
    });
}

/// Asks which collections a game belongs to, then files it there.
fn ask_collections_of(wiring: &Wiring, id: String, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let rows = wiring
            .shell
            .update(cx, |shell, _| {
                shell
                    .live()
                    .map_or_else(Vec::new, |model| model.memberships_of(&id))
            })
            .unwrap_or_default();
        let form = cx.new(|cx| {
            let job_wiring = wiring.clone();
            CollectionPicker::new(
                rows,
                Rc::new(move |membership, _, cx| {
                    let service = job_wiring.backend.service.clone();
                    let id = id.clone();
                    job(
                        &job_wiring,
                        cx,
                        false,
                        Reload::Lists,
                        None,
                        async move {
                            let mut collections = membership.collections;
                            if let Some(new) = membership.new {
                                service.create_collection(&new).await?;
                                collections.push(new);
                            }
                            service.set_game_collections(&id, &collections).await
                        },
                        |shell, result, cx| {
                            shell.toast(
                                outcome(
                                    result,
                                    |()| "合集已更新".to_owned(),
                                    "没能更新合集".to_owned(),
                                ),
                                cx,
                            )
                        },
                    );
                }),
                window,
                cx,
            )
        });
        CollectionPicker::open(form, window, cx);
    });
}

/// Opens the add-account dialog (IA `accounts.md`). With `then_launch` the
/// game the person asked to start begins as soon as the account is saved.
/// Deferred: this may run inside one of the shell's own updates.
fn open_add_account(wiring: &Wiring, then_launch: bool, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let handle = window.window_handle();
        let first = wiring
            .shell
            .update(cx, |shell, _| {
                shell.live().is_none_or(|model| model.accounts.is_empty())
            })
            .unwrap_or(true);
        let form = cx.new(|cx| {
            let form = cx.weak_entity();
            let wiring = wiring.clone();
            AccountForm::new(
                Rc::new(move |request, _, cx| {
                    add_account(&wiring, form.clone(), handle, request, then_launch, cx);
                }),
                first,
                window,
                cx,
            )
        });
        AccountForm::open(form, window, cx);
    });
}

/// Opens the Microsoft sign-in dialog. Nothing is asked of Microsoft until
/// the person presses the button in it.
fn open_microsoft_sign_in(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let dialog = cx.new(|cx| {
            let weak: WeakEntity<SignInDialog> = cx.weak_entity();
            SignInDialog::new(Rc::new(move |intent, _, cx| match intent {
                SignInIntent::Start => start_microsoft_sign_in(&wiring, weak.clone(), cx),
                SignInIntent::Cancel => {
                    if let Some(cancel) = wiring.state.borrow_mut().sign_in_cancel.take() {
                        cancel.cancel();
                    }
                }
            }))
        });
        SignInDialog::open(dialog, window, cx);
    });
}

/// Asks for a code, shows it (and opens the page), waits for the browser, and
/// then adds the account. Failures are told in the dialog, in words.
fn start_microsoft_sign_in(wiring: &Wiring, dialog: WeakEntity<SignInDialog>, cx: &mut App) {
    let cancel = CancellationToken::new();
    wiring.state.borrow_mut().sign_in_cancel = Some(cancel.clone());
    let (code_tx, mut code_rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        service
            .microsoft_sign_in(
                move |code| {
                    let _ = code_tx.send((code.user_code.clone(), code.verification_uri.clone()));
                },
                cancel,
            )
            .await
    });
    let shown = dialog.clone();
    cx.spawn(async move |cx| {
        if let Some((code, address)) = code_rx.recv().await {
            let _ = shown.update(cx, |dialog, cx| dialog.code_arrived(&code, &address, cx));
            cx.update(|cx| platform::open_address(&address, cx));
        }
    })
    .detach();
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        wiring.state.borrow_mut().sign_in_cancel = None;
        match result {
            Ok((_, name)) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.signed_in(cx));
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::success(format!("已登录 {name}")), cx)
                });
                cx.update(|cx| reload(&wiring, Reload::Lists, cx));
            }
            // The person cancelled: the dialog is already gone.
            Err(ServiceError::Auth(lumilio_core::AuthError::Cancelled)) => {}
            Err(error) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.failed(account_failure(&error), cx));
            }
        }
    })
    .detach();
}

fn add_account(
    wiring: &Wiring,
    form: WeakEntity<AccountForm>,
    window: gpui_kit::AnyWindowHandle,
    request: AccountRequest,
    then_launch: bool,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = request.clone();
    let handle = wiring.backend.spawn(async move {
        service
            .add_account(&wanted.name, wanted.uuid.as_deref())
            .await
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        if let Err(error) = result {
            let _ = form.update(cx, |form, cx| form.saved(Err(account_failure(&error)), cx));
            return;
        }
        let _ = form.update(cx, |form, cx| form.saved(Ok(()), cx));
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::success(format!("已添加账户 {}", request.name)), cx);
        });
        cx.update(|cx| reload(&wiring, Reload::Lists, cx));
        if then_launch {
            // The model learns of the account on the reload above; give it a beat.
            let _ = cx.update_window(window, |_, window, cx| {
                let wiring = wiring.clone();
                window.defer(cx, move |window, cx| {
                    let _ = wiring
                        .shell
                        .update(cx, |shell, cx| shell.request_continue(window, cx));
                });
            });
        }
    })
    .detach();
}

/// Opens the new-game dialog (IA `library.md#新建游戏`). Deferred: this may
/// run inside one of the shell's own updates.
fn open_new_game(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let handle = window.window_handle();
        let form = cx.new(|cx| {
            let form = cx.weak_entity();
            let wiring = wiring.clone();
            NewGameForm::new(
                Rc::new(move |intent, _, cx| {
                    new_game_intent(&wiring, form.clone(), handle, intent, cx);
                }),
                window,
                cx,
            )
        });
        NewGameForm::open(form, window, cx);
    });
}

fn new_game_intent(
    wiring: &Wiring,
    form: WeakEntity<NewGameForm>,
    window: gpui_kit::AnyWindowHandle,
    intent: NewGameIntent,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    match intent {
        NewGameIntent::LoadGameVersions => {
            let handle = wiring
                .backend
                .spawn(async move { service.game_versions().await });
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(Ok(entries)) => Ok(entries),
                    Ok(Err(error)) => Err(("读不到版本列表".to_owned(), error.to_string())),
                    Err(error) => Err(("读不到版本列表".to_owned(), error.to_string())),
                };
                let _ = form.update(cx, |form, cx| form.game_versions_arrived(result, cx));
            })
            .detach();
        }
        NewGameIntent::LoadLoaderVersions {
            loader,
            game_version,
        } => {
            let wanted = game_version.clone();
            let handle = wiring
                .backend
                .spawn(async move { service.loader_versions(loader, &wanted).await });
            cx.spawn(async move |cx| {
                let failed = format!("读不到 {} 的版本", lumilio_ui::live::loader_label(loader));
                let result = match handle.await {
                    Ok(Ok(versions)) => Ok(versions),
                    Ok(Err(error)) => Err((failed, error.to_string())),
                    Err(error) => Err((failed, error.to_string())),
                };
                let _ = form.update(cx, |form, cx| {
                    form.loader_versions_arrived(loader, &game_version, result, cx)
                });
            })
            .detach();
        }
        NewGameIntent::Create(request) => create_game(wiring, form, window, request, cx),
    }
}

/// Creates the game, then (if asked) downloads its files in the background,
/// shows it and makes it the current game.
fn create_game(
    wiring: &Wiring,
    form: WeakEntity<NewGameForm>,
    window: gpui_kit::AnyWindowHandle,
    request: NewGameRequest,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = request.clone();
    let handle = wiring.backend.spawn(async move {
        service
            .create_instance(
                &wanted.name,
                Some(&wanted.game_version),
                wanted.loader,
                wanted.loader_version.as_deref(),
            )
            .await
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        let record = match result {
            Ok(record) => record,
            Err(error) => {
                let message = match &error {
                    ServiceError::Launch(LaunchServiceError::LoaderUnsupported(loader)) => format!(
                        "{} 还不能安装，支持正在路上",
                        lumilio_ui::live::loader_label(*loader)
                    ),
                    _ => "没有创建成功，可以再试一次".to_owned(),
                };
                let _ = form.update(cx, |form, cx| {
                    form.created(Err((message, error.to_string())), cx)
                });
                return;
            }
        };
        let _ = form.update(cx, |form, cx| form.created(Ok(()), cx));
        let id = record.id.clone();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                Toast::success(if request.install {
                    format!("已创建 {}，正在下载游戏文件", record.name)
                } else {
                    format!("已创建 {}", record.name)
                }),
                cx,
            );
        });
        cx.update(|cx| {
            reload(&wiring, Reload::All, cx);
            if request.install {
                install_new_game(&wiring, id.clone(), cx);
            }
        });
        let _ = cx.update_window(window, |_, window, cx| {
            open_instance(&wiring, id.clone(), None, window, cx);
            choose_current(&wiring, id, cx);
        });
    })
    .detach();
}

/// Downloads a new game's files; progress lives in Activity.
fn install_new_game(wiring: &Wiring, id: String, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let target = id.clone();
    let after = wiring.clone();
    job(
        wiring,
        cx,
        true,
        Reload::Lists,
        None,
        async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(async move { while rx.recv().await.is_some() {} });
            service
                .install_instance(&id, tx, CancellationToken::new())
                .await
        },
        move |shell, result, cx| {
            if let Err(error) = result {
                shell.toast(
                    Toast::error("游戏文件没有下载完，开始游戏时会再试")
                        .technical(error.to_string()),
                    cx,
                );
            }
            // An open page for this game shows the new install state.
            if let Some(view) = shell
                .live_instance()
                .filter(|view| view.read(cx).id() == target)
                .map(|view| view.downgrade())
            {
                load_instance(&after, target, view, cx);
            }
        },
    );
}

/// Asks for a local `.mrpack` and imports it as a new instance. Cancelling the
/// picker does nothing.
fn import_pack(wiring: &Wiring, cx: &mut App) {
    let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("选择整合包（.mrpack，或 MultiMC / Prism 的 .zip）".into()),
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = picked.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        cx.update(|cx| import_pack_file(&wiring, path, cx));
    })
    .detach();
}

/// Imports a chosen or dropped `.mrpack` as a new game.
fn import_pack_file(wiring: &Wiring, path: std::path::PathBuf, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    job(
        wiring,
        cx,
        true,
        Reload::All,
        Some(Toast::info(format!("开始导入 {name}，进度在动态里"))),
        async move {
            service
                .import_modpack_file(&path, CancellationToken::new())
                .await
        },
        move |shell, result, cx| {
            let toast = outcome(
                result,
                |record| format!("已导入整合包 {}", record.name),
                format!("没有导入 {name}"),
            );
            shell.toast(toast, cx);
        },
    );
}

// ── real instance views ────────────────────────────────────────────────

/// Opens a game's page; `then` is something to ask of the page once it is up.
fn open_instance(
    wiring: &Wiring,
    id: String,
    then: Option<InstanceIntent>,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let target = id.clone();
        let callbacks = wiring.clone();
        let Ok(view) = wiring.shell.update(cx, |shell, cx| {
            shell.open_live_instance(
                id,
                move |source| {
                    Rc::new(move |intent, window, cx| {
                        instance_intent(&callbacks, &target, source.clone(), intent, window, cx);
                    })
                },
                cx,
            )
        }) else {
            return;
        };
        let id = target_id(&view, cx);
        // A game already running has printed things the page should show.
        let running = wiring
            .state
            .borrow()
            .game_log
            .as_ref()
            .filter(|(running_id, _)| *running_id == id)
            .map(|(_, lines)| lines.iter().cloned().collect::<Vec<_>>());
        if let Some(lines) = running {
            view.update(cx, |view, cx| view.game_output(lines, true, cx));
        }
        load_instance(&wiring, id.clone(), view.downgrade(), cx);
        if let Some(intent) = then {
            instance_intent(&wiring, &id, view.downgrade(), intent, window, cx);
        }
    });
}

fn target_id(view: &Entity<InstanceDetailView>, cx: &App) -> String {
    view.read(cx).id().to_owned()
}

fn load_instance(wiring: &Wiring, id: String, view: WeakEntity<InstanceDetailView>, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        let record = service.instance(&id).await?;
        Ok::<_, ServiceError>((record, service.settings().await))
    });
    let memory = wiring
        .backend
        .spawn(async { lumilio_core::total_memory_mb() });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.loaded(result, cx));
        if let Ok(total) = memory.await {
            let _ = view.update(cx, |view, cx| view.set_machine_memory(total, cx));
        }
    })
    .detach();
}

fn instance_intent(
    wiring: &Wiring,
    id: &str,
    view: WeakEntity<InstanceDetailView>,
    intent: InstanceIntent,
    window: &mut Window,
    cx: &mut App,
) {
    match intent {
        InstanceIntent::Play => play(wiring, id.to_owned(), window, cx),
        InstanceIntent::OpenRuntimeChange => {
            open_runtime_change(wiring, id.to_owned(), view, window, cx)
        }
        InstanceIntent::PlayWorld(world) => {
            wiring.state.borrow_mut().next_world = Some(world);
            play(wiring, id.to_owned(), window, cx);
        }
        InstanceIntent::Reload => load_instance(wiring, id.to_owned(), view, cx),
        InstanceIntent::OpenCrash(file) => open_crash(wiring, id.to_owned(), view, file, cx),
        InstanceIntent::Load(section) => load_section(wiring, id.to_owned(), view, section, cx),
        InstanceIntent::OpenAccounts => {
            let shell = wiring.shell.clone();
            window.defer(cx, move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.go_to(Route::Accounts, window, cx));
            });
        }
        question @ (InstanceIntent::AskCopy
        | InstanceIntent::AskDelete
        | InstanceIntent::Resolve(_)) => {
            let _ = view.update(cx, |view, cx| view.ask_later(question, cx));
        }
        InstanceIntent::SetCurrent => choose_current(wiring, id.to_owned(), cx),
        InstanceIntent::Stop => {
            // Stops the launch or the running game, if it is this one.
            let this_one = wiring
                .state
                .borrow()
                .game_log
                .as_ref()
                .is_some_and(|(running, _)| running == id);
            if this_one && let Some(cancel) = wiring.state.borrow().cancel.clone() {
                cancel.cancel();
            }
        }
        InstanceIntent::ExportLog(crash) => export_log(wiring, id.to_owned(), crash, cx),
        InstanceIntent::OpenFolder(folder) => open_folder(wiring, id.to_owned(), view, folder, cx),
        InstanceIntent::RevealPath(relative) => {
            // Only plain names below the game directory; never a way out of it.
            let plain = std::path::Path::new(&relative)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)));
            if plain {
                cx.reveal_path(&wiring.backend.service.game_dir(id).join(relative));
            }
        }
        InstanceIntent::LoadVersions(project) => {
            let service = wiring.backend.service.clone();
            let wanted = project.clone();
            let handle = wiring
                .backend
                .spawn(async move { service.project_versions(&wanted).await });
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                let _ = view.update(cx, |view, cx| view.versions_arrived(&project, result, cx));
            })
            .detach();
        }
        InstanceIntent::RevealContent { kind, file_name } => {
            let folder = kind.install_folder().unwrap_or_default();
            let path = wiring
                .backend
                .service
                .game_dir(id)
                .join(folder)
                .join(file_name);
            cx.reveal_path(&path);
        }
        InstanceIntent::OpenProject { kind, slug } => open_project(wiring, kind, slug, window, cx),
        // ia[discover]: 锁定目标（从游戏页进入） | 游戏页内容标签「浏览 Mod / 资源包 / 光影」 | 安装目标换成那个游戏（右下角芯片显示），类型预选；不改变当前游戏 | —
        // ia[instance.content]: 浏览并安装 | L4a 主要 → 发现页（类型预选） | 安装完回到本页可见 | L-CONT-01
        InstanceIntent::BrowseContent(kind) => {
            let shell = wiring.shell.clone();
            let target = id.to_owned();
            let wiring = wiring.clone();
            window.defer(cx, move |window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.set_install_target(target, cx);
                    shell.browse(kind, window, cx)
                });
                refresh_installed(&wiring, cx);
            });
        }
        InstanceIntent::ImportContent(kind) => {
            let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("选择要添加的文件".into()),
            });
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Ok(Ok(Some(files))) = picked.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::AddFiles { kind, files },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ExportWorld(folder) => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let chosen = platform::pick_save_path(cx, &start, &format!("{folder}.zip"));
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::ExportWorldTo { folder, path },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::BackupGame => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let suggested = format!("{}.lumilio-backup.zip", id).replace('/', "-");
            let chosen = platform::pick_save_path(cx, &start, &suggested);
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::BackupGameTo(path),
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ExportPack => open_export(wiring, id.to_owned(), view, window, cx),
        InstanceIntent::ExportPackAs(spec) => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let suggested = format!("{}-{}.{}", spec.name, spec.version, spec.format.extension())
                .replace('/', "-");
            let chosen = platform::pick_save_path(cx, &start, &suggested);
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::ExportPackTo { spec, path },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ImportWorld => {
            let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("选择世界的 .zip".into()),
            });
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Ok(Ok(Some(files))) = picked.await else {
                    return;
                };
                let Some(path) = files.into_iter().next() else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::AddWorld(path),
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::Install
        | InstanceIntent::Repair
        | InstanceIntent::ChangeRuntime { .. }
        | InstanceIntent::Copy { .. }
        | InstanceIntent::Delete
        | InstanceIntent::SetContent { .. }
        | InstanceIntent::DeleteContent { .. }
        | InstanceIntent::CopyWorld(_)
        | InstanceIntent::BackupWorld(_)
        | InstanceIntent::ExportWorldTo { .. }
        | InstanceIntent::ExportPackTo { .. }
        | InstanceIntent::AddWorld(_)
        | InstanceIntent::DeleteWorld(_)
        | InstanceIntent::InstallJava(_)
        | InstanceIntent::BackupGameTo(_)
        | InstanceIntent::CreateSnapshot
        | InstanceIntent::CreateSnapshotAs { .. }
        | InstanceIntent::RestoreSnapshot(_)
        | InstanceIntent::DeleteSnapshot(_)
        | InstanceIntent::SwitchContent { .. }
        | InstanceIntent::UpdateContent { .. }
        | InstanceIntent::AddFiles { .. } => {
            write_instance(wiring, id.to_owned(), view, intent, window, cx)
        }
        action @ (InstanceIntent::Rename(_)
        | InstanceIntent::SaveMemory(_)
        | InstanceIntent::SaveSettings(_)) => {
            let service = wiring.backend.service.clone();
            let id = id.to_owned();
            let memory = matches!(action, InstanceIntent::SaveMemory(_));
            let settings_dialog = matches!(action, InstanceIntent::SaveSettings(_));
            let handle = wiring.backend.spawn(async move {
                match action {
                    InstanceIntent::Rename(name) => service.rename(&id, &name).await?,
                    InstanceIntent::SaveMemory(settings)
                    | InstanceIntent::SaveSettings(settings) => {
                        service.update_instance_settings(&id, settings).await?
                    }
                    _ => unreachable!("only save intents reach this branch"),
                }
                Ok::<_, ServiceError>((service.instance(&id).await?, service.settings().await))
            });
            let wiring = wiring.clone();
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                let _ = view.update(cx, |view, cx| {
                    if settings_dialog {
                        view.settings_saved(result, cx);
                    } else {
                        view.saved(memory, result, cx);
                    }
                });
                cx.update(|cx| reload(&wiring, Reload::All, cx));
            })
            .detach();
        }
    }
}

/// Reads one section's data off the interface thread.
fn load_section(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    section: Section,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        match section {
            Section::Content(kind) => Arrived::Content(
                kind,
                service
                    .content_details(&id, kind)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Worlds => {
                Arrived::Worlds(service.worlds(&id).await.map_err(|error| error.to_string()))
            }
            Section::Snapshots => Arrived::Snapshots(
                service
                    .snapshots(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::History => Arrived::History(
                service
                    .history(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Logs => {
                Arrived::Logs(service.logs(&id).await.map_err(|error| error.to_string()))
            }
            Section::Problems => Arrived::Problems(
                service
                    .problems(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Files => files_arrived(&service, &id, String::new()).await,
            Section::Size => Arrived::Size(
                service
                    .instance_size(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
        }
    });
    cx.spawn(async move |cx| {
        let arrived = match handle.await {
            Ok(arrived) => arrived,
            Err(error) => failed_section(section, error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.arrived(arrived, cx));
    })
    .detach();
}

fn open_crash(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    file: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let name = file.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.crash_report(&id, &name).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.crash_arrived(file, result, cx));
    })
    .detach();
}

fn failed_section(section: Section, detail: String) -> Arrived {
    match section {
        Section::Content(kind) => Arrived::Content(kind, Err(detail)),
        Section::Worlds => Arrived::Worlds(Err(detail)),
        Section::Snapshots => Arrived::Snapshots(Err(detail)),
        Section::History => Arrived::History(Err(detail)),
        Section::Logs => Arrived::Logs(Err(detail)),
        Section::Problems => Arrived::Problems(Err(detail)),
        Section::Files => Arrived::Files(String::new(), Err(detail)),
        Section::Size => Arrived::Size(Err(detail)),
    }
}

/// One folder of the game directory, as the Files part shows it.
async fn files_arrived(service: &crate::backend::Service, id: &str, folder: String) -> Arrived {
    let result = service
        .list_files(id, &folder)
        .await
        .map_err(|error| error.to_string());
    Arrived::Files(folder, result)
}

fn open_folder(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    folder: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = folder.clone();
    let handle = wiring
        .backend
        .spawn(async move { files_arrived(&service, &id, wanted).await });
    cx.spawn(async move |cx| {
        let arrived = match handle.await {
            Ok(arrived) => arrived,
            Err(error) => Arrived::Files(folder, Err(error.to_string())),
        };
        let _ = view.update(cx, |view, cx| view.arrived(arrived, cx));
    })
    .detach();
}

/// What a batch of content changes amounts to, in words.
fn content_notice(results: &[ContentResult], did: &str) -> Result<String, String> {
    let failed: Vec<String> = results
        .iter()
        .filter_map(|result| {
            result
                .outcome
                .as_ref()
                .err()
                .map(|error| format!("{}: {error}", result.file_name))
        })
        .collect();
    if !failed.is_empty() {
        return Err(failed.join("\n"));
    }
    let changed = results
        .iter()
        .filter(|result| !matches!(result.outcome, Ok(ContentEffect::Unchanged)))
        .count();
    Ok(if changed == 0 {
        "没有需要改动的文件".to_owned()
    } else {
        format!("已{did} {changed} 个文件")
    })
}

/// Lists the game's top level and opens the export dialog over it.
fn open_export(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = id.clone();
    let handle = wiring.backend.spawn(async move {
        let record = service.instance(&wanted).await?;
        let entries = service.list_files(&wanted, "").await?;
        // What is inside each folder, so single files can be left out. A
        // folder that cannot be read is just offered whole.
        let mut below = std::collections::BTreeMap::new();
        for folder in entries.iter().filter(|entry| entry.is_dir) {
            if let Ok(inside) = service.list_files(&wanted, &folder.name).await {
                below.insert(folder.name.clone(), inside);
            }
        }
        Ok::<_, ServiceError>((record.name, entries, below))
    });
    let wiring = wiring.clone();
    let window_handle = window.window_handle();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = cx.update_window(window_handle, |_, window, cx| match result {
            Ok((name, entries, below)) => {
                let form = cx.new(|cx| {
                    let (job_wiring, job_view, job_id) = (wiring.clone(), view.clone(), id.clone());
                    ExportForm::new(
                        &name,
                        &entries,
                        &below,
                        Rc::new(move |spec, window, cx| {
                            instance_intent(
                                &job_wiring,
                                &job_id,
                                job_view.clone(),
                                InstanceIntent::ExportPackAs(spec),
                                window,
                                cx,
                            );
                        }),
                        window,
                        cx,
                    )
                });
                ExportForm::open(form, window, cx);
            }
            Err(detail) => {
                let _ = view.update(cx, |view, cx| {
                    view.toast(
                        Toast::error("没能读取游戏的文件，无法导出").technical(detail),
                        cx,
                    )
                });
            }
        });
    })
    .detach();
}

/// Opens the dialog that changes a game's version and loader. What the person
/// picks becomes a write on the instance, so the page shows it busy, and a
/// failure leaves the game on its old version.
fn open_runtime_change(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((game_version, loader, loader_version)) =
        view.read_with(cx, |view, _| view.runtime()).ok().flatten()
    else {
        return;
    };
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let handle = window.window_handle();
        let form = cx.new(|cx| {
            let form: WeakEntity<NewGameForm> = cx.weak_entity();
            let (on_form, on_snapshot) = (wiring.clone(), wiring.clone());
            let (form_id, form_view) = (id.clone(), view.clone());
            let (snapshot_id, snapshot_view) = (id.clone(), view.clone());
            NewGameForm::for_change(
                Rc::new(move |intent, window, cx| match intent {
                    NewGameIntent::Create(request) => {
                        // The change runs as a write on the game; the dialog is done.
                        let _ = form.update(cx, |form, cx| form.created(Ok(()), cx));
                        instance_intent(
                            &on_form,
                            &form_id,
                            form_view.clone(),
                            InstanceIntent::ChangeRuntime {
                                loader: request.loader,
                                game_version: request.game_version,
                                loader_version: request.loader_version,
                            },
                            window,
                            cx,
                        );
                    }
                    other => new_game_intent(&on_form, form.clone(), handle, other, cx),
                }),
                RuntimeChange {
                    game_version,
                    loader,
                    loader_version,
                    snapshot: Rc::new(move |window, cx| {
                        instance_intent(
                            &on_snapshot,
                            &snapshot_id,
                            snapshot_view.clone(),
                            InstanceIntent::CreateSnapshot,
                            window,
                            cx,
                        );
                    }),
                },
                window,
                cx,
            )
        });
        NewGameForm::open(form, window, cx);
    });
}

/// One write on an instance: runs it, tells the view, and refreshes what it
/// made stale. Failures keep the screen as it was and say what to do next.
fn write_instance(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    intent: InstanceIntent,
    _window: &mut Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let cancel = CancellationToken::new();
    let deleting = matches!(intent, InstanceIntent::Delete);
    let installing = matches!(
        intent,
        InstanceIntent::Install | InstanceIntent::Repair | InstanceIntent::ChangeRuntime { .. }
    );
    let work = {
        let id = id.clone();
        async move {
            let done = |notice: &str, refresh: Vec<Section>| Ok((notice.to_owned(), refresh));
            match intent {
                InstanceIntent::SetContent {
                    kind,
                    files,
                    enabled,
                } => {
                    let results = service
                        .set_content_state(&id, kind, &files, enabled)
                        .await?;
                    let did = if enabled { "启用" } else { "停用" };
                    match content_notice(&results, did) {
                        Ok(text) => done(&text, vec![Section::Content(kind)]),
                        Err(detail) => Err(ServiceError::Remote(detail)),
                    }
                }
                InstanceIntent::DeleteContent { kind, files } => {
                    let results = service.delete_content(&id, kind, &files).await?;
                    match content_notice(&results, "删除") {
                        Ok(text) => done(&text, vec![Section::Content(kind)]),
                        Err(detail) => Err(ServiceError::Remote(detail)),
                    }
                }
                InstanceIntent::SwitchContent {
                    kind,
                    file_name,
                    project,
                    version_id,
                } => {
                    let name = service
                        .switch_content_version(
                            &id,
                            kind,
                            &file_name,
                            &project,
                            &version_id,
                            cancel,
                        )
                        .await?;
                    done(&format!("已换成 {name}"), vec![Section::Content(kind)])
                }
                InstanceIntent::UpdateContent { kind, updates } => {
                    // One by one: each file stands or falls on its own.
                    let total = updates.len();
                    let mut failed = Vec::new();
                    for (file, project, version) in updates {
                        if let Err(error) = service
                            .switch_content_version(
                                &id,
                                kind,
                                &file,
                                &project,
                                &version,
                                cancel.clone(),
                            )
                            .await
                        {
                            failed.push(format!("{file}: {error}"));
                        }
                    }
                    if failed.is_empty() {
                        done(
                            &format!("已更新 {total} 个文件"),
                            vec![Section::Content(kind)],
                        )
                    } else {
                        Err(ServiceError::Remote(format!(
                            "{} 个更新成功，{} 个没有成功\n{}",
                            total - failed.len(),
                            failed.len(),
                            failed.join("\n")
                        )))
                    }
                }
                InstanceIntent::AddFiles { kind, files } => {
                    let results = service.import_content(&id, kind, files).await?;
                    let added = results.iter().filter(|(_, result)| result.is_ok()).count();
                    let refused: Vec<String> = results
                        .iter()
                        .filter_map(|(name, result)| {
                            result
                                .as_ref()
                                .err()
                                .map(|error| format!("{name}: {error}"))
                        })
                        .collect();
                    if refused.is_empty() {
                        done(
                            &format!("已添加 {added} 个文件"),
                            vec![Section::Content(kind)],
                        )
                    } else {
                        Err(ServiceError::Remote(format!(
                            "添加了 {added} 个，{} 个没有添加\n{}",
                            refused.len(),
                            refused.join("\n")
                        )))
                    }
                }
                InstanceIntent::CopyWorld(folder) => {
                    let name = service.copy_world(&id, &folder, None).await?;
                    done(&format!("已复制为「{name}」"), vec![Section::Worlds])
                }
                InstanceIntent::BackupWorld(folder) => {
                    service
                        .create_snapshot(&id, SnapshotScope::World(folder.clone()), "手动备份")
                        .await?;
                    done(
                        &format!("已备份「{folder}」，可以在历史的快照里恢复"),
                        vec![Section::Snapshots],
                    )
                }
                InstanceIntent::ExportWorldTo { folder, path } => {
                    service.export_world(&id, &folder, &path).await?;
                    done(&format!("已导出到 {}", path.display()), Vec::new())
                }
                InstanceIntent::ExportPackTo { spec, path } => {
                    let report = service.export_modpack(&id, spec, &path, cancel).await?;
                    done(&export_notice(&report, &path), Vec::new())
                }
                InstanceIntent::AddWorld(path) => {
                    let name = service.import_world(&id, &path).await?;
                    done(&format!("已导入「{name}」"), vec![Section::Worlds])
                }
                InstanceIntent::DeleteWorld(folder) => {
                    service.delete_world(&id, &folder).await?;
                    done("世界已删除", vec![Section::Worlds])
                }
                InstanceIntent::CreateSnapshot => {
                    service
                        .create_snapshot(&id, SnapshotScope::Full, "手动快照")
                        .await?;
                    done("快照已创建", vec![Section::Snapshots])
                }
                InstanceIntent::CreateSnapshotAs { note, world } => {
                    let (scope, label) = match world {
                        Some(folder) => (
                            SnapshotScope::World(folder),
                            if note.is_empty() {
                                "手动备份".to_owned()
                            } else {
                                note
                            },
                        ),
                        None => (
                            SnapshotScope::Full,
                            if note.is_empty() {
                                "手动快照".to_owned()
                            } else {
                                note
                            },
                        ),
                    };
                    service.create_snapshot(&id, scope, &label).await?;
                    done("快照已创建", vec![Section::Snapshots])
                }
                InstanceIntent::RestoreSnapshot(snapshot) => {
                    service.restore_snapshot(&id, &snapshot).await?;
                    done("已恢复快照", vec![Section::Worlds, Section::Snapshots])
                }
                InstanceIntent::DeleteSnapshot(snapshot) => {
                    service.delete_snapshot(&id, &snapshot).await?;
                    done("快照已删除", vec![Section::Snapshots])
                }
                InstanceIntent::Install => {
                    // Progress lives in Activity; the channel only has to drain.
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    service.install_instance(&id, tx, cancel).await?;
                    done("游戏文件已安装", Vec::new())
                }
                InstanceIntent::BackupGameTo(path) => {
                    service.backup_instance(&id, &path, cancel).await?;
                    done(&format!("已备份到 {}", path.display()), Vec::new())
                }
                InstanceIntent::InstallJava(major) => {
                    service.install_java(major, cancel).await?;
                    done("Java 已安装", vec![Section::Problems])
                }
                InstanceIntent::Repair => {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    service.repair_instance(&id, tx, cancel).await?;
                    done("游戏文件已检查，缺的和损坏的已补上", Vec::new())
                }
                InstanceIntent::ChangeRuntime {
                    loader,
                    game_version,
                    loader_version,
                } => {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    let record = service
                        .change_runtime(
                            &id,
                            &game_version,
                            loader,
                            loader_version.as_deref(),
                            tx,
                            cancel,
                        )
                        .await?;
                    done(
                        &format!("已更换为 {}", lumilio_ui::live::instance_meta(&record)),
                        Vec::new(),
                    )
                }
                InstanceIntent::Copy {
                    name,
                    include_worlds,
                } => {
                    let record = service
                        .copy_instance(&id, &name, include_worlds, cancel)
                        .await?;
                    done(&format!("已复制为「{}」", record.name), Vec::new())
                }
                InstanceIntent::Delete => {
                    service.delete_instance(&id).await?;
                    done("游戏已删除", Vec::new())
                }
                _ => unreachable!("only writes reach this function"),
            }
        }
    };
    let handle = wiring
        .backend
        .spawn(async move { work.await as Result<(String, Vec<Section>), ServiceError> });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let succeeded = result.is_ok();
        let done = match result {
            Ok((notice, refresh)) => Operated {
                notice,
                technical: None,
                refresh,
            },
            Err(detail) => Operated {
                notice: "没有成功，游戏保持原样，可以稍后重试".into(),
                technical: Some(detail),
                refresh: Vec::new(),
            },
        };
        let stale = view
            .update(cx, |view, cx| view.operated(done, cx))
            .unwrap_or_default();
        cx.update(|cx| {
            for section in stale {
                load_section(&wiring, id.clone(), view.clone(), section, cx);
            }
            if succeeded && installing {
                load_instance(&wiring, id.clone(), view.clone(), cx);
            }
            if succeeded && deleting {
                let shell = wiring.shell.clone();
                let _ = shell.update(cx, |shell, cx| shell.forget_instance(&id, cx));
            }
            reload(&wiring, Reload::All, cx);
        });
    })
    .detach();
}

// ── settings ────────────────────────────────────────────────────────────

/// Applies what the saved preferences say about the look and the motion.
fn apply_preferences(preferences: &Preferences, window: &mut Window, cx: &mut App) {
    platform::apply_appearance(preferences.appearance, window, cx);
    platform::apply_motion(preferences.motion, cx);
}

/// Reads the saved preferences once at start-up and applies them.
fn apply_saved_preferences(wiring: &Wiring, window: gpui_kit::AnyWindowHandle, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.settings().await.preferences });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(preferences) = handle.await else {
            return;
        };
        wiring.state.borrow_mut().preferences = preferences.clone();
        let _ = cx.update_window(window, |_, window, cx| {
            apply_preferences(&preferences, window, cx)
        });
        // The Library comes back ordered and filtered as it was left.
        let _ = wiring.shell.update(cx, |shell, cx| {
            use lumilio_ui::kit::ViewIntent;
            use lumilio_ui::pages::live::{LIBRARY_LOADER, LIBRARY_SORT};
            shell.apply_view_intent(
                ViewIntent::Choose(LIBRARY_SORT, usize::from(preferences.library_sort)),
                cx,
            );
            shell.apply_view_intent(
                ViewIntent::Choose(LIBRARY_LOADER, usize::from(preferences.library_loader)),
                cx,
            );
        });
    })
    .detach();
}

/// Saves one setting. A failure is told with its raw cause; success says
/// `notice` (when there is one), applies `after` and shows the new values.
fn change_setting(
    wiring: &Wiring,
    window: &Window,
    work: impl Future<Output = Result<(), ServiceError>> + Send + 'static,
    failed: &'static str,
    notice: Option<&'static str>,
    after: impl FnOnce(&mut Window, &mut App) + 'static,
    cx: &mut App,
) {
    let handle = wiring.backend.spawn(work);
    let wiring = wiring.clone();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        match result {
            Ok(()) => {
                if let Some(notice) = notice {
                    let _ = wiring
                        .shell
                        .update(cx, |shell, cx| shell.toast(Toast::success(notice), cx));
                }
                let _ = cx.update_window(window, |_, window, cx| after(window, cx));
            }
            Err(error) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::error(failed).technical(error.to_string()), cx)
                });
            }
        }
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

/// Reads the settings, the Java list and then the disk use for the Settings page.
fn load_settings(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        let settings = service.settings().await;
        let java = service.java_installations().await;
        let memory = lumilio_core::total_memory_mb();
        (settings, java, memory, service.layout().root().to_owned())
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok((settings, java, memory, root)) = handle.await else {
            return;
        };
        wiring.state.borrow_mut().preferences = settings.preferences.clone();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    let storage = model.settings.as_ref().and_then(|view| view.storage);
                    model.settings = Some(settings_view(&settings, &java, storage, &root, memory));
                },
                cx,
            );
        });
        let service = wiring.backend.service.clone();
        let Ok(usage) = wiring
            .backend
            .spawn(async move { service.storage_usage().await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    if let Some(view) = &mut model.settings {
                        view.storage = Some(usage);
                    }
                },
                cx,
            );
        });
    })
    .detach();
}

fn add_java(wiring: &Wiring, cx: &mut App) {
    let chosen = platform::pick_path(cx, true, true, "选择 Java");
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = wiring.backend.service.clone();
        let Ok(result) = wiring
            .backend
            .spawn(async move { service.add_java(&path).await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                match result {
                    Ok(runtime) => Toast::success(format!("已添加 Java {}", runtime.version())),
                    Err(ServiceError::NoJavaAt(_)) => {
                        Toast::error("这里没有找到 Java，请选 java 程序或 JDK 文件夹")
                    }
                    Err(error) => Toast::error("没能添加 Java").technical(error.to_string()),
                },
                cx,
            )
        });
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

/// Measures what no game uses; if there is any, asks before removing it.
fn check_reclaimable(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.reclaimable().await });
    let wiring = wiring.clone();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let Ok(result) = handle.await else {
            return;
        };
        match result {
            Err(error) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(
                        Toast::error("没能检查游戏文件").technical(error.to_string()),
                        cx,
                    )
                });
            }
            Ok(found) if found.unused.is_empty() => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::info("没有多余的游戏文件"), cx)
                });
            }
            Ok(found) => {
                let size = lumilio_ui::pages::settings::bytes_text(found.bytes());
                let kept = found.kept.clone();
                let _ = cx.update_window(window, |_, window, cx| {
                    lumilio_ui::collections::confirm_reclaim(
                        &size,
                        &kept,
                        {
                            let wiring = wiring.clone();
                            move |_, cx| reclaim(&wiring, cx)
                        },
                        window,
                        cx,
                    );
                });
            }
        }
    })
    .detach();
}

/// Removes the shared game files no game uses.
fn reclaim(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.reclaim().await },
        |shell, result, cx| {
            shell.toast(
                match result {
                    Ok(freed) => Toast::success(format!(
                        "已清理 {}",
                        lumilio_ui::pages::settings::bytes_text(freed)
                    )),
                    Err(error) => Toast::error("没能清理游戏文件").technical(error.to_string()),
                },
                cx,
            );
        },
    );
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(800))
            .await;
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

fn clear_cache(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.clear_cache().await },
        |shell, result, cx| {
            shell.toast(
                match result {
                    Ok(freed) => Toast::success(format!(
                        "已清理 {}",
                        lumilio_ui::pages::settings::bytes_text(freed)
                    )),
                    Err(error) => Toast::error("没能清理缓存").technical(error.to_string()),
                },
                cx,
            );
        },
    );
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        // The sizes change once the deletion finishes.
        cx.background_executor()
            .timer(Duration::from_millis(800))
            .await;
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

fn export_diagnostics(wiring: &Wiring, cx: &mut App) {
    let start = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
    let chosen = platform::pick_save_path(cx, &start, "lumilio-diagnostics.zip");
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = wiring.backend.service.clone();
        let target = path.clone();
        let Ok(result) = wiring
            .backend
            .spawn(async move { service.export_diagnostics(&target).await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                match result {
                    Ok(()) => Toast::success(format!("诊断包已保存到 {}", path.display())),
                    Err(error) => Toast::error("没能导出诊断包").technical(error.to_string()),
                },
                cx,
            )
        });
    })
    .detach();
}

/// Asks for a folder made by another launcher and brings its game over;
/// when it holds several, asks which.
fn import_game(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("选择其他启动器的游戏文件夹".into()),
    });
    let wiring = wiring.clone();
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = picked.await else {
            return;
        };
        let Some(folder) = paths.into_iter().next() else {
            return;
        };
        let service = wiring.backend.service.clone();
        let Ok(found) = wiring
            .backend
            .spawn(async move { service.detect_games(&folder).await })
            .await
        else {
            return;
        };
        let games = match found {
            Ok(games) => games,
            Err(error) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(
                        Toast::error("这个文件夹里没找到能导入的游戏").technical(error.to_string()),
                        cx,
                    )
                });
                return;
            }
        };
        let start = {
            let wiring = wiring.clone();
            move |games: Vec<lumilio_core::FoundGame>, cx: &mut App| {
                for game in games {
                    let service = wiring.backend.service.clone();
                    let name = game.name.clone();
                    job(
                        &wiring,
                        cx,
                        true,
                        Reload::All,
                        Some(Toast::info(format!("开始导入 {name}，进度在动态里"))),
                        async move { service.import_game(game, CancellationToken::new()).await },
                        move |shell, result, cx| {
                            shell.toast(
                                outcome(
                                    result,
                                    |record| format!("已导入「{}」", record.name),
                                    format!("没有导入 {name}"),
                                ),
                                cx,
                            )
                        },
                    );
                }
            }
        };
        if let [only] = games.as_slice() {
            let only = only.clone();
            cx.update(|cx| start(vec![only], cx));
            return;
        }
        let _ = cx.update_window(handle, |_, window, cx| {
            let form =
                cx.new(|_| GamePicker::new(games, Rc::new(move |chosen, _, cx| start(chosen, cx))));
            GamePicker::open(form, window, cx);
        });
    })
    .detach();
}

/// Asks for a backup file and makes a new game of it.
fn restore_backup(wiring: &Wiring, cx: &mut App) {
    let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("选择备份文件（.zip）".into()),
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = picked.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        cx.update(|cx| {
            let service = wiring.backend.service.clone();
            job(
                &wiring,
                cx,
                true,
                Reload::All,
                Some(Toast::info("开始恢复备份，进度在动态里")),
                async move {
                    service
                        .restore_backup(&path, CancellationToken::new())
                        .await
                },
                |shell, result, cx| {
                    shell.toast(
                        outcome(
                            result,
                            |record| format!("已恢复为新游戏「{}」", record.name),
                            "没能恢复这个备份".to_owned(),
                        ),
                        cx,
                    )
                },
            );
        });
    })
    .detach();
}

/// Downloads Java into the launcher, showing progress in Activity, and
/// refreshes the Settings Java list when done.
fn install_java(wiring: &Wiring, major: Option<u32>, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.install_java(major, CancellationToken::new()).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::info("开始下载 Java，进度在动态里"), cx)
        });
        while !handle.is_finished() {
            cx.background_executor().timer(POLL).await;
            cx.update(|cx| reload(&wiring, Reload::Lists, cx));
        }
        let Ok(result) = handle.await else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                outcome(
                    result,
                    |java| format!("已安装 Java {}", java.major()),
                    "没有装上 Java".to_owned(),
                ),
                cx,
            )
        });
        cx.update(|cx| {
            reload(&wiring, Reload::All, cx);
            load_settings(&wiring, cx);
        });
    })
    .detach();
}

/// Runs a failed or cancelled task again from its original input. It is a
/// task of its own in Activity; the old entry stays.
fn retry_task(wiring: &Wiring, action: lumilio_core::RetryAction, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let state = wiring.state.clone();
    job(
        wiring,
        cx,
        true,
        Reload::All,
        Some(Toast::info("已重新开始，进度在动态里")),
        async move {
            // Progress lives in Activity; the channel only has to drain.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(async move { while rx.recv().await.is_some() {} });
            service
                .retry_task(action, tx, CancellationToken::new())
                .await
        },
        move |shell, result, cx| {
            state.borrow_mut().installed_key = None;
            shell.toast(
                outcome(
                    result,
                    |()| "这次成功了".to_owned(),
                    "还是没有成功".to_owned(),
                ),
                cx,
            )
        },
    );
}

/// Milliseconds since the Unix epoch, for working out speeds.
fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// Saves a game's log with names and paths hidden, where the person chooses.
fn export_log(wiring: &Wiring, id: String, crash: Option<String>, cx: &mut App) {
    let start = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
    let suggested = match &crash {
        Some(name) => name.clone(),
        None => "latest-log.txt".to_owned(),
    };
    let chosen = platform::pick_save_path(cx, &start, &suggested);
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = wiring.backend.service.clone();
        let target = path.clone();
        let Ok(result) = wiring
            .backend
            .spawn(async move { service.export_log(&id, crash.as_deref(), &target).await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                match result {
                    Ok(()) => {
                        Toast::success(format!("已保存到 {}（名字和路径已隐去）", path.display()))
                    }
                    Err(error) => Toast::error("没能导出日志").technical(error.to_string()),
                },
                cx,
            )
        });
    })
    .detach();
}

/// What the window does around the game, as the preferences say: hide while
/// it runs, come back to the front when it ends.
fn around_the_game(
    wiring: &Wiring,
    window: gpui_kit::AnyWindowHandle,
    signal: &LaunchSignal,
    after: AfterLaunch,
    cx: &mut gpui_kit::AsyncApp,
) {
    let preferences = wiring.state.borrow().preferences.clone();
    match signal {
        LaunchSignal::Running if after == AfterLaunch::Hide => {
            cx.update(|cx| platform::hide_launcher(cx));
        }
        LaunchSignal::Exited { .. } | LaunchSignal::Failed(_) | LaunchSignal::Cancelled
            if preferences.foreground_on_exit() =>
        {
            let _ = cx.update_window(window, |_, window, cx| platform::bring_to_front(window, cx));
        }
        _ => {}
    }
}

// ── launching ───────────────────────────────────────────────────────────

fn start_launch(wiring: &Wiring, window: gpui_kit::AnyWindowHandle, cx: &mut App) {
    let Some(id) = wiring.state.borrow().continue_id.clone() else {
        return;
    };
    let cancel = CancellationToken::new();
    wiring.state.borrow_mut().cancel = Some(cancel.clone());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let service = wiring.backend.service.clone();
    let after = {
        let (service, id) = (service.clone(), id.clone());
        wiring
            .backend
            .spawn(async move { service.after_launch_for(&id).await })
    };
    let world = wiring.state.borrow_mut().next_world.take();
    let handle = wiring.backend.spawn(async move {
        match world {
            Some(world) => service.launch_world(&id, &world, tx, cancel).await,
            None => service.launch(&id, tx, cancel).await,
        }
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let after = after.await.unwrap_or_default();
        let instance = wiring
            .state
            .borrow()
            .continue_id
            .clone()
            .unwrap_or_default();
        wiring.state.borrow_mut().game_log = Some((instance.clone(), Default::default()));
        // The page can tell the game is on its way before it prints anything.
        push_game_log(&wiring, true, cx);
        let mut unsent = false;
        // The channel closes when the launch is over, so this ends with it.
        loop {
            let update = if unsent {
                let next = futures::future::select(
                    Box::pin(rx.recv()),
                    cx.background_executor().timer(GAME_LOG_FLUSH),
                )
                .await;
                match next {
                    futures::future::Either::Left((update, _)) => update,
                    futures::future::Either::Right(_) => {
                        unsent = false;
                        push_game_log(&wiring, true, cx);
                        continue;
                    }
                }
            } else {
                rx.recv().await
            };
            let Some(update) = update else { break };
            if let LaunchUpdate::Log { text, .. } = &update {
                if let Some((_, lines)) = wiring.state.borrow_mut().game_log.as_mut() {
                    if lines.len() == GAME_LOG_LINES {
                        lines.pop_front();
                    }
                    lines.push_back(text.clone());
                }
                unsent = true;
            }
            if let LaunchUpdate::Signal(signal) = update {
                around_the_game(&wiring, window, &signal, after, cx);
                if wiring
                    .shell
                    .update(cx, |shell, cx| shell.apply_launch_signal(signal, cx))
                    .is_err()
                {
                    return;
                }
            }
        }
        // Launcher errors already ended the session with a signal of their own;
        // anything earlier (unknown instance, no catalog) has not.
        if let Ok(Err(error)) = handle.await
            && !matches!(error, ServiceError::Launch(_))
        {
            // Sign-in trouble is told in words, not as the raw cause.
            let message = match &error {
                ServiceError::Auth(_) | ServiceError::SignInRequired(_) => {
                    account_failure(&error).0
                }
                _ => error.to_string(),
            };
            let signal = LaunchSignal::Failed(LaunchFailure::Step { message });
            let _ = wiring
                .shell
                .update(cx, |shell, cx| shell.apply_launch_signal(signal, cx));
        }
        wiring.state.borrow_mut().cancel = None;
        push_game_log(&wiring, false, cx);
        wiring.state.borrow_mut().game_log = None;
        cx.update(|cx| reload(&wiring, Reload::All, cx));
    })
    .detach();
}

/// Shows the game's output so far on the open page of that game.
fn push_game_log(wiring: &Wiring, running: bool, cx: &mut gpui_kit::AsyncApp) {
    let Some((id, lines)) = wiring
        .state
        .borrow()
        .game_log
        .as_ref()
        .map(|(id, lines)| (id.clone(), lines.iter().cloned().collect::<Vec<_>>()))
    else {
        return;
    };
    let _ = wiring
        .shell
        .update(cx, |shell, cx| shell.game_output(&id, lines, running, cx));
}

/// Launches a chosen instance through Home's launch moment.
fn play(wiring: &Wiring, id: String, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    // Deferred: this may be running inside one of the shell's own updates.
    window.defer(cx, move |window, cx| {
        let state = wiring.state.clone();
        let _ = wiring.shell.update(cx, |shell, cx| {
            if matches!(
                shell.home(),
                HomePresentation::Launching { .. } | HomePresentation::Playing { .. }
            ) {
                shell.toast(Toast::info("已经有游戏在运行"), cx);
                return;
            }
            let Some(card) = shell
                .live()
                .and_then(|model| model.library.iter().find(|card| card.id == id))
                .cloned()
            else {
                return;
            };
            state.borrow_mut().continue_id = Some(id);
            shell.set_home(
                HomePresentation::Continue {
                    subject: Subject {
                        title: card.name,
                        metadata: format!("{} · {}", card.meta, card.played),
                        world: card.world,
                    },
                    recent: Vec::new(),
                },
                cx,
            );
            shell.request_continue(window, cx);
        });
    });
}

// ── discover ────────────────────────────────────────────────────────────

fn search(wiring: &Wiring, query: &DiscoverQuery, cx: &mut App) {
    let seq = {
        let mut state = wiring.state.borrow_mut();
        state.search_seq += 1;
        state.search_seq
    };
    let service = wiring.backend.service.clone();
    let state = wiring.state.clone();
    let query = query.to_search();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.search(&query).await },
        move |shell, result, cx| {
            if state.borrow().search_seq != seq {
                return;
            }
            let now = unix_now();
            shell.update_live(
                |model| match result {
                    Ok(page) => {
                        model.results = search_rows(&page, now);
                        model.search = SearchStatus::Done {
                            total: page.total_hits,
                        };
                    }
                    Err(error) => {
                        model.results.clear();
                        model.search = SearchStatus::Failed(error.to_string());
                    }
                },
                cx,
            );
        },
    );
}

fn load_filters(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.discover_filters().await },
        |shell, result, cx| {
            shell.update_live(
                |model| match result {
                    Ok(filters) => model.filters = FilterModel::from_core(&filters),
                    Err(error) => {
                        model.filters.loaded = true;
                        model.filters.error = Some(error.to_string());
                    }
                },
                cx,
            );
        },
    );
}

/// Opening Discover for the first time loads the filter choices and lists
/// what is popular.
fn first_search(wiring: &Wiring, cx: &mut App) {
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let pending = wiring
            .shell
            .read_with(cx, |shell, _| {
                shell
                    .live()
                    .filter(|model| model.search == SearchStatus::Idle)
                    .map(|model| model.query.clone())
            })
            .ok()
            .flatten();
        let Some(query) = pending else { return };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.search = SearchStatus::Searching, cx);
        });
        cx.update(|cx| {
            load_filters(&wiring, cx);
            search(&wiring, &query, cx);
        });
    })
    .detach();
}

/// Installs a project. `version` picks one file of a mod, pack or shader.
fn install(
    wiring: &Wiring,
    kind: ProjectKind,
    slug: String,
    title: String,
    version: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let cancel = CancellationToken::new();
    if kind == ProjectKind::Modpack {
        job(
            wiring,
            cx,
            true,
            Reload::All,
            Some(Toast::info(format!("开始安装 {title}，进度在动态里"))),
            async move { service.install_modpack(&slug, cancel).await },
            move |shell, result, cx| {
                let toast = outcome(
                    result,
                    |record| format!("已安装整合包 {}", record.name),
                    format!("没有装上 {title}"),
                );
                shell.toast(toast, cx);
            },
        );
        return;
    }
    let target = install_target(wiring, cx);
    let Some((instance, instance_name)) = target else {
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::info("先在游戏库里新建一个游戏"), cx);
        });
        return;
    };
    if kind != ProjectKind::Mod {
        run_install(
            wiring,
            instance,
            instance_name,
            kind,
            slug,
            title,
            version,
            Vec::new(),
            cx,
        );
        return;
    }
    // A mod may need other mods. Not being able to find out must not stop
    // the install, so a failed check installs the mod alone.
    let check = {
        let (instance, slug, version) = (instance.clone(), slug.clone(), version.clone());
        wiring.backend.spawn(async move {
            service
                .dependency_report(&instance, ProjectKind::Mod, &slug, version.as_deref())
                .await
        })
    };
    let wiring = wiring.clone();
    let window_handle = window.window_handle();
    cx.spawn(async move |cx| {
        let report = check.await.ok().and_then(Result::ok).unwrap_or_default();
        cx.update(|cx| {
            if !DependencyPrompt::worth_showing(&report) {
                run_install(
                    &wiring,
                    instance,
                    instance_name,
                    kind,
                    slug,
                    title,
                    version,
                    Vec::new(),
                    cx,
                );
                return;
            }
            let _ = cx.update_window(window_handle, |_, window, cx| {
                let form = cx.new(|_| {
                    let (job_wiring, rows) = (
                        wiring.clone(),
                        report
                            .needs
                            .iter()
                            .chain(report.optional.iter())
                            .cloned()
                            .collect::<Vec<_>>(),
                    );
                    let (instance, instance_name) = (instance.clone(), instance_name.clone());
                    let (slug, title, version) = (slug.clone(), title.clone(), version.clone());
                    DependencyPrompt::new(
                        title.clone(),
                        report,
                        Rc::new(move |chosen, _, cx| {
                            // Each chosen dependency, at the version the check found.
                            let extra: Vec<(String, String, String)> = chosen
                                .iter()
                                .filter_map(|id| {
                                    let need = rows.iter().find(|need| &need.project_id == id)?;
                                    let found = need.version.as_ref()?;
                                    Some((id.clone(), found.id.clone(), need.title.clone()))
                                })
                                .collect();
                            run_install(
                                &job_wiring,
                                instance.clone(),
                                instance_name.clone(),
                                kind,
                                slug.clone(),
                                title.clone(),
                                version.clone(),
                                extra,
                                cx,
                            );
                        }),
                    )
                });
                DependencyPrompt::open(form, window, cx);
            });
        });
    })
    .detach();
}

/// Installs `slug` into the game, after any chosen dependencies (project,
/// version, title), each as its own task in Activity. A dependency that fails
/// is told, and the mod itself is still installed.
#[allow(clippy::too_many_arguments)]
fn run_install(
    wiring: &Wiring,
    instance: String,
    instance_name: String,
    kind: ProjectKind,
    slug: String,
    title: String,
    version: Option<String>,
    dependencies: Vec<(String, String, String)>,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let state = wiring.state.clone();
    job(
        wiring,
        cx,
        true,
        Reload::Lists,
        Some(Toast::info(format!("开始安装 {title}，进度在动态里"))),
        async move {
            let mut failed = Vec::new();
            for (project, version_id, name) in dependencies {
                if service
                    .install_version(
                        &instance,
                        kind,
                        &project,
                        &version_id,
                        CancellationToken::new(),
                    )
                    .await
                    .is_err()
                {
                    failed.push(name);
                }
            }
            let installed = match version {
                Some(version) => {
                    service
                        .install_version(&instance, kind, &slug, &version, CancellationToken::new())
                        .await
                }
                None => {
                    service
                        .install_content(&instance, kind, &slug, CancellationToken::new())
                        .await
                }
            };
            (installed, failed)
        },
        move |shell, (result, failed), cx| {
            state.borrow_mut().installed_key = None;
            let installed = result.is_ok();
            let mut toast = outcome(
                result,
                |file| format!("已把 {file} 装进 {instance_name}"),
                format!("没有装上 {title}"),
            );
            // A failed install already says so; a dependency that failed
            // under a successful one is the news.
            if installed && !failed.is_empty() {
                toast = Toast::error(format!(
                    "{} 没有装上，{title} 可能进不了游戏",
                    failed.join("、")
                ));
            }
            shell.toast(toast, cx);
        },
    );
}

/// The instance installs go into: its id and name.
fn install_target(wiring: &Wiring, cx: &mut App) -> Option<(String, String)> {
    wiring
        .shell
        .read_with(cx, |shell, _| {
            shell.live().and_then(|model| {
                let id = model.install_target.clone()?;
                let name = model
                    .library
                    .iter()
                    .find(|card| card.id == id)
                    .map(|card| card.name.clone())?;
                Some((id, name))
            })
        })
        .ok()
        .flatten()
}

/// A project's detail, in place of the Discover list: it appears at once and
/// fills in when the data arrives.
fn open_project(
    wiring: &Wiring,
    kind: ProjectKind,
    slug: String,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    // Deferred: this may be running inside one of the shell's own updates.
    window.defer(cx, move |_, cx| {
        let project = slug.clone();
        let _ = wiring
            .shell
            .update(cx, |shell, cx| shell.open_detail(kind, &project, cx));
        let service = wiring.backend.service.clone();
        let handle = wiring
            .backend
            .spawn(async move { service.project_detail(&slug).await });
        let shell = wiring.shell.clone();
        let project = slug_of(&shell, cx);
        cx.spawn(async move |cx| {
            let Ok(result) = handle.await else { return };
            let state = match result {
                Ok(detail) => DetailState::Ready(Box::new(detail)),
                Err(error) => DetailState::Failed(error.to_string()),
            };
            if let Some(project) = project {
                let _ = shell.update(cx, |shell, cx| {
                    shell.set_detail_state(&project, state, cx);
                    // Dev hook: `LUMILIO_DETAIL_TAB=1|2` starts on Versions / Gallery.
                    if let Some(tab) = std::env::var("LUMILIO_DETAIL_TAB")
                        .ok()
                        .and_then(|tab| tab.parse::<usize>().ok())
                    {
                        shell.select_detail_tab(tab, cx);
                    }
                });
            }
        })
        .detach();
    });
}

/// The slug of the project the shell currently shows.
fn slug_of(shell: &WeakEntity<LauncherShell>, cx: &mut App) -> Option<String> {
    shell
        .read_with(cx, |shell, _| {
            shell.detail_project().map(|(_, slug)| slug.to_owned())
        })
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_operation_says_it_plainly_and_keeps_the_cause_behind_details() {
        let failed = outcome(
            Err::<String, _>("os error 28: no space left"),
            |file| format!("已装好 {file}"),
            "没有装上 Sodium".to_owned(),
        );
        assert_eq!(
            failed,
            Toast::error("没有装上 Sodium").technical("os error 28: no space left")
        );
        assert!(
            !failed.text.contains("os error"),
            "no raw error as headline"
        );
        let done = outcome(
            Ok::<_, String>("sodium.jar"),
            |file| format!("已装好 {file}"),
            String::new(),
        );
        assert_eq!(done, Toast::success("已装好 sodium.jar"));
    }
}
