//! The look preview on an account's detail: the player drawn on the CPU
//! (`lumilio-skin-render`), turned by dragging or the arrow keys and zoomed
//! by the wheel or + / −, as the schematic preview's orbit mode is (design
//! language, native model preview). Frames are drawn off the UI thread; a
//! newer request replaces a waiting one, and replaced frames leave GPUI's
//! image cache (ADR 0028).

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, Context, FocusHandle, MouseButton, ObjectFit, Pixels, Point, RenderImage, ScrollDelta,
    Task, Window, canvas, div, img, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use lumilio_skin_render::{Arms, BackEquipment, Camera, Player, Texture, render};

use crate::kit;
use crate::theme::ShellColors;
use crate::tr;

#[cfg(test)]
mod tests;

/// The preview's height for a window this tall: what is left under the
/// account's head and above the navigation, within limits. Its width follows
/// the column.
fn height_for(window_height: f32) -> f32 {
    (window_height - 420.).clamp(220., 480.)
}

/// The pictures a preview draws. No skin is the plain grey figure.
#[derive(Clone, Debug, Default)]
pub struct Look {
    pub skin: Option<Arc<Texture>>,
    pub cape: Option<Arc<Texture>>,
    pub arms: Arms,
}

impl Look {
    /// The pictures core loaded for an account.
    #[must_use]
    pub fn from_core(look: &lumilio_core::AccountLook) -> Self {
        let texture = |pixels: &lumilio_core::SkinPixels| {
            Texture::new(pixels.width, pixels.height, pixels.rgba.clone()).map(Arc::new)
        };
        Self {
            skin: look.skin.as_ref().and_then(texture),
            cape: look.cape.as_ref().and_then(texture),
            arms: match look.model {
                lumilio_core::SkinModel::Wide => Arms::Classic,
                lumilio_core::SkinModel::Slim => Arms::Slim,
            },
        }
    }
}

enum State {
    Loading,
    Ready(Look),
    Failed { message: String, detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Request {
    camera: Camera,
    back_equipment: BackEquipment,
    width: u32,
    height: u32,
    /// Which look the frame is of; a frame of an older look is dropped.
    look: u64,
}

pub struct SkinViewer {
    state: State,
    look: u64,
    camera: Camera,
    back_equipment: BackEquipment,
    focus: FocusHandle,
    drag: Option<Point<Pixels>>,
    image: Option<Arc<RenderImage>>,
    old_images: Vec<Arc<RenderImage>>,
    /// The frame drawn or being drawn.
    shown: Option<Request>,
    /// The newest request that arrived while a frame was being drawn.
    queued: Option<Request>,
    drawing: Option<Task<()>>,
    release_registered: bool,
}

impl SkinViewer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: State::Loading,
            look: 0,
            camera: Camera::HOME,
            back_equipment: BackEquipment::Cape,
            focus: cx.focus_handle(),
            drag: None,
            image: None,
            old_images: Vec::new(),
            shown: None,
            queued: None,
            drawing: None,
            release_registered: false,
        }
    }

    /// Shows the look core loaded, or why it could not be loaded.
    pub fn set_look(&mut self, look: Result<Look, (String, String)>, cx: &mut Context<Self>) {
        self.look += 1;
        self.shown = None;
        self.queued = None;
        if let Some(old) = self.image.take() {
            self.old_images.push(old);
        }
        self.state = match look {
            Ok(look) => State::Ready(look),
            Err((message, detail)) => State::Failed { message, detail },
        };
        cx.notify();
    }

    fn turn(&mut self, yaw: f32, pitch: f32, cx: &mut Context<Self>) {
        self.camera = Camera {
            yaw: self.camera.yaw + yaw,
            pitch: self.camera.pitch + pitch,
            ..self.camera
        }
        .clamped();
        cx.notify();
    }

    fn show_back_equipment(&mut self, equipment: BackEquipment, cx: &mut Context<Self>) {
        if self.back_equipment != equipment {
            self.back_equipment = equipment;
            cx.notify();
        }
    }

    fn zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        self.camera = Camera {
            zoom: self.camera.zoom * factor,
            ..self.camera
        }
        .clamped();
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.camera = Camera::HOME;
        cx.notify();
    }

    fn request(&mut self, width: f32, height: f32, scale: f32, cx: &mut Context<Self>) {
        if !matches!(self.state, State::Ready(_)) || width < 1. || height < 1. {
            return;
        }
        let request = Request {
            camera: self.camera,
            back_equipment: self.back_equipment,
            width: (width * scale).round() as u32,
            height: (height * scale).round() as u32,
            look: self.look,
        };
        if self.shown == Some(request) {
            return;
        }
        if self.drawing.is_some() {
            self.queued = Some(request);
        } else {
            self.draw(request, cx);
        }
    }

    fn draw(&mut self, request: Request, cx: &mut Context<Self>) {
        let State::Ready(look) = &self.state else {
            return;
        };
        let look = look.clone();
        self.shown = Some(request);
        self.drawing = Some(cx.spawn(async move |this, cx| {
            let frame = cx
                .background_executor()
                .spawn(async move {
                    let player = Player {
                        skin: look.skin.as_deref(),
                        arms: look.arms,
                        cape: look.cape.as_deref(),
                        back_equipment: request.back_equipment,
                        outer_layer: true,
                    };
                    render(&player, request.camera, request.width, request.height)
                })
                .await;
            let _ = this.update(cx, |this, cx| this.arrived(request, frame, cx));
        }));
    }

    fn arrived(
        &mut self,
        request: Request,
        frame: lumilio_skin_render::Frame,
        cx: &mut Context<Self>,
    ) {
        self.drawing = None;
        if request.look == self.look
            && request.back_equipment == self.back_equipment
            && let Some(buffer) = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
        {
            let image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
            if let Some(old) = self.image.replace(image) {
                self.old_images.push(old);
            }
        }
        if let Some(next) = self.queued.take()
            && Some(next) != self.shown
        {
            self.draw(next, cx);
        }
        cx.notify();
    }
}

impl Render for SkinViewer {
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
        let colors = ShellColors::from_theme(cx.theme());
        let reset = {
            let this = cx.weak_entity();
            move |_: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |this, cx| this.reset(cx));
            }
        };
        let this = cx.weak_entity();
        let measure = canvas(
            move |bounds, window, cx: &mut App| {
                let _ = this.update(cx, |this, cx| {
                    this.request(
                        bounds.size.width.into(),
                        bounds.size.height.into(),
                        window.scale_factor(),
                        cx,
                    );
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let content = match (&self.state, &self.image) {
            (State::Failed { message, detail }, _) => v_flex()
                .debug_selector(|| "skin-view-error".into())
                .items_center()
                .gap_3()
                .px_4()
                .text_sm()
                .text_color(colors.muted)
                .child(message.clone())
                .child(kit::technical("skin-view-technical", detail.clone()))
                .into_any_element(),
            (State::Ready(_), Some(image)) => img(image.clone())
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            _ => div()
                .debug_selector(|| "skin-view-loading".into())
                .text_sm()
                .text_color(colors.muted)
                .child(tr!("skin-view-loading"))
                .into_any_element(),
        };
        // ia[accounts]: 观察外观 | 详情里的立体预览 | 拖动或方向键旋转，滚轮或 + / − 缩放，双击或 R 复位；第二层与披风一起画
        let viewport = div()
            .id("skin-view")
            .role(gpui::Role::Image)
            .aria_label(tr!("skin-view-aria"))
            .debug_selector(|| "skin-view".into())
            .relative()
            .w_full()
            .h(px(height_for(f32::from(window.viewport_size().height))))
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
                if let Some(previous) = this.drag.replace(event.position) {
                    this.turn(
                        f32::from(event.position.x - previous.x) * 0.012,
                        f32::from(event.position.y - previous.y) * 0.008,
                        cx,
                    );
                }
            }))
            .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                let delta = match event.delta {
                    ScrollDelta::Pixels(point) => f32::from(point.y),
                    ScrollDelta::Lines(point) => point.y * 24.,
                };
                this.zoom((delta * 0.01).clamp(-1., 1.).exp(), cx);
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                let handled = match event.keystroke.key.as_str() {
                    "left" => {
                        this.turn(-0.2, 0., cx);
                        true
                    }
                    "right" => {
                        this.turn(0.2, 0., cx);
                        true
                    }
                    "up" => {
                        this.turn(0., -0.15, cx);
                        true
                    }
                    "down" => {
                        this.turn(0., 0.15, cx);
                        true
                    }
                    "+" | "=" => {
                        this.zoom(1.15, cx);
                        true
                    }
                    "-" => {
                        this.zoom(1. / 1.15, cx);
                        true
                    }
                    "r" => {
                        this.reset(cx);
                        true
                    }
                    _ => false,
                };
                if handled {
                    cx.stop_propagation();
                }
            }))
            .child(measure)
            .child(content);
        let has_cape = matches!(&self.state, State::Ready(look) if look.cape.is_some());
        let this = cx.weak_entity();
        // ia[accounts]: 披风 / 鞘翅预览 | 立体预览下的分段按键（有披风时） | 同一贴图在披风与鞘翅形态间切换，保留相机；只改变预览，不改变账户穿戴
        let equipment = has_cape.then(|| {
            kit::segments(
                "skin-view-equipment",
                crate::tr_all!["skin-view-cape", "skin-view-elytra"],
                usize::from(self.back_equipment == BackEquipment::Elytra),
                move |index, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.show_back_equipment(
                            if index == 0 {
                                BackEquipment::Cape
                            } else {
                                BackEquipment::Elytra
                            },
                            cx,
                        )
                    });
                },
            )
        });
        v_flex()
            .w_full()
            .gap_2()
            .child(viewport)
            .children(equipment)
            // ia[accounts]: 没有可用默认贴图 | 未安装游戏或客户端 jar 无默认皮肤 | 灰色模型与说明；安装游戏后重新打开详情读取贴图
            .children(
                matches!(&self.state, State::Ready(look) if look.skin.is_none()).then(|| {
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!("skin-view-default-missing"))
                }),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(tr!("skin-view-hint")),
                    )
                    .child(kit::ghost("skin-view-reset", tr!("model-reset"), reset)),
            )
    }
}
