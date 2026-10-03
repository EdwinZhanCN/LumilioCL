//! The few things the launcher asks of the operating system: file pickers,
//! the file manager, hiding and raising the window, and the look the person
//! chose. The application decides when; this is only how.

use std::future::Future;
use std::path::{Path, PathBuf};

use gpui::{App, Global, PathPromptOptions, Window};
use gpui_component::ThemeMode;
use lumilio_core::{Appearance, MotionPreference};

/// What the person chose for the look; `System` follows the OS.
#[derive(Default)]
struct AppearanceChoice(Appearance);

impl Global for AppearanceChoice {}

/// What the operating system asked for motion, remembered before the first
/// override so "follow the system" can go back to it.
struct SystemMotion(bool);

impl Global for SystemMotion {}

/// The chosen look, for the shell's own syncing.
pub(crate) fn chosen_appearance(cx: &App) -> Appearance {
    cx.try_global::<AppearanceChoice>()
        .map_or(Appearance::System, |choice| choice.0)
}

/// Switches to the chosen look now; later system changes still follow it.
pub fn apply_appearance(appearance: Appearance, window: &mut Window, cx: &mut App) {
    cx.set_global(AppearanceChoice(appearance));
    crate::shell::sync_appearance(window, cx);
}

/// Applies the motion preference to every window.
pub fn apply_motion(motion: MotionPreference, cx: &mut App) {
    if cx.try_global::<SystemMotion>().is_none() {
        let system = cx.reduce_motion();
        cx.set_global(SystemMotion(system));
    }
    let system = cx.global::<SystemMotion>().0;
    cx.set_reduce_motion(match motion {
        MotionPreference::System => system,
        MotionPreference::Reduce => true,
        MotionPreference::Full => false,
    });
}

/// The theme mode a choice means right now, for tests and previews.
#[must_use]
pub fn mode_of(appearance: Appearance) -> Option<ThemeMode> {
    match appearance {
        Appearance::System => None,
        Appearance::Light => Some(ThemeMode::Light),
        Appearance::Dark => Some(ThemeMode::Dark),
    }
}

/// Lets the person choose one file or folder; `None` when they cancel.
pub fn pick_path(
    cx: &App,
    files: bool,
    directories: bool,
    prompt: &'static str,
) -> impl Future<Output = Option<PathBuf>> + use<> {
    let answer = cx.prompt_for_paths(PathPromptOptions {
        files,
        directories,
        multiple: false,
        prompt: Some(prompt.into()),
    });
    async move {
        answer
            .await
            .ok()
            .and_then(Result::ok)
            .flatten()
            .and_then(|paths| paths.into_iter().next())
    }
}

/// Lets the person choose where a new file goes; `None` when they cancel.
pub fn pick_save_path(
    cx: &App,
    directory: &Path,
    suggested_name: &str,
) -> impl Future<Output = Option<PathBuf>> + use<> {
    let answer = cx.prompt_for_new_path(directory, Some(suggested_name));
    async move { answer.await.ok().and_then(Result::ok).flatten() }
}

/// Shows a file or folder in the system file manager.
pub fn reveal(path: &Path, cx: &App) {
    cx.reveal_path(path);
}

/// Opens an address in the person's browser.
pub fn open_address(address: &str, cx: &App) {
    cx.open_url(address);
}

/// Hides the launcher while the game runs.
pub fn hide_launcher(cx: &App) {
    cx.hide();
}

/// Brings the launcher back to the front.
pub fn bring_to_front(window: &Window, cx: &App) {
    cx.activate(true);
    window.activate_window();
}
