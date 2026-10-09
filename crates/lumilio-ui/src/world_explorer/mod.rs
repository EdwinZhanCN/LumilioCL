//! Host-owned map viewport. Background data and rendering enter through mailboxes.
mod camera;
mod canvas;
mod edit;
mod layers;
mod objects;
mod seed;
mod select;
mod stats;
mod toolbar;
mod xaero;
pub use objects::ObjectKey;
pub use seed::VERSIONS as MANUAL_VERSIONS;
#[cfg(test)]
mod perf;
#[cfg(test)]
mod tests;
mod worker;

use crate::{theme::ShellColors, tr};
use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, FocusHandle, MouseButton, ObjectFit, Pixels, Point, RenderImage, Task,
    Window, canvas, div, img, px, relative,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use lumilio_core::world_map::{MapSchedule, Viewport, WorldMapContext};
use lumilio_core::{CancellationToken, MapFailure, MapProviders};
use lumilio_map_render::{Grid, Sprite, Tile};
use lumilio_plugin_api::map::{
    MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest, TileKey, TileReply,
    TileRequest, WorldContext, WorldId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

pub enum Command {
    Contexts,
    SaveSeed {
        seed: i64,
        version: String,
    },
    Tile {
        generation: u64,
        request: Box<TileRequest>,
        cancel: CancellationToken,
    },
    /// The layers the plugins offer for one world.
    Overlays {
        context: WorldContext,
    },
    /// The person confirmed that this save uses this Minimap directory.
    LinkXaero {
        folder: String,
        dir: String,
    },
    /// Apply an edit the person confirmed in the dialog.
    Apply {
        plugin: String,
        edit: Box<lumilio_plugin_api::map::ObjectEdit>,
    },
    /// Ask whether the game's files could be written now.
    CanEdit,
    Objects {
        generation: u64,
        plugin: String,
        request: Box<OverlayRequest>,
        key: ObjectKey,
        cancel: CancellationToken,
    },
}
pub enum Event {
    Seed(Result<(Vec<WorldMapContext>, WorldContext), String>),
    Contexts(Result<(Vec<WorldMapContext>, MapProviders), String>),
    Tile {
        generation: u64,
        key: TileKey,
        result: Result<TileReply, MapFailure>,
    },
    Overlays {
        context: WorldContext,
        layers: Vec<(String, OverlayInfo)>,
    },
    /// The worlds again, after a link changed what they can show.
    Linked(Result<Vec<WorldMapContext>, String>),
    /// The result of an applied edit; the error is a message id.
    Applied(Result<(), String>),
    CanEdit(bool),
    Objects {
        generation: u64,
        key: ObjectKey,
        result: Result<Vec<MapObject>, MapFailure>,
    },
}
pub struct Connection {
    pub send: async_channel::Sender<Command>,
    pub receive: async_channel::Receiver<Event>,
}
pub type Load = Rc<dyn Fn(u64, &mut Window, &mut App)>;

/// Tiles dispatched at once. More only lengthens the queue ahead of whatever
/// the next pan makes important, and cancelled work still holds its worker
/// until the next poll.
const IN_FLIGHT: usize = 12;
/// Decoded tiles kept across zooms (256 KiB each). Coarser or finer neighbours
/// stand in for the visible level while its own tiles are still being made.
const TILE_CAP: usize = 512;
/// Stand-ins drawn under a frame, so one frame stays a bounded number of draws.
const STAND_INS: usize = 192;
/// Structure layers shown before the user chooses any.
const DEFAULT_LAYERS: &[&str] = &[
    "save.positions",
    "structure.village",
    "structure.stronghold",
    "structure.fortress",
    "structure.end-city",
];
/// Icons drawn at once; the most important win when a view holds more.
const MAX_ICONS: usize = 400;
/// Area rectangles drawn at once.
const MAX_FILLS: usize = 6000;
/// Icon edge on screen, in pixels.
const ICON: u32 = 28;

/// A tile's pixels, prepared once on arrival. `rgba` is `None` for a tile the
/// provider answered as empty; it is drawn as the empty-tile pattern.
struct Loaded {
    rgba: Option<Arc<[u8]>>,
    /// Blocks the provider had no colour for in this tile.
    unknown: Vec<String>,
    /// Names the texture on the render thread; a retried tile gets a new one.
    revision: u64,
    used: u64,
}

fn pattern(failed: bool) -> Arc<[u8]> {
    (0..256 * 256)
        .flat_map(|pixel| {
            let shade = if ((pixel % 256) / 16 + (pixel / 256) / 16) % 2 == 0 {
                80
            } else {
                96
            };
            if failed {
                [shade + 32, shade, shade, 255]
            } else {
                [shade, shade, shade, 255]
            }
        })
        .collect()
}

/// RGBA for a reply, and the blocks it had no colour for; areas the provider
/// has no data for show as flat grey.
fn prepare(reply: TileReply) -> (Option<Arc<[u8]>>, Vec<String>) {
    match reply {
        TileReply::Image(image) => (Some(image.rgba.into()), Vec::new()),
        TileReply::Partial {
            image,
            coverage,
            unknown,
        } => (
            Some(
                image
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(coverage)
                    .flat_map(|(pixel, covered)| {
                        if covered > 0 {
                            *pixel
                        } else {
                            [96, 96, 96, 255]
                        }
                    })
                    .collect(),
            ),
            unknown,
        ),
        TileReply::Empty => (None, Vec::new()),
    }
}

pub struct MapView {
    load: Load,
    asked: bool,
    focus: FocusHandle,
    connection: Option<Connection>,
    data_task: Option<Task<()>>,
    worker: Option<worker::Worker>,
    render_task: Option<Task<()>>,
    contexts: Vec<WorldMapContext>,
    providers: MapProviders,
    context: Option<WorldContext>,
    base: usize,
    camera: camera::Camera,
    layers: layers::Layers,
    objects: objects::Objects,
    layer_scroll: gpui::ScrollHandle,
    schedule: MapSchedule,
    tiles: BTreeMap<TileKey, Loaded>,
    clock: u64,
    revision: u64,
    pending_pixels: Arc<[u8]>,
    failed_pixels: Arc<[u8]>,
    visible: Vec<TileKey>,
    /// Shown tiles to ask for again, because the files they were drawn from
    /// may have changed; each keeps showing until its answer replaces it.
    stale: BTreeSet<TileKey>,
    failed: BTreeMap<TileKey, MapFailure>,
    size: [u32; 2],
    bounds: Option<Bounds<Pixels>>,
    drag: Option<Point<Pixels>>,
    /// Where the left button went down, until it comes up: a click if it
    /// barely moved.
    press: Option<Point<Pixels>>,
    selected: Option<MapObject>,
    /// Whether the files could be written now (no game running).
    can_edit: bool,
    edit_dialog: Option<gpui::WeakEntity<edit::EditDialog>>,
    /// A message id for an edit that failed with no dialog open.
    edit_error: Option<String>,
    /// The next click on the map places a new object.
    placing: bool,
    /// An edit's answer waiting for the next render, which has the window.
    applied: Option<Result<(), String>>,
    cursor: [f64; 2],
    serial: u64,
    image: Option<Arc<RenderImage>>,
    old: Vec<Arc<RenderImage>>,
    error: Option<String>,
    no_gpu: bool,
    form: Option<seed::Form>,
    release: bool,
    stats: Arc<stats::Stats>,
    /// When the latest frame was shown, until the view next renders.
    shown_at: Option<Instant>,
    backend: Backend,
    painter: std::rc::Rc<std::cell::RefCell<canvas::Painter>>,
    report: (stats::Snapshot, Instant),
}

/// How the map reaches the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Backend {
    /// Tiles and icons painted by GPUI itself (`canvas.rs`). The default: it
    /// draws at device resolution with no readback, and measured and looked
    /// better than the wgpu path in the maintainer's side-by-side.
    Canvas,
    /// Composed on our own wgpu device and read back as one image. Kept for
    /// comparison, behind `LUMILIO_MAP_BACKEND=wgpu`.
    Wgpu,
}

impl Backend {
    fn from_env() -> Self {
        Self::parse(std::env::var("LUMILIO_MAP_BACKEND").ok().as_deref())
    }
    fn parse(choice: Option<&str>) -> Self {
        match choice {
            Some("wgpu") => Self::Wgpu,
            _ => Self::Canvas,
        }
    }
}
impl MapView {
    pub fn new(load: Load, cx: &mut Context<Self>) -> Self {
        Self {
            load,
            asked: false,
            focus: cx.focus_handle(),
            connection: None,
            data_task: None,
            worker: None,
            render_task: None,
            contexts: vec![],
            providers: MapProviders::default(),
            context: None,
            base: 0,
            camera: camera::Camera::default(),
            layers: layers::Layers::default(),
            objects: objects::Objects::new(DEFAULT_LAYERS),
            layer_scroll: gpui::ScrollHandle::new(),
            schedule: MapSchedule::default(),
            tiles: BTreeMap::new(),
            clock: 0,
            revision: 0,
            pending_pixels: pattern(false),
            failed_pixels: pattern(true),
            visible: Vec::new(),
            stale: BTreeSet::new(),
            failed: BTreeMap::new(),
            size: [0, 0],
            bounds: None,
            drag: None,
            press: None,
            selected: None,
            can_edit: false,
            edit_dialog: None,
            edit_error: None,
            placing: false,
            applied: None,
            cursor: [0., 0.],
            serial: 0,
            image: None,
            old: vec![],
            error: None,
            no_gpu: false,
            form: None,
            release: false,
            stats: Arc::default(),
            shown_at: None,
            backend: Backend::from_env(),
            painter: Default::default(),
            report: (stats::Snapshot::default(), Instant::now()),
        }
    }
    pub fn connect(&mut self, connection: Connection, cx: &mut Context<Self>) {
        self.attach(connection, cx);
        if self.backend == Backend::Wgpu {
            self.start_renderer(cx);
        }
    }
    /// Listens to the application's events and asks for the world list.
    fn attach(&mut self, connection: Connection, cx: &mut Context<Self>) {
        let receive = connection.receive.clone();
        let _ = connection.send.try_send(Command::Contexts);
        self.connection = Some(connection);
        self.data_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receive.recv().await {
                if this.update(cx, |this, cx| this.event(event, cx)).is_err() {
                    break;
                }
            }
        }));
    }
    /// Starts the render thread and shows its frames.
    fn start_renderer(&mut self, cx: &mut Context<Self>) {
        match worker::Worker::start() {
            Ok(worker) => {
                let receive = worker.output.clone();
                let stats = worker.stats.clone();
                let report = std::env::var_os("LUMILIO_MAP_STATS").is_some();
                self.render_task = Some(cx.spawn(async move |this, cx| {
                    let (mut since, mut last) = (stats::Snapshot::default(), Instant::now());
                    while let Ok(result) = receive.recv().await {
                        if report && last.elapsed() >= Duration::from_secs(2) {
                            let now = stats.snapshot();
                            eprintln!("[map-stats] {}", now.since(since).line());
                            (since, last) = (now, Instant::now());
                        }
                        if this
                            .update(cx, |this, cx| {
                                match result {
                                    Ok(done) if done.serial == this.serial => {
                                        let building = Instant::now();
                                        let frame = done.frame;
                                        if let Some(buffer) = image::RgbaImage::from_raw(
                                            frame.width,
                                            frame.height,
                                            frame.bgra,
                                        ) {
                                            let image =
                                                Arc::new(RenderImage::new([image::Frame::new(
                                                    buffer,
                                                )]));
                                            if let Some(old) = this.image.replace(image) {
                                                this.old.push(old);
                                            }
                                            let stats = &this.stats;
                                            stats.displayed.fetch_add(1, Relaxed);
                                            stats::add(&stats.image_us, building.elapsed());
                                            stats::add(&stats.latency_us, done.issued.elapsed());
                                            this.shown_at = Some(Instant::now());
                                        }
                                    }
                                    Ok(_) => {
                                        this.stats.discarded.fetch_add(1, Relaxed);
                                    }
                                    Err(error) => {
                                        this.no_gpu = error == lumilio_map_render::Error::NoGpu;
                                        this.error = Some(format!("{error:?}"));
                                    }
                                }
                                cx.notify();
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }));
                self.stats = worker.stats.clone();
                self.worker = Some(worker);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Seed(Ok((contexts, context))) => {
                if let Some(form) = &self.form
                    && form.submitted.as_ref().is_none_or(|submitted| {
                        context.seed != Some(submitted.0)
                            || context.version.as_ref() != Some(&submitted.1)
                    })
                {
                    return;
                }
                self.contexts = contexts;
                self.context = Some(context);
                self.error = None;
                self.reset_view();
                self.context_changed();
                self.refresh();
            }
            Event::Seed(Err(error)) => {
                self.error = Some(error);
                if let Some(form) = &mut self.form {
                    form.seed_error = Some("map-seed-save-failed");
                    form.submitted = None;
                }
            }
            Event::Contexts(Ok((contexts, providers))) => {
                self.contexts = contexts;
                self.providers = providers;
                self.context = self.contexts.first().map(|world| world.context.clone());
                self.context_changed();
                self.refresh();
            }
            Event::Contexts(Err(error)) => self.error = Some(error),
            Event::Linked(Ok(contexts)) => {
                // Stay on the world being looked at; it now carries the link.
                let shown = self.context.as_ref().map(|context| context.world.clone());
                self.contexts = contexts;
                if let Some(world) = self
                    .contexts
                    .iter()
                    .find(|world| Some(&world.context.world) == shown.as_ref())
                {
                    let dimension = self
                        .context
                        .as_ref()
                        .map(|context| context.dimension.clone());
                    let mut context = world.context.clone();
                    if let Some(dimension) = dimension {
                        context.dimension = dimension;
                    }
                    self.context = Some(context);
                }
                self.context_changed();
                self.refresh();
            }
            Event::Linked(Err(error)) => self.error = Some(error),
            Event::Applied(result) => self.applied = Some(result),
            Event::CanEdit(yes) => self.can_edit = yes,
            Event::Overlays { context, layers } => {
                if self.context.as_ref() != Some(&context) {
                    return;
                }
                self.objects.layers = layers;
                self.probe_edit();
                self.refresh_objects();
            }
            Event::Objects {
                generation,
                key,
                result,
            } => {
                if !self.objects.accept(generation, &key) {
                    return;
                }
                match result {
                    Ok(found) => self.objects.store(key, found),
                    Err(error) => self.objects.fail(
                        key,
                        match error {
                            MapFailure::Failed(reason) => reason,
                            _ => String::new(),
                        },
                    ),
                }
                self.dispatch_objects();
                self.frame();
            }
            Event::Tile {
                generation,
                key,
                result,
            } => {
                if !self.schedule.accept(generation, &key) {
                    return;
                }
                self.stale.remove(&key);
                match result {
                    Ok(reply) => {
                        self.failed.remove(&key);
                        self.revision += 1;
                        let (rgba, unknown) = prepare(reply);
                        self.tiles.insert(
                            key,
                            Loaded {
                                rgba,
                                unknown,
                                revision: self.revision,
                                used: self.clock,
                            },
                        );
                        self.evict();
                    }
                    Err(error) => {
                        self.failed.insert(key, error);
                    }
                }
                self.dispatch();
                self.frame();
            }
        }
        cx.notify();
    }
    /// The world or dimension changed: what it can show is asked for again.
    fn context_changed(&mut self) {
        self.selected = None;
        self.objects.clear();
        self.objects.layers.clear();
        if let (Some(connection), Some(context)) = (&self.connection, &self.context) {
            let _ = connection.send.try_send(Command::Overlays {
                context: context.clone(),
            });
        }
    }
    /// The viewport in blocks, `[x0, z0, x1, z1]`.
    fn view_blocks(&self) -> [f64; 4] {
        let half = [
            f64::from(self.size[0]) * self.camera.scale / 2.,
            f64::from(self.size[1]) * self.camera.scale / 2.,
        ];
        [
            self.camera.x - half[0],
            self.camera.z - half[1],
            self.camera.x + half[0],
            self.camera.z + half[1],
        ]
    }
    /// Icons for the visible objects as screen-space sprites for the frame,
    /// the more important drawn later. They are painted into the map image
    /// itself, so they share its layering and clipping and never intercept the
    /// mouse.
    fn sprites(&self) -> Vec<Sprite> {
        let (width, height) = (f64::from(self.size[0]), f64::from(self.size[1]));
        let objects = self.objects.visible(self.view_blocks());
        let skip = objects.len().saturating_sub(MAX_ICONS);
        objects
            .into_iter()
            .skip(skip)
            .filter_map(|object| {
                let MapObjectKind::Icon { icon, at } = &object.kind else {
                    return None;
                };
                let pixels = match object.color {
                    Some(color) => crate::map_icons::marker(*icon, color),
                    None => crate::map_icons::pixels(*icon),
                };
                Some(Sprite {
                    id: pixels.name.clone(),
                    width: pixels.width,
                    height: pixels.height,
                    rgba: pixels.rgba.clone(),
                    x: (at.x - self.camera.x) / self.camera.scale + width / 2.,
                    y: (at.z - self.camera.z) / self.camera.scale + height / 2.,
                    size: ICON,
                    opacity: if object.approximate { 0.7 } else { 1. },
                })
            })
            .collect()
    }
    /// Rectangles for the visible area layers (slime chunks): each heat cell
    /// in view, translucent, under the icons.
    fn fills(&self) -> Vec<canvas::Fill> {
        let view = self.view_blocks();
        let (width, height) = (f64::from(self.size[0]), f64::from(self.size[1]));
        let to_screen = |x: f64, z: f64| {
            (
                (x - self.camera.x) / self.camera.scale + width / 2.,
                (z - self.camera.z) / self.camera.scale + height / 2.,
            )
        };
        let mut fills: Vec<canvas::Fill> = self.selection_fill().into_iter().collect();
        for object in self.objects.visible(view) {
            let MapObjectKind::Heat { cell, values } = &object.kind else {
                continue;
            };
            let [r, g, b] = object.color.unwrap_or([255, 255, 255]);
            for (at, strength) in values {
                if at.x + cell < view[0]
                    || at.x > view[2]
                    || at.z + cell < view[1]
                    || at.z > view[3]
                {
                    continue;
                }
                let (left, top) = to_screen(at.x, at.z);
                let (right, bottom) = to_screen(at.x + cell, at.z + cell);
                let alpha = (strength.clamp(0., 1.) * 110.).round() as u8;
                fills.push(canvas::Fill {
                    rect: [left, top, right, bottom],
                    rgba: [r, g, b, alpha],
                });
                if fills.len() >= MAX_FILLS {
                    return fills;
                }
            }
        }
        fills
    }
    /// The canvas backend's drawing surface, filling the viewport: it paints
    /// this frame's tiles, icons and grid straight through GPUI.
    fn canvas_layer(&mut self) -> Option<gpui::Canvas<()>> {
        if self.backend != Backend::Canvas {
            return None;
        }
        self.report_stats();
        if self.size.contains(&0) {
            return None;
        }
        let frame = canvas::Frame {
            camera: lumilio_map_render::Camera {
                x: self.camera.x,
                z: self.camera.z,
                blocks_per_pixel: self.camera.scale,
            },
            tiles: self.scene_tiles(),
            sprites: self.sprites(),
            grid: Grid {
                chunks: self.layers.chunks,
                regions: self.layers.regions,
            },
            fills: self.fills(),
        };
        let (painter, stats) = (self.painter.clone(), self.stats.clone());
        Some(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    painter.borrow_mut().paint(window, bounds, &frame, &stats)
                },
            )
            .absolute()
            .size_full(),
        )
    }
    /// `LUMILIO_MAP_STATS=1`: a summary of the last two seconds on stderr.
    fn report_stats(&mut self) {
        if std::env::var_os("LUMILIO_MAP_STATS").is_none()
            || self.report.1.elapsed() < Duration::from_secs(2)
        {
            return;
        }
        let now = self.stats.snapshot();
        eprintln!("[map-stats] {}", now.since(self.report.0).line());
        self.report = (now, Instant::now());
    }
    fn refresh_objects(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        if self.size.contains(&0) {
            return;
        }
        self.objects
            .update(&context, self.view_blocks(), self.camera.scale);
        self.dispatch_objects();
    }
    fn dispatch_objects(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        for next in self.objects.next(&context) {
            let sent = self.connection.as_ref().is_some_and(|connection| {
                connection
                    .send
                    .try_send(Command::Objects {
                        generation: next.generation,
                        plugin: next.plugin,
                        request: Box::new(next.request),
                        key: next.key.clone(),
                        cancel: next.cancel,
                    })
                    .is_ok()
            });
            if !sent {
                self.objects.accept(next.generation, &next.key);
                self.objects.fail(next.key, String::new());
            }
        }
    }
    /// The game's files may have changed (the game just exited): the visible
    /// tiles are asked for again, and the host redraws only those whose files
    /// did change; tiles out of view are forgotten.
    pub fn files_changed(&mut self, cx: &mut Context<Self>) {
        let visible: BTreeSet<TileKey> = self.visible.iter().cloned().collect();
        self.tiles.retain(|key, _| visible.contains(key));
        self.stale = visible
            .into_iter()
            .filter(|key| self.tiles.contains_key(key))
            .collect();
        self.failed.clear();
        self.dispatch();
        cx.notify();
    }
    fn reset_view(&mut self) {
        self.schedule.update(&[]);
        self.stale.clear();
        self.tiles.clear();
        self.failed.clear();
        self.visible.clear();
        self.serial = self.serial.wrapping_add(1);
        if let Some(old) = self.image.take() {
            self.old.push(old);
        }
    }
    fn refresh(&mut self) {
        let started = Instant::now();
        self.refresh_now();
        stats::add(&self.stats.refresh_us, started.elapsed());
        self.stats.refreshes.fetch_add(1, Relaxed);
    }
    fn refresh_now(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        let Some((provider, base)) = self.providers.base_maps.get(self.base) else {
            return;
        };
        let template = TileKey {
            provider: provider.clone(),
            base_map: base.id.clone(),
            world: context.world.clone(),
            dimension: context.dimension.clone(),
            level: self.camera.level(),
            tx: 0,
            tz: 0,
        };
        let visible = Viewport {
            x: self.camera.x,
            z: self.camera.z,
            blocks_per_pixel: self.camera.scale,
            width: self.size[0],
            height: self.size[1],
        }
        .visible(&template);
        self.schedule.update(&visible);
        self.failed.retain(|key, _| visible.contains(key));
        self.stale.retain(|key| visible.contains(key));
        self.visible = visible;
        self.dispatch();
        self.refresh_objects();
        self.frame();
    }
    /// Starts requests for visible tiles that have neither pixels nor a
    /// failure, nearest the centre first, up to [`IN_FLIGHT`] at a time. A
    /// finished tile calls this again, so the queue drains as the map settles.
    fn dispatch(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        let wanted: Vec<TileKey> = self
            .visible
            .iter()
            .filter(|key| {
                (!self.tiles.contains_key(key) || self.stale.contains(key))
                    && !self.failed.contains_key(key)
            })
            .cloned()
            .collect();
        for key in wanted {
            if self.schedule.pending_len() >= IN_FLIGHT {
                break;
            }
            let Some((generation, cancel)) = self.schedule.begin(&key) else {
                continue;
            };
            if let Some(connection) = &self.connection
                && let Err(error) = connection.send.try_send(Command::Tile {
                    generation,
                    request: Box::new(TileRequest {
                        context: context.clone(),
                        key: key.clone(),
                        pixels: 256,
                    }),
                    cancel,
                })
            {
                self.schedule.accept(generation, &key);
                self.failed
                    .insert(key, MapFailure::Failed(error.to_string()));
            }
        }
    }
    /// Drops the least recently drawn tiles beyond [`TILE_CAP`], never one of
    /// the visible ones.
    fn evict(&mut self) {
        let extra = self.tiles.len().saturating_sub(TILE_CAP);
        if extra == 0 {
            return;
        }
        let visible: BTreeSet<&TileKey> = self.visible.iter().collect();
        let mut stale: Vec<(u64, TileKey)> = self
            .tiles
            .iter()
            .filter(|(key, _)| !visible.contains(key))
            .map(|(key, tile)| (tile.used, key.clone()))
            .collect();
        stale.sort();
        for (_, key) in stale.into_iter().take(extra) {
            self.tiles.remove(&key);
        }
    }
    /// Hands the render thread what to draw. Costs a handful of `Arc` clones
    /// per tile, so it can run on every pan and tile arrival.
    ///
    /// Draw order, bottom to top: patterns for visible tiles without pixels,
    /// cached tiles of other levels over them (coarsest first) so a zoom never
    /// shows a hole, then the visible level's own tiles.
    fn frame(&mut self) {
        // The canvas backend paints from the view's state on every render.
        if self.size.contains(&0) || self.backend == Backend::Canvas {
            return;
        }
        self.serial = self.serial.wrapping_add(1);
        let tiles = self.scene_tiles();
        let sprites = self.sprites();
        if let Some(worker) = &self.worker {
            worker.mailbox.put(worker::Request {
                serial: self.serial,
                issued: Instant::now(),
                camera: lumilio_map_render::Camera {
                    x: self.camera.x,
                    z: self.camera.z,
                    blocks_per_pixel: self.camera.scale,
                },
                tiles,
                sprites,
                grid: Grid {
                    chunks: self.layers.chunks,
                    regions: self.layers.regions,
                },
                width: self.size[0],
                height: self.size[1],
            });
        }
    }
    /// The one line of status under the map, most important first.
    fn status_line(&self) -> Option<String> {
        let status = if self.failed.values().any(
            |failure| matches!(failure,MapFailure::Failed(id) if id=="map-version-unsupported"),
        ) {
            Some(tr!("map-version-unsupported"))
        } else if self
            .failed
            .values()
            .any(|failure| *failure == MapFailure::ProviderStopped)
        {
            Some(tr!("map-provider-stopped"))
        } else if self.no_gpu {
            Some(tr!("map-no-gpu"))
        } else if self.base_kind() == Some("map-base-seed")
            && self
                .context
                .as_ref()
                .is_none_or(|context| context.seed.is_none())
        {
            Some(tr!("map-seed-needed"))
        } else if self.base_kind() == Some("map-base-save")
            && self
                .context
                .as_ref()
                .is_none_or(|context| !matches!(context.world, WorldId::Save { .. }))
        {
            Some(tr!("map-save-needed"))
        } else if self.objects.failed_with("map-xaero-unreadable") {
            Some(tr!("map-xaero-unreadable"))
        } else if self.objects.failed_with("map-save-unreadable")
            || self.failed.values().any(
                |failure| matches!(failure, MapFailure::Failed(id) if id == "map-save-unreadable"),
            )
        {
            Some(tr!("map-save-unreadable"))
        } else if self.error.is_some() {
            Some(tr!("map-render-failed"))
        } else if !self.failed.is_empty() || self.objects.failed() > 0 {
            Some(tr!("map-tile-failed"))
        } else {
            None
        };
        // ia[plugin.world-explorer]: 查看存档底图 | 底图分段 ·「存档」 | 从单人存档的 Region 文件画地表（按高度明暗、按群系着色）；没生成完的区块显示无数据；颜色表里没有的方块画成淡紫色并在状态行写明种数 | 粗缩放由宿主用细一级合成
        let unknown = self.unknown_blocks();
        let status = status
            .map(|text| text.to_string())
            .or_else(|| (unknown > 0).then(|| tr!("map-unknown-blocks", count = unknown)));
        self.edit_error
            .as_deref()
            .map(edit::failure_text)
            .or_else(|| self.placing.then(|| tr!("map-placing-hint").to_string()))
            .or(status)
    }
    /// The catalog id of the chosen base map, which says what it needs.
    fn base_kind(&self) -> Option<&str> {
        self.providers
            .base_maps
            .get(self.base)
            .map(|(_, base)| base.kind_id.as_str())
    }
    /// Distinct blocks the visible tiles had no colour for.
    fn unknown_blocks(&self) -> usize {
        self.visible
            .iter()
            .filter_map(|key| self.tiles.get(key))
            .flat_map(|tile| &tile.unknown)
            .collect::<BTreeSet<_>>()
            .len()
    }
    fn scene_tiles(&mut self) -> Vec<Tile> {
        self.clock += 1;
        let clock = self.clock;
        let placed = |key: &TileKey, id: String, rgba: &Arc<[u8]>| {
            let span = f64::from(key.blocks_per_pixel()?) * 256.;
            Some(Tile {
                id,
                x: f64::from(key.tx) * span,
                z: f64::from(key.tz) * span,
                span,
                rgba: rgba.clone(),
            })
        };
        let mut tiles = Vec::new();
        for key in &self.visible {
            match self.tiles.get(key) {
                Some(Loaded { rgba: Some(_), .. }) => {}
                _ if self.failed.contains_key(key) => {
                    tiles.extend(placed(key, "failed".into(), &self.failed_pixels))
                }
                _ => tiles.extend(placed(key, "pending".into(), &self.pending_pixels)),
            }
        }
        if let Some(template) = self.visible.first() {
            let half = [
                f64::from(self.size[0]) * self.camera.scale / 2.,
                f64::from(self.size[1]) * self.camera.scale / 2.,
            ];
            let shown: BTreeSet<&TileKey> = self.visible.iter().collect();
            let mut stand_ins: Vec<&TileKey> = self
                .tiles
                .iter()
                .filter(|(key, tile)| {
                    tile.rgba.is_some()
                        && key.level != template.level
                        && key.provider == template.provider
                        && key.base_map == template.base_map
                        && key.world == template.world
                        && key.dimension == template.dimension
                        && !shown.contains(key)
                        && key.blocks_per_pixel().is_some_and(|scale| {
                            let span = f64::from(scale) * 256.;
                            let (x, z) = (f64::from(key.tx) * span, f64::from(key.tz) * span);
                            x < self.camera.x + half[0]
                                && x + span > self.camera.x - half[0]
                                && z < self.camera.z + half[1]
                                && z + span > self.camera.z - half[1]
                        })
                })
                .map(|(key, _)| key)
                .collect();
            stand_ins.sort_by(|a, b| b.level.cmp(&a.level).then_with(|| a.cmp(b)));
            // Keep the coarsest: they cover the most ground per draw.
            stand_ins.truncate(STAND_INS);
            let keys: Vec<TileKey> = stand_ins.into_iter().cloned().collect();
            for key in keys {
                if let Some(tile) = self.tiles.get_mut(&key) {
                    tile.used = clock;
                    if let Some(rgba) = &tile.rgba {
                        tiles.extend(placed(&key, format!("tile-{}", tile.revision), rgba));
                    }
                }
            }
        }
        for key in &self.visible {
            if let Some(tile) = self.tiles.get_mut(key) {
                tile.used = clock;
                if let Some(rgba) = &tile.rgba {
                    tiles.extend(placed(key, format!("tile-{}", tile.revision), rgba));
                }
            }
        }
        tiles
    }
}
impl Render for MapView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(shown) = self.shown_at.take() {
            stats::add(&self.stats.paint_lag_us, shown.elapsed());
            self.stats.paints.fetch_add(1, Relaxed);
        }
        for old in self.old.drain(..) {
            let _ = window.drop_image(old);
        }
        if !self.release {
            cx.on_release_in(window, |this, window, _| {
                for image in this.old.drain(..).chain(this.image.take()) {
                    let _ = window.drop_image(image);
                }
                this.painter.borrow_mut().release(window);
                if let Some(connection) = &this.connection {
                    connection.send.close();
                    connection.receive.close();
                }
            })
            .detach();
            self.release = true;
        }
        if !self.asked {
            self.asked = true;
            let load = self.load.clone();
            let id = cx.entity_id().as_u64();
            window.defer(cx, move |window, cx| load(id, window, cx));
        }
        let colors = ShellColors::from_theme(cx.theme());
        if let Some(result) = self.applied.take() {
            let target = cx.weak_entity();
            window.defer(cx, move |window, cx| {
                let _ = target.update(cx, |this, cx| this.edit_applied(result, window, cx));
            });
        }
        if self.form.is_none() {
            let target = cx.weak_entity();
            // InputState installs focus/blur observers. Create it outside the
            // drawing phase so those observers join the window's live state.
            window.defer(cx, move |window, cx| {
                let _ = target.update(cx, |this, cx| {
                    this.ensure_form(window, cx);
                    cx.notify();
                });
            });
            return v_flex().w_full().h_full();
        }
        self.ensure_form(window, cx);
        let toolbar = self.toolbar();
        let target = cx.weak_entity();
        let measure = canvas(
            move |bounds, _, cx| {
                let _ = target.update(cx, |this, _| {
                    this.bounds = Some(bounds);
                    let size = [
                        f32::from(bounds.size.width).round().clamp(1., 4096.) as u32,
                        f32::from(bounds.size.height).round().clamp(1., 4096.) as u32,
                    ];
                    if size != this.size {
                        this.size = size;
                        this.refresh();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let status = self.status_line();
        // Floating panels sit over the map; pressing or wheeling on them must
        // not pan or zoom the map beneath.
        let float = || {
            div()
                .absolute()
                .p_2()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.surface)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        };
        let retry = self.retry_button(cx);
        // ia[plugin.world-explorer]: 平移与缩放 | 地图视口 · 拖动 / 滚轮 / + − 与方向键 | 锚点缩放；过期世代与旧视口结果不进入当前帧
        let viewport = div()
            .id("world-map")
            .debug_selector(|| "world-map".into())
            .track_focus(&self.focus)
            .tab_stop(true)
            .relative()
            .w_full()
            .flex_1()
            .min_h(px(192.))
            .overflow_hidden()
            .bg(colors.surface_subtle)
            .border_1()
            .border_color(if self.focus.is_focused(window) {
                colors.focus
            } else {
                colors.border
            })
            .children(
                self.image
                    .clone()
                    .filter(|_| self.backend == Backend::Wgpu)
                    .map(|image| img(image).size_full().object_fit(ObjectFit::Fill)),
            )
            .children(self.canvas_layer())
            .child(measure)
            .child(float().top_3().left_3().child(self.view_controls(cx)))
            .child(float().top_3().right_3().child(self.jump_controls(cx)))
            .child(
                float().bottom_3().left_3().max_w(relative(0.7)).child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .flex_wrap()
                        .child(
                            div()
                                .debug_selector(|| "map-cursor".into())
                                .child(format!("X {:.0}  Z {:.0}", self.cursor[0], self.cursor[1])),
                        )
                        .children(status.map(|status| {
                            div()
                                .debug_selector(|| "map-status".into())
                                .text_color(colors.muted)
                                .child(status)
                        }))
                        .children(retry)
                        .children(self.place_key(cx))
                        .children(self.xaero_link_prompt(cx)),
                ),
            )
            .child(float().bottom_3().right_3().child(self.layer_popover(cx)))
            .children(
                self.selection_card(cx)
                    .map(|card| float().left_3().bottom(px(56.)).child(card)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.drag = Some(event.position);
                    this.press = Some(event.position);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    this.drag = None;
                    if let Some(down) = this.press.take() {
                        if this.placing {
                            if let Some(at) = this.click_point(down, event.position) {
                                this.create_at(MapPoint { x: at[0], z: at[1] }, window, cx);
                            }
                        } else {
                            this.click(down, event.position);
                        }
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.drag = None;
                    this.press = None;
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some(bounds) = this.bounds {
                    let local = event.position - bounds.origin;
                    this.cursor = this.camera.world(
                        [f32::from(local.x) as f64, f32::from(local.y) as f64],
                        this.size,
                    );
                }
                if let Some(last) = this.drag.take()
                    && event.pressed_button == Some(MouseButton::Left)
                {
                    let delta = event.position - last;
                    this.camera
                        .pan(f32::from(delta.x) as f64, f32::from(delta.y) as f64);
                    this.drag = Some(event.position);
                    this.refresh();
                }
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(16.));
                let at = this
                    .bounds
                    .map(|bounds| event.position - bounds.origin)
                    .unwrap_or(gpui::point(px(0.), px(0.)));
                this.camera.zoom(
                    (-f32::from(delta.y) as f64 * 0.01).exp(),
                    [f32::from(at.x) as f64, f32::from(at.y) as f64],
                    this.size,
                );
                this.refresh();
                cx.stop_propagation();
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if !this.focus.is_focused(window) {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "+" | "=" => this.camera.zoom(
                        0.8,
                        [f64::from(this.size[0]) / 2., f64::from(this.size[1]) / 2.],
                        this.size,
                    ),
                    "-" => this.camera.zoom(
                        1.25,
                        [f64::from(this.size[0]) / 2., f64::from(this.size[1]) / 2.],
                        this.size,
                    ),
                    "left" => this.camera.pan(32., 0.),
                    "right" => this.camera.pan(-32., 0.),
                    "up" => this.camera.pan(0., 32.),
                    "down" => this.camera.pan(0., -32.),
                    _ => return,
                }
                this.refresh();
                cx.stop_propagation();
                cx.notify();
            }));
        v_flex()
            .gap_3()
            .w_full()
            .h_full()
            .min_h_0()
            .child(toolbar)
            .child(viewport)
    }
}
