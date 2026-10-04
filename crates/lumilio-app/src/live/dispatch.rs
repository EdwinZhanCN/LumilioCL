use super::accounts::{open_add_account, open_microsoft_sign_in};
use super::discover::{first_search, install, install_target, load_filters, open_project, search};
use super::instance::open_instance;
use super::jobs::{job, outcome, reload, retry_task};
use super::launch::{play, start_launch};
use super::library::{
    ask_collection_name, ask_collections_of, choose_current, import_game, restore_backup,
};
use super::new_game::{import_pack, import_pack_file, open_new_game};
use super::settings::{
    add_java, apply_preferences, change_setting, check_reclaimable, clear_cache,
    export_diagnostics, install_java, load_settings, reclaim,
};
use super::{Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, Window};
use lumilio_core::CancellationToken;
use lumilio_ui::home::HomeIntent;
use lumilio_ui::instance_detail::InstanceIntent;
use lumilio_ui::live::{LiveIntent, account_failure, account_rows};
use lumilio_ui::platform;
use lumilio_ui::route::Route;
use lumilio_ui::shell::ShellIntent;
use lumilio_ui::toast::Toast;

pub(super) fn on_shell_intent(
    wiring: &Wiring,
    intent: ShellIntent,
    window: &mut Window,
    cx: &mut App,
) {
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

pub(super) fn on_live_intent(
    wiring: &Wiring,
    intent: LiveIntent,
    window: &mut Window,
    cx: &mut App,
) {
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
        LiveIntent::RememberDiscover(discover) => {
            let preferences = {
                let mut state = wiring.state.borrow_mut();
                state.preferences.discover = discover;
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
