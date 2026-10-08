//! Host-owned map viewport. Background data and rendering enter through mailboxes.
mod camera;
mod layers;
mod seed;
mod toolbar;
pub use seed::VERSIONS as MANUAL_VERSIONS;
#[cfg(test)]
mod tests;
mod worker;

use crate::{theme::ShellColors, tr};
use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, FocusHandle, MouseButton, ObjectFit, Pixels, Point, RenderImage, Task,
    Window, canvas, div, img, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use lumilio_core::world_map::{MapSchedule, Viewport, WorldMapContext};
use lumilio_core::{CancellationToken, MapFailure, MapProviders};
use lumilio_map_render::{Grid, Tile};
use lumilio_plugin_api::map::{TileKey, TileReply, TileRequest, WorldContext};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

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
}
pub enum Event {
    Seed(Result<(Vec<WorldMapContext>, WorldContext), String>),
    Contexts(Result<(Vec<WorldMapContext>, MapProviders), String>),
    Tile {
        generation: u64,
        key: TileKey,
        result: Result<TileReply, MapFailure>,
    },
}
pub struct Connection {
    pub send: async_channel::Sender<Command>,
    pub receive: async_channel::Receiver<Event>,
}
pub type Load = Rc<dyn Fn(u64, &mut Window, &mut App)>;

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
    schedule: MapSchedule,
    tiles: BTreeMap<TileKey, TileReply>,
    visible: Vec<TileKey>,
    failed: BTreeMap<TileKey, MapFailure>,
    size: [u32; 2],
    bounds: Option<Bounds<Pixels>>,
    drag: Option<Point<Pixels>>,
    cursor: [f64; 2],
    serial: u64,
    image: Option<Arc<RenderImage>>,
    old: Vec<Arc<RenderImage>>,
    error: Option<String>,
    no_gpu: bool,
    form: Option<seed::Form>,
    release: bool,
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
            schedule: MapSchedule::default(),
            tiles: BTreeMap::new(),
            visible: Vec::new(),
            failed: BTreeMap::new(),
            size: [0, 0],
            bounds: None,
            drag: None,
            cursor: [0., 0.],
            serial: 0,
            image: None,
            old: vec![],
            error: None,
            no_gpu: false,
            form: None,
            release: false,
        }
    }
    pub fn connect(&mut self, connection: Connection, cx: &mut Context<Self>) {
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
        match worker::Worker::start() {
            Ok(worker) => {
                let receive = worker.output.clone();
                self.render_task = Some(cx.spawn(async move |this, cx| {
                    while let Ok(result) = receive.recv().await {
                        if this
                            .update(cx, |this, cx| {
                                match result {
                                    Ok((serial, frame)) if serial == this.serial => {
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
                                        }
                                    }
                                    Ok(_) => {}
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
                self.refresh();
            }
            Event::Contexts(Err(error)) => self.error = Some(error),
            Event::Tile {
                generation,
                key,
                result,
            } => {
                if !self.schedule.accept(generation, &key) {
                    return;
                }
                match result {
                    Ok(reply) => {
                        self.failed.remove(&key);
                        self.tiles.insert(key, reply);
                    }
                    Err(error) => {
                        self.failed.insert(key, error);
                    }
                }
                self.frame();
            }
        }
        cx.notify();
    }
    fn reset_view(&mut self) {
        self.schedule.update(&[]);
        self.tiles.clear();
        self.failed.clear();
        self.visible.clear();
        self.serial = self.serial.wrapping_add(1);
        if let Some(old) = self.image.take() {
            self.old.push(old);
        }
    }
    fn refresh(&mut self) {
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
        self.visible = visible.clone();
        self.tiles.retain(|key, _| visible.contains(key));
        self.failed.retain(|key, _| visible.contains(key));
        for key in visible {
            if self.tiles.contains_key(&key) || self.failed.contains_key(&key) {
                continue;
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
        self.frame();
    }
    fn frame(&mut self) {
        if self.size.contains(&0) {
            return;
        }
        self.serial = self.serial.wrapping_add(1);
        let tiles = self
            .visible
            .iter()
            .filter_map(|key| {
                let rgba = match self.tiles.get(key) {
                    Some(TileReply::Image(image)) => image.rgba.clone(),
                    Some(TileReply::Partial { image, coverage }) => image
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .zip(coverage)
                        .flat_map(|(pixel, covered)| {
                            if *covered > 0 {
                                *pixel
                            } else {
                                [96, 96, 96, 255]
                            }
                        })
                        .collect(),
                    _ => (0..256 * 256)
                        .flat_map(|pixel| {
                            let shade = if ((pixel % 256) / 16 + (pixel / 256) / 16) % 2 == 0 {
                                80
                            } else {
                                96
                            };
                            if self.failed.contains_key(key) {
                                [shade + 32, shade, shade, 255]
                            } else {
                                [shade, shade, shade, 255]
                            }
                        })
                        .collect(),
                };
                let span = f64::from(key.blocks_per_pixel()?) * 256.;
                Some(Tile {
                    id: format!(
                        "{key:?}:{:?}:{}:{}",
                        self.context,
                        self.failed.contains_key(key),
                        self.tiles.contains_key(key)
                    ),
                    x: f64::from(key.tx) * span,
                    z: f64::from(key.tz) * span,
                    span,
                    rgba,
                })
            })
            .collect();
        if let Some(worker) = &self.worker {
            worker.mailbox.put(worker::Request {
                serial: self.serial,
                camera: lumilio_map_render::Camera {
                    x: self.camera.x,
                    z: self.camera.z,
                    blocks_per_pixel: self.camera.scale,
                },
                tiles,
                grid: Grid {
                    chunks: self.layers.chunks,
                    regions: self.layers.regions,
                },
                width: self.size[0],
                height: self.size[1],
            });
        }
    }
}
impl Render for MapView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.old.drain(..) {
            let _ = window.drop_image(old);
        }
        if !self.release {
            cx.on_release_in(window, |this, window, _| {
                for image in this.old.drain(..).chain(this.image.take()) {
                    let _ = window.drop_image(image);
                }
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
        let toolbar = self.toolbar(cx);
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
        let status = if self.failed.values().any(
            |failure| matches!(failure,MapFailure::Failed(id) if id=="map-version-unsupported"),
        ) {
            tr!("map-version-unsupported")
        } else if self
            .failed
            .values()
            .any(|failure| *failure == MapFailure::ProviderStopped)
        {
            tr!("map-provider-stopped")
        } else if self.no_gpu {
            tr!("map-no-gpu")
        } else if self
            .context
            .as_ref()
            .is_none_or(|context| context.seed.is_none())
        {
            tr!("map-seed-needed")
        } else if self.error.is_some() {
            tr!("map-render-failed")
        } else if !self.failed.is_empty() {
            tr!("map-tile-failed")
        } else {
            tr!("map-no-data")
        };
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
                    .map(|image| img(image).size_full().object_fit(ObjectFit::Fill)),
            )
            .child(measure)
            .child(
                div()
                    .absolute()
                    .right_3()
                    .bottom_3()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(self.layer_popover(cx)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.drag = Some(event.position);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
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
            .child(
                h_flex()
                    .gap_3()
                    .child(status)
                    .child(format!("X {:.0}  Z {:.0}", self.cursor[0], self.cursor[1])),
            )
            .children(self.retry_button(cx))
    }
}
