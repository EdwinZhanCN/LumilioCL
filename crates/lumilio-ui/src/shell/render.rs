use super::{LauncherShell, ShellIntent};
use crate::home::{HomeIntent, ShellHomeColors};
use crate::kit::{Emit, ViewIntent};
use crate::live::{DiscoverChange, LiveIntent};
use crate::navigation::Leading;
use crate::project_detail::ProjectDetailView;
use crate::route::Route;
use crate::theme::ShellColors;
use crate::{home, kit, navigation, pages, placeholders, theme, toast};
use gpui::prelude::*;
use gpui::{App, Context, IntoElement, Render, Window, div};
use gpui_component::ActiveTheme as _;
use gpui_component::{TITLE_BAR_HEIGHT, TitleBar, v_flex};
use std::rc::Rc;

impl Render for LauncherShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        toast::flush(&mut self.toasts, window, cx);
        let colors = ShellColors::from_theme(cx.theme());
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

        let view_callback = cx.listener(|this, intent: &ViewIntent, window, cx| {
            this.apply_view_intent(*intent, cx);
            this.remember_library_view(*intent, window, cx);
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
            }
        };
        let current_instance = self.current_instance(cx);
        let current_account = self.current_account(cx);

        self.ensure_live_controls(window, cx);
        self.ensure_account_viewer(window, cx);
        self.sync_library_dropdowns(window, cx);
        let change_callback = cx.listener(|this, change: &DiscoverChange, window, cx| {
            this.change_query(change.clone(), window, cx);
        });
        let change_handler: pages::live::DiscoverChangeHandler =
            Rc::new(move |change, window: &mut Window, app: &mut App| {
                change_callback(&change, window, app)
            });
        let live_ctx = match (&self.live, &self.live_controls, &self.live_handler) {
            (Some(model), Some(controls), Some(handler)) => {
                let filter = controls
                    .read(cx)
                    .library_filter
                    .read(cx)
                    .value()
                    .to_string();
                let version_filter = controls
                    .read(cx)
                    .version_search
                    .read(cx)
                    .value()
                    .to_string();
                Some(pages::live::LiveCtx {
                    colors,
                    model,
                    state: &self.view,
                    emit: emit_view.clone(),
                    handler: handler.clone(),
                    change: change_handler.clone(),
                    controls: controls.read(cx),
                    filter,
                    version_filter,
                    account_viewer: self.account_viewer.as_ref().map(|viewer| &viewer.view),
                    wardrobe: self
                        .account_viewer
                        .as_ref()
                        .and_then(|viewer| viewer.wardrobe.as_ref()),
                    offline_skin: self
                        .account_viewer
                        .as_ref()
                        .and_then(|viewer| viewer.offline.as_ref()),
                })
            }
            _ => None,
        };
        let live_page = live_ctx
            .as_ref()
            .filter(|_| self.route != Route::Home)
            .map(|ctx| {
                match (self.route, &self.detail) {
                    (Route::Discover, Some(slot)) => slot.view.clone().into_any_element(),
                    (Route::Library, _) => {
                        let handler = ctx.handler.clone();
                        // ia[library]: 拖入整合包 | 把文件拖到页面上 | 同“导入整合包”（.mrpack，或 MultiMC/Prism/本启动器备份的 .zip）；不是的提示一句
                        div()
                            .id("live-library-drop")
                            .size_full()
                            .on_drop::<gpui::ExternalPaths>(move |paths, window, cx| {
                                handler(LiveIntent::DropFiles(paths.paths().to_vec()), window, cx)
                            })
                            .child(kit::page("live-library", pages::live::library(ctx)))
                            .into_any_element()
                    }
                    // Settings frames itself: its header and tabs stay put.
                    (Route::Settings, _) => pages::settings::render(ctx).into_any_element(),
                    (Route::Accounts, _) => pages::live::accounts(ctx).into_any_element(),
                    (Route::Discover, None) => {
                        kit::page("live-discover", pages::live::discover(ctx)).into_any_element()
                    }
                    _ => kit::page("live-activity", pages::live::activity(ctx)).into_any_element(),
                }
            });

        let body = if let Some(view) = &self.live_instance {
            view.clone().into_any_element()
        } else if let Some(page) = live_page {
            page
        } else if self.route == Route::Home {
            self.render_home(home_colors, home_handler, live_ctx.as_ref(), window)
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

        v_flex()
            .id("lumilio-shell")
            .relative()
            .size_full()
            .key_context("lumilio-shell")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_shortcut))
            .bg(colors.background)
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
            // behind it fades out instead of colliding with the icons.
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(theme::NAV_VEIL)
                    .bg(gpui::linear_gradient(
                        180.,
                        gpui::linear_color_stop(colors.background.opacity(0.), 0.),
                        gpui::linear_color_stop(colors.background.opacity(0.94), 0.62),
                    )),
            )
            .child(navigation::render(
                self.route,
                self.activity.active_tasks,
                colors,
                Rc::new(route_callback),
                leading,
                current_instance,
                current_account,
            ))
            // A gallery image shown large covers everything, the navigation too.
            .children(
                self.detail
                    .as_ref()
                    .and_then(|slot| ProjectDetailView::viewer_overlay(&slot.view, cx)),
            )
    }
}
