//! A picture the person chose, behind Home instead of the procedural world
//! (ADR 0044).
//!
//! The picture is drawn as it is. Two soft shades keep the copy readable, and
//! the bottom edge dissolves into the page with the same ordered dither as the
//! world does (`PixelGrid::fade_bottom`), so the picture meets the page
//! without a seam and Home looks the same either way.

use std::path::PathBuf;

use gpui::{
    AnyElement, Bounds, Hsla, ObjectFit, Window, canvas, div, fill, img, linear_color_stop,
    linear_gradient, point, prelude::*, px,
};

use super::noise::{bayer4, smoothstep};
use super::{MAX_COLUMNS, MIN_COLUMNS, TARGET_TEXEL};
use crate::theme;

/// How dark the shades behind the copy get, at their darkest.
const SIDE_SHADE: f32 = 0.55;
const BOTTOM_SHADE: f32 = 0.45;

/// The Home hero showing `path`, dissolving into `page` at the bottom.
pub(super) fn view(path: PathBuf, page: Hsla) -> AnyElement {
    let black = |alpha: f32| gpui::black().opacity(alpha);
    div()
        .id("home-hero")
        .relative()
        .size_full()
        .overflow_hidden()
        .child(img(path).size_full().object_fit(ObjectFit::Cover))
        // Left to right, behind the copy.
        .child(div().absolute().inset_0().bg(linear_gradient(
            90.,
            linear_color_stop(black(SIDE_SHADE), 0.),
            linear_color_stop(black(0.), 0.62),
        )))
        // Top to bottom, under the dissolve.
        .child(div().absolute().inset_0().bg(linear_gradient(
            180.,
            linear_color_stop(black(0.), 0.55),
            linear_color_stop(black(BOTTOM_SHADE), 1.),
        )))
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .w_full()
                .h(theme::HERO_FADE)
                .child(dissolve(page)),
        )
        .into_any_element()
}

/// The band at the bottom: each row is pulled toward the page, and more of
/// its texels are the page itself the lower it is; the last row is the page.
fn dissolve(page: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| paint_dissolve(bounds, page, window),
    )
    .size_full()
}

/// How far row `band` of `rows` is pulled toward the page before the
/// texels that are the page itself go on top.
pub(super) fn veil(band: usize, rows: usize) -> f32 {
    (band + 1) as f32 / rows as f32 * 0.65
}

/// Whether this texel of the band is the page itself. The last row always is,
/// so the picture ends exactly in the page colour.
pub(super) fn is_page(x: usize, band: usize, rows: usize) -> bool {
    let t = (band + 1) as f32 / rows as f32;
    bayer4(x as i32, band as i32) < smoothstep(0., 1., t)
}

fn paint_dissolve(bounds: Bounds<gpui::Pixels>, page: Hsla, window: &mut Window) {
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if width < 1. || height < 1. {
        return;
    }
    let columns = (width / TARGET_TEXEL)
        .round()
        .clamp(MIN_COLUMNS, MAX_COLUMNS);
    let texel = width / columns;
    let rows = (height / texel).round().max(1.);
    let row_height = height / rows;
    window.paint_layer(bounds, |window| {
        for band in 0..rows as usize {
            let top = bounds.origin.y + px(band as f32 * row_height);
            let bottom = bounds.origin.y + px((band + 1) as f32 * row_height);
            window.paint_quad(fill(
                Bounds::from_corners(
                    point(bounds.origin.x, top),
                    point(bounds.origin.x + bounds.size.width, bottom),
                ),
                page.opacity(veil(band, rows as usize)),
            ));
            for x in 0..columns as usize {
                if is_page(x, band, rows as usize) {
                    let left = bounds.origin.x + px(x as f32 * texel);
                    let right = bounds.origin.x + px((x + 1) as f32 * texel);
                    window.paint_quad(fill(
                        Bounds::from_corners(point(left, top), point(right, bottom)),
                        page,
                    ));
                }
            }
        }
    });
}
