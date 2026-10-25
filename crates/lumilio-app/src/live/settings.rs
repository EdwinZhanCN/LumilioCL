use gpui_kit::{App, AppContext as _, Window};
use lumilio_core::{Preferences, WallpaperChoice, remove_wallpapers};
use lumilio_ui::platform;

use super::Wiring;

/// Applies what the saved preferences say about the look, the motion and
/// the language, and keeps them for what the application saves with them.
pub(super) fn apply_preferences(
    wiring: &Wiring,
    preferences: &Preferences,
    window: &mut Window,
    cx: &mut App,
) {
    wiring.state.borrow_mut().preferences = preferences.clone();
    platform::apply_appearance(preferences.appearance, window, cx);
    platform::apply_look(&preferences.look, window, cx);
    apply_wallpaper(wiring, preferences.look.wallpaper.clone(), cx);
    platform::apply_motion(preferences.motion, cx);
    lumilio_ui::i18n::apply_language(preferences.language, cx);
    let translation = preferences.translation.enabled;
    let _ = wiring
        .shell
        .update(cx, |shell, cx| shell.set_translation(translation, cx));
    // Plugin text is the plugin's own; tell the host which tag to read it in
    // without blocking the UI thread.
    let service = wiring.backend.service.clone();
    let tag = lumilio_ui::i18n::locale().tag().to_owned();
    drop(
        wiring
            .backend
            .spawn(async move { service.set_plugin_locale(&tag).await }),
    );
}

/// Shows the chosen wallpaper once its file is found, off the UI thread. The
/// world or a bare Home clears the stored pictures; a picture whose file is
/// gone shows the world and says so on the Appearance tab.
fn apply_wallpaper(wiring: &Wiring, chosen: WallpaperChoice, cx: &mut App) {
    let dir = wiring.backend.service.layout().themes();
    let bare = chosen == WallpaperChoice::Bare;
    let check = wiring.backend.spawn(async move {
        let WallpaperChoice::Picture(name) = chosen else {
            let cleared = dir.clone();
            let _ = tokio::task::spawn_blocking(move || remove_wallpapers(&cleared, None)).await;
            return (None, false);
        };
        // A hand-edited name must stay inside the folder.
        let plain = std::path::Path::new(&name).file_name() == Some(name.as_ref());
        let path = dir.join(&name);
        if plain
            && tokio::fs::metadata(&path)
                .await
                .is_ok_and(|meta| meta.is_file())
        {
            (Some(path), false)
        } else {
            (None, true)
        }
    });
    cx.spawn(async move |cx| {
        if let Ok((path, missing)) = check.await {
            cx.update(|cx| platform::apply_wallpaper(path, bare, missing, cx));
        }
    })
    .detach();
}

/// Reads the saved preferences once at start-up and applies them, after
/// Settings has read the local themes.
pub(super) fn apply_saved_preferences(
    wiring: &Wiring,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.settings().await.preferences });
    if let Ok(settings) = wiring
        .shell
        .read_with(cx, |shell, _| shell.settings_page().clone())
    {
        settings.update(cx, |settings, cx| settings.reload_themes(window, cx));
    }
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(preferences) = lumilio_ui::runtime::answer(handle, cx).await else {
            return;
        };
        let _ = cx.update_window(window, |_, window, cx| {
            apply_preferences(&wiring, &preferences, window, cx)
        });
        // The Library comes back ordered and filtered as it was left, and
        // Discover keeps its advanced exclusions.
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.apply_discover_preferences(preferences.discover.clone(), cx);
            shell.library().update(cx, |library, cx| {
                library.apply_preferences(preferences.library_sort, preferences.library_loader, cx)
            });
        });
    })
    .detach();
}
