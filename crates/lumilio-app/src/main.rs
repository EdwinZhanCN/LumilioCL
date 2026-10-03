mod assets;
mod backend;
mod images;
mod live;

use gpui_kit::{App, WindowBounds, WindowOptions, px, size};

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
    let mut app = gpui_kit::application().with_assets(assets::AppAssets);
    match images::ImageClient::new() {
        Ok(client) => app = app.with_http_client(client),
        Err(reason) => eprintln!("pictures from the web will not load: {reason}"),
    }
    app.run(move |cx: &mut App| {
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
                "退出 LumilioCL",
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
        let minimum = std::env::var("LUMILIO_REVIEW_MIN").as_deref() == Ok("1");
        let review_size = if minimum {
            size(px(720.), px(480.))
        } else {
            size(px(1080.), px(720.))
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(review_size, cx)),
            window_min_size: Some(size(px(720.), px(480.))),
            titlebar: Some(lumilio_ui::window_titlebar()),
            app_id: Some("dev.lumilio.launcher".to_owned()),
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
