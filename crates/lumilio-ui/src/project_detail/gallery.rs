//! The Gallery tab and its image viewer. After `Gallery.vue` and the
//! `ImageViewerEditor` it opens (GPL-3.0-only; ADR 0022).

use super::{ProjectDetailView, date_label};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit;
use crate::theme::ShellColors;
use crate::tr;
use gpui::prelude::*;
use gpui::{Context, ObjectFit, div, img, px};
use gpui_component::{Icon, StyledExt as _, h_flex, v_flex};
use lumilio_core::Project;

impl ProjectDetailView {
    // ia[discover]: 画廊 | 详情页「画廊」页签（有图才出现）：卡片带标题、说明、日期 | 点一张在页内放大，可前后翻、在浏览器打开、关闭
    pub(super) fn gallery(
        &self,
        project: &Project,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if project.gallery.is_empty() {
            return kit::empty(tr!("project-gallery-empty"), "", colors).into_any_element();
        }
        let view = cx.entity();
        h_flex()
            .w_full()
            .flex_wrap()
            .gap_4()
            .debug_selector(|| "detail-gallery".into())
            .children(project.gallery.iter().enumerate().map(|(index, image)| {
                let view = view.clone();
                let created = date_label(&image.created);
                v_flex()
                    .id(("detail-gallery", index))
                    .w(px(300.))
                    .gap_2()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.viewer = Some(index);
                            cx.notify();
                        })
                    })
                    .child(
                        div()
                            .w_full()
                            .h(px(150.))
                            .rounded(px(12.))
                            .overflow_hidden()
                            .bg(colors.surface_subtle)
                            .child(
                                img(image.url.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                    )
                    .children((!image.title.is_empty()).then(|| {
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(image.title.clone())
                    }))
                    .children((!image.description.is_empty()).then(|| {
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .line_clamp(3)
                            .child(image.description.clone())
                    }))
                    .children(
                        (!created.is_empty())
                            .then(|| div().text_xs().text_color(colors.muted).child(created)),
                    )
            }))
            .into_any_element()
    }

    /// The large view of one gallery image. The shell draws it as its last
    /// child, so it covers the navigation and the title bar too.
    pub fn viewer_overlay(view: &gpui::Entity<Self>, cx: &gpui::App) -> Option<gpui::AnyElement> {
        let this = view.read(cx);
        let at = this.viewer?;
        let super::DetailState::Ready(detail) = &this.state else {
            return None;
        };
        let gallery = &detail.project.gallery;
        let image = gallery.get(at)?;
        let total = gallery.len();
        let step_view = view.clone();
        let step = move |delta: isize| {
            let view = step_view.clone();
            move |_: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
                cx.stop_propagation();
                view.update(cx, |view, cx| {
                    let next = (at as isize + delta).rem_euclid(total as isize) as usize;
                    view.viewer = Some(next);
                    cx.notify();
                })
            }
        };
        let close_with = {
            let view = view.clone();
            move || {
                let view = view.clone();
                move |_: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
                    view.update(cx, |view, cx| {
                        view.viewer = None;
                        cx.notify();
                    })
                }
            }
        };
        let url = image.full_url.clone();
        Some(
            v_flex()
                .id("detail-viewer")
                .debug_selector(|| "detail-viewer".into())
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .bg(gpui::black().opacity(0.86))
                .items_center()
                .justify_center()
                .gap_3()
                .px_6()
                .pt(gpui_component::TITLE_BAR_HEIGHT + px(8.))
                .pb_6()
                .on_click(close_with())
                .child(
                    div().flex_1().min_h_0().w_full().child(
                        img(image.full_url.clone())
                            .size_full()
                            .object_fit(ObjectFit::Contain),
                    ),
                )
                .child(
                    v_flex()
                        .items_center()
                        .gap_1()
                        .children((!image.title.is_empty()).then(|| {
                            div()
                                .text_color(gpui::white())
                                .font_semibold()
                                .child(image.title.clone())
                        }))
                        .children((!image.description.is_empty()).then(|| {
                            div()
                                .text_sm()
                                .text_color(gpui::white().opacity(0.75))
                                .child(image.description.clone())
                        })),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Key::new("detail-viewer-prev")
                                .icon(Icon::new(UiIcon::Back))
                                .white()
                                .on_click(step(-1)),
                        )
                        .child(
                            div()
                                .font_family(crate::theme::MONO_FONT)
                                .text_xs()
                                .text_color(gpui::white().opacity(0.75))
                                .child(format!("{} / {}", at + 1, total)),
                        )
                        .child(
                            Key::new("detail-viewer-next")
                                .icon(Icon::new(UiIcon::Next))
                                .white()
                                .on_click(step(1)),
                        )
                        .child(
                            Key::new("detail-viewer-open")
                                .icon(Icon::new(UiIcon::External))
                                .tooltip(tr!("project-open-in-browser"))
                                .white()
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    cx.open_url(&url);
                                }),
                        )
                        .child(
                            Key::new("detail-viewer-close")
                                .icon(Icon::new(UiIcon::Close))
                                .tooltip(tr!("common-close"))
                                .white()
                                .debug_selector(|| "detail-viewer-close".into())
                                .on_click(close_with()),
                        ),
                )
                .into_any_element(),
        )
    }
}
