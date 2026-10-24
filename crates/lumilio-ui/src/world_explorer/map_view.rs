use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

use gpui::{App, Bounds, Context, FocusHandle, Pixels, Point, RenderImage, Task, Window};
use lumilio_core::world_map::store::annotations::Annotation;
use lumilio_core::world_map::{MapSchedule, WorldMapContext};
use lumilio_core::{CancellationToken, MapFailure, MapProviders};
use lumilio_plugin_api::map::{
    Dimension, MapObject, MapPoint, OverlayInfo, OverlayRequest, TileKey, TileReply, TileRequest,
    WorldContext, WorldId,
};

use super::objects::ObjectKey;
use super::window::PopOut;
use super::{camera, canvas, edit, layers, objects, seed, stats, worker};
use crate::tr;

pub enum Command {
    Contexts,
    Annotations {
        context: WorldContext,
    },
    PutAnnotation {
        context: WorldContext,
        annotation: Box<Annotation>,
    },
    RemoveAnnotation {
        context: WorldContext,
        id: i64,
    },
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
    Annotations {
        context: WorldContext,
        result: Result<Vec<Annotation>, String>,
    },
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
pub(super) const IN_FLIGHT: usize = 12;
/// Decoded tiles kept across zooms (256 KiB each). Coarser or finer neighbours
/// stand in for the visible level while its own tiles are still being made.
pub(super) const TILE_CAP: usize = 512;
/// Stand-ins drawn under a frame, so one frame stays a bounded number of draws.
pub(super) const STAND_INS: usize = 192;
/// Structure layers shown before the user chooses any.
pub(super) const DEFAULT_LAYERS: &[&str] = &[
    "litematica.placements",
    "save.positions",
    "structure.village",
    "structure.stronghold",
    "structure.fortress",
    "structure.end-city",
];
/// Icons drawn at once; the most important win when a view holds more.
pub(super) const MAX_ICONS: usize = 400;
/// Area rectangles drawn at once.
pub(super) const MAX_FILLS: usize = 6000;
/// Icon edge on screen, in pixels.
pub(super) const ICON: u32 = 28;

/// A tile's pixels, prepared once on arrival. `rgba` is `None` for a tile the
/// provider answered as empty; it is drawn as the empty-tile pattern.
pub(super) struct Loaded {
    pub(super) rgba: Option<Arc<[u8]>>,
    /// Blocks the provider had no colour for in this tile.
    pub(super) unknown: Vec<String>,
    /// Names the texture on the render thread; a retried tile gets a new one.
    pub(super) revision: u64,
    pub(super) used: u64,
}

pub(super) fn pattern(failed: bool) -> Arc<[u8]> {
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
pub(super) fn prepare(reply: TileReply) -> (Option<Arc<[u8]>>, Vec<String>) {
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
    pub(super) load: Load,
    /// Opens the map in a window of its own; `None` for a map that already is.
    pub(super) pop_out: Option<PopOut>,
    /// The world and dimension another map showed, shown once the worlds arrive.
    pub(super) restore: Option<(WorldId, Dimension)>,
    pub(super) asked: bool,
    pub(super) focus: FocusHandle,
    pub(super) connection: Option<Connection>,
    pub(super) data_task: Option<Task<()>>,
    pub(super) worker: Option<worker::Worker>,
    pub(super) render_task: Option<Task<()>>,
    pub(super) contexts: Vec<WorldMapContext>,
    pub(super) providers: MapProviders,
    pub(super) context: Option<WorldContext>,
    pub(super) base: usize,
    pub(super) xaero_maps: BTreeMap<(WorldId, Dimension), String>,
    pub(super) camera: camera::Camera,
    pub(super) layers: layers::Layers,
    pub(super) objects: objects::Objects,
    pub(super) layers_ready: bool,
    pub(super) annotations: Vec<Annotation>,
    pub(super) layer_scroll: gpui::ScrollHandle,
    pub(super) schedule: MapSchedule,
    pub(super) tiles: BTreeMap<TileKey, Loaded>,
    pub(super) clock: u64,
    pub(super) revision: u64,
    pub(super) pending_pixels: Arc<[u8]>,
    pub(super) failed_pixels: Arc<[u8]>,
    pub(super) visible: Vec<TileKey>,
    /// Shown tiles to ask for again, because the files they were drawn from
    /// may have changed; each keeps showing until its answer replaces it.
    pub(super) stale: BTreeSet<TileKey>,
    pub(super) failed: BTreeMap<TileKey, MapFailure>,
    pub(super) size: [u32; 2],
    pub(super) bounds: Option<Bounds<Pixels>>,
    pub(super) drag: Option<Point<Pixels>>,
    /// Where the left button went down, until it comes up: a click if it
    /// barely moved.
    pub(super) press: Option<Point<Pixels>>,
    pub(super) selected: Option<MapObject>,
    /// Whether the files could be written now (no game running).
    pub(super) can_edit: bool,
    pub(super) edit_dialog: Option<gpui::WeakEntity<edit::EditDialog>>,
    /// A message id for an edit that failed with no dialog open.
    pub(super) edit_error: Option<String>,
    /// The next click on the map places a new object.
    pub(super) placing: bool,
    pub(super) marking: bool,
    pub(super) routing: bool,
    pub(super) route_points: Vec<MapPoint>,
    pub(super) measure_mode: bool,
    pub(super) measure_start: Option<MapPoint>,
    pub(super) measure_end: Option<MapPoint>,
    pub(super) annotation_pending: bool,
    /// An edit's answer waiting for the next render, which has the window.
    pub(super) applied: Option<Result<(), String>>,
    pub(super) cursor: [f64; 2],
    pub(super) serial: u64,
    pub(super) image: Option<Arc<RenderImage>>,
    pub(super) old: Vec<Arc<RenderImage>>,
    pub(super) error: Option<String>,
    pub(super) no_gpu: bool,
    pub(super) form: Option<seed::Form>,
    pub(super) release: bool,
    pub(super) stats: Arc<stats::Stats>,
    /// When the latest frame was shown, until the view next renders.
    pub(super) shown_at: Option<Instant>,
    pub(super) backend: Backend,
    pub(super) painter: std::rc::Rc<std::cell::RefCell<canvas::Painter>>,
    pub(super) report: (stats::Snapshot, Instant),
}

/// How the map reaches the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Backend {
    /// Tiles and icons painted by GPUI itself (`canvas.rs`). The default: it
    /// draws at device resolution with no readback, and measured and looked
    /// better than the wgpu path in the maintainer's side-by-side.
    Canvas,
    /// Composed on our own wgpu device and read back as one image. Kept for
    /// comparison, behind `LUMILIO_MAP_BACKEND=wgpu`.
    Wgpu,
}

impl Backend {
    pub(super) fn from_env() -> Self {
        Self::parse(std::env::var("LUMILIO_MAP_BACKEND").ok().as_deref())
    }
    pub(super) fn parse(choice: Option<&str>) -> Self {
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
            pop_out: None,
            restore: None,
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
            xaero_maps: BTreeMap::new(),
            camera: camera::Camera::default(),
            layers: layers::Layers::default(),
            objects: objects::Objects::new(DEFAULT_LAYERS),
            layers_ready: false,
            annotations: Vec::new(),
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
            marking: false,
            routing: false,
            route_points: Vec::new(),
            measure_mode: false,
            measure_start: None,
            measure_end: None,
            annotation_pending: false,
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
    pub(super) fn attach(&mut self, connection: Connection, cx: &mut Context<Self>) {
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
    pub(super) fn start_renderer(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn event(&mut self, event: Event, cx: &mut Context<Self>) {
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
                self.context = self.initial_context();
                self.context_changed();
                self.refresh();
            }
            Event::Contexts(Err(error)) => self.error = Some(error),
            Event::Annotations { context, result } => {
                if self.context.as_ref() != Some(&context) {
                    return;
                }
                if self.annotation_pending {
                    self.annotation_pending = false;
                    self.applied = Some(result.as_ref().map(|_| ()).map_err(Clone::clone));
                }
                match result {
                    Ok(annotations) => {
                        self.annotations = annotations;
                        self.error = None;
                        self.frame();
                    }
                    Err(error) => self.error = Some(error),
                }
            }
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
                self.layers_ready = true;
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
    /// `LUMILIO_MAP_STATS=1`: a summary of the last two seconds on stderr.
    pub(super) fn report_stats(&mut self) {
        if std::env::var_os("LUMILIO_MAP_STATS").is_none()
            || self.report.1.elapsed() < Duration::from_secs(2)
        {
            return;
        }
        let now = self.stats.snapshot();
        eprintln!("[map-stats] {}", now.since(self.report.0).line());
        self.report = (now, Instant::now());
    }
    /// What went wrong or is missing, as the messages menu words it. Placing
    /// a waypoint is a mode, not a message, so its hint is not here.
    #[cfg(test)]
    pub(super) fn status_line(&self) -> Option<String> {
        self.edit_error
            .as_deref()
            .map(edit::failure_text)
            .or_else(|| self.placing.then(|| tr!("map-placing-hint").to_string()))
            .or_else(|| self.base_status())
    }
    pub(super) fn problem_line(&self) -> Option<String> {
        self.edit_error
            .as_deref()
            .map(edit::failure_text)
            .or_else(|| self.base_status())
    }
    pub(super) fn base_status(&self) -> Option<String> {
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
        } else if self.base_kind() == Some("map-base-xaero") && self.xaero_map_ids().is_empty() {
            Some(tr!("map-xaero-needed"))
        } else if self.base_kind() == Some("map-base-xaero")
            && self.xaero_map_ids().len() > 1
            && self.xaero_map_choice().is_none()
        {
            Some(tr!("map-xaero-choose-map"))
        } else if self.failed.values().any(|failure| matches!(failure, MapFailure::Failed(id) if id == "map-xaero-choose-map")) {
            Some(tr!("map-xaero-choose-map"))
        } else if self.failed.values().any(|failure| matches!(failure, MapFailure::Failed(id) if id == "map-xaero-region-too-large")) {
            Some(tr!("map-xaero-region-too-large"))
        } else if self.failed.values().any(|failure| matches!(failure, MapFailure::Failed(id) if id == "map-xaero-unsupported")) {
            Some(tr!("map-xaero-unsupported"))
        } else if self.failed.values().any(|failure| matches!(failure, MapFailure::Failed(id) if id == "map-xaero-unreadable")) {
            Some(tr!("map-xaero-unreadable"))
        } else if self.objects.failed_with("map-xaero-unreadable") {
            Some(tr!("map-xaero-unreadable"))
        } else if self.objects.failed_with("map-litematica-unreadable") {
            Some(tr!("map-litematica-unreadable"))
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
        // ia[plugin.world-explorer]: 查看存档底图 | 底图分段 ·「存档」 | 从单人存档的 Region 文件画地表（按高度明暗、按群系着色）；没生成完的区块显示无数据；颜色表里没有的方块画成淡紫色并在消息里写明种数 | 粗缩放由宿主用细一级合成
        let unknown = self.unknown_blocks();
        status
            .map(|text| text.to_string())
            .or_else(|| (unknown > 0).then(|| tr!("map-unknown-blocks", count = unknown)))
    }
    /// The catalog id of the chosen base map, which says what it needs.
    pub(super) fn base_kind(&self) -> Option<&str> {
        self.providers
            .base_maps
            .get(self.base)
            .map(|(_, base)| base.kind_id.as_str())
    }
    /// Distinct blocks the visible tiles had no colour for.
    pub(super) fn unknown_blocks(&self) -> usize {
        self.visible
            .iter()
            .filter_map(|key| self.tiles.get(key))
            .flat_map(|tile| &tile.unknown)
            .collect::<BTreeSet<_>>()
            .len()
    }
}
