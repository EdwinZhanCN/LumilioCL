use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use gpui::prelude::*;
use gpui::{Context, MouseButton, ObjectFit, Window, div, img, px, relative};
use gpui_component::{h_flex, v_flex};
use lumilio_core::world_map::store::annotations::{AnnotationKind, route_length};
use lumilio_map_render::{Grid, Sprite, Tile};
use lumilio_plugin_api::map::{MapIcon, MapObject, MapObjectKind, MapPoint, TileKey};

use super::{Backend, ICON, Loaded, MAX_FILLS, MAX_ICONS, MapView, STAND_INS, canvas, edit, stats};
use crate::key::KeySize;
use crate::theme::ShellColors;
use crate::tr;

impl MapView {
    /// The viewport in blocks, `[x0, z0, x1, z1]`.
    pub(super) fn view_blocks(&self) -> [f64; 4] {
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
    pub(super) fn sprites(&self) -> Vec<Sprite> {
        let (width, height) = (f64::from(self.size[0]), f64::from(self.size[1]));
        let annotations = self.annotation_objects();
        let mut objects: Vec<&MapObject> = self.objects.visible(self.view_blocks());
        objects.extend(annotations.iter());
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

    pub(super) fn annotation_objects(&self) -> Vec<MapObject> {
        let Some(context) = &self.context else {
            return Vec::new();
        };
        let view = self.view_blocks();
        self.annotations
            .iter()
            .filter(|item| item.world == context.world && item.dimension == context.dimension)
            .filter(|item| {
                item.points.iter().any(|at| {
                    at.x >= view[0] && at.x <= view[2] && at.z >= view[1] && at.z <= view[3]
                }) || (item.kind == AnnotationKind::Route
                    && item.points.windows(2).any(|segment| {
                        let (a, b) = (segment[0], segment[1]);
                        a.x.min(b.x) <= view[2]
                            && a.x.max(b.x) >= view[0]
                            && a.z.min(b.z) <= view[3]
                            && a.z.max(b.z) >= view[1]
                    }))
            })
            .map(|item| MapObject {
                id: format!("annotation:{}", item.id),
                raw_id: item.id.to_string(),
                source: "lumilio.launcher".into(),
                world: item.world.clone(),
                dimension: item.dimension.clone(),
                kind: match item.kind {
                    AnnotationKind::Marker => MapObjectKind::Icon {
                        icon: MapIcon::Marker,
                        at: item.points[0],
                    },
                    AnnotationKind::Route => MapObjectKind::Polyline(item.points.clone()),
                },
                label: Some(item.name.clone()),
                label_id: Some(
                    match item.kind {
                        AnnotationKind::Marker => "map-annotation-marker",
                        AnnotationKind::Route => "map-annotation-route",
                    }
                    .into(),
                ),
                priority: 100,
                approximate: false,
                color: Some(item.color),
                note: if let (Some(source), Some(raw_id)) =
                    (&item.linked_source, &item.linked_raw_id)
                {
                    (self.layers_ready && self.objects.link_missing(source, raw_id))
                        .then(|| tr!("map-annotation-link-missing").to_owned())
                } else if item.kind == AnnotationKind::Route {
                    Some(tr!(
                        "map-route-length",
                        length = route_length(&item.points).round() as i64
                    ))
                } else {
                    None
                },
                share: None,
                editable: vec![],
            })
            .collect()
    }
    /// Rectangles for the visible area layers (slime chunks): each heat cell
    /// in view, translucent, under the icons.
    pub(super) fn fills(&self) -> Vec<canvas::Fill> {
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
    pub(super) fn canvas_layer(&mut self) -> Option<gpui::Canvas<()>> {
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
    pub(super) fn route_layer(&self) -> gpui::Canvas<()> {
        let view = self.view_blocks();
        let mut routes: Vec<_> = self
            .annotations
            .iter()
            .filter(|item| {
                item.kind == AnnotationKind::Route
                    && item.points.windows(2).any(|segment| {
                        let (a, b) = (segment[0], segment[1]);
                        a.x.min(b.x) <= view[2]
                            && a.x.max(b.x) >= view[0]
                            && a.z.min(b.z) <= view[3]
                            && a.z.max(b.z) >= view[1]
                    })
            })
            .map(|item| (item.points.clone(), item.color))
            .collect();
        routes.extend(self.objects.visible(view).into_iter().filter_map(|object| {
            let MapObjectKind::Polyline(points) = &object.kind else {
                return None;
            };
            Some((points.clone(), object.color.unwrap_or([106, 199, 225])))
        }));
        if self.route_points.len() >= 2 {
            let shade = edit::SWATCHES[6];
            routes.push((
                self.route_points.clone(),
                [(shade >> 16) as u8, (shade >> 8) as u8, shade as u8],
            ));
        }
        let camera = self.camera;
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                canvas::paint_routes(window, bounds, camera, &routes);
            },
        )
        .absolute()
        .size_full()
    }
    pub(super) fn scene_tiles(&mut self) -> Vec<Tile> {
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
        let colors = ShellColors::current(cx);
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
        // Controls that carry their own face sit on the map without a panel.
        let bare = || {
            div()
                .absolute()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        };
        // A bordered plate for read-outs beside the messages key. Collapsed it is
        // exactly as tall as that key; the jump fields make it grow.
        let plate = || {
            h_flex()
                .flex_none()
                .items_center()
                .min_h(px(KeySize::Small.height()))
                .px_2()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.surface)
        };
        let jumping = self.form.as_ref().is_some_and(|form| form.jumping);
        let hint = if self.placing {
            Some(tr!("map-placing-hint").to_owned())
        } else {
            self.annotation_hint()
        }
        .map(|message| {
            div()
                .debug_selector(|| "map-hint".into())
                .text_xs()
                .text_color(colors.muted)
                .child(message)
        });
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
            .child(self.route_layer())
            .child(measure)
            .child(bare().top_3().left_3().child(self.view_controls(cx)))
            .child(bare().top_3().right_3().child(self.base_controls(cx)))
            .child(
                // The messages key stays at the far left; the read-out is a
                // plate of its own beside it, as tall as the key.
                bare().bottom_3().left_3().max_w(relative(0.7)).child(
                    h_flex()
                        .gap_2()
                        .items_end()
                        .child(self.notice_popover(cx))
                        .child(
                            plate()
                                .when(jumping, |plate| plate.py_1())
                                .child(self.coordinates(cx)),
                        )
                        .children(hint.map(|hint| plate().child(hint))),
                ),
            )
            .child(
                bare().bottom_3().right_3().child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .children(self.pop_out_key(cx))
                        .child(self.annotation_controls(cx))
                        .child(self.layer_popover(cx)),
                ),
            )
            .children(
                self.selection_card(cx)
                    .map(|card| float().right_3().bottom(px(56.)).child(card)),
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
                        } else if this.marking || this.routing || this.measure_mode {
                            if let Some(at) = this.click_point(down, event.position) {
                                this.map_annotation_click(
                                    MapPoint { x: at[0], z: at[1] },
                                    window,
                                    cx,
                                );
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
            // ia[plugin.world-explorer]: 取消地图工具 | 地图视口 · Esc | 取消正在放置的标记、路线或测距，丢弃未保存的点
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
                    "escape" => {
                        if !(this.marking
                            || this.routing
                            || this.measure_mode
                            || this.placing
                            || this.measure_start.is_some())
                        {
                            return;
                        }
                        this.marking = false;
                        this.routing = false;
                        this.route_points.clear();
                        this.measure_mode = false;
                        this.measure_start = None;
                        this.measure_end = None;
                        this.placing = false;
                        this.frame();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
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
