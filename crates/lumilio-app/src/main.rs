// A released Windows build opens no console window beside the launcher.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod backend;
mod images;
mod live;

use gpui_kit::{App, WindowBounds, WindowOptions, px, size};
use lumilio_ui::tr;

/// The launcher's platform identity: the macOS bundle identifier, the Windows
/// AppUserModelID, the Wayland app id that ties the window to its `.desktop`
/// file. Fixed once released (assets/icons/PACKAGING.md §0); development
/// builds take the `.dev` variant so they never share it with the release.
const APP_ID: &str = if cfg!(debug_assertions) {
    "app.lumilio.LumilioCL.dev"
} else {
    "app.lumilio.LumilioCL"
};

fn main() {
    let opened = backend::data_root(|name| std::env::var(name).ok())
        .ok_or_else(|| "no data folder could be determined".to_owned())
        .and_then(backend::Backend::open);
    let backend = match opened {
        Ok(backend) => {
            // Until Diagnostics shows them, recovery results go to stderr.
            for note in backend.service.startup_notes() {
                eprintln!("LumilioCL start-up recovery: {note:?}");
            }
            backend
        }
        Err(reason) => {
            eprintln!("LumilioCL could not open its data folder: {reason}");
            std::process::exit(1);
        }
    };
    // Listens for the life of the window; dropping it closes every session.
    let _agents = serve_agents(&backend);
    let mut app = gpui_kit::application().with_assets(assets::AppAssets);
    match images::ImageClient::new() {
        Ok(client) => app = app.with_http_client(client),
        Err(reason) => eprintln!("pictures from the web will not load: {reason}"),
    }
    app.run(move |cx: &mut App| {
        cx.set_app_identity(APP_ID, "LumilioCL");
        gpui_kit::init(cx);
        lumilio_ui::assets::register_fonts(cx);
        let quit_key = if cfg!(target_os = "macos") {
            "cmd-q"
        } else {
            "ctrl-q"
        };
        cx.bind_keys([gpui_kit::KeyBinding::new(
            quit_key,
            lumilio_ui::shell::Quit,
            None,
        )]);
        cx.on_action(|_: &lumilio_ui::shell::Quit, cx| cx.quit());
        cx.set_menus([gpui_kit::Menu {
            name: "LumilioCL".into(),
            disabled: false,
            items: vec![gpui_kit::MenuItem::action(
                tr!("app-quit"),
                lumilio_ui::shell::Quit,
            )],
        }]);

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        // Isolated visual-review hooks; they never persist system preferences.
        // LUMILIO_REVIEW_SIZE=<width>x<height> fixes the window for recordings.
        let minimum = std::env::var("LUMILIO_REVIEW_MIN").as_deref() == Ok("1");
        let requested = std::env::var("LUMILIO_REVIEW_SIZE")
            .ok()
            .and_then(|value| review_size(&value));
        let review_size = match requested {
            Some((width, height)) => size(px(width), px(height)),
            None if minimum => size(px(720.), px(480.)),
            None => size(px(1080.), px(720.)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(review_size, cx)),
            window_min_size: Some(size(px(720.), px(480.))),
            titlebar: Some(lumilio_ui::window_titlebar()),
            app_id: Some(APP_ID.to_owned()),
            ..WindowOptions::default()
        };

        gpui_kit::open_window(options, cx, move |window, cx| {
            let shell = live::build(backend, window, cx);
            if std::env::var("LUMILIO_REVIEW_STILL").as_deref() == Ok("1") {
                cx.set_reduce_motion(true);
            }
            shell
        })
        .expect("failed to open the LumilioCL window");

        cx.activate(true);
    });
}

/// Agents connect over loopback HTTP (plan agent-mcp): when the settings say
/// so, or with `LUMILIO_MCP=1` while Settings has no switch yet. A port that
/// moved because the configured one was busy is kept, so an agent's saved
/// address stays valid next time.
fn serve_agents(backend: &backend::Backend) -> Option<lumilio_agent::Running> {
    let settings = backend.block_on(backend.service.settings());
    let enabled = settings.agent.enabled || std::env::var("LUMILIO_MCP").as_deref() == Ok("1");
    if !enabled {
        return None;
    }
    let service = backend.service.clone();
    match backend.block_on(lumilio_agent::serve(service, settings.agent.port)) {
        Ok(running) => {
            if running.port() != settings.agent.port {
                let moved = lumilio_core::AgentSettings {
                    port: running.port(),
                    ..settings.agent
                };
                if let Err(error) = backend.block_on(backend.service.set_agent_settings(moved)) {
                    eprintln!("LumilioCL could not keep the agent port: {error}");
                }
            }
            eprintln!("LumilioCL agents connect at {}", running.url());
            Some(running)
        }
        Err(error) => {
            eprintln!("LumilioCL agents cannot connect: {error}");
            None
        }
    }
}

/// Parses `<width>x<height>` in logical pixels, no smaller than the window's
/// minimum size.
/// The window size the review hooks ask for: `LUMILIO_REVIEW_SIZE`, or the
/// smallest window with `LUMILIO_REVIEW_MIN=1`.
pub(crate) fn review_size_hint() -> Option<(f32, f32)> {
    std::env::var("LUMILIO_REVIEW_SIZE")
        .ok()
        .and_then(|value| review_size(&value))
        .or_else(|| {
            (std::env::var("LUMILIO_REVIEW_MIN").as_deref() == Ok("1")).then_some((720., 480.))
        })
}

fn review_size(value: &str) -> Option<(f32, f32)> {
    let (width, height) = value.trim().split_once(['x', 'X'])?;
    let width: f32 = width.trim().parse().ok()?;
    let height: f32 = height.trim().parse().ok()?;
    (width >= 720. && height >= 480. && width.is_finite() && height.is_finite())
        .then_some((width, height))
}

#[cfg(test)]
mod tests;
