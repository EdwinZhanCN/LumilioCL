//! Modal schematic viewer. Meshing and GPU readback stay on the worker;
//! a frame clock runs only while first-person input is captured.

mod input;
mod modal;
#[cfg(test)]
mod tests;
mod worker;

pub use modal::ModelView;

use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, FocusHandle, MouseButton, ObjectFit, Pixels, Point, RenderImage,
    ScrollDelta, Task, Window, canvas, div, img, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use lumilio_core::ModelPreview;
use lumilio_schematic_render::{SceneError, View};

use crate::{
    controls::Segments,
    key::Key,
    kit,
    theme::{self, ShellColors},
};
use input::{FlightInput, Mode};
use worker::{Event, Request, Worker};

type Load = Rc<dyn Fn(u64, &mut Window, &mut App)>;

fn linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn physical_size(width: f32, height: f32, scale: f32) -> Option<(u32, u32)> {
    if width < 1. || height < 1. {
        return None;
    }
    let fit = (4096. / (width * scale).max(height * scale)).min(1.);
    Some((
        (width * scale * fit).round().max(1.) as u32,
        (height * scale * fit).round().max(1.) as u32,
    ))
}

#[derive(Debug, PartialEq)]
enum State {
    Loading,
    Ready,
    Failed {
        message: &'static str,
        detail: Option<String>,
    },
}

impl State {
    fn failed(error: SceneError) -> Self {
        let message = match &error {
            SceneError::NoGpu => "这台电脑没有可用的图形设备，无法显示 3D 预览",
            SceneError::Parse(_) => "读不了这个投影文件",
            SceneError::Pack(_) => "读不了游戏的贴图包",
            SceneError::Mesh(_) => "没能生成 3D 预览",
            SceneError::Render(_) => "没能绘制 3D 预览",
        };
        let detail = (error != SceneError::NoGpu).then(|| error.to_string());
        Self::Failed { message, detail }
    }
}

/// One plain sentence about blocks left out of the picture; the IDs go to 技术详情.
fn undrawable_note(undrawable: &[String]) -> Option<(String, String)> {
    (!undrawable.is_empty()).then(|| {
        (
            format!(
                "有 {} 种方块这个游戏版本画不出来，预览里没有它们",
                undrawable.len()
            ),
            undrawable.join("\n"),
        )
    })
}

struct Viewer {
    load: Load,
    asked: bool,
    state: State,
    /// Block IDs the game's assets cannot draw, reported once per load.
    undrawable: Vec<String>,
    view: View,
    worker: Option<Worker>,
    task: Option<Task<()>>,
    image: Option<Arc<RenderImage>>,
    old_images: Vec<Arc<RenderImage>>,
    last_request: Option<Request>,
    drag: Option<Point<Pixels>>,
    focus: FocusHandle,
    release_registered: bool,
    mode: Mode,
    explore_position: [f32; 3],
    orbit_view: View,
    flight_view: Option<View>,
    input: FlightInput,
    capture: Option<lumilio_pointer::Capture>,
    capture_error: Option<&'static str>,
    viewport: Option<Bounds<Pixels>>,
    last_tick: Instant,
    clock_pending: bool,
}

impl Viewer {
    fn new(load: Load, cx: &mut Context<Self>) -> Self {
        Self {
            load,
            asked: false,
            state: State::Loading,
            undrawable: Vec::new(),
            view: View::new(),
            worker: None,
            task: None,
            image: None,
            old_images: Vec::new(),
            last_request: None,
            drag: None,
            focus: cx.focus_handle(),
            release_registered: false,
            mode: Mode::Orbital,
            explore_position: [0., 1.62, 3.],
            orbit_view: View::new(),
            flight_view: None,
            input: FlightInput::default(),
            capture: None,
            capture_error: None,
            viewport: None,
            last_tick: Instant::now(),
            clock_pending: false,
        }
    }

    /// Called only for this entity's asset request; replacement views have new IDs.
    pub(crate) fn assets(&mut self, result: Result<ModelPreview, String>, cx: &mut Context<Self>) {
        let preview = match result {
            Ok(preview) => preview,
            Err(detail) => {
                self.state = State::Failed {
                    message: "没能读取 3D 预览",
                    detail: Some(detail),
                };
                cx.notify();
                return;
            }
        };
        let Some(pack) = preview.pack else {
            self.state = State::Failed {
                message: "安装这个游戏后，就能用它的贴图预览投影",
                detail: None,
            };
            cx.notify();
            return;
        };
        match Worker::start(preview.schematic, pack.bytes) {
            Ok(worker) => {
                let output = worker.output();
                self.worker = Some(worker);
                self.task = Some(cx.spawn(async move |this, cx| {
                    while let Ok(result) = output.recv().await {
                        if this
                            .update(cx, |this, cx| {
                                this.frame_arrived(result);
                                cx.notify();
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }));
            }
            Err(error) => self.state = State::failed(SceneError::Render(error.to_string())),
        }
        cx.notify();
    }

    fn frame_arrived(&mut self, result: worker::Output) {
        match result {
            Ok(Event::Loaded {
                undrawable,
                explore_position,
            }) => {
                self.undrawable = undrawable;
                self.explore_position = explore_position;
                self.flight_view = None;
                if self.mode == Mode::Explore {
                    self.view.position = Some(explore_position);
                    self.view.yaw_deg = 0.;
                    self.view.pitch_deg = 0.;
                }
            }
            Ok(Event::Frame(frame)) => {
                if let Some(buffer) =
                    image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
                {
                    let image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
                    if let Some(old) = self.image.replace(image) {
                        self.old_images.push(old);
                    }
                    self.state = State::Ready;
                }
            }
            Err(error) => {
                self.release_capture();
                self.state = State::failed(error);
                self.worker = None;
            }
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.input.clear();
        self.view = View {
            background: self.view.background,
            ..View::new()
        };
        if self.mode == Mode::Explore {
            self.view.position = Some(self.explore_position);
            self.view.yaw_deg = 0.;
            self.view.pitch_deg = 0.;
        }
        cx.notify();
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.release_capture();
        self.capture_error = None;
        self.worker = None;
        self.task = None;
        self.last_request = None;
        self.state = State::Loading;
        self.undrawable.clear();
        (self.load)(cx.entity_id().as_u64(), window, cx);
        cx.notify();
    }

    fn release_capture(&mut self) {
        self.capture = None;
        self.input.clear();
        self.drag = None;
    }

    fn select_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.release_capture();
        self.capture_error = None;
        if mode == self.mode {
            cx.notify();
            return;
        }
        match self.mode {
            Mode::Orbital => self.orbit_view = self.view,
            Mode::Explore => self.flight_view = Some(self.view),
        }
        self.mode = mode;
        self.view = match mode {
            Mode::Orbital => self.orbit_view,
            Mode::Explore => self.flight_view.unwrap_or(View {
                yaw_deg: 0.,
                pitch_deg: 0.,
                position: Some(self.explore_position),
                background: self.view.background,
                ..View::new()
            }),
        };
        cx.notify();
    }

    fn capture(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.capture.is_some() || self.state != State::Ready {
            return;
        }
        let Some(bounds) = self.viewport else {
            return;
        };
        let offset = bounds.center() - position;
        let result = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|_| lumilio_pointer::CaptureError::Unsupported)
            .and_then(|handle| {
                lumilio_pointer::Capture::new(
                    handle.as_raw(),
                    [offset.x.into(), offset.y.into()],
                    window.scale_factor(),
                )
            });
        match result {
            Ok(capture) => {
                self.capture = Some(capture);
                self.capture_error = None;
                self.input.clear();
                self.input.shift = window.modifiers().shift;
                self.last_tick = Instant::now();
                self.schedule_tick(window, cx);
            }
            Err(lumilio_pointer::CaptureError::Unsupported) => {
                self.capture_error = Some("当前窗口系统暂不支持鼠标捕获，请使用 Orbital 模式");
            }
            Err(lumilio_pointer::CaptureError::Unavailable) => {
                self.capture_error = Some("没能捕获鼠标，点击画面重试");
            }
        }
        cx.notify();
    }

    fn schedule_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clock_pending {
            return;
        }
        self.clock_pending = true;
        let viewer = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = viewer.update(cx, |viewer, cx| viewer.tick(window, cx));
        });
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clock_pending = false;
        if self.capture.is_none() {
            return;
        }
        if !window.is_window_active() || !self.focus.is_focused(window) {
            self.release_capture();
            cx.notify();
            return;
        }
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_tick)
            .min(theme::motion::WORLD_TICK);
        self.last_tick = now;
        let delta = match self.capture.as_mut().unwrap().sample() {
            Ok(delta) => delta,
            Err(_) => {
                self.release_capture();
                self.capture_error = Some("鼠标捕获已结束，点击画面重新进入");
                cx.notify();
                return;
            }
        };
        let previous = self.view;
        self.view = self
            .input
            .advance(self.view.look(delta[0] * 0.15, delta[1] * 0.15), elapsed);
        if self.view != previous {
            cx.notify();
        }
        // Input response is intentional motion, including under reduced motion.
        self.schedule_tick(window, cx);
    }

    fn request(&mut self, width: f32, height: f32, scale: f32) {
        // Bound readback memory and stay below the baseline GPU texture limit.
        let Some((width, height)) = physical_size(width, height, scale) else {
            return;
        };
        let request = Request {
            view: self.view,
            width,
            height,
        };
        if let Some(worker) = &self.worker
            && self.last_request != Some(request)
        {
            worker.mailbox.put(request);
            self.last_request = Some(request);
        }
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.old_images.drain(..) {
            window.drop_image(old).ok();
        }
        if !self.release_registered {
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.release_capture();
                    cx.notify();
                }
            })
            .detach();
            cx.observe_window_bounds(window, |this, _, cx| {
                // A resize/move invalidates the capture anchor and native clip.
                this.release_capture();
                cx.notify();
            })
            .detach();
            cx.on_blur(&self.focus.clone(), window, |this, _, cx| {
                this.release_capture();
                cx.notify();
            })
            .detach();
            cx.on_release_in(window, |this, window, _| {
                this.release_capture();
                for image in this.old_images.drain(..).chain(this.image.take()) {
                    window.drop_image(image).ok();
                }
            })
            .detach();
            self.release_registered = true;
        }
        if !self.asked {
            self.asked = true;
            let (load, id) = (self.load.clone(), cx.entity_id().as_u64());
            window.defer(cx, move |window, cx| load(id, window, cx));
        }
        let colors = ShellColors::from_theme(cx.theme());
        let rgb = colors.surface.to_rgb();
        // Nucleation clears in linear RGB; GPUI's theme colours are sRGB.
        self.view.background = Some([linear(rgb.r), linear(rgb.g), linear(rgb.b), 1.]);
        let this = cx.weak_entity();
        let measure = canvas(
            move |bounds, window, cx| {
                let _ = this.update(cx, |this, _| {
                    this.viewport = Some(bounds);
                    this.request(
                        bounds.size.width.into(),
                        bounds.size.height.into(),
                        window.scale_factor(),
                    )
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let content = match &self.state {
            State::Loading => div()
                .debug_selector(|| "model-loading".into())
                .child("正在生成 3D 预览…")
                .into_any_element(),
            State::Ready => img(self.image.clone().unwrap())
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            State::Failed { message, detail } => v_flex()
                .debug_selector(|| "model-error".into())
                .items_center()
                .gap_3()
                .child(*message)
                .children(
                    detail
                        .as_ref()
                        .map(|detail| kit::technical("model-technical", detail.clone())),
                )
                .into_any_element(),
        };
        // ia[instance]: 观察 3D 投影 | 3D 预览弹窗画面 | Orbital 拖拽旋转、滚轮缩放；Explore 点击捕获鼠标后第一人称转向，WASD 移动、空格上升、Shift 下降；Esc 先释放鼠标；失焦自动释放 | 自由飞行，不含重力与碰撞；动画方块暂时静止
        let viewport = div()
            .id("model-viewport")
            .role(gpui::Role::Image)
            .aria_label("3D 投影预览；Orbital 方向键旋转、加减键缩放；Explore 点击或 Enter 捕获鼠标，WASD 移动、空格上升、Shift 下降，Esc 释放；R 复位")
            .debug_selector(|| "model-viewport".into())
            .relative()
            .w_full()
            .h(px((f32::from(window.viewport_size().height) - 240.).clamp(160., 640.)))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .bg(colors.surface)
            .border_1()
            .border_color(if self.focus.is_focused(window) {
                colors.focus
            } else {
                colors.border
            })
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_action(cx.listener(|this, _: &gpui_component::dialog::Confirm, window, cx| {
                if this.mode == Mode::Explore && this.capture.is_none() {
                    this.capture(window.mouse_position(), window, cx);
                }
                cx.stop_propagation();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    if this.mode == Mode::Explore {
                        this.capture(event.position, window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    this.drag = Some(event.position);
                    if event.click_count == 2 {
                        this.reset(cx);
                    }
                    cx.stop_propagation();
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
                if this.mode != Mode::Orbital { return; }
                if !event.dragging() {
                    this.drag = None;
                    return;
                }
                if let Some(previous) = this.drag {
                    this.drag = Some(event.position);
                    this.view = this.view.orbit(
                        f32::from(event.position.x - previous.x) * 0.4,
                        f32::from(event.position.y - previous.y) * 0.4,
                    );
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                cx.stop_propagation();
                if this.mode != Mode::Orbital { return; }
                let delta = match event.delta {
                    ScrollDelta::Pixels(point) => f32::from(point.y),
                    ScrollDelta::Lines(point) => point.y * 24.,
                };
                this.view = this.view.zoomed((delta * 0.01).clamp(-1., 1.).exp());
                cx.stop_propagation();
                cx.notify();
            }))
            .on_key_up(cx.listener(|this, event: &gpui::KeyUpEvent, _, cx| {
                if this.capture.is_some() && this.input.key(&event.keystroke.key, false) {
                    cx.stop_propagation();
                }
            }))
            .on_modifiers_changed(cx.listener(|this, event: &gpui::ModifiersChangedEvent, _, _| {
                if this.capture.is_some() {
                    if event.modifiers.control || event.modifiers.platform || event.modifiers.alt {
                        this.input.clear();
                    } else { this.input.shift = event.modifiers.shift; }
                }
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if this.mode == Mode::Explore {
                    if event.keystroke.key == "enter" && this.capture.is_none() {
                        this.capture(window.mouse_position(), window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    if this.capture.is_some() {
                        if event.keystroke.modifiers.control || event.keystroke.modifiers.platform || event.keystroke.modifiers.alt {
                            this.input.clear();
                            return;
                        }
                        this.input.shift = event.keystroke.modifiers.shift;
                        if this.input.key(&event.keystroke.key, true) {
                            cx.stop_propagation();
                            return;
                        }
                        if matches!(event.keystroke.key.as_str(), "r" | "R") { this.reset(cx); }
                    }
                    return;
                }
                let (dx, dy, zoom) = match event.keystroke.key.as_str() {
                    "r" | "R" => {
                        this.reset(cx);
                        cx.stop_propagation();
                        return;
                    }
                    "left" => (-10., 0., 1.),
                    "right" => (10., 0., 1.),
                    "up" => (0., -10., 1.),
                    "down" => (0., 10., 1.),
                    "+" | "=" => (0., 0., 1.1),
                    "-" => (0., 0., 1. / 1.1),
                    _ => return,
                };
                this.view = this.view.orbit(dx, dy).zoomed(zoom);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(content)
            .child(measure);
        // ia[instance]: 复位 3D 投影视角 | 弹窗「复位视角」 | 当前模式回到初始视角；Orbital 可双击画面，聚焦画面后可按 R
        let reset = Key::new("model-reset")
            .label("复位视角")
            .ghost()
            .disabled(self.state != State::Ready)
            .on_click(cx.listener(|this, _, _, cx| this.reset(cx)));
        // ia[instance]: 重试 3D 投影预览 | 弹窗出错后的「重试」 | 重新读取当前投影与游戏贴图并生成预览；失败仍显示说明
        let retry = matches!(self.state, State::Failed { .. }).then(|| {
            Key::new("model-retry")
                .label("重试")
                .ghost()
                .on_click(cx.listener(|this, _, window, cx| this.retry(window, cx)))
        });
        let note = (self.state == State::Ready)
            .then(|| undrawable_note(&self.undrawable))
            .flatten()
            .map(|(sentence, ids)| {
                h_flex()
                    .debug_selector(|| "model-undrawable".into())
                    .w_full()
                    .gap_2()
                    .child(sentence)
                    .child(kit::technical("model-undrawable-technical", ids))
            });
        let viewer = cx.weak_entity();
        // ia[instance]: 切换 3D 投影观察模式 | 预览弹窗 Orbital / Explore 分段 | 两种模式分别保留视角；切换时释放鼠标并停止移动
        let modes = Segments::new(
            "model-mode",
            &["Orbital", "Explore"],
            usize::from(self.mode == Mode::Explore),
            move |index, _, cx| {
                let _ = viewer.update(cx, |viewer, cx| {
                    viewer.select_mode(
                        if index == 0 {
                            Mode::Orbital
                        } else {
                            Mode::Explore
                        },
                        cx,
                    )
                });
            },
        );
        let hint = match self.mode {
            Mode::Orbital => "拖拽旋转 · 滚轮缩放 · 方向键旋转 · + / − 缩放",
            Mode::Explore if self.capture.is_some() => {
                "WASD 移动 · 空格上升 · Shift 下降 · Esc 释放鼠标"
            }
            Mode::Explore => "点击画面或按 Enter 进入 · WASD 移动 · 空格上升 · Shift 下降",
        };
        v_flex()
            .w_full()
            .gap_2()
            .text_sm()
            .text_color(colors.muted)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_3()
                    .child(modes)
                    .child(h_flex().gap_2().children(retry).child(reset)),
            )
            .child(viewport)
            .children(note)
            .child(div().text_xs().child(hint))
            .children(
                self.capture_error
                    .map(|message| div().text_sm().child(message)),
            )
    }
}
