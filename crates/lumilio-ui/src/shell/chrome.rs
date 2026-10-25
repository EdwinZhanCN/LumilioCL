use gpui::{App, Window};
use gpui_component::{Theme, TitleBar};

use crate::theme;

/// Keeps the gpui-component theme in step with the system appearance, now
/// and whenever it changes.
pub fn follow_system_appearance(window: &mut Window, cx: &mut App) {
    sync_appearance(window, cx);
    window.observe_window_appearance(sync_appearance).detach();
}

pub(crate) fn sync_appearance(window: &mut Window, cx: &mut App) {
    // A look chosen in Settings wins over the system; the review variable
    // (for screenshots) wins over everything.
    let chosen = crate::platform::mode_of(crate::platform::chosen_appearance(cx));
    match std::env::var("LUMILIO_REVIEW_APPEARANCE").as_deref() {
        Ok("light") => Theme::change(gpui_component::ThemeMode::Light, Some(window), cx),
        Ok("dark") => Theme::change(gpui_component::ThemeMode::Dark, Some(window), cx),
        _ => match chosen {
            Some(mode) => Theme::change(mode, Some(window), cx),
            None => Theme::sync_system_appearance(Some(window), cx),
        },
    }
    theme::tune(cx);
    let background = cx
        .try_global::<theme::Look>()
        .map_or_else(Default::default, |look| look.theme.window_background);
    let blurred = background == theme::WindowBackground::Blurred;
    let dark = Theme::global(cx).mode.is_dark();
    // GPUI's own blur shows what is behind the window sharp on current macOS;
    // where our backdrop takes over, the window itself is only transparent.
    let ours = raw_window_handle::HasWindowHandle::window_handle(&*window)
        .is_ok_and(|handle| lumilio_backdrop::set_blur(handle.as_raw(), blurred, dark));
    let material = if blurred && ours {
        theme::WindowBackground::Transparent
    } else {
        background
    };
    window.set_background_appearance(material.to_gpui());
}

/// Title bar options for the launcher window: transparent, so Home's world
/// can run under the traffic lights.
pub fn window_titlebar() -> gpui::TitlebarOptions {
    gpui::TitlebarOptions {
        title: Some("LumilioCL".into()),
        ..TitleBar::title_bar_options()
    }
}
