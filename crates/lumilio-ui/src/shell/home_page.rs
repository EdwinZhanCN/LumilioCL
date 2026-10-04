use super::{LauncherShell, ShellIntent};
use crate::home::{HomeIntent, HomePresentation, ShellHomeColors};
use crate::live::LiveIntent;
use crate::route::Route;
use crate::{home, theme};
use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div, px};
use gpui_component::v_flex;
use lumilio_core::LaunchSignal;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How often the play-time readout refreshes while the game runs.
pub(super) const PLAY_CLOCK_INTERVAL: Duration = Duration::from_secs(20);

impl LauncherShell {
    /// Goes to Home and presses Continue, as if the person had.
    pub fn request_continue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.route = Route::Home;
        self.live_instance = None;
        self.emit(ShellIntent::Home(HomeIntent::Continue), window, cx);
        cx.notify();
    }

    pub fn home(&self) -> &HomePresentation {
        &self.home
    }

    /// Replaces the Home state and points the hero at it.
    pub fn set_home(&mut self, home: HomePresentation, cx: &mut Context<Self>) {
        let playing = matches!(home, HomePresentation::Playing { .. });
        let mode = home.hero_mode();
        self.home = home;
        self.hero.update(cx, |hero, cx| hero.set_mode(mode, cx));
        match (playing, self.play_clock.is_some()) {
            (true, false) => {
                self.play_clock = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(PLAY_CLOCK_INTERVAL).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }));
            }
            (false, true) => self.play_clock = None,
            _ => {}
        }
        cx.notify();
    }

    /// Feeds one launch signal into the Home launch moment.
    pub fn apply_launch_signal(&mut self, signal: LaunchSignal, cx: &mut Context<Self>) {
        let home = std::mem::take(&mut self.home).on_launch_signal(signal, Instant::now());
        self.set_home(home, cx);
    }

    /// What the page below Home's world can open or do: the games with
    /// something wrong, and the recent cards.
    pub(super) fn home_links(&self) -> home::HomeLinks {
        let Some(handler) = self.live_handler.clone() else {
            return home::HomeLinks::default();
        };
        let open = handler.clone();
        let act = handler;
        home::HomeLinks {
            attention: self
                .live
                .as_ref()
                .map(|model| model.attention.clone())
                .unwrap_or_default(),
            on_open: Some(Rc::new(move |id, window, cx| {
                open(LiveIntent::OpenInstance(id), window, cx)
            })),
            on_act: Some(Rc::new(move |id, action, window, cx| {
                act(LiveIntent::Resolve(id, action), window, cx)
            })),
        }
    }

    /// Home: the world runs full-bleed from the top of the window and
    /// dissolves into the page; everything else sits on the content column.
    pub(super) fn render_home(
        &self,
        colors: ShellHomeColors,
        home_handler: Option<home::HomeIntentHandler>,
        window: &Window,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let hero = self.hero.clone();
        let on_continue_hover: home::HoverHandler = Rc::new(move |hovered, _, cx| {
            hero.update(cx, |hero, cx| hero.set_flare(hovered, cx));
        });
        let overlay =
            home::render_overlay(&self.home, home_handler.clone(), Some(on_continue_hover));
        // The world is as tall as its share of the window, so it is sized in
        // pixels: the whole page scrolls and the world scrolls away with it.
        let world_height = px(
            (f32::from(window.viewport_size().height) * theme::HERO_SHARE)
                .max(f32::from(theme::HERO_MIN_HEIGHT)),
        );
        let links = self.home_links();
        let has_attention = !links.attention.is_empty() && self.home.recent_is_instances();
        let body = (self.home.has_body() || has_attention)
            .then(|| home::render_body(&self.home, home_handler, &links, colors));

        div()
            .id("home-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(world_height)
                            .flex_none()
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .size_full()
                                    .child(self.hero.clone()),
                            )
                            .child(
                                div().absolute().inset_0().flex().justify_center().child(
                                    theme::content_column()
                                        .debug_selector(|| "home-overlay-column".into())
                                        .h_full()
                                        .justify_end()
                                        .pb(theme::HERO_FADE)
                                        .children(overlay.map(|overlay| {
                                            div()
                                                .debug_selector(|| "home-overlay".into())
                                                .child(overlay)
                                        })),
                                ),
                            ),
                    )
                    .children(body.map(|body| {
                        theme::content_column()
                            .debug_selector(|| "home-body".into())
                            .pt(px(4.))
                            .pb(theme::BOTTOM_SAFE_AREA)
                            .child(body)
                    })),
            )
            .into_any_element()
    }
}
