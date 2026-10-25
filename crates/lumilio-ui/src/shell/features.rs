//! Which feature answers what a page asks: the shell's own features first,
//! the application for the rest.

use std::rc::Rc;

use gpui::WeakEntity;

use super::LauncherShell;
use crate::features::accounts::Accounts;
use crate::features::activity::Activity;
use crate::features::library::Library;
use crate::features::new_game::NewGame;
use crate::features::settings::Settings;
use crate::live::{LiveHandler, LiveIntent};

/// Sends the intents the shell or a feature it owns answers there, and every
/// other one to `app`.
pub(super) fn route(shell: &LauncherShell, app: LiveHandler) -> LiveHandler {
    let this = shell.this.clone();
    let new_game: WeakEntity<NewGame> = shell.new_game.downgrade();
    let library: WeakEntity<Library> = shell.library.downgrade();
    let activity: WeakEntity<Activity> = shell.activity_page.downgrade();
    let settings: WeakEntity<Settings> = shell.settings_page.downgrade();
    let accounts: WeakEntity<Accounts> = shell.accounts.downgrade();
    Rc::new(move |intent, window, cx| match intent {
        LiveIntent::NewInstance => {
            let _ = new_game.update(cx, |new_game, cx| new_game.open(window, cx));
        }
        LiveIntent::ImportPack => {
            let shown_in = window.window_handle();
            let _ = new_game.update(cx, |new_game, cx| new_game.import_pack(shown_in, cx));
        }
        LiveIntent::DropFiles(paths) => {
            let shown_in = window.window_handle();
            let _ = new_game.update(cx, |new_game, cx| {
                new_game.import_files(paths, shown_in, cx);
            });
        }
        LiveIntent::InstallTarget(id) => {
            // Pages may ask while the shell is updating.
            let this = this.clone();
            window.defer(cx, move |_, cx| {
                let _ = this.update(cx, |shell, cx| shell.choose_current(id, cx));
            });
        }
        LiveIntent::ToggleFavorite(id) => {
            let _ = library.update(cx, |library, cx| library.toggle_favorite(id, cx));
        }
        LiveIntent::NewCollection => {
            let _ = library.update(cx, |library, cx| {
                library.ask_collection_name(None, window, cx);
            });
        }
        LiveIntent::RenameCollection(name) => {
            let _ = library.update(cx, |library, cx| {
                library.ask_collection_name(Some(name), window, cx);
            });
        }
        LiveIntent::DeleteCollection(name) => {
            let _ = library.update(cx, |library, cx| library.delete_collection(name, cx));
        }
        LiveIntent::EditCollections(id) => {
            let _ = library.update(cx, |library, cx| {
                library.ask_collections_of(id, window, cx);
            });
        }
        LiveIntent::ImportGame => {
            let _ = library.update(cx, |library, cx| library.import_game(window, cx));
        }
        LiveIntent::RestoreBackup => {
            let _ = library.update(cx, |library, cx| library.restore_backup(cx));
        }
        LiveIntent::RevealGame(id) => {
            let _ = library.read_with(cx, |library, cx| library.reveal_game(&id, cx));
        }
        LiveIntent::OpenGamesFolder => {
            let _ = library.read_with(cx, |library, cx| library.open_games_folder(cx));
        }
        LiveIntent::Preset(intent) => {
            let _ = library.update(cx, |library, cx| library.preset(intent, window, cx));
        }
        LiveIntent::CancelTask(task) => {
            let _ = activity.update(cx, |activity, _| activity.cancel(task));
        }
        LiveIntent::PauseTask(task) => {
            let _ = activity.update(cx, |activity, cx| activity.pause(task, cx));
        }
        LiveIntent::ResumeTask(task) => {
            let _ = activity.update(cx, |activity, cx| activity.resume(task, cx));
        }
        LiveIntent::RetryTask(action) => {
            let _ = activity.update(cx, |activity, cx| activity.retry(action, cx));
        }
        LiveIntent::ClearFinished => {
            let _ = activity.update(cx, |activity, cx| activity.clear_finished(cx));
        }
        intent @ (LiveIntent::LoadSettings
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
        | LiveIntent::ExportDiagnostics) => {
            let _ = settings.update(cx, |settings, cx| settings.answer(intent, window, cx));
        }
        LiveIntent::SelectAccount(key) => {
            // The chip and the page may ask while the shell is updating.
            let this = this.clone();
            window.defer(cx, move |_, cx| {
                let _ = this.update(cx, |shell, cx| shell.choose_account(key, cx));
            });
        }
        LiveIntent::NewAccount => {
            let _ = accounts.update(cx, |accounts, cx| accounts.add_offline(false, window, cx));
        }
        LiveIntent::MicrosoftSignIn => {
            let _ = accounts.update(cx, |accounts, cx| accounts.sign_in_microsoft(window, cx));
        }
        LiveIntent::ThirdPartySignIn => {
            let _ = accounts.update(cx, |accounts, cx| accounts.sign_in_third_party(window, cx));
        }
        LiveIntent::ManageAuthServers => {
            let _ = accounts.update(cx, |accounts, cx| accounts.manage_servers(window, cx));
        }
        LiveIntent::RefreshAccount(key) => {
            let _ = accounts.update(cx, |accounts, cx| accounts.refresh(key, cx));
        }
        LiveIntent::RemoveAccount(key) => {
            let _ = accounts.update(cx, |accounts, cx| accounts.remove(key, cx));
        }
        LiveIntent::OpenSkinSite(address) => {
            let _ = accounts.update(cx, |accounts, cx| accounts.open_skin_site(address, cx));
        }
        other => app(other, window, cx),
    })
}
