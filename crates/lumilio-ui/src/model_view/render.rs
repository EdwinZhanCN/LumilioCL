use gpui::prelude::*;
use gpui::{Context, MouseButton, ObjectFit, ScrollDelta, Window, canvas, div, img, px};
use gpui_component::{Sizable as _, h_flex, v_flex};

use super::input::Mode;
use super::{CLICK_SLOP, MARK_WIDTH, State, Viewer, undrawable_entities_note, undrawable_note};
use crate::controls::Segments;
use crate::key::{Key, KeySize};
use crate::theme::ShellColors;
use crate::{kit, tr};

/// One of the studio's floating tool plates on the picture's bottom edge:
/// the caller pins it to a corner and fills it. Presses and the wheel on it
/// stay off the picture, so a key never turns the model or takes the
/// cursor.
pub(super) fn tool_plate(colors: ShellColors) -> gpui::Div {
    h_flex()
        .absolute()
        .bottom_3()
        .items_end()
        .gap_2()
        .p(px(6.))
        .rounded(px(6.))
        .bg(colors.body.panel)
        .border_1()
        .border_color(colors.border)
        .shadow_sm()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
}

pub(super) fn linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.old_images.drain(..) {
            window.drop_image(old).ok();
        }
        if !self.release_registered {
            if !self.inline {
                let focus = self.focus.clone();
                window.defer(cx, move |window, cx| window.focus(&focus, cx));
            }
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
        let colors = ShellColors::current(cx);
        let rgb = colors.surface.to_rgb();
        // Nucleation clears in linear RGB; GPUI's theme colours are sRGB.
        self.view.background = Some([linear(rgb.r), linear(rgb.g), linear(rgb.b), 1.]);
        let focus = colors.focus.to_rgb();
        self.selection_color = [linear(focus.r), linear(focus.g), linear(focus.b), 1.];
        self.sync_selection();
        let ink = colors.foreground.to_rgb();
        self.mark_style = (
            [linear(ink.r), linear(ink.g), linear(ink.b), 1.],
            MARK_WIDTH * window.scale_factor(),
        );
        self.sync_mark();
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
                .child(tr!("model-loading"))
                .into_any_element(),
            State::Ready => img(self
                .image
                .clone()
                .expect("Ready is set only after receiving a frame"))
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
        // ia[plugin.litematica]: Explore 准星与视线读数 | 工作室视口 · Explore 捕获中 | 画面中心有准星，读数板实时写视线中心方块的图标、名字、属性与坐标；单击检视准星下的那一个方块（描边与检视卡） | 读数在视角变化后更新（D27、T32）
        let crosshair =
            (self.inline && self.mode == Mode::Explore && self.capture.is_some()).then(|| {
                let bar = |w: f32, h: f32| {
                    div()
                        .absolute()
                        .w(px(w))
                        .h(px(h))
                        .left(px((14. - w) / 2.))
                        .top(px((14. - h) / 2.))
                        .bg(colors.foreground)
                        .border_1()
                        .border_color(colors.surface)
                };
                div()
                    .debug_selector(|| "model-crosshair".into())
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .relative()
                            .size(px(14.))
                            .child(bar(14., 3.))
                            .child(bar(3., 14.)),
                    )
            });
        // ia[instance]: 观察 3D 投影 | 3D 预览弹窗画面 | Orbital 拖拽旋转、滚轮缩放；Explore 点击捕获鼠标后第一人称转向，WASD 移动、空格上升、Shift 下降；Esc 先释放鼠标；失焦自动释放 | 自由飞行，不含重力与碰撞；动画方块暂时静止
        let viewport = div()
            .id("model-viewport")
            .role(gpui::Role::Image)
            .aria_label(tr!("model-aria"))
            .debug_selector(|| "model-viewport".into())
            .relative()
            .w_full()
            .map(|viewport| {
                if self.inline {
                    // The studio's picture fills its column, a stage with no
                    // frame of its own (D27).
                    viewport.h_full().min_h(px(96.)).rounded(px(6.))
                } else {
                    viewport.h(px(
                        (f32::from(window.viewport_size().height) - 240.).clamp(160., 640.)
                    ))
                }
            })
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .bg(colors.surface)
            .border_1()
            .border_color(if self.focus.is_focused(window) {
                colors.focus
            } else if self.inline {
                gpui::transparent_black()
            } else {
                colors.border
            })
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_action(
                cx.listener(|this, _: &gpui_component::dialog::Confirm, window, cx| {
                    if this.mode == Mode::Explore && this.capture.is_none() {
                        this.capture(window.mouse_position(), window, cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    if this.mode == Mode::Explore {
                        if this.capture.is_some() && this.inline {
                            this.inspect_at((0.5, 0.5), cx);
                        } else {
                            this.capture(event.position, window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    // ia[plugin.litematica]: 框选 | 工作室视口 Orbital 下按住 Ctrl（或 ⌘）从一个方块拖到另一个方块 | 两块之间的长方体被选中：画面里描出它的边、略染上焦点色，材料面板列出选区的尺寸与材料；Esc 清除 | 选区可作替换的范围（T25）
                    if this.inline
                        && (event.modifiers.control || event.modifiers.platform)
                        && this.state == State::Ready
                    {
                        this.start_selecting(event.position, cx);
                        cx.stop_propagation();
                        return;
                    }
                    this.drag = Some(event.position);
                    this.press = Some(event.position);
                    if event.click_count == 2 {
                        this.reset(cx);
                    }
                    cx.stop_propagation();
                }),
            )
            // ia[plugin.litematica]: 检视一个方块 | 工作室视口 Orbital 下单击画面 | 在后台沿视线逐格找到第一个方块，画面里只给这一个方块描边，视口左下出现它的检视卡；材料面板的选中不变；点到空处放下 | 只检视方块，不检视实体；「只看这种」时点不到被藏起的方块（T32、D31）
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.drag = None;
                    if this.selecting.is_some() {
                        this.finish_selecting(event.position, cx);
                        return;
                    }
                    let Some(press) = this.press.take() else {
                        return;
                    };
                    let moved = event.position - press;
                    if this.inline
                        && this.mode == Mode::Orbital
                        && this.state == State::Ready
                        && f32::from(moved.x).abs() < CLICK_SLOP
                        && f32::from(moved.y).abs() < CLICK_SLOP
                    {
                        this.pick(event.position, cx);
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
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered {
                    this.leave(cx);
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if this.mode != Mode::Orbital || this.selecting.is_some() {
                    return;
                }
                if !event.dragging() {
                    this.drag = None;
                    // ia[plugin.litematica]: 指着方块看是什么 | 工作室视口 · Orbital 下移动鼠标 | 读数板写出指针下方块的图标、名字、属性与坐标；移出画面恢复操作提示 | 一次只问一个拾取，跟着指针走（D27）
                    if this.inline && this.state == State::Ready {
                        this.hover(event.position, cx);
                    }
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
                if this.mode != Mode::Orbital {
                    return;
                }
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
            .on_modifiers_changed(
                cx.listener(|this, event: &gpui::ModifiersChangedEvent, _, _| {
                    if this.capture.is_some() {
                        if event.modifiers.control
                            || event.modifiers.platform
                            || event.modifiers.alt
                        {
                            this.input.clear();
                        } else {
                            this.input.shift = event.modifiers.shift;
                        }
                    }
                }),
            )
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                // The picture's keys are the picture's: a key typed into a
                // field on its tool plates (the layer) or a menu over it
                // reaches here too, and must not turn, zoom or reset it.
                if !this.focus.is_focused(window) {
                    return;
                }
                // ia[plugin.litematica]: 放下检视的方块 | 工作室视口聚焦时按 Esc（Explore 捕获中先释放鼠标） | 描边与检视卡消失，读数板回到跟着指针 | 也可点检视卡的关闭键或点画面空处（T32）
                if event.keystroke.key == "escape"
                    && this.capture.is_none()
                    && this.inspected.is_some()
                {
                    this.let_go(cx);
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.key == "escape"
                    && this.capture.is_none()
                    && this.selection.is_some()
                {
                    this.set_selection(None, cx);
                    cx.stop_propagation();
                    return;
                }
                if this.capture.is_none() && this.slice.is_some() {
                    let by = match event.keystroke.key.as_str() {
                        "[" => -1,
                        "]" => 1,
                        _ => 0,
                    };
                    if by != 0 {
                        this.step_layer(by, cx);
                        cx.stop_propagation();
                        return;
                    }
                }
                if this.mode == Mode::Explore {
                    if event.keystroke.key == "enter" && this.capture.is_none() {
                        this.capture(window.mouse_position(), window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    if this.capture.is_some() {
                        // ia[plugin.litematica]: 退出 Explore 的鼠标捕获 | 工作室视口 · Explore 捕获中按 Esc | 释放鼠标，停止移动，视线读数清空；再点画面或按 Enter 重新进入 | 只释放鼠标，不切换模式（D24）
                        if event.keystroke.key == "escape" {
                            this.release_capture();
                            cx.notify();
                            cx.stop_propagation();
                            return;
                        }
                        if event.keystroke.modifiers.control
                            || event.keystroke.modifiers.platform
                            || event.keystroke.modifiers.alt
                        {
                            this.input.clear();
                            return;
                        }
                        this.input.shift = event.keystroke.modifiers.shift;
                        if this.input.key(&event.keystroke.key, true) {
                            cx.stop_propagation();
                            return;
                        }
                        if matches!(event.keystroke.key.as_str(), "r" | "R") {
                            this.reset(cx);
                        }
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
            .child(measure)
            .children(crosshair);
        // The studio's plates hold small keys; the modal's head has regular
        // ones.
        let size = if self.inline {
            KeySize::Small
        } else {
            KeySize::Regular
        };
        // ia[instance]: 重试 3D 投影预览 | 弹窗出错后的「重试」 | 重新读取当前投影与游戏贴图并生成预览；失败仍显示说明
        let retry = matches!(self.state, State::Failed { .. }).then(|| {
            Key::new("model-retry")
                .label(tr!("common-retry"))
                .ghost()
                .size(size)
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
        let entity_note = (self.state == State::Ready)
            .then(|| undrawable_entities_note(&self.undrawable_entities))
            .flatten()
            .map(|(sentence, ids)| {
                h_flex()
                    .debug_selector(|| "model-undrawable-entities".into())
                    .w_full()
                    .gap_2()
                    .child(sentence)
                    .child(kit::technical("model-undrawable-entities-technical", ids))
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
        if self.inline {
            // The studio's controls float on the picture's bottom corners;
            // its read-out (what the pointer is on, else these controls'
            // hint) is the studio's, above them (D27). Leading, keys for the
            // camera: Orbital / Explore and 角度 (复位视角, then the
            // presets). Trailing, what is drawn: 切片 as a dropdown with its
            // label, as the discover page's 排序方式, and while slicing the
            // layer field over it. Two small plates fit side by side at the
            // window's minimum, where one strip of every key wrapped over
            // the read-out. Every key's bottom sits on one line.
            self.want_slice_controls(window, cx);
            self.sync_slice_controls(window, cx);
            // ia[plugin.litematica]: 换到预设角度 / 复位视角 | 工作室视口左下角 Orbital / Explore 旁的「角度」下拉：复位视角（R），然后北、东、南、西、顶、等轴 | 复位让当前模式回到初始视角；预设转到环绕视角的该方向并重新框住整个投影，在 Explore 里会先切回 Orbital | 也可双击画面或在视口聚焦时按 R 复位；等轴是透视下的等轴方向，不是正交投影（T10）
            let angles = kit::with_menu(
                Key::new("model-angle")
                    .label(tr!("model-angle"))
                    .white()
                    .small()
                    .disabled(self.state != State::Ready)
                    .debug_selector(|| "model-angle".into()),
                self.angle_entries(cx),
                gpui::Anchor::BottomLeft,
                colors,
            );
            let tools = tool_plate(colors)
                .debug_selector(|| "model-tools".into())
                .left_3()
                .child(modes)
                .child(angles)
                .children(retry);
            let view = tool_plate(colors)
                .debug_selector(|| "model-tools-view".into())
                .right_3()
                .flex_col()
                .items_end()
                .children(self.layer_field(colors))
                .children(self.slice_select(colors));
            return viewport.child(tools).child(view).into_any_element();
        }
        // ia[instance]: 复位 3D 投影视角 | 弹窗「复位视角」 | 当前模式回到初始视角；Orbital 可双击画面，聚焦画面后可按 R
        let reset = Key::new("model-reset")
            .label(tr!("model-reset"))
            .ghost()
            .size(size)
            .debug_selector(|| "model-reset".into())
            .disabled(self.state != State::Ready)
            .on_click(cx.listener(|this, _, _, cx| this.reset(cx)));
        let hint = match self.mode {
            Mode::Orbital => tr!("model-hint-orbital"),
            Mode::Explore if self.capture.is_some() => tr!("model-hint-explore-captured"),
            Mode::Explore => tr!("model-hint-explore"),
        };
        v_flex()
            .w_full()
            .gap_2()
            .text_sm()
            .text_color(colors.muted)
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(modes)
                    .child(h_flex().gap_2().children(retry).child(reset)),
            )
            .child(viewport)
            .children(note)
            .children(entity_note)
            .child(div().text_xs().child(hint))
            .children(
                self.capture_error
                    .map(|message| div().text_sm().child(message)),
            )
            .into_any_element()
    }
}
