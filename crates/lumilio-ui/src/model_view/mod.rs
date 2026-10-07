//! Inline schematic preview (ADR 0028). Interface chrome surrounds game art;
//! no animation clock, blocking I/O, meshing or GPU work runs on the UI thread.

#[cfg(test)]
mod tests;
mod worker;

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, Context, FocusHandle, MouseButton, ObjectFit, Pixels, Point, RenderImage, ScrollDelta,
    Task, Window, canvas, div, img, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use lumilio_core::ModelPreview;
use lumilio_schematic_render::{SceneError, View};

use crate::{key::Key, kit, theme::ShellColors};
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

pub struct ModelView {
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
}

impl ModelView {
    pub(crate) fn new(load: Load, cx: &mut Context<Self>) -> Self {
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
            Ok(Event::Loaded { undrawable }) => self.undrawable = undrawable,
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
                self.state = State::failed(error);
                self.worker = None;
            }
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.view = View {
            background: self.view.background,
            ..View::new()
        };
        cx.notify();
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.worker = None;
        self.task = None;
        self.last_request = None;
        self.state = State::Loading;
        self.undrawable.clear();
        (self.load)(cx.entity_id().as_u64(), window, cx);
        cx.notify();
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

impl Render for ModelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.old_images.drain(..) {
            window.drop_image(old).ok();
        }
        if !self.release_registered {
            cx.on_release_in(window, |this, window, _| {
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
        // ia[instance]: 旋转与缩放 3D 投影 | 投影详情内联预览 | 拖拽旋转，滚轮缩放且页面不跟着滚动；使用游戏贴图；读取或渲染失败显示说明；游戏版本画不出的方块在预览下方说明，ID 在技术详情 | ADR 0028、0034；动画方块暂时静止
        let viewport = div()
            .id("model-viewport")
            .role(gpui::Role::Image)
            .aria_label("3D 投影预览；方向键旋转，加减键缩放，R 复位")
            .debug_selector(|| "model-viewport".into())
            .relative()
            .w_full()
            .h(px(360.))
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
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
                let delta = match event.delta {
                    ScrollDelta::Pixels(point) => f32::from(point.y),
                    ScrollDelta::Lines(point) => point.y * 24.,
                };
                this.view = this.view.zoomed((delta * 0.01).clamp(-1., 1.).exp());
                cx.stop_propagation();
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
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
        // ia[instance]: 复位 3D 投影视角 | 预览下方「复位视角」 | 回到初始视角与缩放；也可双击预览或聚焦后按 R
        let reset = Key::new("model-reset")
            .label("复位视角")
            .ghost()
            .disabled(self.state != State::Ready)
            .on_click(cx.listener(|this, _, _, cx| this.reset(cx)));
        // ia[instance]: 重试 3D 投影预览 | 预览出错后的「重试」 | 重新读取当前投影与游戏贴图并生成预览；失败仍显示说明
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
        v_flex()
            .w_full()
            .gap_2()
            .text_sm()
            .text_color(colors.muted)
            .child(viewport)
            .children(note)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child("拖拽旋转 · 滚轮缩放 · 方向键旋转 · + / − 缩放")
                    .child(h_flex().gap_2().children(retry).child(reset)),
            )
    }
}
