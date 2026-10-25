use gpui_kit::{App, AppContext as _, Window};
use lumilio_ui::home::HomeIntent;
use lumilio_ui::java_conflict::{JavaConflict, JavaConflictAnswer};
use lumilio_ui::live::{LiveIntent, PlaceTarget, account_rows};
use lumilio_ui::missing_folder::MissingFolderAnswer;
use lumilio_ui::route::Route;
use lumilio_ui::shell::ShellIntent;
use lumilio_ui::toast::Toast;
use lumilio_ui::{java_conflict, missing_folder, tr};

use super::jobs::reload;
use super::launch::{play, start_launch, technical_details};
use super::open_game::open_instance;
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
        ShellIntent::Navigate(Route::Library | Route::Activity | Route::Accounts) => {
            reload(wiring, Reload::Lists, cx)
        }
        ShellIntent::Home(HomeIntent::TechnicalDetails) => technical_details(wiring, window, cx),
        // Discover loads its own first page and Settings reads its own
        // settings (the shell tells them they were shown), and the shell's
        // New Game opens its own dialog.
        ShellIntent::Navigate(Route::Home | Route::Discover | Route::Settings)
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
            _ => {
                if let Ok(accounts) = wiring
                    .shell
                    .read_with(cx, |shell, _| shell.accounts().clone())
                {
                    accounts.update(cx, |accounts, cx| accounts.add_offline(true, window, cx));
                }
            }
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
        | LiveIntent::ClearFinished
        | LiveIntent::LoadSettings
        | LiveIntent::CheckUpdates
        | LiveIntent::SetAutoUpdate(_)
        | LiveIntent::SetPreferences(_)
        | LiveIntent::SetLaunchDefaults(_)
        | LiveIntent::SetMemory { .. }
        | LiveIntent::SetConcurrency(_)
        | LiveIntent::AddMirrorPreset(_)
        | LiveIntent::SetMirrors(_)
        | LiveIntent::SetDownloadSource(_)
        | LiveIntent::SetAutoDownloadBlockAssets(_)
        | LiveIntent::SetJavaRoots(_)
        | LiveIntent::SetJavaDisabled { .. }
        | LiveIntent::SetJavaSource(_)
        | LiveIntent::SetAutoInstallJava(_)
        | LiveIntent::RemoveJava(_)
        | LiveIntent::AddJava
        | LiveIntent::DownloadJava { .. }
        | LiveIntent::SetPluginEnabled { .. }
        | LiveIntent::SetPluginValue { .. }
        | LiveIntent::ResetPlugin(_)
        | LiveIntent::EditPluginSetting { .. }
        | LiveIntent::ReloadThemes
        | LiveIntent::SetWallpaper(_)
        | LiveIntent::Reveal(_)
        | LiveIntent::ClearCache
        | LiveIntent::ClearTranslationCache
        | LiveIntent::ClearMapCache
        | LiveIntent::CheckReclaimable
        | LiveIntent::Reclaim
        | LiveIntent::ExportDiagnostics
        | LiveIntent::NewAccount
        | LiveIntent::MicrosoftSignIn
        | LiveIntent::ThirdPartySignIn
        | LiveIntent::ManageAuthServers
        | LiveIntent::OpenSkinSite(_)
        | LiveIntent::RefreshAccount(_)
        | LiveIntent::SelectAccount(_)
        | LiveIntent::RemoveAccount(_)
        | LiveIntent::OpenInstance(_)
        | LiveIntent::Resolve(..)
        | LiveIntent::CopyGameOf(_)
        | LiveIntent::DeleteGameOf(_)
        | LiveIntent::ExportPackOf(_) => {}
        LiveIntent::ApplyUpdate => update::apply(wiring),
        LiveIntent::StopGame(id) => {
            // Stops the launch or the running game, if it is this one.
            let this_one = wiring
                .backend
                .service
                .context()
                .session
                .is_some_and(|session| session.is_active() && session.instance == id);
            if this_one && let Some(cancel) = wiring.state.borrow().cancel.clone() {
                cancel.cancel();
            }
        }
        LiveIntent::Refresh => {
            reload(wiring, Reload::Lists, cx);
            super::context::check_again(wiring, cx);
        }
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
    }
}
