use super::locations::Location;
use super::{LauncherShell, ShellIntent};
use crate::home::{HomeIntent, HomePresentation, RecoveryDetail, ShellHomeColors};
use crate::live::LiveIntent;
use crate::route::Route;
use crate::toast::Toast;
use crate::{home, pages, theme};
use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div, px};
use gpui_component::{TITLE_BAR_HEIGHT, v_flex};
use lumilio_core::{LaunchFailure, LaunchSignal};
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
        if matches!(&signal, LaunchSignal::Running) {
            self.release_for_game(cx);
        }
        let was_recovering = matches!(self.home, HomePresentation::Recovery { .. });
        let home = std::mem::take(&mut self.home).on_launch_signal(signal, Instant::now());
        // Home explains a failure in place; anywhere else it would wait
        // unseen until the person went back.
        if let HomePresentation::Recovery {
            subject, detail, ..
        } = &home
            && !was_recovering
            && !matches!(self.location(), Location::Page(Route::Home))
        {
            let text = format!("{} · {}", subject.title, home::recovery_sentence(detail));
            let toast = match detail {
                RecoveryDetail::Failed {
                    failure: LaunchFailure::Step { message, .. },
                    ..
                } => Toast::error(text).technical(message.clone()),
                _ => Toast::error(text),
            };
            self.toast(toast, cx);
        }
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
        // An answer from before the change is not shown while games are
        // checked again; with nothing shown before, there is nothing to replace.
        let checking = self
            .live
            .as_ref()
            .is_some_and(|model| model.attention_checking && !model.attention.is_empty());
        home::HomeLinks {
            attention: self
                .live
                .as_ref()
                .map(|model| {
                    let mut rows = if checking {
                        Vec::new()
                    } else {
                        model.attention.clone()
                    };
                    rows.extend(crate::game_settings::attention_rows(
                        &model.game_settings,
                        &model.library,
                    ));
                    rows
                })
                .unwrap_or_default(),
            checking,
            revision: self
                .live
                .as_ref()
                .map_or(0, |model| model.attention_revision),
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
    /// A `bare` Home has no world: its title sits on the body, as tall as it
    /// needs (plan translucent-themes, D2).
    pub(super) fn render_home(
        &self,
        colors: ShellHomeColors,
        home_handler: Option<home::HomeIntentHandler>,
        live: Option<&pages::live::LiveCtx>,
        bare: bool,
        window: &Window,
    ) -> AnyElement {
        let hero = self.hero.clone();
        let on_continue_hover: home::HoverHandler = Rc::new(move |hovered, _, cx| {
            hero.update(cx, |hero, cx| hero.set_flare(hovered, cx));
        });
        let text = if bare {
            colors.foreground
        } else {
            home::on_art()
        };
        let overlay = home::render_overlay(
            &self.home,
            home_handler.clone(),
            (!bare).then_some(on_continue_hover),
            text,
        )
        .map(|overlay| {
            div()
                .debug_selector(|| "home-overlay".into())
                .child(overlay)
        });
        let top = if bare {
            theme::content_column()
                .debug_selector(|| "home-overlay-column".into())
                .pt(TITLE_BAR_HEIGHT + px(40.))
                .pb(px(32.))
                .children(overlay)
                .into_any_element()
        } else {
            self.render_world(overlay, window)
        };
        let links = self.home_links();
        let has_attention =
            (!links.attention.is_empty() || links.checking) && self.home.subject().is_some();
        let sections =
            live.and_then(|ctx| pages::live::home_sections(ctx, &self.home, self.can_start()));
        let body = (self.home.has_body() || has_attention || sections.is_some())
            .then(|| home::render_body(&self.home, home_handler, &links, colors, sections));

        div()
            .id("home-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .child(top)
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

    /// The world with Home's title over it, dissolving into the page.
    fn render_world(&self, overlay: Option<gpui::Div>, window: &Window) -> AnyElement {
        let world_height = theme::world_height(window);
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
                        .children(overlay),
                ),
            )
            .into_any_element()
    }
}
