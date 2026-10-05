use super::{ModelRequest, page_url, serve};
use gpui::prelude::*;
use gpui::{
    App, Context, IntoElement, Render, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, px, size,
};
use gpui_component::Root;
use wry::WebViewBuilder;
use wry::http::{Response, header};

/// Windows serves a custom protocol as `http://<name>.localhost`.
#[cfg(target_os = "windows")]
const ORIGIN: &str = "http://lumilio.localhost";
#[cfg(not(target_os = "windows"))]
const ORIGIN: &str = "lumilio://localhost";

/// Shown in place of the viewer when the webview could not be created.
struct Failed(SharedString);

impl Render for Failed {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .child(self.0.clone())
    }
}

pub(super) fn open(request: ModelRequest, cx: &mut App) -> Result<(), String> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1000.), px(720.)), cx)),
        window_min_size: Some(size(px(480.), px(360.))),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from(format!("{} · 3D 预览", request.title))),
            ..TitlebarOptions::default()
        }),
        ..WindowOptions::default()
    };
    cx.open_window(options, move |window, cx| {
        let url = page_url(ORIGIN, &request);
        let built = WebViewBuilder::new()
            .with_custom_protocol("lumilio".into(), move |_, http| {
                let served = serve(http.uri().path(), &request);
                Response::builder()
                    .status(served.status)
                    .header(header::CONTENT_TYPE, served.content_type)
                    .header("Cross-Origin-Opener-Policy", "same-origin")
                    .header("Cross-Origin-Embedder-Policy", "require-corp")
                    .header("Cross-Origin-Resource-Policy", "same-origin")
                    .body(std::borrow::Cow::Owned(served.body))
                    .expect("a response with valid headers")
            })
            .with_url(url)
            .build_as_child(window);
        match built {
            Ok(webview) => {
                let view = cx.new(|cx| gpui_wry::WebView::new(webview, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            }
            Err(error) => {
                let message = format!("没能打开 3D 预览：{error}");
                let view = cx.new(|_| Failed(message.into()));
                cx.new(|cx| Root::new(view, window, cx))
            }
        }
    })
    .map(|_| ())
    .map_err(|error| error.to_string())
}
