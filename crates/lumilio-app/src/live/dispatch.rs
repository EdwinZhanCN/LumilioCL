use gpui_kit::{App, AppContext as _, Window};
use lumilio_ui::home::HomeIntent;
use lumilio_ui::instance_detail::InstanceIntent;
use lumilio_ui::java_conflict::{JavaConflict, JavaConflictAnswer};
use lumilio_ui::live::{LiveIntent, PlaceTarget, account_failure, account_rows};
use lumilio_ui::missing_folder::MissingFolderAnswer;
use lumilio_ui::route::Route;
use lumilio_ui::shell::ShellIntent;
use lumilio_ui::toast::Toast;
use lumilio_ui::{java_conflict, missing_folder, platform, tr};

use super::accounts::{
    load_account_face, load_account_look, open_add_account, open_auth_servers,
    open_microsoft_sign_in, open_skin_dialog, open_third_party_sign_in,
};
use super::instance::open_instance;
use super::jobs::{job, outcome, reload};
use super::launch::{play, start_launch, technical_details};
use super::settings::{
    add_java, apply_preferences, change_setting, check_reclaimable, clear_cache,
    export_diagnostics, install_java, load_settings, reclaim, reload_themes, set_wallpaper,
};
use super::{Reload, Wiring, update};

/// Asks about a Java that cannot run the game. An answer starts the launch
/// again (with the automatic Java this once, or as chosen); cancelling drops
/// the world or server the launch was headed for.
fn ask_java(wiring: &Wiring, conflict: &JavaConflict, window: &mut Window, cx: &mut App) {
    let targets = {
        let mut state = wiring.state.borrow_mut();
        (state.next_world.take(), state.next_server.take())
    };
    let wiring = wiring.clone();
    java_conflict::ask(
        conflict,
        std::rc::Rc::new(move |answer, window, cx| {
            let Some(id) = wiring.backend.service.context().current_instance else {
                return;
            };
            if answer == JavaConflictAnswer::Switch {
                wiring.backend.service.launch_next_with_automatic_java(&id);
            }
            {
                let mut state = wiring.state.borrow_mut();
                state.java_answered = Some(id);
                (state.next_world, state.next_server) = targets.clone();
            }
            let _ = wiring
                .shell
                .update(cx, |shell, cx| shell.request_continue(window, cx));
        }),
        window,
        cx,
    );
}

pub(super) fn on_shell_intent(
    wiring: &Wiring,
    intent: ShellIntent,
    window: &mut Window,
    cx: &mut App,
) {
    match intent {
        // Home explains a failure only while its game is current, so
        // recovering retries that game.
        ShellIntent::Home(HomeIntent::Continue | HomeIntent::Recover) => {
            continue_launch(wiring, window.window_handle(), cx);
        }
        ShellIntent::Home(HomeIntent::CancelLaunch | HomeIntent::StopGame) => {
            if let Some(cancel) = wiring.state.borrow().cancel.clone() {
                cancel.cancel();
            }
        }
        ShellIntent::Navigate(Route::Settings) => load_settings(wiring, cx),
        ShellIntent::Navigate(Route::Library | Route::Activity | Route::Accounts) => {
            reload(wiring, Reload::Lists, cx)
        }
        ShellIntent::Home(HomeIntent::TechnicalDetails) => technical_details(wiring, window, cx),
        // Discover loads its own first page (the shell tells it it was shown),
        // and the shell's New Game opens its own dialog.
        ShellIntent::Navigate(Route::Home | Route::Discover)
        | ShellIntent::Home(HomeIntent::Import | HomeIntent::Create) => {}
    }
}

/// Asks about a game whose folder was moved or deleted (plan
/// game-folder-missing). Playing it with an empty folder makes one and starts
/// again; viewing opens its page. The world or server the launch was headed
/// for is dropped: an empty folder has no worlds.
fn ask_missing_folder(wiring: &Wiring, id: String, name: &str, window: &mut Window, cx: &mut App) {
    {
        let mut state = wiring.state.borrow_mut();
        state.next_world = None;
        state.next_server = None;
    }
    let wiring = wiring.clone();
    missing_folder::ask(
        name,
        std::rc::Rc::new(move |choice, window, cx| match choice {
            MissingFolderAnswer::ViewGame => open_instance(&wiring, id.clone(), None, window, cx),
            MissingFolderAnswer::StartEmpty => {
                let service = wiring.backend.service.clone();
                let id = id.clone();
                let made = wiring
                    .backend
                    .spawn(async move { service.new_game_folder(&id).await });
                let (wiring, window) = (wiring.clone(), window.window_handle());
                cx.spawn(async move |cx| {
                    let Some(made) = lumilio_ui::runtime::answer(made, cx).await else {
                        return;
                    };
                    if let Err(error) = made {
                        let toast =
                            Toast::error(tr!("missing-folder-failed")).technical(error.to_string());
                        let _ = wiring.shell.update(cx, |shell, cx| shell.toast(toast, cx));
                        return;
                    }
                    let _ = cx.update_window(window, |_, window, cx| {
                        let _ = wiring
                            .shell
                            .update(cx, |shell, cx| shell.request_continue(window, cx));
                    });
                })
                .detach();
            }
        }),
        window,
        cx,
    );
}

/// Starts the current game, once its folder is there, there is an account
/// and its Java can run it.
fn continue_launch(wiring: &Wiring, window: gpui_kit::AnyWindowHandle, cx: &mut App) {
    // Asked of the service, not the page: an account added a moment ago
    // may not have reached the page's model yet.
    let service = wiring.backend.service.clone();
    let current = wiring.backend.service.context().current_instance;
    let answered = wiring.state.borrow_mut().java_answered.take();
    let check_java = current.clone().filter(|id| answered.as_ref() != Some(id));
    let work = wiring.backend.spawn(async move {
        // Whether to play this game at all comes before account and Java.
        let missing = match current {
            Some(id) if !service.game_dir(&id).is_dir() => service
                .instance(&id)
                .await
                .ok()
                .map(|record| (record.id, record.name)),
            _ => None,
        };
        let has_account = account_rows(&service.settings().await)
            .iter()
            .any(|row| row.selected);
        let conflict = match check_java {
            Some(id) => service
                .instance_java(&id)
                .await
                .ok()
                .and_then(|java| JavaConflict::of(&java)),
            None => None,
        };
        (missing, has_account, conflict)
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let (missing, has_account, conflict) = lumilio_ui::runtime::answer(work, cx)
            .await
            .unwrap_or((None, true, None));
        if missing.is_none() && has_account && conflict.is_none() {
            cx.update(|cx| start_launch(&wiring, window, cx));
            return;
        }
        let _ = wiring.shell.update(cx, |shell, cx| shell.abort_launch(cx));
        let _ = cx.update_window(window, |_, window, cx| match (missing, conflict) {
            (Some((id, name)), _) => ask_missing_folder(&wiring, id, &name, window, cx),
            // The chosen Java cannot run the game: ask once, then start.
            (None, Some(conflict)) if has_account => ask_java(&wiring, &conflict, window, cx),
            // No account: ask for one, then start where the person left off.
            _ => open_add_account(&wiring, true, window, cx),
        });
    })
    .detach();
}

pub(super) fn on_live_intent(
    wiring: &Wiring,
    intent: LiveIntent,
    window: &mut Window,
    cx: &mut App,
) {
    match intent {
        // The shell and its features answer these; they never reach here.
        LiveIntent::NewInstance
        | LiveIntent::ImportPack
        | LiveIntent::DropFiles(_)
        | LiveIntent::InstallTarget(_)
        | LiveIntent::ToggleFavorite(_)
        | LiveIntent::NewCollection
        | LiveIntent::RenameCollection(_)
        | LiveIntent::DeleteCollection(_)
        | LiveIntent::EditCollections(_)
        | LiveIntent::ImportGame
        | LiveIntent::RestoreBackup
        | LiveIntent::RevealGame(_)
        | LiveIntent::OpenGamesFolder
        | LiveIntent::Preset(_)
        | LiveIntent::CancelTask(_)
        | LiveIntent::PauseTask(_)
        | LiveIntent::ResumeTask(_)
        | LiveIntent::RetryTask(_)
        | LiveIntent::ClearFinished => {}
        LiveIntent::OpenInstance(id) => open_instance(wiring, id, None, window, cx),
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
        LiveIntent::ExportPackOf(id) => {
            open_instance(wiring, id, Some(InstanceIntent::ExportPack), window, cx)
        }
        LiveIntent::Refresh => {
            reload(wiring, Reload::Lists, cx);
            super::context::check_again(wiring, cx);
        }
        LiveIntent::NewAccount => open_add_account(wiring, false, window, cx),
        LiveIntent::MicrosoftSignIn => open_microsoft_sign_in(wiring, window, cx),
        LiveIntent::ThirdPartySignIn => open_third_party_sign_in(wiring, window, cx),
        LiveIntent::ManageAuthServers => open_auth_servers(wiring, window, cx),
        LiveIntent::EditSkin(key) => open_skin_dialog(wiring, key, window, cx),
        LiveIntent::LoadAccountLook(key) => load_account_look(wiring, key, cx),
        LiveIntent::LoadAccountFace(key) => load_account_face(wiring, key, cx),
        LiveIntent::Wardrobe {
            key,
            revision,
            action,
        } => super::wardrobe::run(wiring, key, revision, action, cx),
        LiveIntent::SaveInlineSkin { revision, intent } => {
            super::wardrobe::save_offline(wiring, revision, intent, cx)
        }
        LiveIntent::OpenSkinSite(address) => super::wardrobe::open_site(wiring, address, cx),
        LiveIntent::RefreshAccount(key) => {
            let service = wiring.backend.service.clone();
            job(
                wiring,
                cx,
                false,
                Reload::Nothing,
                None,
                async move { service.refresh_account(&key).await },
                |shell, result, cx| {
                    shell.toast(
                        match result {
                            Ok(()) => Toast::success(tr!("account-refreshed")),
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
                Reload::Nothing,
                None,
                async move { service.select_account(&name).await },
                |shell, result, cx| {
                    if let Err(error) = result {
                        shell.toast(
                            Toast::error(tr!("account-switch-failed")).technical(error.to_string()),
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
                Reload::Nothing,
                None,
                async move { service.remove_account(&name).await },
                move |shell, result, cx| {
                    shell.toast(
                        outcome(
                            result,
                            |()| tr!("account-removed", name = removed.as_str()),
                            tr!("account-remove-failed").to_owned(),
                        ),
                        cx,
                    );
                },
            );
        }
        LiveIntent::LoadSettings => load_settings(wiring, cx),
        LiveIntent::CheckUpdates => update::check_manually(wiring),
        LiveIntent::SetAutoUpdate(enabled) => {
            let service = wiring.backend.service.clone();
            let update_wiring = wiring.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_auto_update(enabled).await },
                tr!("settings-save-failed"),
                None,
                move |_, _| update::set_automatic(&update_wiring, enabled),
                cx,
            );
        }
        LiveIntent::ApplyUpdate => update::apply(wiring),
        LiveIntent::SetPluginEnabled { id, enabled } => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_plugin_enabled(&id, enabled).await },
                tr!("settings-plugin-save-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetPluginValue { id, key, value } => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_plugin_value(&id, &key, value).await },
                tr!("settings-plugin-save-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::ResetPlugin(id) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.reset_plugin(&id).await },
                tr!("settings-plugin-reset-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::EditPluginSetting { id, key } => {
            super::settings::edit_plugin_setting(wiring, id, key, window, cx);
        }
        LiveIntent::ReloadThemes => reload_themes(wiring, window.window_handle(), cx),
        LiveIntent::SetWallpaper(path) => set_wallpaper(wiring, path, window, cx),
        LiveIntent::SetPreferences(preferences) => {
            let service = wiring.backend.service.clone();
            let saved = preferences.clone();
            let apply_wiring = wiring.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_preferences(saved).await },
                tr!("settings-save-failed"),
                None,
                move |window, cx| apply_preferences(&apply_wiring, &preferences, window, cx),
                cx,
            );
        }
        LiveIntent::SetLaunchDefaults(launch) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_launch_defaults(launch).await },
                tr!("settings-save-failed"),
                Some(tr!("settings-saved-next-launch")),
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
                tr!("settings-memory-save-failed"),
                Some(tr!("settings-saved-next-launch")),
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
                tr!("settings-save-failed"),
                Some(tr!("settings-saved-next-download")),
                |_, _| {},
                cx,
            );
        }
        LiveIntent::AddMirrorPreset(preset) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.add_mirror_preset(preset).await },
                tr!("settings-mirror-add-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetMirrors(mirrors) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_mirror_rules(mirrors).await },
                tr!("settings-mirrors-save-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetDownloadSource(preference) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_download_source(preference).await },
                tr!("settings-download-source-save-failed"),
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
                tr!("settings-java-roots-save-failed"),
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
                tr!("settings-java-save-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::AddJava => add_java(wiring, cx),
        LiveIntent::InstallJava(major) => install_java(wiring, None, major, cx),
        LiveIntent::DownloadJava { source, major } => {
            install_java(wiring, Some(source), Some(major), cx);
        }
        LiveIntent::RemoveJava(home) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.remove_java(&home).await },
                tr!("settings-java-remove-failed"),
                Some(tr!("settings-java-removed")),
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetJavaSource(source) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_java_source(source).await },
                tr!("settings-java-save-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetAutoDownloadBlockAssets(enabled) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_auto_download_block_assets(enabled).await },
                tr!("settings-save-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::SetAutoInstallJava(enabled) => {
            let service = wiring.backend.service.clone();
            change_setting(
                wiring,
                window,
                async move { service.set_auto_install_java(enabled).await },
                tr!("settings-java-save-failed"),
                None,
                |_, _| {},
                cx,
            );
        }
        LiveIntent::Reveal(path) => platform::reveal(&path, cx),
        LiveIntent::ClearCache => clear_cache(wiring, cx),
        LiveIntent::ClearMapCache => super::settings::clear_map_cache(wiring, cx),
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
        LiveIntent::PlayPlace(id, target) => {
            let mut state = wiring.state.borrow_mut();
            match target {
                PlaceTarget::World(world) => state.next_world = Some(world),
                PlaceTarget::Server(address) => state.next_server = Some(address),
            }
            drop(state);
            play(wiring, id, window, cx);
        }
        LiveIntent::ClearTranslationCache => super::settings::clear_cache(wiring, cx),
    }
}
