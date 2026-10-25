use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Context, IntoElement, Render, Window, div};
use gpui_component::{TITLE_BAR_HEIGHT, TitleBar, v_flex};

use super::{LauncherShell, ShellIntent};
use crate::features::discover::detail::ProjectDetailView;
use crate::home::{HomeIntent, ShellHomeColors};
use crate::kit::{Emit, ViewIntent};
use crate::live::LiveIntent;
use crate::navigation::{Leading, ReadyUpdate};
use crate::route::Route;
use crate::theme::ShellColors;
use crate::{home, navigation, pages, placeholders, theme, toast};

impl Render for LauncherShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        toast::flush_posted(window, cx);
        let colors = ShellColors::current(cx);
        let home_colors = ShellHomeColors {
            foreground: colors.foreground,
            muted: colors.muted,
            border: colors.border,
            surface: colors.surface_subtle.opacity(0.5),
            primary: colors.primary,
            primary_foreground: colors.primary_foreground,
            focus: colors.focus,
            danger: colors.danger,
        };
        let home_handler = self.intent_handler.as_ref().map(|_| {
            let callback = cx.listener(|this, intent: &HomeIntent, window, cx| {
                this.emit(ShellIntent::Home(*intent), window, cx);
            });
            Rc::new(
                move |intent: HomeIntent, window: &mut Window, cx: &mut App| {
                    callback(&intent, window, cx)
                },
            ) as home::HomeIntentHandler
        });
        let route_callback = cx.listener(|this, route: &Route, window, cx| {
            this.select_route(*route, window, cx);
        });

        let view_callback = cx.listener(|this, intent: &ViewIntent, _, cx| {
            this.apply_view_intent(*intent, cx);
        });
        let emit_view: Emit = Rc::new(move |intent, window: &mut Window, app: &mut App| {
            view_callback(&intent, window, app)
        });

        let leading = {
            let back = cx.listener(|shell, _: &(), window, cx| {
                shell.go_back(Some(window), cx);
            });
            let forward = cx.listener(|shell, _: &(), window, cx| {
                shell.go_forward(Some(window), cx);
            });
            Leading {
                title: self.location_title(cx),
                can_back: self.can_go_back(),
                can_forward: self.can_go_forward(),
                on_back: Rc::new(move |window, cx| back(&(), window, cx)),
                on_forward: Rc::new(move |window, cx| forward(&(), window, cx)),
                update: self
                    .live
                    .as_ref()
                    .and_then(|model| model.update_status.ready_version())
                    .and_then(|version| {
                        let handler = self.live_handler.clone()?;
                        Some(ReadyUpdate {
                            version: version.to_owned().into(),
                            game_running: self.game_active(),
                            on_apply: Rc::new(move |window, cx| {
                                handler(LiveIntent::ApplyUpdate, window, cx)
                            }),
                        })
                    }),
            }
        };
        let current_instance = self.current_instance(cx);
        let current_account = self.current_account(cx);

        if let Some(handler) = &self.live_handler {
            let handler = handler.clone();
            self.library
                .update(cx, |library, _| library.set_handler(handler.clone()));
            self.activity_page
                .update(cx, |activity, _| activity.set_handler(handler.clone()));
            self.settings_page
                .update(cx, |settings, _| settings.set_handler(handler.clone()));
            self.accounts
                .update(cx, |accounts, _| accounts.set_handler(handler));
        }
        let accounts_shown = self.route == Route::Accounts;
        self.accounts
            .update(cx, |accounts, _| accounts.set_shown(accounts_shown));
        // Discover's answers belong to the detail on screen, if any.
        let shown_detail = match self.location() {
            super::locations::Location::Project(slot) => Some(slot.view.entity_id()),
            _ => None,
        };
        self.discover
            .update(cx, |discover, _| discover.set_shown_detail(shown_detail));
        let live_ctx = match (&self.live, &self.live_handler) {
            (Some(model), Some(handler)) => Some(pages::live::LiveCtx {
                colors,
                model,
                state: &self.view,
                emit: emit_view.clone(),
                handler: handler.clone(),
                window_width: f32::from(window.viewport_size().width),
                window_height: f32::from(window.viewport_size().height),
            }),
            _ => None,
        };
        let live_page = live_ctx
            .as_ref()
            .filter(|_| self.route != Route::Home)
            .map(|_| {
                match (self.route, &self.detail) {
                    (Route::Discover, Some(slot)) => slot.view.clone().into_any_element(),
                    (Route::Library, _) => self.library.clone().into_any_element(),
                    // Settings frames itself: its header and tabs stay put.
                    (Route::Settings, _) => self.settings_page.clone().into_any_element(),
                    (Route::Accounts, _) => self.accounts.clone().into_any_element(),
                    (Route::Discover, None) => self.discover.clone().into_any_element(),
                    _ => self.activity_page.clone().into_any_element(),
                }
            });

        let body = if let Some(view) = &self.live_instance {
            view.clone().into_any_element()
        } else if let Some(page) = live_page {
            page
        } else if self.route == Route::Home {
            let bare = cx
                .try_global::<theme::Wallpaper>()
                .is_some_and(|wallpaper| wallpaper.bare);
            self.render_home(home_colors, home_handler, live_ctx.as_ref(), bare, window)
        } else {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .pt(TITLE_BAR_HEIGHT)
                .pb(theme::BOTTOM_SAFE_AREA)
                .px_8()
                .child(placeholders::render(self.route, colors))
                .into_any_element()
        };

        let shell = v_flex()
            .id("lumilio-shell")
            .relative()
            .size_full()
            .key_context("lumilio-shell")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_shortcut))
            // The framework's root paints the page under the shell; painting it
            // again would stack a translucent page into an opaque one.
            .text_color(colors.foreground)
            .child(body)
            // The title bar is transparent and floats over everything, so on
            // Home the world reaches the top edge under the traffic lights.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(TitleBar::new().bg(gpui::transparent_black()).border_b_0()),
            )
            // A veil in the page colour under the capsule: content scrolling
            // behind it fades out instead of colliding with the icons. On a
            // translucent page it would only stack into a dark band, and the
            // capsule is solid there anyway: content passes behind the glass.
            .when(colors.background.a >= 1., |shell| {
                shell.child(veil(colors.background))
            })
            .child(navigation::render(
                self.route,
                self.activity.active_tasks,
                colors,
                Rc::new(route_callback),
                leading,
                navigation::Trailing {
                    account: current_account,
                    instance: current_instance,
                },
                f32::from(window.viewport_size().width),
            ))
            // A gallery image shown large covers everything, the navigation too.
            .children(
                self.detail
                    .as_ref()
                    .and_then(|slot| ProjectDetailView::viewer_overlay(&slot.view, cx)),
            );
        // Every picture loaded by path or address in the window stays within
        // the picture budget.
        crate::pictures::scope(cx).size_full().child(shell)
    }
}

/// The veil under the navigation: it eases in, starting slowly over its full
/// height and closing over the lower half, so it has no edge where it begins
/// and no band where it ends. One straight ramp shows both on a translucent
/// page. Together the two ramps reach 94 % of the page colour at the bottom.
fn veil(page: gpui::Hsla) -> impl IntoElement {
    let ramp = |to: f32| {
        gpui::linear_gradient(
            180.,
            gpui::linear_color_stop(page.opacity(0.), 0.),
            gpui::linear_color_stop(page.opacity(to), 1.),
        )
    };
    div()
        .debug_selector(|| "nav-veil".into())
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(theme::NAV_VEIL)
        .bg(ramp(0.5))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(theme::NAV_VEIL / 2.)
                .bg(ramp(0.88)),
        )
}
