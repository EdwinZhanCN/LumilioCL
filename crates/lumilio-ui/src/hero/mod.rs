//! Home hero: procedural pixel-art scenes that reflect Home's state
//! (ADR 0004, design language §5).
//!
//! Each frame the current scene is rasterised on the CPU into a small texel
//! grid (pure Rust, see [`raster`]), composited with an outgoing scene or the
//! unloaded void during chunk loading, and painted with one quad per rectangle
//! of identical texels. [`director`] decides what is shown; showcase copy, the
//! live HUD chip, and slide indicators are ordinary GPUI elements on top.

mod director;
pub(crate) mod noise;
pub(crate) mod raster;
mod scenes;
mod timeline;
mod transition;
mod wallpaper;

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt as _, Bounds, ClickEvent, Context, Hsla, IntoElement, Pixels, Render,
    Rgba, Task, Window, canvas, div, ease_out_quint, fill, point, prelude::*, px, relative,
};
use gpui_component::{ActiveTheme as _, StyledExt as _, TITLE_BAR_HEIGHT, h_flex, v_flex};

use crate::theme::{self, motion};
use director::Director;
pub use director::HeroMode;
use raster::{PixelGrid, Rgb};
pub use scenes::{Landmark, Scene};

/// Preferred on-screen texel size in logical pixels.
const TARGET_TEXEL: f32 = 7.;
/// Bounds on the grid width so CPU rasterisation stays cheap on large windows.
const MIN_COLUMNS: f32 = 32.;
const MAX_COLUMNS: f32 = 190.;
/// Once rendering stops (Home left the screen) the ticker stops notifying.
const VISIBLE_GRACE: Duration = Duration::from_millis(250);
const SCRIM_STRENGTH: f32 = 0.62;

pub struct HeroCarousel {
    director: Director,
    last_tick: Option<Instant>,
    rendered_at: Option<Instant>,
    /// Scene grid size from the most recent paint, for the HUD readout.
    grid_size: Rc<Cell<(i32, i32)>>,
    _ticker: Task<()>,
}

impl HeroCarousel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                // Scenes advance at the game's own 20 ticks per second.
                cx.background_executor().timer(motion::WORLD_TICK).await;
                let alive = this.update(cx, |this, cx| {
                    let visible = this
                        .rendered_at
                        .is_some_and(|at| at.elapsed() < VISIBLE_GRACE);
                    if visible && !cx.reduce_motion() && this.director.animating() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });

        Self {
            director: Director::new(),
            last_tick: None,
            rendered_at: None,
            grid_size: Rc::new(Cell::new((170, 72))),
            _ticker: ticker,
        }
    }

    /// Points the hero at a Home state. Cheap to call with an unchanged mode.
    pub fn set_mode(&mut self, mode: HeroMode, cx: &mut Context<Self>) {
        if self.director.set_mode(mode, cx.reduce_motion()) {
            cx.notify();
        }
    }

    /// The pointer rests on (or leaves) Continue; a focused scene answers.
    pub fn set_flare(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.director.set_flare(on, cx.reduce_motion()) {
            cx.notify();
        }
    }

    fn select(&mut self, slide: usize, cx: &mut Context<Self>) {
        self.director.select(slide, cx.reduce_motion());
        cx.notify();
    }
}

/// Everything the paint closure needs, captured by value.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameSpec {
    scene: Scene,
    age: f32,
    /// Outgoing scene, its clock, and transition progress.
    leaving: Option<(Scene, f32, f32)>,
    /// Share of the world loaded; below 1 the rest is the unloaded void.
    reveal: f32,
    /// Darken for a frame that sits behind foreground content.
    dim: bool,
    /// Pointer resting on Continue, eased, in `[0, 1]`.
    flare: f32,
}

/// How the world meets the page: the band that dissolves, and into what.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Framing {
    fade_rows: usize,
    page: Rgb,
}

/// Grid columns/rows and the exact texel size for a canvas of this size.
fn grid_layout(width: f32, height: f32) -> (usize, usize, f32, f32) {
    let width = width.max(1.);
    let height = height.max(1.);
    let columns = (width / TARGET_TEXEL)
        .round()
        .clamp(MIN_COLUMNS, MAX_COLUMNS);
    let texel = width / columns;
    let rows = (height / texel).round().max(12.);
    (
        columns as usize,
        rows as usize,
        width / columns,
        height / rows,
    )
}

/// Rows the scene composes for: half the fade band hangs below it as the
/// scene's last row extended, so no scene content moves into the dissolve.
fn scene_rows(rows: usize, framing: Framing) -> usize {
    rows.saturating_sub(framing.fade_rows / 2).max(8).min(rows)
}

fn rasterize(frame: FrameSpec, columns: usize, rows: usize, framing: Framing) -> PixelGrid {
    let scene_rows = scene_rows(rows, framing);
    let mut grid = PixelGrid::new(columns, scene_rows);
    frame.scene.render(&mut grid, frame.age, frame.flare);
    if let Some((scene, age, progress)) = frame.leaving {
        let mut outgoing = PixelGrid::new(columns, scene_rows);
        scene.render(&mut outgoing, age, 0.);
        transition::composite(&outgoing, &mut grid, progress);
    }
    if frame.reveal < 1. {
        let void = transition::void(columns, scene_rows);
        transition::composite(&void, &mut grid, frame.reveal);
    }
    if frame.dim {
        grid.dim(director::DIM);
    }
    grid.apply_scrim(SCRIM_STRENGTH);
    grid.extend_to(rows);
    grid.fade_bottom(framing.fade_rows, framing.page);
    grid
}

pub(crate) fn paint_grid(
    bounds: Bounds<Pixels>,
    grid: &PixelGrid,
    texel: (f32, f32),
    window: &mut Window,
) {
    let origin = bounds.origin;
    let (tw, th) = texel;
    // The rects never overlap, so they share one draw order in a single layer
    // instead of each being placed in the scene's bounds tree.
    window.paint_layer(bounds, |window| {
        for rect in grid.rects() {
            let left = origin.x + px(rect.start as f32 * tw);
            let right = origin.x + px((rect.start + rect.len) as f32 * tw);
            let top = origin.y + px(rect.row as f32 * th);
            let bottom = origin.y + px((rect.row + rect.rows) as f32 * th);
            let color = Rgba {
                r: ((rect.color >> 16) & 0xff) as f32 / 255.,
                g: ((rect.color >> 8) & 0xff) as f32 / 255.,
                b: (rect.color & 0xff) as f32 / 255.,
                a: 1.,
            };
            window.paint_quad(fill(
                Bounds::from_corners(point(left, top), point(right, bottom)),
                color,
            ));
        }
    });
}

pub fn accent(scene: Scene) -> Hsla {
    gpui::rgb(match scene {
        Scene::Dawn => 0xffc35c,
        Scene::Caves => 0xffb03a,
        Scene::Redstone => 0xff3e1e,
        Scene::Portal => 0x9a55f5,
        Scene::Hearth(_) => 0xff9a3a,
    })
    .into()
}

impl Render for HeroCarousel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A chosen picture replaces the world, scenes and their copy with it.
        if let Some(path) = cx
            .try_global::<theme::Wallpaper>()
            .and_then(|wallpaper| wallpaper.path.clone())
        {
            return wallpaper::view(path, cx.theme().background).into_any_element();
        }
        let reduce_motion = cx.reduce_motion();
        let now = Instant::now();
        let step = self
            .last_tick
            .map_or(0., |last| now.duration_since(last).as_secs_f32());
        self.director.tick(step, reduce_motion);
        self.last_tick = Some(now);
        self.rendered_at = Some(now);

        let frame = self.director.frame(reduce_motion);
        let scene = frame.scene;
        let showcase = self.director.mode() == HeroMode::Showcase;
        let current = self.director.showcase().current();
        let grid_size = self.grid_size.clone();
        let hud = self.director.hud(grid_size.get());
        let dwell = self.director.showcase().dwell_progress();
        let white = gpui::white();
        let mono = cx.theme().mono_font_family.clone();
        let page = cx.theme().background.to_rgb();
        let page = Rgb::new(page.r, page.g, page.b);

        let art = canvas(
            move |bounds, _, _| {
                let (columns, rows, tw, th) =
                    grid_layout(f32::from(bounds.size.width), f32::from(bounds.size.height));
                let fade_rows = ((f32::from(theme::HERO_FADE) / th).round() as usize).min(rows / 3);
                let framing = Framing { fade_rows, page };
                grid_size.set((columns as i32, scene_rows(rows, framing) as i32));
                (rasterize(frame, columns, rows, framing), (tw, th))
            },
            move |bounds, (grid, texel), window, _| paint_grid(bounds, &grid, texel, window),
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let copy = v_flex()
            .max_w(px(470.))
            .gap(px(6.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(div().size(px(8.)).bg(accent(scene)))
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(white.opacity(0.78))
                            .child(scene.eyebrow()),
                    ),
            )
            .child(
                div()
                    .text_size(crate::theme::font_px(28.))
                    .line_height(px(36.))
                    .font_semibold()
                    .text_color(white)
                    .child(scene.title()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(white.opacity(0.74))
                    .child(scene.caption()),
            )
            .with_animation(
                ("hero-copy", current),
                Animation::new(motion::SCENE).with_easing(ease_out_quint()),
                |copy, delta| copy.opacity(delta).mt(motion::LIFT * (1. - delta)),
            );

        // A quiet readout: it is an easter egg, not information.
        let chip = hud.map(|hud| {
            div()
                .px(px(8.))
                .py(px(3.))
                .rounded(px(3.))
                .bg(gpui::black().opacity(0.28))
                .font_family(mono)
                .text_xs()
                .text_color(white.opacity(0.62))
                .child(hud)
        });

        let indicators = h_flex()
            .gap(px(2.))
            .children((0..Scene::ALL.len()).map(|index| {
                let active = index == current;
                let bar = div()
                    .h(px(6.))
                    .w(px(if active { 30. } else { 10. }))
                    .rounded(px(1.))
                    .overflow_hidden()
                    .bg(white.opacity(if active { 0.28 } else { 0.4 }))
                    .when(active, |bar| {
                        bar.child(
                            div()
                                .h_full()
                                .w(relative(if reduce_motion { 1. } else { dwell }))
                                .bg(white.opacity(0.92)),
                        )
                    });
                div()
                    .id(("hero-slide", index))
                    .px(px(3.))
                    .py(px(9.))
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.85))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select(index, cx);
                    }))
                    .child(bar)
            }));

        // Copy, readout, and indicators sit on the shared content column,
        // below the title bar and above the dissolve.
        let column = theme::content_column()
            .h_full()
            .pt(TITLE_BAR_HEIGHT + px(12.))
            .pb(theme::HERO_FADE)
            .justify_between()
            .child(h_flex().justify_end().children(chip))
            .child(
                h_flex()
                    .items_end()
                    .justify_between()
                    .when(showcase, |row| row.child(copy).child(indicators)),
            );

        div()
            .id("home-hero")
            .relative()
            .size_full()
            .overflow_hidden()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.director.set_paused(*hovered);
                cx.notify();
            }))
            .child(art)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .justify_center()
                    .child(column),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
