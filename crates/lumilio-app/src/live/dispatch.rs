use gpui_kit::{App, Window};
use lumilio_ui::live::LiveIntent;
use lumilio_ui::route::Route;
use lumilio_ui::shell::ShellIntent;
use lumilio_ui::toast::Toast;

use super::jobs::reload;
use super::{Reload, Wiring, update};

pub(super) fn on_shell_intent(wiring: &Wiring, intent: ShellIntent, _: &mut Window, cx: &mut App) {
    match intent {
        ShellIntent::Navigate(Route::Library | Route::Activity | Route::Accounts) => {
            reload(wiring, Reload::Lists, cx)
        }
        // Discover loads its own first page and Settings reads its own
        // settings (the shell tells them they were shown), and the shell
        // answers Home: it launches, stops, explains and opens New Game.
        ShellIntent::Navigate(Route::Home | Route::Discover | Route::Settings)
        | ShellIntent::Home(_) => {}
    }
}

pub(super) fn on_live_intent(wiring: &Wiring, intent: LiveIntent, _: &mut Window, cx: &mut App) {
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
        | LiveIntent::ExportPackOf(_)
        | LiveIntent::Play(_)
        | LiveIntent::PlayPlace(..) => {}
        LiveIntent::ApplyUpdate => update::apply(wiring, cx),
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
    }
}
