//! Host-owned map viewport. Background data and rendering enter through mailboxes.
mod camera;
mod layers;
#[cfg(test)]
mod tests;
mod worker;

use crate::{
    controls::Segments,
    key::Key,
    settings_dialog::{DialogSpec, FieldKind, FieldSpec, SettingsDialog},
    theme::ShellColors,
    tr, tr_all,
};
use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, FocusHandle, MouseButton, ObjectFit, Pixels, Point, RenderImage, Task,
    Window, canvas, div, img, px,
};
use gpui_component::{ActiveTheme as _, Selectable as _, Sizable as _, h_flex, v_flex};
use lumilio_core::world_map::{MapSchedule, Viewport, WorldMapContext};
use lumilio_core::{CancellationToken, MapFailure, MapProviders};
use lumilio_map_render::{Grid, Tile};
use lumilio_plugin_api::map::{Dimension, TileKey, TileReply, TileRequest, WorldContext};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

pub enum Command {
    Contexts,
    Tile {
        generation: u64,
        request: Box<TileRequest>,
        cancel: CancellationToken,
    },
}
pub enum Event {
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
                                    Err(error) => this.error = Some(format!("{error:?}")),
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
            if let Some(connection) = &self.connection {
                let _ = connection.send.try_send(Command::Tile {
                    generation,
                    request: Box::new(TileRequest {
                        context: context.clone(),
                        key,
                        pixels: 256,
                    }),
                    cancel,
                });
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
    fn jump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = cx.weak_entity();
        SettingsDialog::open(
            DialogSpec {
                title: tr!("map-jump"),
                intro: None,
                fields: [("X", self.camera.x), ("Z", self.camera.z)]
                    .into_iter()
                    .map(|(label, value)| FieldSpec {
                        label,
                        help: None,
                        placeholder: "",
                        value: format!("{value:.0}"),
                        kind: FieldKind::Line,
                    })
                    .collect(),
                parse: Rc::new(|values| {
                    let x = values[0]
                        .parse::<f64>()
                        .map_err(|_| tr!("map-coordinates-invalid").to_owned())?;
                    let z = values[1]
                        .parse::<f64>()
                        .map_err(|_| tr!("map-coordinates-invalid").to_owned())?;
                    if !x.is_finite()
                        || !z.is_finite()
                        || x.abs() > 29_900_000.
                        || z.abs() > 29_900_000.
                    {
                        return Err(tr!("map-coordinates-invalid").into());
                    }
                    Ok([x, z])
                }),
                reset: None,
            },
            Rc::new(move |at, _, cx| {
                let _ = target.update(cx, |this, cx| {
                    this.camera.x = at[0];
                    this.camera.z = at[1];
                    this.refresh();
                    cx.notify();
                });
            }),
            window,
            cx,
        );
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
        let dimensions = match self.context.as_ref().map(|context| &context.dimension) {
            Some(Dimension::Nether) => 1,
            Some(Dimension::End) => 2,
            _ => 0,
        };
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 切换维度 | 地图工具栏 · 维度分段 | 保留相机；清除旧维度瓦片并取消旧请求
        let dimensions = Segments::new(
            "map-dimension",
            tr_all!["map-overworld", "map-nether", "map-end"],
            dimensions,
            move |index, _, cx| {
                let _ = target.update(cx, |this, cx| {
                    if let Some(context) = &mut this.context {
                        context.dimension =
                            [Dimension::Overworld, Dimension::Nether, Dimension::End][index]
                                .clone();
                    }
                    this.tiles.clear();
                    this.failed.clear();
                    this.refresh();
                    cx.notify();
                });
            },
        );
        // ia[plugin.world-explorer]: 选择世界 | 地图顶部 · 存档名称 | 使用该世界的种子与版本，取消上一世界请求
        let worlds = h_flex()
            .gap_2()
            .flex_wrap()
            .children(self.contexts.iter().enumerate().map(|(index, world)| {
                Key::new(("map-world", index))
                    .label(world.name.clone())
                    .white()
                    .small()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.context = Some(this.contexts[index].context.clone());
                        this.tiles.clear();
                        this.failed.clear();
                        this.refresh();
                        cx.notify();
                    }))
            }));
        // ia[plugin.world-explorer]: 选择底图 | 地图工具栏 · 底图按键 | 单选；保留相机与维度，取消旧底图请求
        let bases = h_flex()
            .gap_2()
            .children(
                self.providers
                    .base_maps
                    .iter()
                    .enumerate()
                    .map(|(index, (_, base))| {
                        Key::new(("map-base", index))
                            .label(
                                crate::i18n::lookup(&base.kind_id)
                                    .unwrap_or_else(|| base.kind_id.clone()),
                            )
                            .white()
                            .small()
                            .selected(index == self.base)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.base = index;
                                this.tiles.clear();
                                this.failed.clear();
                                this.refresh();
                                cx.notify();
                            }))
                    }),
            );
        // ia[plugin.world-explorer]: 显示区块与 Region 网格 | 地图工具栏 · 网格开关 | 只改变宿主 Overlay；粗缩放自动隐藏区块细线
        let layers = h_flex()
            .gap_2()
            .child(
                Key::new("map-chunks")
                    .label(tr!("map-chunks"))
                    .white()
                    .small()
                    .selected(self.layers.chunks)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.layers.chunks = !this.layers.chunks;
                        this.frame();
                        cx.notify();
                    })),
            )
            .child(
                Key::new("map-regions")
                    .label(tr!("map-regions"))
                    .white()
                    .small()
                    .selected(self.layers.regions)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.layers.regions = !this.layers.regions;
                        this.frame();
                        cx.notify();
                    })),
            );
        // ia[plugin.world-explorer]: 跳到坐标 | 地图工具栏 ·「跳到坐标…」 | 校验 X/Z；移动地图中心
        let jump = Key::new("map-jump")
            .label(tr!("map-jump"))
            .white()
            .small()
            .on_click(cx.listener(|this, _, window, cx| this.jump(window, cx)));
        // ia[plugin.world-explorer]: 重试失败瓦片 | 地图状态行 · 瓦片坐标按键 | 重新派发该块；连续五次故障停用 Provider 到重启
        let retries =
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(self.failed.keys().cloned().enumerate().map(|(index, key)| {
                    Key::new(("map-retry", index))
                        .label(tr!("map-retry-tile", x = key.tx, z = key.tz))
                        .white()
                        .small()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.failed.remove(&key);
                            this.refresh();
                            cx.notify();
                        }))
                }));
        let status = if self.failed.values().any(
            |failure| matches!(failure,MapFailure::Failed(id) if id=="map-version-unsupported"),
        ) {
            tr!("map-version-unsupported")
        } else if self
            .context
            .as_ref()
            .is_none_or(|context| context.seed.is_none())
        {
            tr!("map-seed-needed")
        } else if self.error.is_some() {
            tr!("map-render-failed")
        } else {
            tr!("map-no-data")
        };
        // ia[plugin.world-explorer]: 平移与缩放 | 地图视口 · 拖动 / 滚轮 / + − 与方向键 | 锚点缩放；过期世代与旧视口结果不进入当前帧
        let viewport = div()
            .id("world-map")
            .track_focus(&self.focus)
            .relative()
            .w_full()
            .h_96()
            .overflow_hidden()
            .bg(colors.surface_subtle)
            .children(
                self.image
                    .clone()
                    .map(|image| img(image).size_full().object_fit(ObjectFit::Fill)),
            )
            .child(measure)
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
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
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
            .child(worlds)
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(dimensions)
                    .child(bases)
                    .child(layers)
                    .child(jump),
            )
            .child(viewport)
            .child(
                h_flex()
                    .gap_3()
                    .child(status)
                    .child(format!("X {:.0}  Z {:.0}", self.cursor[0], self.cursor[1])),
            )
            .child(retries)
    }
}
