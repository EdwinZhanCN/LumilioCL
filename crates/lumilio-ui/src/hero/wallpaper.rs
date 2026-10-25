//! A picture the person chose, behind Home instead of the procedural world
//!.
//!
//! The picture is drawn as it is. Two soft shades keep the copy readable, and
//! the bottom edge dissolves into the page with the same ordered dither as the
//! world does (`PixelGrid::fade_bottom`), so the picture meets the page
//! without a seam and Home looks the same either way. On a translucent page
//! the picture fades into the glass instead.

use std::path::{Path, PathBuf};

use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, Entity, Hsla, ObjectFit, Window, canvas, div, fill, img, linear_color_stop,
    linear_gradient, point, px,
};

use super::noise::{bayer4, smoothstep};
use super::{MAX_COLUMNS, MIN_COLUMNS, TARGET_TEXEL};
use crate::pictures::PictureCache;
use crate::theme;

/// How dark the shades behind the copy get, at their darkest.
const SIDE_SHADE: f32 = 0.55;
const BOTTOM_SHADE: f32 = 0.45;

/// Strips the bottom band is cut into on a translucent page: 2 px each, fine
/// enough that the steps of opacity do not show.
const FADE_STRIPS: usize = 44;

/// The Home hero showing `path`, `height` tall on a display of this `scale`,
/// dissolving into `page` at the bottom. The picture comes through `pictures`, which keeps only the one on
/// screen.
pub(super) fn view(
    path: PathBuf,
    page: Hsla,
    height: gpui::Pixels,
    scale: f32,
    pictures: &Entity<PictureCache>,
) -> AnyElement {
    let hero = div()
        .id("home-hero")
        .relative()
        .size_full()
        .overflow_hidden();
    if page.a >= 1. {
        return hero
            .child(picture(&path, pictures))
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(theme::HERO_FADE)
                    .child(dissolve(page)),
            )
            .into_any_element();
    }
    // A translucent page cannot take the dissolve: its texels would let the
    // picture through. The picture fades out itself instead, into the glass
    // behind it. A picture has no alpha mask, so the band is layers of the
    // whole picture, each clipped from the top of the band down to a lower
    // edge: nested, they leave no seam where clip edges round apart.
    // Clip edges on whole device pixels: a fractional edge is antialiased into a
    // half-covered row, a light line at every layer.
    let above = px((f32::from(height - theme::HERO_FADE) * scale).round() / scale);
    let strip = theme::HERO_FADE / FADE_STRIPS as f32;
    let band = |top: gpui::Pixels, tall: gpui::Pixels| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(top)
            .h(tall)
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(-top)
                    .h(height)
                    .child(picture(&path, pictures)),
            )
    };
    // A pixel of overlap: the band's first layer starts under the picture.
    hero.child(band(px(0.), above + px(1.)))
        .children(
            fade_layers(FADE_STRIPS)
                .into_iter()
                .enumerate()
                .map(|(index, alpha)| band(above, strip * (index + 1) as f32).opacity(alpha)),
        )
        .into_any_element()
}

/// The opacity of each nested layer of the band, top to bottom: layer `k`
/// covers strips `0..=k`, so strip `j` is under layers `j..`. Stacked, they
/// give each strip the eased fade `1 - smoothstep` at its middle.
pub(super) fn fade_layers(strips: usize) -> Vec<f32> {
    let wanted = |j: usize| 1. - smoothstep(0., 1., (j as f32 + 0.5) / strips as f32);
    // How much of what is behind still shows through strip `j`.
    let through = |j: usize| if j == strips { 1. } else { 1. - wanted(j) };
    (0..strips)
        .map(|j| 1. - through(j) / through(j + 1))
        .collect()
}

/// The picture with the two shades that keep the copy readable.
fn picture(path: &Path, pictures: &Entity<PictureCache>) -> gpui::Div {
    let black = |alpha: f32| gpui::black().opacity(alpha);
    div()
        .relative()
        .size_full()
        .child(
            img(path.to_path_buf())
                .image_cache(pictures)
                .size_full()
                .object_fit(ObjectFit::Cover),
        )
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
